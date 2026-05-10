//! HTML rendering.
//!
//! `renderer`    — the `Renderer` trait (interface).
//! `askama_impl` — the Askama-backed implementation.
//! `views`       — view-model structs consumed by templates.
//! `builder`     — domain-model → view-model conversion (pure functions).

pub mod askama_impl;
pub mod builder;
pub mod renderer;
pub mod views;

pub use askama_impl::AskamaRenderer;
pub use renderer::Renderer;

use std::collections::HashMap;

use crate::config::{SiteConfig, ThemeConfig};
use crate::domain::ContentBundle;
use crate::render::builder::{build_section_views, build_toc};
use crate::render::views::{AdminArticleEntry, PageContent, PageView};

// ── View constructors ─────────────────────────────────────────────────────────

/// Build a `PageView` for a fully-loaded article page.
pub fn article_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
    is_admin: bool,
    static_base: &str,
    strings: HashMap<String, String>,
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
        static_base.to_string(),
        strings,
    );
    view.article_slug = bundle.article.slug.clone();
    view.toc = build_toc(&bundle.article);
    view.sections = build_section_views(&bundle.article, static_base);
    view.stories = bundle.stories.clone();
    view
}

/// Build a `PageView` for the empty-state page (no articles yet).
pub fn empty_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    is_mobile: bool,
    is_admin: bool,
    static_base: &str,
    strings: HashMap<String, String>,
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
        static_base.to_string(),
        strings,
    )
}

/// Build a `PageView` for the admin article list page.
pub fn admin_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    is_mobile: bool,
    articles: Vec<AdminArticleEntry>,
    static_base: &str,
    strings: HashMap<String, String>,
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
        static_base.to_string(),
        strings,
    );
    view.admin_articles = articles;
    view
}
