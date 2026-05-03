//! Content loading.
//!
//! Scans `content/` for every `.toml` and `.md` file; each file is one article.
//! One article is selected at random to render on the main page.
//! The sidebar shows a random subset of story headers from all articles.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::ContentConfig;

// ---------- Raw TOML schema ----------

#[derive(Debug, Clone, Deserialize)]
pub struct RawArticle {
    pub title: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub published: String,
    pub sections: Vec<RawSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSection {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub heading: String,
    #[serde(default)]
    pub paragraphs: Vec<String>,
    /// For `type = "ad"` sections.
    #[serde(default)]
    pub slot: String,
}

// ---------- Domain types ----------

/// A single inline element within a paragraph.
#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Image {
        src: String,
        alt: String,
        flow: String,
    },
    Link {
        href: String,
        text: String,
    },
}

/// A rendered paragraph as a list of inline elements.
#[derive(Debug, Clone)]
pub struct Paragraph(pub Vec<Inline>);

/// A heading level used in articles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingLevel {
    H2,
    H3,
    H4,
}

/// A processed article section ready for the template.
#[derive(Debug, Clone)]
pub enum Section {
    Heading {
        level: HeadingLevel,
        id: String,
        heading: String,
        paragraphs: Vec<Paragraph>,
    },
    Paragraphs {
        id: String,
        paragraphs: Vec<Paragraph>,
    },
    Ad {
        id: String,
        slot: String,
    },
}

impl Section {
    pub fn id(&self) -> &str {
        match self {
            Section::Heading { id, .. } => id,
            Section::Paragraphs { id, .. } => id,
            Section::Ad { id, .. } => id,
        }
    }

    pub fn heading(&self) -> Option<&str> {
        match self {
            Section::Heading { heading, .. } => Some(heading),
            _ => None,
        }
    }
}

/// A fully processed article.
#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub slug: String,
    pub author: String,
    pub published: String,
    pub sections: Vec<Section>,
}

/// A brief card shown in the sidebar.
#[derive(Debug, Clone)]
pub struct StoryHeader {
    pub title: String,
    pub slug: String,
    pub snippet: String,
}

/// Everything the index page needs.
#[derive(Debug, Clone)]
pub struct ContentBundle {
    pub article: Article,
    pub stories: Vec<StoryHeader>,
}

// ---------- Loader ----------

pub fn load(cfg: &ContentConfig) -> Result<ContentBundle> {
    let articles = load_all_articles(&cfg.dir)?;
    anyhow::ensure!(
        !articles.is_empty(),
        "no .toml or .md files found in {:?}",
        cfg.dir
    );

    let idx = random_index(articles.len());
    let main_article = articles[idx].clone();

    let mut stories: Vec<StoryHeader> = articles
        .iter()
        .map(|a| StoryHeader {
            title: a.title.clone(),
            slug: a.slug.clone(),
            snippet: first_text_snippet(a),
        })
        .collect();

    shuffle(&mut stories);

    Ok(ContentBundle {
        article: main_article,
        stories,
    })
}

// ---------- Parsing ----------

fn load_all_articles(dir: &Path) -> Result<Vec<Article>> {
    let mut articles = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading content directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| "reading directory entry")?;
        let path = entry.path();

        let ext = path.extension().and_then(|e| e.to_str());
        let article = match ext {
            Some("toml") => {
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let raw_article: RawArticle = toml::from_str(&raw)
                    .with_context(|| format!("parsing TOML at {}", path.display()))?;
                process_article(raw_article)
            }
            Some("md") => {
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                parse_markdown_article(&raw)
                    .with_context(|| format!("parsing Markdown at {}", path.display()))?
            }
            _ => continue,
        };

        articles.push(article);
    }

    articles.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(articles)
}

// ---------- Markdown parser ----------
//
// Format:
//   Optional TOML front matter fenced by `+++` lines.
//   Body uses standard Markdown headings (## / ### / ####).
//   Images:  ![alt](src){flow=right}
//   Links:   [text](href)
//   Ad slots: <!-- ad: slot-name -->
//   Paragraphs: blank-line separated blocks of text.

fn parse_markdown_article(raw: &str) -> Result<Article> {
    let (front, body) = split_front_matter(raw);

    // Parse front matter fields (simple key = "value" pairs).
    let mut title = String::new();
    let mut slug = String::new();
    let mut author = String::new();
    let mut published = String::new();

    for line in front.lines() {
        let line = line.trim();
        if let Some((key, val)) = split_kv(line) {
            match key {
                "title" => title = val.to_string(),
                "slug" => slug = val.to_string(),
                "author" => author = val.to_string(),
                "published" => published = val.to_string(),
                _ => {}
            }
        }
    }

    anyhow::ensure!(
        !title.is_empty(),
        "Markdown article is missing a `title` in front matter"
    );

    if slug.is_empty() {
        slug = slugify(&title);
    }

    let sections = parse_md_body(body);

    Ok(Article {
        title,
        slug,
        author,
        published,
        sections,
    })
}

/// Split `+++\n...\n+++\n` front matter from body. Returns ("", full_text)
/// if no front matter is present.
fn split_front_matter(raw: &str) -> (&str, &str) {
    let raw = raw.trim_start_matches('\n');
    if !raw.starts_with("+++") {
        return ("", raw);
    }
    // Skip past the opening +++
    let after_open = &raw[3..].trim_start_matches('\n');
    if let Some(close) = after_open.find("\n+++") {
        let front = &after_open[..close];
        let body = &after_open[close + 4..]; // skip \n+++
        let body = body.trim_start_matches('\n');
        (front, body)
    } else {
        ("", raw)
    }
}

/// Parse `key = "value"` or `key = value` lines.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    let val = line[eq + 1..].trim().trim_matches('"');
    if key.is_empty() {
        None
    } else {
        Some((key, val))
    }
}

/// Parse the Markdown body into sections.
///
/// State machine: we accumulate plain paragraphs. When we hit a heading or
/// ad comment we flush accumulated paragraphs then open a new section.
fn parse_md_body(body: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    // Pending paragraphs not yet attached to a heading.
    let mut pending_paras: Vec<Paragraph> = Vec::new();
    // The current open heading section (level, id, heading, paragraphs).
    let mut current_heading: Option<(HeadingLevel, String, String, Vec<Paragraph>)> = None;
    // Counter for generating unique ids for paragraph-only blocks.
    let mut para_block_idx: usize = 0;

    let flush_pending = |sections: &mut Vec<Section>,
                         pending: &mut Vec<Paragraph>,
                         heading: &mut Option<(HeadingLevel, String, String, Vec<Paragraph>)>,
                         idx: &mut usize| {
        if pending.is_empty() {
            return;
        }
        if let Some((_, _, _, hparas)) = heading {
            hparas.append(pending);
        } else {
            *idx += 1;
            sections.push(Section::Paragraphs {
                id: format!("para-{idx}"),
                paragraphs: std::mem::take(pending),
            });
        }
    };

    let flush_heading =
        |sections: &mut Vec<Section>,
         heading: &mut Option<(HeadingLevel, String, String, Vec<Paragraph>)>| {
            if let Some((level, id, h, paras)) = heading.take() {
                sections.push(Section::Heading {
                    level,
                    id,
                    heading: h,
                    paragraphs: paras,
                });
            }
        };

    // Collect non-empty lines into paragraph blocks separated by blank lines.
    let mut current_para_lines: Vec<&str> = Vec::new();

    let emit_para = |line_buf: &mut Vec<&str>, pending: &mut Vec<Paragraph>| {
        if line_buf.is_empty() {
            return;
        }
        let text = line_buf.join(" ");
        line_buf.clear();
        if !text.trim().is_empty() {
            pending.push(parse_md_paragraph(&text));
        }
    };

    for line in body.lines() {
        // Heading?
        if let Some(stripped) = line.strip_prefix("#### ") {
            emit_para(&mut current_para_lines, &mut pending_paras);
            flush_pending(
                &mut sections,
                &mut pending_paras,
                &mut current_heading,
                &mut para_block_idx,
            );
            flush_heading(&mut sections, &mut current_heading);
            let heading = stripped.trim().to_string();
            current_heading = Some((HeadingLevel::H4, slugify(&heading), heading, Vec::new()));
            continue;
        }
        if let Some(stripped) = line.strip_prefix("### ") {
            emit_para(&mut current_para_lines, &mut pending_paras);
            flush_pending(
                &mut sections,
                &mut pending_paras,
                &mut current_heading,
                &mut para_block_idx,
            );
            flush_heading(&mut sections, &mut current_heading);
            let heading = stripped.trim().to_string();
            current_heading = Some((HeadingLevel::H3, slugify(&heading), heading, Vec::new()));
            continue;
        }
        if let Some(stripped) = line.strip_prefix("## ") {
            emit_para(&mut current_para_lines, &mut pending_paras);
            flush_pending(
                &mut sections,
                &mut pending_paras,
                &mut current_heading,
                &mut para_block_idx,
            );
            flush_heading(&mut sections, &mut current_heading);
            let heading = stripped.trim().to_string();
            current_heading = Some((HeadingLevel::H2, slugify(&heading), heading, Vec::new()));
            continue;
        }

        // Ad comment: <!-- ad: slot-name -->
        if let Some(slot) = parse_ad_comment(line) {
            emit_para(&mut current_para_lines, &mut pending_paras);
            flush_pending(
                &mut sections,
                &mut pending_paras,
                &mut current_heading,
                &mut para_block_idx,
            );
            flush_heading(&mut sections, &mut current_heading);
            sections.push(Section::Ad {
                id: format!("ad-{slot}"),
                slot,
            });
            continue;
        }

        // Blank line: end of a paragraph.
        if line.trim().is_empty() {
            emit_para(&mut current_para_lines, &mut pending_paras);
            continue;
        }

        current_para_lines.push(line);
    }

    // Flush whatever remains.
    emit_para(&mut current_para_lines, &mut pending_paras);
    flush_pending(
        &mut sections,
        &mut pending_paras,
        &mut current_heading,
        &mut para_block_idx,
    );
    flush_heading(&mut sections, &mut current_heading);

    sections
}

/// Parse `<!-- ad: slot-name -->` returning `Some("slot-name")`.
fn parse_ad_comment(line: &str) -> Option<String> {
    let line = line.trim();
    let inner = line.strip_prefix("<!--")?.strip_suffix("-->")?;
    let inner = inner.trim();
    let slot = inner.strip_prefix("ad:")?;
    Some(slot.trim().to_string())
}

/// Parse a single paragraph line into inlines, supporting:
///   ![alt](src){flow=right}   → Image
///   [text](href)              → Link
///   everything else           → Text
fn parse_md_paragraph(raw: &str) -> Paragraph {
    let mut inlines = Vec::new();
    let mut rest = raw;

    while !rest.is_empty() {
        // Image: ![alt](src){flow=...} or ![alt](src)
        if let Some(img_start) = rest.find("![") {
            if img_start > 0 {
                inlines.push(Inline::Text(rest[..img_start].to_string()));
            }
            let after = &rest[img_start + 2..];
            if let Some(alt_end) = after.find("](") {
                let alt = &after[..alt_end];
                let after_alt = &after[alt_end + 2..];
                if let Some(src_end) = after_alt.find(')') {
                    let src = &after_alt[..src_end];
                    let after_src = &after_alt[src_end + 1..];
                    // Optional {flow=...}
                    let (flow, consumed) = if after_src.starts_with('{') {
                        if let Some(brace_end) = after_src.find('}') {
                            let attrs = &after_src[1..brace_end];
                            let flow = attrs.strip_prefix("flow=").unwrap_or("center");
                            (flow.to_string(), brace_end + 1)
                        } else {
                            ("center".to_string(), 0)
                        }
                    } else {
                        ("center".to_string(), 0)
                    };
                    inlines.push(Inline::Image {
                        src: src.to_string(),
                        alt: alt.to_string(),
                        flow,
                    });
                    rest = &after_src[consumed..];
                    continue;
                }
            }
            // Malformed image — emit as text.
            inlines.push(Inline::Text(rest[img_start..img_start + 2].to_string()));
            rest = &rest[img_start + 2..];
            continue;
        }

        // Link: [text](href)
        if let Some(link_start) = rest.find('[') {
            if link_start > 0 {
                inlines.push(Inline::Text(rest[..link_start].to_string()));
            }
            let after = &rest[link_start + 1..];
            if let Some(text_end) = after.find("](") {
                let text = &after[..text_end];
                let after_text = &after[text_end + 2..];
                if let Some(href_end) = after_text.find(')') {
                    let href = &after_text[..href_end];
                    inlines.push(Inline::Link {
                        href: href.to_string(),
                        text: text.to_string(),
                    });
                    rest = &after_text[href_end + 1..];
                    continue;
                }
            }
            // Malformed link — emit as text.
            inlines.push(Inline::Text(rest[link_start..link_start + 1].to_string()));
            rest = &rest[link_start + 1..];
            continue;
        }

        // Plain text — no more special tokens.
        inlines.push(Inline::Text(rest.to_string()));
        break;
    }

    Paragraph(inlines)
}

// ---------- TOML processing (unchanged) ----------

fn process_article(raw: RawArticle) -> Article {
    let sections = raw.sections.into_iter().map(process_section).collect();

    let slug = if raw.slug.is_empty() {
        slugify(&raw.title)
    } else {
        raw.slug
    };

    Article {
        title: raw.title,
        slug,
        author: raw.author,
        published: raw.published,
        sections,
    }
}

fn process_section(raw: RawSection) -> Section {
    match raw.kind.as_str() {
        "h2" | "h3" | "h4" => {
            let level = match raw.kind.as_str() {
                "h3" => HeadingLevel::H3,
                "h4" => HeadingLevel::H4,
                _ => HeadingLevel::H2,
            };
            Section::Heading {
                level,
                id: raw.id,
                heading: raw.heading,
                paragraphs: raw.paragraphs.iter().map(|p| parse_paragraph(p)).collect(),
            }
        }
        "paragraphs" => Section::Paragraphs {
            id: raw.id,
            paragraphs: raw.paragraphs.iter().map(|p| parse_paragraph(p)).collect(),
        },
        "ad" => Section::Ad {
            id: raw.id,
            slot: raw.slot,
        },
        other => {
            tracing::warn!(kind = %other, "unknown section type; treating as paragraphs");
            Section::Paragraphs {
                id: raw.id,
                paragraphs: raw.paragraphs.iter().map(|p| parse_paragraph(p)).collect(),
            }
        }
    }
}

// ---------- TOML inline parser ----------

fn parse_paragraph(raw: &str) -> Paragraph {
    let mut inlines = Vec::new();
    let mut rest = raw;

    while let Some(start) = rest.find("[[") {
        if start > 0 {
            inlines.push(Inline::Text(rest[..start].to_string()));
        }

        let after_open = &rest[start + 2..];
        if let Some(end_offset) = after_open.find("]]") {
            let tag_body = &after_open[..end_offset];
            rest = &after_open[end_offset + 2..];

            if let Some(inline) = parse_inline_tag(tag_body) {
                inlines.push(inline);
            }
        } else {
            inlines.push(Inline::Text(rest[start..].to_string()));
            rest = "";
            break;
        }
    }

    if !rest.is_empty() {
        inlines.push(Inline::Text(rest.to_string()));
    }

    Paragraph(inlines)
}

fn parse_inline_tag(body: &str) -> Option<Inline> {
    let colon = body.find(':')?;
    let kind = &body[..colon];
    let attrs_str = &body[colon + 1..];

    let attrs: std::collections::HashMap<&str, &str> = attrs_str
        .split('|')
        .filter_map(|pair| {
            let eq = pair.find('=')?;
            Some((&pair[..eq], &pair[eq + 1..]))
        })
        .collect();

    match kind {
        "image" => Some(Inline::Image {
            src: attrs.get("src").unwrap_or(&"").to_string(),
            alt: attrs.get("alt").unwrap_or(&"").to_string(),
            flow: attrs.get("flow").unwrap_or(&"").to_string(),
        }),
        "link" => Some(Inline::Link {
            href: attrs.get("src").unwrap_or(&"#").to_string(),
            text: attrs.get("text").unwrap_or(&"link").to_string(),
        }),
        _ => {
            tracing::warn!(kind = %kind, "unknown inline tag type");
            None
        }
    }
}

// ---------- Helpers ----------

fn first_text_snippet(article: &Article) -> String {
    for section in &article.sections {
        let paragraphs = match section {
            Section::Heading { paragraphs, .. } => paragraphs,
            Section::Paragraphs { paragraphs, .. } => paragraphs,
            Section::Ad { .. } => continue,
        };
        for para in paragraphs {
            let text: String = para
                .0
                .iter()
                .map(|inline| match inline {
                    Inline::Text(t) => t.as_str(),
                    Inline::Link { text, .. } => text.as_str(),
                    Inline::Image { alt, .. } => alt.as_str(),
                })
                .collect::<Vec<_>>()
                .join("");

            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.chars().take(120).collect::<String>()
                    + if trimmed.len() > 120 { "…" } else { "" };
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
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len
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
        let j = (state as usize) % (i + 1);
        slice.swap(i, j);
    }
}
