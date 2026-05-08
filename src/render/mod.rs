//! HTML rendering.
//!
//! Translates the domain model into a single `PageView` and renders the
//! one shared `index.html` frame. The frame includes different partials
//! depending on `PageView::content`.

pub mod builder;
pub mod views;

use anyhow::{Context, Result};
use askama::Template;

use crate::config::{SiteConfig, ThemeConfig};
use crate::domain::{ContentBundle, StoryHeader};
use crate::render::builder::{build_section_views, build_toc};
use crate::render::views::{AdminArticleEntry, PageContent, PageView};

// ── Template ──────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "index.html")]
struct PageTemplate {
    // Flatten PageView fields — Askama needs direct field access.
    site_title: String,
    page_title: String,
    article_slug: String,
    theme: String,
    year: u16,
    is_mobile: bool,
    is_admin: bool,
    footer: crate::config::FooterConfig,
    /// `"article"` | `"empty"` | `"admin"`
    content: String,
    stories: Vec<StoryHeader>,
    toc: Vec<crate::render::views::TocEntry>,
    sections: Vec<crate::render::views::SectionView>,
    admin_articles: Vec<AdminArticleEntry>,
    static_base: String,
}

impl PageTemplate {
    fn from_view(view: PageView) -> Self {
        Self {
            site_title: view.site_title,
            page_title: view.page_title,
            article_slug: view.article_slug,
            theme: view.theme,
            year: view.year,
            is_mobile: view.is_mobile,
            is_admin: view.is_admin,
            footer: view.footer,
            content: view.content,
            stories: view.stories,
            toc: view.toc,
            sections: view.sections,
            admin_articles: view.admin_articles,
            static_base: view.static_base,
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Render any page mode through the single shared frame.
pub fn render_page(view: PageView) -> Result<String> {
    PageTemplate::from_view(view)
        .render()
        .context("rendering page template")
}

// ── View constructors ─────────────────────────────────────────────────────────

/// Build a `PageView` for a fully-loaded article page.
pub fn article_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
    is_admin: bool,
) -> PageView {
    let mut view = PageView::base(
        site.title.clone(),
        bundle.article.title.clone(),
        theme.name.clone(),
        site.footer_year,
        is_mobile,
        is_admin,
        site.footer.clone(),
        PageContent::Article,
        site.server.static_base.clone(),
    );
    view.article_slug = bundle.article.slug.clone();
    view.toc = build_toc(&bundle.article);
    view.sections = build_section_views(&bundle.article);
    view.stories = bundle.stories.clone();
    view.footer = site.footer.clone();
    view.static_base = site.static_base.clone();
    view
}

/// Build a `PageView` for the empty-state page (no articles yet).
pub fn empty_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    is_mobile: bool,
    is_admin: bool,
) -> PageView {
    PageView::base(
        site.title.clone(),
        site.title.clone(),
        theme.name.clone(),
        site.footer_year,
        is_mobile,
        is_admin,
        site.footer.clone(),
        PageContent::Empty,
    )
}

/// Build a `PageView` for the admin article list page.
pub fn admin_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    is_mobile: bool,
    articles: Vec<AdminArticleEntry>,
) -> PageView {
    let mut view = PageView::base(
        site.title.clone(),
        "Articles".to_string(),
        theme.name.clone(),
        site.footer_year,
        is_mobile,
        true,
        site.footer.clone(),
        PageContent::Admin,
    );
    view.admin_articles = articles;
    view.footer = site.footer.clone();
    view
}
