//! HTML rendering.
//!
//! View models (what the template actually sees) and the rendering function
//! that turns a `ContentBundle` + site config into a finished HTML string.
//!
//! All view types use plain structs with a discriminant string so the
//! Askama template can use simple `{% if %}` checks rather than `{% match %}`,
//! which avoids any edge-cases in Askama's enum-match support.

use anyhow::{Context, Result};
use askama::Template;

use crate::config::{SiteConfig, ThemeConfig};
use crate::content::{
    Article, ContentBundle, HeadingLevel, Inline, Paragraph, Section, StoryHeader,
};

// ---------- View models ----------

#[derive(Clone)]
pub struct TocEntry {
    pub anchor: String,
    pub label: String,
    /// "h2" | "h3" | "h4"
    pub level: String,
}

/// A single rendered inline span inside a paragraph.
/// `kind` is one of: "text" | "image" | "link"
#[derive(Clone)]
pub struct InlineView {
    pub kind: String,
    // "text"
    pub text: String,
    // "image"
    pub src: String,
    pub alt: String,
    pub flow: String,
    // "link"
    pub href: String,
    pub link_text: String,
}

impl InlineView {
    fn text(t: &str) -> Self {
        Self {
            kind: "text".into(),
            text: t.into(),
            ..Self::empty()
        }
    }
    fn image(src: &str, alt: &str, flow: &str) -> Self {
        Self {
            kind: "image".into(),
            src: src.into(),
            alt: alt.into(),
            flow: if flow.is_empty() {
                "center".into()
            } else {
                flow.into()
            },
            ..Self::empty()
        }
    }
    fn link(href: &str, link_text: &str) -> Self {
        Self {
            kind: "link".into(),
            href: href.into(),
            link_text: link_text.into(),
            ..Self::empty()
        }
    }
    fn empty() -> Self {
        Self {
            kind: String::new(),
            text: String::new(),
            src: String::new(),
            alt: String::new(),
            flow: String::new(),
            href: String::new(),
            link_text: String::new(),
        }
    }
}

/// A paragraph — just the list of inline spans.
#[derive(Clone)]
pub struct ParagraphView {
    pub inlines: Vec<InlineView>,
}

/// A section as seen by the template.
/// `kind` is one of: "heading" | "paragraphs" | "ad"
/// `level` is one of: "h2" | "h3" | "h4" (only meaningful for kind="heading")
#[derive(Clone)]
pub struct SectionView {
    pub kind: String,
    pub id: String,
    // heading / paragraphs
    pub heading: String,
    pub level: String,
    pub paragraphs: Vec<ParagraphView>,
    // ad
    pub slot: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexView {
    site_title: String,
    page_title: String,
    theme: String,
    stories: Vec<StoryHeader>,
    toc: Vec<TocEntry>,
    sections: Vec<SectionView>,
    year: u16,
    is_mobile: bool,
}

// ---------- Rendering ----------

pub fn render_index(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
) -> Result<String> {
    let toc = build_toc(&bundle.article);
    let sections = build_section_views(&bundle.article);

    let view = IndexView {
        site_title: site.title.clone(),
        page_title: bundle.article.title.clone(),
        theme: theme.name.clone(),
        stories: bundle.stories.clone(),
        toc,
        sections,
        year: site.footer_year,
        is_mobile,
    };

    view.render().context("rendering index template")
}

fn build_toc(article: &Article) -> Vec<TocEntry> {
    article
        .sections
        .iter()
        .filter_map(|s| match s {
            Section::Heading {
                level, id, heading, ..
            } => Some(TocEntry {
                anchor: id.clone(),
                label: heading.clone(),
                level: match level {
                    HeadingLevel::H2 => "h2",
                    HeadingLevel::H3 => "h3",
                    HeadingLevel::H4 => "h4",
                }
                .into(),
            }),
            _ => None,
        })
        .collect()
}

fn build_section_views(article: &Article) -> Vec<SectionView> {
    article.sections.iter().map(section_to_view).collect()
}

fn section_to_view(s: &Section) -> SectionView {
    match s {
        Section::Heading {
            level,
            id,
            heading,
            paragraphs,
        } => SectionView {
            kind: "heading".into(),
            id: id.clone(),
            heading: heading.clone(),
            level: match level {
                HeadingLevel::H2 => "h2",
                HeadingLevel::H3 => "h3",
                HeadingLevel::H4 => "h4",
            }
            .into(),
            paragraphs: paragraphs.iter().map(para_to_view).collect(),
            slot: String::new(),
        },
        Section::Paragraphs { id, paragraphs } => SectionView {
            kind: "paragraphs".into(),
            id: id.clone(),
            heading: String::new(),
            level: String::new(),
            paragraphs: paragraphs.iter().map(para_to_view).collect(),
            slot: String::new(),
        },
        Section::Ad { id, slot } => SectionView {
            kind: "ad".into(),
            id: id.clone(),
            heading: String::new(),
            level: String::new(),
            paragraphs: vec![],
            slot: slot.clone(),
        },
    }
}

fn para_to_view(p: &Paragraph) -> ParagraphView {
    ParagraphView {
        inlines: p
            .0
            .iter()
            .map(|inline| match inline {
                Inline::Text(t) => InlineView::text(t),
                Inline::Image { src, alt, flow } => InlineView::image(src, alt, flow),
                Inline::Link { href, text } => InlineView::link(href, text),
            })
            .collect(),
    }
}
