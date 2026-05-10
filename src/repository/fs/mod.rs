//! Filesystem implementation of `ArticleRepository`.
//!
//! Articles are stored as `.md` files in a single flat directory.
//! The directory is rescanned on every `list()` / `get()` call so new
//! files are picked up without a server restart.

pub mod front_matter;
pub mod loader;
pub mod parser;
pub mod slug;
pub mod util;

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::domain::Article;
use crate::repository::{ArticleMeta, ArticleRepository};

// ── Repository struct ─────────────────────────────────────────────────────────

/// Filesystem-backed article store.
pub struct FsArticleRepository {
    dir: PathBuf,
}

impl FsArticleRepository {
    /// Create a new repository rooted at `dir`.
    ///
    /// Returns an error if `dir` does not exist or is not a directory.
    pub fn new(dir: PathBuf) -> Result<Self> {
        anyhow::ensure!(
            dir.is_dir(),
            "content directory does not exist or is not a directory: {}",
            dir.display()
        );
        Ok(Self { dir })
    }
}

// ── ArticleRepository impl ────────────────────────────────────────────────────

impl ArticleRepository for FsArticleRepository {
    fn list(&self) -> Result<Vec<ArticleMeta>> {
        loader::scan_as_meta(&self.dir)
    }

    fn get(&self, slug: &str) -> Result<Article> {
        let path = self.dir.join(format!("{slug}.md"));
        anyhow::ensure!(path.exists(), "no article with slug '{slug}'");
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        parser::parse_article(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    fn random_slug(&self) -> Result<String> {
        let metas = loader::scan(&self.dir)?;
        anyhow::ensure!(!metas.is_empty(), "no articles found in {:?}", self.dir);
        let idx = util::random_index(metas.len());
        Ok(metas[idx].slug.clone())
    }

    fn save(&self, raw: &str) -> Result<String> {
        anyhow::ensure!(!raw.trim().is_empty(), "article content is empty");

        parser::parse_article(raw)
            .context("invalid article: must be valid Markdown with at least one H1 heading")?;

        let s = slug::derive(raw);
        slug::validate(&s)?;

        let path = self.dir.join(format!("{s}.md"));
        slug::path_traversal_guard(&self.dir, &path)?;

        std::fs::write(&path, raw.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;

        Ok(s)
    }

    fn delete(&self, slug: &str) -> Result<()> {
        slug::validate_chars(slug)?;
        let path = self.dir.join(format!("{slug}.md"));
        slug::path_traversal_guard(&self.dir, &path)?;

        anyhow::ensure!(path.exists(), "no article with slug '{slug}'");
        std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))
    }
}
