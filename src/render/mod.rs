//! HTML rendering.
//!
//! `renderer`    — the `Renderer` trait (interface).
//! `askama_impl` — the Askama-backed implementation.
//! `views`       — view-model structs consumed by templates.
//! `builder`     — domain-model → view-model conversion (pure functions).

pub mod askama_impl;
pub mod builder;
pub mod pagination;
pub mod renderer;
pub mod views;

pub use askama_impl::AskamaRenderer;
pub use renderer::Renderer;

use std::collections::HashMap;

use crate::config::{SiteConfig, ThemeConfig};
use crate::domain::ContentBundle;
use crate::render::views::{AdminArticleEntry, PageContent, PageView};

// ── View constructors ─────────────────────────────────────────────────────────

/// Build a `PageView` for a fully-loaded article page.
/// Build a `PageView` for a fully-loaded article page.
///
/// `requested_page` is 1-based. When pagination is disabled it is ignored.
/// Out-of-range values are clamped to the last valid page.
pub fn article_view(
    site: &SiteConfig,
    theme: &ThemeConfig,
    bundle: &ContentBundle,
    is_mobile: bool,
    is_admin: bool,
    static_base: &str,
    strings: HashMap<String, String>,
    pagination_cfg: &crate::config::PaginationConfig,
    requested_page: usize,
) -> PageView {
    let all_sections = &bundle.article.sections;

    // ── Paginate (or treat everything as one page) ────────────────────────
    let pages: Vec<Vec<crate::domain::Section>> = if pagination_cfg.enabled {
        pagination::paginate(all_sections, pagination_cfg.symbol_limit)
    } else {
        vec![all_sections.to_vec()]
    };

    let total_pages = pages.len();
    // clamp: 1-based, so valid range is 1..=total_pages
    let current_page = requested_page.clamp(1, total_pages);
    let page_sections = &pages[current_page - 1];

    // ── Page-aware TOC ───────────────────────────────────────────────────
    let page_map = if pagination_cfg.enabled {
        pagination::build_page_map(&pages)
    } else {
        std::collections::HashMap::new()
    };
    let toc = builder::build_toc_paged(&bundle.article, &page_map);

    // ── Build view ───────────────────────────────────────────────────────
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
    view.toc = toc;
    view.sections = builder::build_section_views_from(page_sections, static_base);
    view.stories = bundle.stories.clone();
    view.article_slug = bundle.article.slug.clone();
    view.article_author = bundle.article.author.clone();
    view.article_published = bundle.article.published.clone();
    view.article_description = bundle.article.description.clone();
    view.article_keywords = bundle.article.keywords.clone();
    view.article_cover = bundle.article.cover.clone();
    view.article_og_title = bundle.article.og_title.clone();
    view.article_category = bundle.article.category.clone();
    view.article_tags = bundle.article.tags.clone();
    view.article_reading_time = bundle.article.reading_time.clone();
    view.current_page = current_page;
    view.total_pages = total_pages;
    view.has_prev = current_page > 1;
    view.has_next = current_page < total_pages;
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
