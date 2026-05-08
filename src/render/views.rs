//! View models consumed by Askama templates.
//!
//! All three page modes (article, empty, admin) share a single `PageView`
//! struct. Fields unused in a given mode are left empty — the template only
//! renders what the active content partial touches.

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
    /// Drives `{% if content == "..." %}` branches in the template.
    pub content: String,

    // ── Article mode only (empty otherwise) ───────────────────────────────────
    pub article_slug: String,
    pub sections: Vec<SectionView>,
    pub toc: Vec<TocEntry>,
    pub stories: Vec<StoryHeader>,

    // ── Admin list mode only (empty otherwise) ────────────────────────────────
    pub admin_articles: Vec<AdminArticleEntry>,
}

impl PageView {
    /// Convenience: build the shared fields that every mode needs.
    pub fn base(
        site_title: String,
        page_title: String,
        theme: String,
        year: u16,
        is_mobile: bool,
        is_admin: bool,
        footer: crate::config::FooterConfig,
        content: PageContent,
    ) -> Self {
        Self {
            site_title,
            page_title,
            theme,
            year,
            is_mobile,
            is_admin,
            footer,
            content: content.as_str().to_string(),
            article_slug: String::new(),
            sections: Vec::new(),
            toc: Vec::new(),
            stories: Vec::new(),
            admin_articles: Vec::new(),
        }
    }
}

// ── Admin article entry ───────────────────────────────────────────────────────

/// Lightweight row shown in the admin article list.
/// Moved here from the inline struct in `handlers.rs`.
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
}

// ── Inlines ───────────────────────────────────────────────────────────────────

/// A rendered inline node.
///
/// `kind`: `"text"` | `"softbreak"` | `"hardbreak"` | `"code"` |
///         `"strong"` | `"em"` | `"link"` | `"image"`
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
    pub fn leaf(kind: &str, text: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            text: text.into(),
            ..Self::empty()
        }
    }

    pub fn with_children(kind: &str, children: Vec<InlineView>) -> Self {
        Self {
            kind: kind.into(),
            children,
            ..Self::empty()
        }
    }

    pub fn empty() -> Self {
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

// ── Blocks ────────────────────────────────────────────────────────────────────

/// A block-level view node.
///
/// `kind`: `"paragraph"` | `"ad"` | `"list"` | `"table"` |
///         `"blockquote"` | `"rule"` | `"code"`
#[derive(Clone)]
pub struct BlockView {
    pub kind: String,
    // paragraph
    pub inlines: Vec<InlineView>,
    // ad
    pub slot: String,
    // list
    pub ordered: bool,
    pub items: Vec<ListItemView>,
    // table
    pub headers: Vec<Vec<InlineView>>,
    pub rows: Vec<Vec<Vec<InlineView>>>,
    // blockquote / nested blocks
    pub children: Vec<BlockView>,
    // code block
    pub lang: String,
    pub code: String,
}

impl BlockView {
    pub fn empty() -> Self {
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

#[derive(Clone)]
pub struct ListItemView {
    pub blocks: Vec<BlockView>,
}

// ── Sections ──────────────────────────────────────────────────────────────────

/// A section view — maps 1-to-1 with `domain::Section`.
#[derive(Clone)]
pub struct SectionView {
    pub has_heading: bool,
    pub heading_level: String, // "h2" .. "h6"
    pub heading_id: String,
    pub heading_text: String,
    pub blocks: Vec<BlockView>,
}

// ── Re-export domain type used in PageView ────────────────────────────────────

pub use crate::domain::StoryHeader;
