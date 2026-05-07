//! View models consumed by Askama templates.
//!
//! These structs are intentionally dumb — no logic, only data. They exist
//! to decouple the template from the domain model so neither side needs to
//! know about the other's internals.

/// One entry in the table of contents.
#[derive(Clone)]
pub struct TocEntry {
    pub anchor: String,
    pub label: String,
    /// `"h2"` | `"h3"` | `"h4"` | `"h5"` | `"h6"`
    pub level: String,
}

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

/// A section view — maps 1-to-1 with `domain::Section`.
#[derive(Clone)]
pub struct SectionView {
    pub has_heading: bool,
    pub heading_level: String, // "h2" .. "h6"
    pub heading_id: String,
    pub heading_text: String,
    pub blocks: Vec<BlockView>,
}
