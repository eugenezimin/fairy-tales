//! Domain-model → view-model conversion.
//!
//! Pure functions; no I/O, no template logic.
//! Also owns the construction helpers for view types (`PageView::base`,
//! `InlineView::leaf/with_children/empty`, `BlockView::empty`) — these are
//! part of the conversion concern, not the type definitions.

use std::collections::HashMap;

use crate::config::FooterConfig;
use crate::domain::{Article, Block, Inline, Section};
use crate::render::views::{
    BlockView, InlineView, ListItemView, PageContent, PageView, SectionView, TocEntry,
};

// ── PageView construction ─────────────────────────────────────────────────────

impl PageView {
    /// Build the shared fields that every page mode needs.
    pub fn base(
        site_title: String,
        page_title: String,
        theme: String,
        year: u16,
        is_mobile: bool,
        is_admin: bool,
        footer: FooterConfig,
        content: PageContent,
        static_base: String,
        strings: HashMap<String, String>,
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
            static_base,
            strings,
        }
    }
}

// ── InlineView construction ───────────────────────────────────────────────────

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

// ── BlockView construction ────────────────────────────────────────────────────

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
// ── TOC ───────────────────────────────────────────────────────────────────────

pub fn build_toc(article: &Article) -> Vec<TocEntry> {
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

// ── Sections ──────────────────────────────────────────────────────────────────

pub fn build_section_views(article: &Article, static_base: &str) -> Vec<SectionView> {
    article
        .sections
        .iter()
        .map(|s| section_view(s, static_base))
        .collect()
}

fn section_view(s: &Section, static_base: &str) -> SectionView {
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
        blocks: s
            .blocks
            .iter()
            .map(|b| block_view(b, static_base))
            .collect(),
    }
}

// ── Blocks ────────────────────────────────────────────────────────────────────

pub fn block_view(b: &Block, static_base: &str) -> BlockView {
    match b {
        Block::Paragraph(inlines) => BlockView {
            kind: "paragraph".into(),
            inlines: inlines
                .iter()
                .map(|i| inline_view(i, static_base))
                .collect(),
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
                    blocks: item
                        .blocks
                        .iter()
                        .map(|b| block_view(b, static_base))
                        .collect(),
                })
                .collect(),
            ..BlockView::empty()
        },
        Block::Table { headers, rows } => BlockView {
            kind: "table".into(),
            headers: headers
                .iter()
                .map(|cell| cell.iter().map(|i| inline_view(i, static_base)).collect())
                .collect(),
            rows: rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.iter().map(|i| inline_view(i, static_base)).collect())
                        .collect()
                })
                .collect(),
            ..BlockView::empty()
        },
        Block::BlockQuote(blocks) => BlockView {
            kind: "blockquote".into(),
            children: blocks.iter().map(|b| block_view(b, static_base)).collect(),
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

// ── Inlines ───────────────────────────────────────────────────────────────────

pub fn inline_view(i: &Inline, static_base: &str) -> InlineView {
    match i {
        Inline::Text(t) => InlineView::leaf("text", t),
        Inline::Code(t) => InlineView::leaf("code", t),
        Inline::SoftBreak => InlineView::leaf("softbreak", ""),
        Inline::HardBreak => InlineView::leaf("hardbreak", ""),
        Inline::Strong(ch) => InlineView::with_children(
            "strong",
            ch.iter().map(|i| inline_view(i, static_base)).collect(),
        ),

        Inline::Em(ch) => InlineView::with_children(
            "em",
            ch.iter().map(|i| inline_view(i, static_base)).collect(),
        ),
        Inline::Link {
            href,
            title,
            children,
        } => InlineView {
            kind: "link".into(),
            href: href.clone(),
            link_title: title.clone(),
            children: children
                .iter()
                .map(|i| inline_view(i, static_base))
                .collect(),
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
