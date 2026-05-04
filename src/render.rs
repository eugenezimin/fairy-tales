//! HTML rendering.
//!
//! Translates the `ContentBundle` domain model into view models that the
//! Askama template can consume without logic.

use anyhow::{Context, Result};
use askama::Template;

use crate::config::{SiteConfig, ThemeConfig};
use crate::content::{Article, Block, ContentBundle, Inline, Section, StoryHeader};

// ── View models ───────────────────────────────────────────────────────────────

/// One entry in the table of contents.
#[derive(Clone)]
pub struct TocEntry {
    pub anchor: String,
    pub label: String,
    /// "h2" | "h3" | "h4" | "h5" | "h6"
    pub level: String,
}

/// A rendered inline node.
/// `kind`: "text" | "softbreak" | "hardbreak" | "code" | "strong" | "em" | "link" | "image"
#[derive(Clone)]
pub struct InlineView {
    pub kind: String,
    // text / code
    pub text: String,
    // strong / em / link children
    pub children: Vec<InlineView>,
    // link
    pub href: String,
    pub link_title: String,
    // image
    pub src: String,
    pub alt: String,
    pub img_title: String,
}

impl InlineView {
    fn leaf(kind: &str, text: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            text: text.into(),
            ..Self::empty()
        }
    }
    fn with_children(kind: &str, children: Vec<InlineView>) -> Self {
        Self {
            kind: kind.into(),
            children,
            ..Self::empty()
        }
    }
    fn empty() -> Self {
        Self {
            kind: String::new(),
            text: String::new(),
            children: Vec::new(),
            href: String::new(),
            link_title: String::new(),
            src: String::new(),
            alt: String::new(),
            img_title: String::new(),
        }
    }
}

/// A block-level view node.
/// `kind`: "paragraph" | "ad"
#[derive(Clone)]
pub struct BlockView {
    pub kind: String,
    // paragraph / inline children
    pub inlines: Vec<InlineView>,
    // ad
    pub slot: String,
    // list
    pub ordered: bool,
    pub items: Vec<ListItemView>,
    // table
    pub headers: Vec<Vec<InlineView>>,
    pub rows: Vec<Vec<Vec<InlineView>>>,
    // blockquote / nested
    pub children: Vec<BlockView>,
    // code block
    pub lang: String,
    pub code: String,
}

#[derive(Clone)]
pub struct ListItemView {
    pub blocks: Vec<BlockView>,
}

/// A section view (maps 1-to-1 with `content::Section`).
#[derive(Clone)]
pub struct SectionView {
    pub has_heading: bool,
    pub heading_level: String, // "h2" .. "h6"
    pub heading_id: String,
    pub heading_text: String,
    pub blocks: Vec<BlockView>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexView {
    site_title: String,
    page_title: String,
    article_slug: String,
    theme: String,
    stories: Vec<StoryHeader>,
    toc: Vec<TocEntry>,
    sections: Vec<SectionView>,
    year: u16,
    is_mobile: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn render_index(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
) -> Result<String> {
    let toc = build_toc(&bundle.article);
    let sections = build_section_views(&bundle.article);

    IndexView {
        site_title: site.title.clone(),
        page_title: bundle.article.title.clone(),
        article_slug: bundle.article.slug.clone(),
        theme: theme.name.clone(),
        stories: bundle.stories.clone(),
        toc,
        sections,
        year: site.footer_year,
        is_mobile,
    }
    .render()
    .context("rendering index template")
}

// ── TOC ───────────────────────────────────────────────────────────────────────

fn build_toc(article: &Article) -> Vec<TocEntry> {
    article
        .sections
        .iter()
        .filter_map(|s| s.heading.as_ref())
        .map(|h| TocEntry {
            anchor: h.id.clone(),
            label: h.text.clone(),
            level: format!("h{}", h.level),
        })
        .collect()
}

// ── Section / Block / Inline conversion ───────────────────────────────────────

fn build_section_views(article: &Article) -> Vec<SectionView> {
    article.sections.iter().map(section_view).collect()
}

fn section_view(s: &Section) -> SectionView {
    SectionView {
        has_heading: s.heading.is_some(),
        heading_level: s
            .heading
            .as_ref()
            .map(|h| format!("h{}", h.level))
            .unwrap_or_default(),
        heading_id: s.heading.as_ref().map(|h| h.id.clone()).unwrap_or_default(),
        heading_text: s
            .heading
            .as_ref()
            .map(|h| h.text.clone())
            .unwrap_or_default(),
        blocks: s.blocks.iter().map(block_view).collect(),
    }
}

fn block_view(b: &Block) -> BlockView {
    match b {
        Block::Paragraph(inlines) => BlockView {
            kind: "paragraph".into(),
            inlines: inlines.iter().map(inline_view).collect(),
            ..BlockView::empty()
        },
        Block::Ad(slot) => BlockView {
            kind: "ad".into(),
            slot: slot.clone(),
            ..BlockView::empty()
        },
        Block::List { ordered, items } => BlockView {
            kind: "list".into(),
            ordered: *ordered,
            items: items
                .iter()
                .map(|item| ListItemView {
                    blocks: item.blocks.iter().map(block_view).collect(),
                })
                .collect(),
            ..BlockView::empty()
        },
        Block::Table { headers, rows } => BlockView {
            kind: "table".into(),
            headers: headers
                .iter()
                .map(|cell| cell.iter().map(inline_view).collect())
                .collect(),
            rows: rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.iter().map(inline_view).collect())
                        .collect()
                })
                .collect(),
            ..BlockView::empty()
        },
        Block::BlockQuote(blocks) => BlockView {
            kind: "blockquote".into(),
            children: blocks.iter().map(block_view).collect(),
            ..BlockView::empty()
        },
        Block::Rule => BlockView {
            kind: "rule".into(),
            ..BlockView::empty()
        },
        Block::CodeBlock { lang, code } => BlockView {
            kind: "code".into(),
            lang: lang.clone(),
            code: code.clone(),
            ..BlockView::empty()
        },
    }
}

impl BlockView {
    fn empty() -> Self {
        Self {
            kind: String::new(),
            inlines: Vec::new(),
            slot: String::new(),
            ordered: false,
            items: Vec::new(),
            headers: Vec::new(),
            rows: Vec::new(),
            children: Vec::new(),
            lang: String::new(),
            code: String::new(),
        }
    }
}

fn inline_view(i: &Inline) -> InlineView {
    match i {
        Inline::Text(t) => InlineView::leaf("text", t),
        Inline::Code(t) => InlineView::leaf("code", t),
        Inline::SoftBreak => InlineView::leaf("softbreak", ""),
        Inline::HardBreak => InlineView::leaf("hardbreak", ""),
        Inline::Strong(ch) => {
            InlineView::with_children("strong", ch.iter().map(inline_view).collect())
        }
        Inline::Em(ch) => InlineView::with_children("em", ch.iter().map(inline_view).collect()),
        Inline::Link {
            href,
            title,
            children,
        } => InlineView {
            kind: "link".into(),
            href: href.clone(),
            link_title: title.clone(),
            children: children.iter().map(inline_view).collect(),
            ..InlineView::empty()
        },
        Inline::Image { src, alt, title } => InlineView {
            kind: "image".into(),
            src: src.clone(),
            alt: alt.clone(),
            img_title: title.clone(),
            ..InlineView::empty()
        },
    }
}
