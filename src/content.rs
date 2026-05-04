//! Content loading.
//!
//! Articles are Markdown files. On every request the content directory is
//! rescanned so new articles are picked up without a restart. Only the
//! article that will actually be rendered is fully parsed; all others are
//! read just far enough to extract the title and slug for the sidebar.
//!
//! ## Markdown conventions
//!
//! ```markdown
//! +++
//! slug    = "my-article"
//! author  = "Jane Doe"
//! published = "2026-04-01"
//! +++
//!
//! # Article Title (first h1 becomes the title)
//!
//! Intro paragraph…
//!
//! ## Section heading
//!
//! Body paragraph…
//!
//! <!-- ad: top-banner -->
//! ```
//!
//! Front matter (between `+++` fences) is optional. The first H1 is the title.
//! `<!-- ad: <slot> -->` comments are turned into `Block::Ad` slots.

use anyhow::{Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::path::Path;

use crate::config::ContentConfig;

// ── Domain model ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    SoftBreak,
    HardBreak,
    Code(String), // inline `code`
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Link {
        href: String,
        title: String,
        children: Vec<Inline>,
    },
    Image {
        src: String,
        alt: String,
        title: String,
    },
}

/// A rendered block inside a section.
#[derive(Debug, Clone)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Ad(String),
    /// Ordered or unordered list.
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    /// A GFM table.
    Table {
        headers: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// Block quote.
    BlockQuote(Vec<Block>),
    /// Thematic break <hr>.
    Rule,
    /// Fenced code block.
    CodeBlock {
        lang: String,
        code: String,
    },
}

#[derive(Debug, Clone)]
pub struct ListItem {
    /// A list item can contain paragraphs, nested lists, etc.
    pub blocks: Vec<Block>,
}

/// A logical section of the article. A section begins at a heading (h2–h6)
/// and contains all blocks up to the next heading of equal or higher rank.
/// The intro section before the first heading has `heading: None`.
#[derive(Debug, Clone)]
pub struct Section {
    pub heading: Option<Heading>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct Heading {
    /// 2..=6  (h1 is consumed as the article title)
    pub level: u8,
    /// URL-safe id derived from the heading text (for anchor links / TOC)
    pub id: String,
    /// Plain-text label shown in the TOC
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub slug: String,
    pub author: String,
    pub published: String,
    pub sections: Vec<Section>,
}

/// Lightweight card shown in the sidebar.
#[derive(Debug, Clone)]
pub struct StoryHeader {
    pub title: String,
    pub slug: String,
    pub snippet: String,
}

/// Everything the page needs for one render.
pub struct ContentBundle {
    pub article: Article,
    pub stories: Vec<StoryHeader>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Rescan the content directory, then fully parse only the article to render.
///
/// `requested_slug` — `Some("my-slug")` for a specific article, `None` to pick randomly.
pub fn load(cfg: &ContentConfig, requested_slug: Option<&str>) -> Result<ContentBundle> {
    let metas = scan_metas(&cfg.dir)?;
    anyhow::ensure!(!metas.is_empty(), "no .md files found in {:?}", cfg.dir);

    let chosen_idx = if let Some(slug) = requested_slug {
        metas
            .iter()
            .position(|m| m.slug == slug)
            .with_context(|| format!("no article with slug '{slug}'"))?
    } else {
        random_index(metas.len())
    };

    // Sidebar cards — cheap metadata only.
    let mut stories: Vec<StoryHeader> = metas
        .iter()
        .map(|m| StoryHeader {
            title: m.title.clone(),
            slug: m.slug.clone(),
            snippet: String::new(),
        })
        .collect();

    // Fully parse only the chosen article.
    let article = parse_article_file(&metas[chosen_idx].path)?;
    stories[chosen_idx].snippet = first_text_snippet(&article);

    shuffle(&mut stories);

    Ok(ContentBundle { article, stories })
}

// ── Scanning ──────────────────────────────────────────────────────────────────

struct ArticleMeta {
    title: String,
    slug: String,
    path: std::path::PathBuf,
}

/// Read every `.md` file just enough to extract title + slug.
fn scan_metas(dir: &Path) -> Result<Vec<ArticleMeta>> {
    let mut metas = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading content directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.context("reading directory entry")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let (front, body) = split_front_matter(&raw);
        let mut slug = front_matter_value(front, "slug").unwrap_or_default();
        let title = extract_title_from_body(body);

        if title.is_empty() {
            tracing::warn!(path = %path.display(), "skipping: no H1 title found");
            continue;
        }
        if slug.is_empty() {
            slug = slugify(&title);
        }

        metas.push(ArticleMeta { title, slug, path });
    }

    metas.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(metas)
}

/// Cheap: only parse until we find the first H1, ignore the rest.
fn extract_title_from_body(body: &str) -> String {
    let parser = Parser::new_ext(body, Options::empty());
    let mut in_h1 = false;
    let mut title = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => {
                in_h1 = true;
            }
            Event::End(TagEnd::Heading(_)) if in_h1 => break,
            Event::Text(t) if in_h1 => title.push_str(&t),
            _ => {}
        }
    }
    title
}

// ── Full article parse ────────────────────────────────────────────────────────

fn parse_article_file(path: &Path) -> Result<Article> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_article(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn parse_article(raw: &str) -> Result<Article> {
    let (front, body) = split_front_matter(raw);

    let slug_fm = front_matter_value(front, "slug").unwrap_or_default();
    let author = front_matter_value(front, "author").unwrap_or_default();
    let published = front_matter_value(front, "published").unwrap_or_default();

    let (title, sections) = parse_body(body);

    anyhow::ensure!(!title.is_empty(), "article has no H1 title");

    let slug = if slug_fm.is_empty() {
        slugify(&title)
    } else {
        slug_fm
    };

    Ok(Article {
        title,
        slug,
        author,
        published,
        sections,
    })
}

// ── Markdown body parser ──────────────────────────────────────────────────────

fn parse_body(body: &str) -> (String, Vec<Section>) {
    let opts =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_SMART_PUNCTUATION;

    let events: Vec<Event<'_>> = Parser::new_ext(body, opts).collect();
    let mut pos = 0;

    let mut title = String::new();
    let mut sections: Vec<Section> = Vec::new();
    let mut current = Section {
        heading: None,
        blocks: Vec::new(),
    };

    while pos < events.len() {
        match &events[pos] {
            // ── H1 → title ───────────────────────────────────────────────
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => {
                pos += 1;
                let (inlines, consumed) =
                    collect_inlines(&events[pos..], TagEnd::Heading(HeadingLevel::H1));
                pos += consumed + 1; // +1 for the End event
                if title.is_empty() {
                    title = inlines_to_plain_text(&inlines);
                }
            }

            // ── H2–H6 → new section ──────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level;
                sections.push(std::mem::replace(
                    &mut current,
                    Section {
                        heading: None,
                        blocks: Vec::new(),
                    },
                ));
                pos += 1;
                let (inlines, consumed) = collect_inlines(&events[pos..], TagEnd::Heading(level));
                pos += consumed + 1;
                let text = inlines_to_plain_text(&inlines);
                current.heading = Some(Heading {
                    level: heading_level_to_u8(level),
                    id: slugify(&text),
                    text,
                });
            }

            // ── Any other block ───────────────────────────────────────────
            _ => {
                let (block, consumed) = parse_block(&events[pos..]);
                pos += consumed;
                if let Some(b) = block {
                    current.blocks.push(b);
                }
            }
        }
    }

    sections.push(current);

    if sections
        .first()
        .map(|s| s.heading.is_none() && s.blocks.is_empty())
        .unwrap_or(false)
    {
        sections.remove(0);
    }

    (title, sections)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn push_inline(stack: &mut Vec<Vec<Inline>>, inline: Inline) {
    if let Some(frame) = stack.last_mut() {
        frame.push(inline);
    }
}

fn inlines_to_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) => out.push_str(t),
            Inline::Code(t) => out.push_str(t),
            Inline::Strong(ch) | Inline::Em(ch) => out.push_str(&inlines_to_plain_text(ch)),
            Inline::Link { children, .. } => out.push_str(&inlines_to_plain_text(children)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
        }
    }
    out
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn parse_ad_comment(html: &str) -> Option<String> {
    let inner = html.trim().strip_prefix("<!--")?.strip_suffix("-->")?;
    let slot = inner.trim().strip_prefix("ad:")?;
    Some(slot.trim().to_string())
}

// ── Front matter ─────────────────────────────────────────────────────────────

/// Splits `+++\nkey = value\n+++\n` front matter from the body.
/// Returns `("", raw)` if no front matter is present.
fn split_front_matter(raw: &str) -> (&str, &str) {
    let raw = raw.trim_start_matches('\n');
    if !raw.starts_with("+++") {
        return ("", raw);
    }
    let after_open = raw[3..].trim_start_matches('\n');
    if let Some(close) = after_open.find("\n+++") {
        let front = &after_open[..close];
        let body = after_open[close + 4..].trim_start_matches('\n');
        (front, body)
    } else {
        ("", raw)
    }
}

fn front_matter_value(front: &str, key: &str) -> Option<String> {
    for line in front.lines() {
        let line = line.trim();
        let eq = line.find('=')?;
        let k = line[..eq].trim();
        if k == key {
            let v = line[eq + 1..].trim().trim_matches('"');
            return Some(v.to_string());
        }
    }
    None
}

// ── Snippet / shuffle / slug ──────────────────────────────────────────────────

fn first_text_snippet(article: &Article) -> String {
    for section in &article.sections {
        for block in &section.blocks {
            if let Block::Paragraph(inlines) = block {
                let text = inlines_to_plain_text(inlines);
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return if trimmed.len() > 120 {
                        format!("{}…", &trimmed[..120])
                    } else {
                        trimmed
                    };
                }
            }
        }
    }
    String::new()
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn random_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % len
}

fn shuffle<T>(slice: &mut [T]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345);
    let n = slice.len();
    let mut state = seed;
    for i in (1..n).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        slice.swap(i, (state as usize) % (i + 1));
    }
}

/// Parse one block-level element from the front of `events`.
/// Returns `(Option<Block>, events_consumed)`.
fn parse_block(events: &[Event<'_>]) -> (Option<Block>, usize) {
    if events.is_empty() {
        return (None, 0);
    }
    match &events[0] {
        Event::Start(Tag::Paragraph) => {
            let (inlines, consumed) = collect_inlines(&events[1..], TagEnd::Paragraph);
            (
                if inlines.is_empty() {
                    None
                } else {
                    Some(Block::Paragraph(inlines))
                },
                1 + consumed + 1,
            )
        }

        Event::Html(html) | Event::InlineHtml(html) => {
            let block = parse_ad_comment(html).map(Block::Ad);
            (block, 1)
        }

        Event::Start(Tag::List(first_num)) => {
            let ordered = first_num.is_some();
            let (items, consumed) = collect_list_items(&events[1..]);
            (Some(Block::List { ordered, items }), 1 + consumed + 1)
        }

        Event::Start(Tag::Table(_)) => {
            let (table, consumed) = collect_table(&events[1..]);
            (Some(table), 1 + consumed + 1)
        }

        Event::Start(Tag::BlockQuote(_)) => {
            let (blocks, consumed) = collect_block_children(&events[1..], TagEnd::BlockQuote(None));
            (Some(Block::BlockQuote(blocks)), 1 + consumed + 1)
        }

        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                pulldown_cmark::CodeBlockKind::Indented => String::new(),
            };
            let (code, consumed) = collect_code_text(&events[1..]);
            (Some(Block::CodeBlock { lang, code }), 1 + consumed + 1)
        }

        Event::Rule => (Some(Block::Rule), 1),

        // Skip soft/hard breaks at block level, end-tags that bubble up, etc.
        _ => (None, 1),
    }
}

/// Collect inline events until the matching end tag. Returns `(inlines, events_consumed_before_end)`.
fn collect_inlines<'a>(events: &[Event<'a>], end: TagEnd) -> (Vec<Inline>, usize) {
    let mut inlines = Vec::new();
    let mut pos = 0;
    let mut link_meta: Vec<(String, String)> = Vec::new(); // (href, title)
    let mut img_meta: Vec<(String, String)> = Vec::new(); // (src,  title)
    // Stack of child-inline collectors for nested strong/em/link/image.
    let mut stack: Vec<Vec<Inline>> = vec![Vec::new()];

    while pos < events.len() {
        match &events[pos] {
            Event::End(t) if *t == end => {
                inlines = stack.pop().unwrap_or_default();
                return (inlines, pos);
            }

            Event::Text(t) => {
                let s = t.to_string();
                stack.last_mut().unwrap().push(Inline::Text(s));
            }
            Event::Code(t) => {
                stack.last_mut().unwrap().push(Inline::Code(t.to_string()));
            }
            Event::SoftBreak => {
                stack.last_mut().unwrap().push(Inline::SoftBreak);
            }
            Event::HardBreak => {
                stack.last_mut().unwrap().push(Inline::HardBreak);
            }

            Event::Html(h) | Event::InlineHtml(h) => {
                // inline HTML — treat as raw text
                stack.last_mut().unwrap().push(Inline::Text(h.to_string()));
            }

            Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis) => {
                stack.push(Vec::new());
            }
            Event::End(TagEnd::Strong) => {
                let ch = stack.pop().unwrap_or_default();
                stack.last_mut().unwrap().push(Inline::Strong(ch));
            }
            Event::End(TagEnd::Emphasis) => {
                let ch = stack.pop().unwrap_or_default();
                stack.last_mut().unwrap().push(Inline::Em(ch));
            }

            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                link_meta.push((dest_url.to_string(), title.to_string()));
                stack.push(Vec::new());
            }
            Event::End(TagEnd::Link) => {
                let children = stack.pop().unwrap_or_default();
                let (href, lt) = link_meta.pop().unwrap_or_default();
                stack.last_mut().unwrap().push(Inline::Link {
                    href,
                    title: lt,
                    children,
                });
            }

            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                img_meta.push((dest_url.to_string(), title.to_string()));
                stack.push(Vec::new()); // alt text children
            }
            Event::End(TagEnd::Image) => {
                let alt_inlines = stack.pop().unwrap_or_default();
                let alt = inlines_to_plain_text(&alt_inlines);
                let (src, img_title) = img_meta.pop().unwrap_or_default();
                stack.last_mut().unwrap().push(Inline::Image {
                    src,
                    alt,
                    title: img_title,
                });
            }

            _ => {}
        }
        pos += 1;
    }

    (stack.pop().unwrap_or_default(), pos)
}

/// Collect list items until `End(List)`.
fn collect_list_items(events: &[Event<'_>]) -> (Vec<ListItem>, usize) {
    let mut items = Vec::new();
    let mut pos = 0;

    while pos < events.len() {
        match &events[pos] {
            Event::End(TagEnd::List(_)) => return (items, pos),
            Event::Start(Tag::Item) => {
                pos += 1;
                let (blocks, consumed) = collect_block_children(&events[pos..], TagEnd::Item);
                pos += consumed + 1; // +1 for End(Item)
                items.push(ListItem { blocks });
            }
            _ => {
                pos += 1;
            }
        }
    }
    (items, pos)
}

/// Collect block-level children until the given end tag.
fn collect_block_children(events: &[Event<'_>], end: TagEnd) -> (Vec<Block>, usize) {
    let mut blocks = Vec::new();
    let mut pos = 0;

    while pos < events.len() {
        if let Event::End(t) = &events[pos] {
            if *t == end {
                return (blocks, pos);
            }
        }
        // Headings inside block-children (e.g. blockquote) — treat as paragraph.
        let (block, consumed) = parse_block(&events[pos..]);
        pos += consumed;
        if let Some(b) = block {
            blocks.push(b);
        }
    }
    (blocks, pos)
}

/// Collect a GFM table's headers and rows.
fn collect_table(events: &[Event<'_>]) -> (Block, usize) {
    let mut headers: Vec<Vec<Inline>> = Vec::new();
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut pos = 0;
    let mut in_head = false;
    let mut current_row: Vec<Vec<Inline>> = Vec::new();

    while pos < events.len() {
        match &events[pos] {
            Event::End(TagEnd::Table) => {
                return (Block::Table { headers, rows }, pos);
            }
            Event::Start(Tag::TableHead) => {
                in_head = true;
                pos += 1;
                continue;
            }
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                pos += 1;
                continue;
            }
            Event::Start(Tag::TableRow) => {
                current_row = Vec::new();
                pos += 1;
                continue;
            }
            Event::End(TagEnd::TableRow) => {
                if in_head {
                    headers = current_row.drain(..).collect();
                } else {
                    rows.push(current_row.drain(..).collect());
                }
                pos += 1;
                continue;
            }
            Event::Start(Tag::TableCell) => {
                pos += 1;
                let (inlines, consumed) = collect_inlines(&events[pos..], TagEnd::TableCell);
                pos += consumed + 1;
                current_row.push(inlines);
                continue;
            }
            _ => {}
        }
        pos += 1;
    }
    (Block::Table { headers, rows }, pos)
}

/// Collect raw text inside a code block.
fn collect_code_text(events: &[Event<'_>]) -> (String, usize) {
    let mut code = String::new();
    let mut pos = 0;
    while pos < events.len() {
        match &events[pos] {
            Event::End(TagEnd::CodeBlock) => return (code, pos),
            Event::Text(t) => code.push_str(t),
            _ => {}
        }
        pos += 1;
    }
    (code, pos)
}
