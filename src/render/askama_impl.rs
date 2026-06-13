//! Askama implementation of `Renderer`.
//!
//! All Askama-specific types (`PageTemplate`, `from_view`) live here.
//! Nothing outside this module needs to know about Askama.

use std::collections::HashMap;

use anyhow::{Context, Result};
use askama::Template;

use crate::domain::StoryHeader;
use crate::render::renderer::Renderer;
use crate::render::views::{AdminArticleEntry, PageView, SectionView, TocEntry};

// ── Askama template ───────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "index.html")]
struct PageTemplate {
    site_title: String,
    page_title: String,
    article_slug: String,
    article_author: String,
    article_published: String,
    article_description: String,
    article_keywords: String,
    article_cover: String,
    article_og_title: String,
    article_category: String,
    article_tags: String,
    article_reading_time: String,
    theme: String,
    year: u16,
    is_mobile: bool,
    is_admin: bool,
    footer: crate::config::FooterConfig,
    content: String,
    stories: Vec<StoryHeader>,
    toc: Vec<TocEntry>,
    sections: Vec<SectionView>,
    current_page: usize,
    total_pages: usize,
    has_prev: bool,
    has_next: bool,
    admin_articles: Vec<AdminArticleEntry>,
    static_base: String,
    strings: HashMap<String, String>,
}

impl PageTemplate {
    fn from_view(view: PageView) -> Self {
        Self {
            article_author: view.article_author,
            article_published: view.article_published,
            article_description: view.article_description,
            article_keywords: view.article_keywords,
            article_cover: view.article_cover,
            article_og_title: view.article_og_title,
            article_category: view.article_category,
            article_tags: view.article_tags,
            article_reading_time: view.article_reading_time,
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
            current_page: view.current_page,
            total_pages: view.total_pages,
            has_prev: view.has_prev,
            has_next: view.has_next,
            admin_articles: view.admin_articles,
            static_base: view.static_base,
            strings: view.strings,
        }
    }
}

// ── Renderer impl ─────────────────────────────────────────────────────────────

/// The default renderer — compiles Askama templates at build time.
pub struct AskamaRenderer;

impl Renderer for AskamaRenderer {
    fn render(&self, view: PageView) -> Result<String> {
        PageTemplate::from_view(view)
            .render()
            .context("rendering page template")
    }
}
