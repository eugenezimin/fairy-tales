//! Filesystem implementation of `ArticleRepository`.
//!
//! Articles are stored as `.md` files in a single flat directory.
//! The directory is rescanned on every `list()` / `get()` call so new
//! files are picked up without a server restart.

pub mod front_matter;
pub mod loader;
pub mod parser;
pub mod util;

use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

use crate::domain::Article;
use crate::repository::{ArticleMeta, ArticleRepository};

// ── Repository struct ─────────────────────────────────────────────────────────

/// Filesystem-backed article store.
///
/// All operations are synchronous and performed on the calling thread.
/// This is fine under Axum because handlers call `spawn_blocking` for
/// anything IO-heavy — or more simply, article files are small enough
/// that the latency is negligible.
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

        // Validate the content is a parseable markdown article before touching disk.
        parser::parse_article(raw)
            .context("invalid article: must be valid Markdown with at least one H1 heading")?;

        let slug = derive_slug(raw);
        validate_slug(&slug)?;

        let path = self.dir.join(format!("{slug}.md"));
        path_traversal_guard(&self.dir, &path)?;

        std::fs::write(&path, raw.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;

        Ok(slug)
    }

    fn delete(&self, slug: &str) -> Result<()> {
        validate_slug_chars(slug)?;
        let path = self.dir.join(format!("{slug}.md"));
        path_traversal_guard(&self.dir, &path)?;

        anyhow::ensure!(path.exists(), "no article with slug '{slug}'");
        std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Derive the slug from raw article text.
///
/// Priority: front-matter `slug` field → first H1 → timestamp fallback.
fn derive_slug(raw: &str) -> String {
    let raw = raw.trim_start_matches('\n');

    // 1. Front-matter slug
    let (front, body) = front_matter::split(raw);
    if let Some(v) = front_matter::value(front, "slug") {
        if !v.is_empty() {
            return v;
        }
    }

    // 2. First H1 heading
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return util::slugify(title);
            }
        }
    }

    // 3. Timestamp fallback
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "article-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    )
}

fn validate_slug(slug: &str) -> Result<()> {
    anyhow::ensure!(!slug.is_empty(), "slug is empty");
    validate_slug_chars(slug)
}

fn validate_slug_chars(slug: &str) -> Result<()> {
    anyhow::ensure!(
        slug.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "invalid slug characters in '{slug}'"
    );
    anyhow::ensure!(
        !slug.contains("..") && !slug.contains('/') && !slug.contains('\\'),
        "slug contains path traversal characters"
    );
    Ok(())
}

/// Guard against path traversal: verify `path` is inside `base`.
fn path_traversal_guard(base: &PathBuf, path: &PathBuf) -> Result<()> {
    let canonical_base = std::fs::canonicalize(base)
        .with_context(|| format!("canonicalizing base dir {}", base.display()))?;

    // For new files, canonicalize the parent directory instead.
    let canonical_path = if path.exists() {
        std::fs::canonicalize(path)
            .with_context(|| format!("canonicalizing path {}", path.display()))?
    } else {
        let parent = path.parent().unwrap_or(path);
        std::fs::canonicalize(parent)
            .with_context(|| format!("canonicalizing parent {}", parent.display()))?
            .join(
                path.file_name()
                    .ok_or_else(|| anyhow!("path has no filename"))?,
            )
    };

    anyhow::ensure!(
        canonical_path.starts_with(&canonical_base),
        "path traversal detected: {} is outside {}",
        path.display(),
        base.display()
    );
    Ok(())
}
