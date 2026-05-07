//! HTML rendering.
//!
//! Translates the domain model into Askama view models and renders the
//! template. The only public entry point is `render_index`.

pub mod builder;
pub mod views;

use anyhow::{Context, Result};
use askama::Template;

use crate::config::{SiteConfig, ThemeConfig};
use crate::domain::{ContentBundle, StoryHeader};
use crate::render::builder::{build_section_views, build_toc};
use crate::render::views::{SectionView, TocEntry};

// ── Template ──────────────────────────────────────────────────────────────────

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
    is_admin: bool,
}

// ADD after the existing IndexView template struct and render_index fn:

#[derive(Template)]
#[template(path = "empty.html")]
struct EmptyView {
    site_title: String,
    theme: String,
    year: u16,
    is_mobile: bool,
    is_admin: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn render_index(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
    is_admin: bool,
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
        is_admin,
    }
    .render()
    .context("rendering index template")
}

pub fn render_empty(
    site: &SiteConfig,
    theme: &ThemeConfig,
    is_mobile: bool,
    is_admin: bool,
) -> Result<String> {
    EmptyView {
        site_title: site.title.clone(),
        theme: theme.name.clone(),
        year: site.footer_year,
        is_mobile,
        is_admin,
    }
    .render()
    .context("rendering empty template")
}
