//! Content loading.
//!
//! On every request the content directory is rescanned so new articles
//! are picked up without a restart. Only the article that will actually
//! be rendered is fully parsed; all others are read just far enough to
//! extract the title and slug for the sidebar.

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
    #[serde(default)]
    pub slot: String,
}

// ---------- Domain types ----------

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

#[derive(Debug, Clone)]
pub struct Paragraph(pub Vec<Inline>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingLevel {
    H2,
    H3,
    H4,
}

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

/// Everything the page needs.
pub struct ContentBundle {
    pub article: Article,
    pub stories: Vec<StoryHeader>,
}

// ---------- Public API ----------

/// Rescan the content directory, then fully parse only the article to render.
///
/// `requested_slug` — `Some("my-slug")` for a specific article, `None` to pick randomly.
pub fn load(cfg: &ContentConfig, requested_slug: Option<&str>) -> Result<ContentBundle> {
    let metas = scan_metas(&cfg.dir)?;
    anyhow::ensure!(
        !metas.is_empty(),
        "no .toml or .md files found in {:?}",
        cfg.dir
    );

    let chosen_idx = if let Some(slug) = requested_slug {
        metas
            .iter()
            .position(|m| m.slug == slug)
            .with_context(|| format!("no article with slug '{slug}'"))?
    } else {
        random_index(metas.len())
    };

    // Sidebar cards come from cheap metadata — no sections parsed.
    let mut stories: Vec<StoryHeader> = metas
        .iter()
        .map(|m| StoryHeader {
            title: m.title.clone(),
            slug: m.slug.clone(),
            snippet: String::new(),
        })
        .collect();

    // Fully parse only the chosen article.
    let article = load_one_article(&metas[chosen_idx].path)?;
    stories[chosen_idx].snippet = first_text_snippet(&article);

    shuffle(&mut stories);

    Ok(ContentBundle { article, stories })
}

// ---------- Scanning ----------

struct ArticleMeta {
    title: String,
    slug: String,
    path: std::path::PathBuf,
}

/// Read every content file just enough to extract title + slug.
fn scan_metas(dir: &Path) -> Result<Vec<ArticleMeta>> {
    #[derive(Deserialize)]
    struct TitleOnly {
        title: String,
        #[serde(default)]
        slug: String,
    }

    let mut metas = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading content directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| "reading directory entry")?;
        let path = entry.path();

        let ext = path.extension().and_then(|e| e.to_str());
        let (title, slug) = match ext {
            Some("toml") => {
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let t: TitleOnly = toml::from_str(&raw)
                    .with_context(|| format!("parsing title from {}", path.display()))?;
                let slug = if t.slug.is_empty() {
                    slugify(&t.title)
                } else {
                    t.slug
                };
                (t.title, slug)
            }
            Some("md") => {
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let (front, _) = split_front_matter(&raw);
                let mut title = String::new();
                let mut slug = String::new();
                for line in front.lines() {
                    if let Some((k, v)) = split_kv(line.trim()) {
                        match k {
                            "title" => title = v.to_string(),
                            "slug" => slug = v.to_string(),
                            _ => {}
                        }
                    }
                }
                if title.is_empty() {
                    continue; // skip files with no title
                }
                if slug.is_empty() {
                    slug = slugify(&title);
                }
                (title, slug)
            }
            _ => continue,
        };

        metas.push(ArticleMeta { title, slug, path });
    }

    metas.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(metas)
}

/// Fully parse a single article file (TOML or Markdown).
fn load_one_article(path: &Path) -> Result<Article> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => {
            let raw_article: RawArticle = toml::from_str(&raw)
                .with_context(|| format!("parsing TOML at {}", path.display()))?;
            Ok(process_article(raw_article))
        }
        Some("md") => parse_markdown_article(&raw)
            .with_context(|| format!("parsing Markdown at {}", path.display())),
        _ => anyhow::bail!("unsupported file type: {}", path.display()),
    }
}

// ---------- Markdown parser ----------

fn parse_markdown_article(raw: &str) -> Result<Article> {
    let (front, body) = split_front_matter(raw);

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

fn split_front_matter(raw: &str) -> (&str, &str) {
    let raw = raw.trim_start_matches('\n');
    if !raw.starts_with("+++") {
        return ("", raw);
    }
    let after_open = &raw[3..].trim_start_matches('\n');
    if let Some(close) = after_open.find("\n+++") {
        let front = &after_open[..close];
        let body = &after_open[close + 4..];
        let body = body.trim_start_matches('\n');
        (front, body)
    } else {
        ("", raw)
    }
}

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

fn parse_md_body(body: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut pending_paras: Vec<Paragraph> = Vec::new();
    let mut current_heading: Option<(HeadingLevel, String, String, Vec<Paragraph>)> = None;
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

        if line.trim().is_empty() {
            emit_para(&mut current_para_lines, &mut pending_paras);
            continue;
        }

        current_para_lines.push(line);
    }

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

fn parse_ad_comment(line: &str) -> Option<String> {
    let line = line.trim();
    let inner = line.strip_prefix("<!--")?.strip_suffix("-->")?;
    let inner = inner.trim();
    let slot = inner.strip_prefix("ad:")?;
    Some(slot.trim().to_string())
}

fn parse_md_paragraph(raw: &str) -> Paragraph {
    let mut inlines = Vec::new();
    let mut rest = raw;

    while !rest.is_empty() {
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
            inlines.push(Inline::Text(rest[img_start..img_start + 2].to_string()));
            rest = &rest[img_start + 2..];
            continue;
        }

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
            inlines.push(Inline::Text(rest[link_start..link_start + 1].to_string()));
            rest = &rest[link_start + 1..];
            continue;
        }

        inlines.push(Inline::Text(rest.to_string()));
        break;
    }

    Paragraph(inlines)
}

// ---------- TOML processing ----------

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
