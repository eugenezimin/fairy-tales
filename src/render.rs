//! HTML rendering.
//!
//! View models (what the template actually sees) and the rendering function
//! that turns a `ContentBundle` + site config into a finished HTML string.

use anyhow::{Context, Result};
use askama::Template;

use crate::config::{SiteConfig, ThemeConfig};
use crate::content::{ContentBundle, Section, StoryHeader};

// ---------- View models ----------
// Kept separate from domain types so the template can't accidentally
// depend on storage shape, and so we can compute derived data (like TOC).

#[derive(Clone)]
pub struct TocEntry {
    pub anchor: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexView {
    site_title: String,
    page_title: String,
    theme: String,
    stories: Vec<StoryHeader>,
    toc: Vec<TocEntry>,
    sections: Vec<Section>,
    year: u16,
}

// ---------- Rendering ----------

pub fn render_index(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
) -> Result<String> {
    let toc = build_toc(&bundle.article.sections);

    let view = IndexView {
        site_title: site.title.clone(),
        page_title: site.page_title.clone(),
        theme: theme.name.clone(),
        stories: bundle.stories.clone(),
        toc,
        sections: bundle.article.sections.clone(),
        year: site.footer_year,
    };

    view.render().context("rendering index template")
}

fn build_toc(sections: &[Section]) -> Vec<TocEntry> {
    sections
        .iter()
        .map(|s| TocEntry {
            anchor: s.id.clone(),
            label: s.heading.clone(),
        })
        .collect()
}
