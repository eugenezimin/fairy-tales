//! Domain model.
//!
//! Pure data types that represent the core concepts of the application.
//! This module has zero dependencies on storage, HTTP, or rendering concerns.
//! All other modules depend on this one; this one depends on nothing.

// ── Inline content ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    SoftBreak,
    HardBreak,
    /// Inline `code` span.
    Code(String),
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

// ── Block content ─────────────────────────────────────────────────────────────

/// A block-level element inside a section.
#[derive(Debug, Clone)]
pub enum Block {
    Paragraph(Vec<Inline>),
    /// Advertisement placeholder, identified by slot name.
    Ad(String),
    /// Ordered or unordered list.
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    /// GFM table.
    Table {
        headers: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// Block quote.
    BlockQuote(Vec<Block>),
    /// Thematic break `<hr>`.
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

// ── Article structure ─────────────────────────────────────────────────────────

/// A logical section of an article.
///
/// A section begins at a heading (h2–h6) and contains all blocks up to the
/// next heading of equal or higher rank. The intro section before the first
/// heading has `heading: None`.
#[derive(Debug, Clone)]
pub struct Section {
    pub heading: Option<Heading>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct Heading {
    /// 2..=6  (h1 is consumed as the article title)
    pub level: u8,
    /// URL-safe id derived from the heading text (for anchor links / TOC).
    pub id: String,
    /// Plain-text label shown in the TOC.
    pub text: String,
}

/// A fully-parsed article ready for rendering.
#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub slug: String,
    pub author: String,
    pub published: String,
    pub sections: Vec<Section>,
}

/// Lightweight card shown in the sidebar — no body content.
#[derive(Debug, Clone)]
pub struct StoryHeader {
    pub title: String,
    pub slug: String,
    pub snippet: String,
}

/// Everything a single page render needs.
pub struct ContentBundle {
    pub article: Article,
    pub stories: Vec<StoryHeader>,
}
