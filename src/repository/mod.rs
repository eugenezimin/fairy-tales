//! Repository abstraction for article storage.
//!
//! All content-loading and content-writing operations go through
//! `ArticleRepository`. The rest of the application (handlers, render)
//! depends only on this trait, never on a concrete implementation.
//!
//! # Adding a new backend
//!
//! 1. Create `repository/pg/` (or `s3/`, `nfs/`, …).
//! 2. Implement `ArticleRepository` for your new struct.
//! 3. In `app.rs`, swap `FsArticleRepository::new(…)` for your new type.
//! 4. Nothing else changes.

pub mod fs;

use anyhow::Result;

use crate::domain::{Article, StoryHeader};

// ── Shared metadata type ──────────────────────────────────────────────────────

/// Lightweight article descriptor returned by `list()`.
/// Cheap to produce — no body parsing required.
#[derive(Debug, Clone)]
pub struct ArticleMeta {
    pub title: String,
    pub slug: String,
    /// First ~120 chars of body text, pre-extracted.
    pub snippet: String,
}

// ── The trait ─────────────────────────────────────────────────────────────────

/// Storage-agnostic interface for reading and writing articles.
///
/// Implementations must be `Send + Sync` so they can live in `Arc<dyn …>`
/// inside shared Axum state.
pub trait ArticleRepository: Send + Sync {
    /// Return lightweight metadata for every available article.
    /// The returned vec is sorted deterministically (e.g. by slug).
    fn list(&self) -> Result<Vec<ArticleMeta>>;

    /// Fully parse and return one article by slug.
    fn get(&self, slug: &str) -> Result<Article>;

    /// Return a pseudo-random slug drawn from available articles.
    fn random_slug(&self) -> Result<String>;

    /// Persist a raw article (Markdown + front matter).
    /// Returns the slug that was assigned to the article.
    fn save(&self, raw: &str) -> Result<String>;

    /// Delete the article identified by `slug`.
    fn delete(&self, slug: &str) -> Result<()>;

    /// Return sidebar cards for all articles *except* `exclude_slug`,
    /// shuffled and capped at `limit`.
    ///
    /// A default implementation is provided; backends can override it
    /// with a more efficient query if desired.
    fn sidebar_stories(&self, exclude_slug: &str, limit: usize) -> Result<Vec<StoryHeader>> {
        let mut metas = self.list()?;
        metas.retain(|m| m.slug != exclude_slug);
        fs::util::shuffle(&mut metas);
        metas.truncate(limit);
        Ok(metas
            .into_iter()
            .map(|m| StoryHeader {
                title: m.title,
                slug: m.slug,
                snippet: m.snippet,
            })
            .collect())
    }
}
