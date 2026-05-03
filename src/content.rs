//! Content loading.
//!
//! Scans `content/` for every `.toml` file; each file is one article.
//! One article is selected at random to render on the main page.
//! The sidebar shows a random subset of story headers from all articles.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::ContentConfig;

// ---------- Raw TOML schema ----------
// Matches the article-example.toml format exactly.

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
    /// Returns the section id (used for TOC anchors).
    pub fn id(&self) -> &str {
        match self {
            Section::Heading { id, .. } => id,
            Section::Paragraphs { id, .. } => id,
            Section::Ad { id, .. } => id,
        }
    }

    /// Returns the heading text if this section has one.
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
    /// The article shown in the main column (randomly selected).
    pub article: Article,
    /// Story cards for the sidebar (randomly ordered, all articles).
    pub stories: Vec<StoryHeader>,
}

// ---------- Loader ----------

pub fn load(cfg: &ContentConfig) -> Result<ContentBundle> {
    let articles = load_all_articles(&cfg.dir)?;
    anyhow::ensure!(
        !articles.is_empty(),
        "no .toml files found in {:?}",
        cfg.dir
    );

    // Pick a random article to display using a simple time-based seed.
    // (No external RNG dependency needed for this level of randomness.)
    let idx = random_index(articles.len());
    let main_article = articles[idx].clone();

    // Build story headers from all articles, in a shuffled order.
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

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let raw_article: RawArticle =
            toml::from_str(&raw).with_context(|| format!("parsing TOML at {}", path.display()))?;

        let article = process_article(raw_article);
        articles.push(article);
    }

    // Sort by slug for deterministic ordering before random selection.
    articles.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(articles)
}

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

// ---------- Inline parser ----------
//
// Syntax supported inside paragraph strings:
//   [[image:src=/path|alt=Text|flow=right]]
//   [[link:src=/url|text=Label]]

fn parse_paragraph(raw: &str) -> Paragraph {
    let mut inlines = Vec::new();
    let mut rest = raw;

    while let Some(start) = rest.find("[[") {
        // Emit plain text before the tag.
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
            // Malformed — emit the rest as plain text.
            inlines.push(Inline::Text(rest[start..].to_string()));
            rest = "";
            break;
        }
    }

    // Trailing plain text.
    if !rest.is_empty() {
        inlines.push(Inline::Text(rest.to_string()));
    }

    Paragraph(inlines)
}

fn parse_inline_tag(body: &str) -> Option<Inline> {
    // body is like "image:src=/foo|alt=Bar|flow=right"
    // or "link:src=/home|text=click here"
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

/// Pull the first ~120 chars of plain text from an article for the sidebar.
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

/// Pseudo-random index based on current time nanoseconds.
fn random_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len
}

/// Fisher-Yates shuffle using time-based seed.
fn shuffle<T>(slice: &mut [T]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345);

    let n = slice.len();
    let mut state = seed;

    for i in (1..n).rev() {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        slice.swap(i, j);
    }
}
