//! Markdown-to-domain-model parser.
//!
//! Converts a raw Markdown string (body only — front matter already stripped)
//! into an `Article`. Uses `pulldown-cmark` for event generation; the event
//! stream is then consumed by hand so we can map it to our own domain types
//! rather than emitting HTML directly.

use anyhow::{Result, anyhow};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::domain::{Article, Block, Heading, Inline, ListItem, Section};
use crate::repository::fs::front_matter;

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a complete raw article string (including optional front matter).
pub fn parse_article(raw: &str) -> Result<Article> {
    let (front, body) = front_matter::split(raw);
    let slug_fm = front_matter::value(front, "slug").unwrap_or_default();
    let author = front_matter::value(front, "author").unwrap_or_default();
    let published = front_matter::value(front, "published").unwrap_or_default();
    let description = front_matter::value(front, "description").unwrap_or_default();
    let keywords = front_matter::value(front, "keywords").unwrap_or_default();
    let cover = front_matter::value(front, "cover").unwrap_or_default();
    let og_title = front_matter::value(front, "og_title").unwrap_or_default();
    let category = front_matter::value(front, "category").unwrap_or_default();
    let tags = front_matter::value(front, "tags").unwrap_or_default();
    let reading_time = front_matter::value(front, "reading_time").unwrap_or_default();
    let featured = front_matter::value(front, "featured")
        .map(|v| v == "true")
        .unwrap_or(false);

    let (title, sections) = parse_body(body);

    let raw = raw.trim();
    anyhow::ensure!(!raw.is_empty(), "content is empty");

    // Rough binary/non-UTF8 guard: if >30% of the first 256 bytes
    // are non-ASCII-printable (excluding common whitespace), reject it.
    let sample = raw.as_bytes().iter().take(256);
    let non_print = sample
        .filter(|&&b| b < 0x09 || (b > 0x0d && b < 0x20) || b == 0x7f)
        .count();
    anyhow::ensure!(non_print == 0, "content appears to be a binary file");

    if title.is_empty() {
        return Err(anyhow!("article has no H1 title"));
    }

    let slug = if slug_fm.is_empty() {
        super::util::slugify(&title)
    } else {
        slug_fm
    };

    Ok(Article {
        title,
        slug,
        author,
        published,
        description,
        keywords,
        cover,
        og_title,
        category,
        tags,
        reading_time,
        featured,
        sections,
    })
}

/// Cheap scan: return just the title (first H1) without parsing the full body.
pub fn extract_title(body: &str) -> String {
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

// ── Body parser ───────────────────────────────────────────────────────────────

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
            // H1 → article title
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => {
                pos += 1;
                let (inlines, consumed) =
                    collect_inlines(&events[pos..], TagEnd::Heading(HeadingLevel::H1));
                pos += consumed + 1;
                if title.is_empty() {
                    title = inlines_to_plain_text(&inlines);
                }
            }

            // H2–H6 → new section
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
                    level: heading_level_u8(level),
                    id: super::util::slugify(&text),
                    text,
                });
            }

            // Any other block
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

    // Drop leading empty section (no heading, no blocks)
    if sections
        .first()
        .map(|s| s.heading.is_none() && s.blocks.is_empty())
        .unwrap_or(false)
    {
        sections.remove(0);
    }

    (title, sections)
}

// ── Block parser ──────────────────────────────────────────────────────────────

/// Parse one block-level element from the front of `events`.
/// Returns `(Option<Block>, events_consumed)`.
pub fn parse_block(events: &[Event<'_>]) -> (Option<Block>, usize) {
    if events.is_empty() {
        return (None, 0);
    }

    match &events[0] {
        Event::Start(Tag::Paragraph) => {
            let (inlines, consumed) = collect_inlines(&events[1..], TagEnd::Paragraph);
            let inlines = normalize_breaks(inlines);
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

        Event::Start(Tag::BlockQuote(kind)) => {
            let end = TagEnd::BlockQuote(*kind);
            let (blocks, consumed) = collect_block_children(&events[1..], end);
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

        // Tight-list inline nodes emitted directly inside Item (no Paragraph wrapper).
        Event::Text(_)
        | Event::Code(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::Start(Tag::Strong)
        | Event::Start(Tag::Emphasis)
        | Event::Start(Tag::Link { .. })
        | Event::Start(Tag::Image { .. }) => {
            let mut pos = 0;
            let mut stack: Vec<Vec<Inline>> = vec![Vec::new()];
            let mut link_meta: Vec<(String, String)> = Vec::new();
            let mut img_meta: Vec<(String, String)> = Vec::new();

            while pos < events.len() {
                match &events[pos] {
                    Event::End(TagEnd::Strong) => {
                        let ch = stack.pop().unwrap_or_default();
                        stack.last_mut().unwrap().push(Inline::Strong(ch));
                    }
                    Event::End(TagEnd::Emphasis) => {
                        let ch = stack.pop().unwrap_or_default();
                        stack.last_mut().unwrap().push(Inline::Em(ch));
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
                    // Block-level or unhandled end → stop
                    Event::Start(Tag::Paragraph)
                    | Event::Start(Tag::Heading { .. })
                    | Event::Start(Tag::List(_))
                    | Event::Start(Tag::BlockQuote(_))
                    | Event::Start(Tag::CodeBlock(_))
                    | Event::Start(Tag::Table(_))
                    | Event::Rule
                    | Event::End(_) => break,

                    Event::Text(t) => {
                        stack.last_mut().unwrap().push(Inline::Text(t.to_string()));
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
                        stack.last_mut().unwrap().push(Inline::Text(h.to_string()));
                    }
                    Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis) => {
                        stack.push(Vec::new());
                    }
                    Event::Start(Tag::Link {
                        dest_url, title, ..
                    }) => {
                        link_meta.push((dest_url.to_string(), title.to_string()));
                        stack.push(Vec::new());
                    }
                    Event::Start(Tag::Image {
                        dest_url, title, ..
                    }) => {
                        img_meta.push((dest_url.to_string(), title.to_string()));
                        stack.push(Vec::new());
                    }
                    _ => {}
                }
                pos += 1;
            }

            let inlines = normalize_breaks(stack.pop().unwrap_or_default());
            if inlines.is_empty() {
                (None, pos)
            } else {
                (Some(Block::Paragraph(inlines)), pos)
            }
        }

        // Skip end-tags / blank lines / unrecognised events
        _ => (None, 1),
    }
}

// ── Inline collector ──────────────────────────────────────────────────────────

/// Collect inline events until the matching end tag.
/// Returns `(inlines, events_consumed_before_end)`.
fn collect_inlines<'a>(events: &[Event<'a>], end: TagEnd) -> (Vec<Inline>, usize) {
    let mut pos = 0;
    let mut link_meta: Vec<(String, String)> = Vec::new();
    let mut img_meta: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<Vec<Inline>> = vec![Vec::new()];

    while pos < events.len() {
        match &events[pos] {
            Event::End(t) if *t == end => {
                return (stack.pop().unwrap_or_default(), pos);
            }

            Event::Text(t) => {
                stack.last_mut().unwrap().push(Inline::Text(t.to_string()));
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
                stack.push(Vec::new());
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

// ── List / table / code collectors ───────────────────────────────────────────

fn collect_list_items(events: &[Event<'_>]) -> (Vec<ListItem>, usize) {
    let mut items = Vec::new();
    let mut pos = 0;

    while pos < events.len() {
        match &events[pos] {
            Event::End(TagEnd::List(_)) => return (items, pos),
            Event::Start(Tag::Item) => {
                pos += 1;
                let (blocks, consumed) = collect_block_children(&events[pos..], TagEnd::Item);
                pos += consumed + 1;
                items.push(ListItem { blocks });
            }
            _ => {
                pos += 1;
            }
        }
    }
    (items, pos)
}

fn collect_block_children(events: &[Event<'_>], end: TagEnd) -> (Vec<Block>, usize) {
    let mut blocks = Vec::new();
    let mut pos = 0;

    while pos < events.len() {
        if let Event::End(t) = &events[pos] {
            let matches = match (&end, t) {
                (TagEnd::BlockQuote(_), TagEnd::BlockQuote(_)) => true,
                (TagEnd::Item, TagEnd::Item) => true,
                _ => *t == end,
            };
            if matches {
                return (blocks, pos);
            }
        }
        let (block, consumed) = parse_block(&events[pos..]);
        if consumed == 0 {
            pos += 1;
            continue;
        }
        pos += consumed;
        if let Some(b) = block {
            blocks.push(b);
        }
    }
    (blocks, pos)
}

fn collect_table(events: &[Event<'_>]) -> (Block, usize) {
    let mut headers: Vec<Vec<Inline>> = Vec::new();
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut pos = 0;
    let mut in_head = false;
    let mut current_row: Vec<Vec<Inline>> = Vec::new();

    while pos < events.len() {
        match &events[pos] {
            Event::End(TagEnd::Table) => return (Block::Table { headers, rows }, pos),
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

// ── Inline utilities ──────────────────────────────────────────────────────────

pub fn inlines_to_plain_text(inlines: &[Inline]) -> String {
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

/// Promote SoftBreak→HardBreak when it immediately precedes a Link.
fn normalize_breaks(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::with_capacity(inlines.len());
    let mut iter = inlines.into_iter().peekable();
    while let Some(inline) = iter.next() {
        if matches!(inline, Inline::SoftBreak) {
            if matches!(iter.peek(), Some(Inline::Link { .. })) {
                out.push(Inline::HardBreak);
                continue;
            }
        }
        out.push(inline);
    }
    out
}

fn heading_level_u8(level: HeadingLevel) -> u8 {
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
