//! View models consumed by Askama templates.
//!
//! Pure data — structs and enums only. No constructors, no logic.
//! Construction helpers live in `builder.rs`.

use std::collections::HashMap;

pub use crate::domain::StoryHeader;

// ── Page mode ─────────────────────────────────────────────────────────────────

/// Which content slot the single-frame template should render.
#[derive(Debug, Clone, PartialEq)]
pub enum PageContent {
    Article,
    Empty,
    Admin,
}

impl PageContent {
    /// String tag used in the template for `{% if content == "..." %}` branches
    /// and as the `page--{mode}` body class.
    pub fn as_str(&self) -> &'static str {
        match self {
            PageContent::Article => "article",
            PageContent::Empty => "empty",
            PageContent::Admin => "admin",
        }
    }
}

// ── Top-level page view ───────────────────────────────────────────────────────

/// Everything the single `index.html` frame needs, for any page mode.
pub struct PageView {
    // ── Always present ────────────────────────────────────────────────────────
    pub site_title: String,
    pub page_title: String,
    pub theme: String,
    pub year: u16,
    pub is_mobile: bool,
    pub is_admin: bool,
    pub footer: crate::config::FooterConfig,
    pub content: String,

    // ── Article mode only (empty otherwise) ───────────────────────────────────
    pub article_slug: String,
    pub article_author: String,       // ← new
    pub article_published: String,    // ← new
    pub article_description: String,  // ← new
    pub article_keywords: String,     // ← new
    pub article_cover: String,        // ← new
    pub article_og_title: String,     // ← new
    pub article_category: String,     // ← new
    pub article_tags: String,         // ← new
    pub article_reading_time: String, // ← new
    pub sections: Vec<SectionView>,
    pub toc: Vec<TocEntry>,
    pub stories: Vec<StoryHeader>,

    // ── Pagination (article mode only) ────────────────────────────────────────
    pub current_page: usize,
    pub total_pages: usize,
    pub has_prev: bool,
    pub has_next: bool,

    // ── Admin list mode only (empty otherwise) ────────────────────────────────
    pub admin_articles: Vec<AdminArticleEntry>,
    pub(crate) static_base: String,
    pub strings: HashMap<String, String>,
}

// ── Admin article entry ───────────────────────────────────────────────────────

/// Lightweight row shown in the admin article list.
#[derive(Clone)]
pub struct AdminArticleEntry {
    pub slug: String,
    pub title: String,
    pub preview: String,
}

// ── TOC ───────────────────────────────────────────────────────────────────────

/// One entry in the table of contents.
#[derive(Clone)]
pub struct TocEntry {
    pub anchor: String,
    pub label: String,
    /// `"h2"` | `"h3"` | `"h4"` | `"h5"` | `"h6"`
    pub level: String,
    /// 1-based page number this heading lives on (always 1 when pagination off).
    pub page: usize,
}

// ── Inlines ───────────────────────────────────────────────────────────────────

/// A rendered inline node.
///
/// `kind`: `"text"` | `"softbreak"` | `"hardbreak"` | `"code"` |
///         `"strong"` | `"em"` | `"link"` | `"image"`
#[derive(Clone)]
pub struct InlineView {
    pub kind: String,
    pub text: String,
    pub children: Vec<InlineView>,
    pub href: String,
    pub link_title: String,
    pub src: String,
    pub alt: String,
    pub img_title: String,
}

// ── Blocks ────────────────────────────────────────────────────────────────────

/// A block-level view node.
///
/// `kind`: `"paragraph"` | `"ad"` | `"list"` | `"table"` |
///         `"blockquote"` | `"rule"` | `"code"`
#[derive(Clone)]
pub struct BlockView {
    pub kind: String,
    pub inlines: Vec<InlineView>,
    pub slot: String,
    pub ordered: bool,
    pub items: Vec<ListItemView>,
    pub headers: Vec<Vec<InlineView>>,
    pub rows: Vec<Vec<Vec<InlineView>>>,
    pub children: Vec<BlockView>,
    pub lang: String,
    pub code: String,
}

#[derive(Clone)]
pub struct ListItemView {
    pub blocks: Vec<BlockView>,
}

// ── Sections ──────────────────────────────────────────────────────────────────

/// A section view — maps 1-to-1 with `domain::Section`.
#[derive(Clone)]
pub struct SectionView {
    pub has_heading: bool,
    pub heading_level: String,
    pub heading_id: String,
    pub heading_text: String,
    pub blocks: Vec<BlockView>,
}
