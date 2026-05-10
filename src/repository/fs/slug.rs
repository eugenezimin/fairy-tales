//! Slug derivation, validation, and path-traversal guards.
//!
//! Pure logic — no knowledge of the repository struct or its directory field.

use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

use super::{front_matter, util};

// ── Slug derivation ───────────────────────────────────────────────────────────

/// Derive the slug from raw article text.
///
/// Priority: front-matter `slug` field → first H1 → timestamp fallback.
pub fn derive(raw: &str) -> String {
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

// ── Validation ────────────────────────────────────────────────────────────────

pub fn validate(slug: &str) -> Result<()> {
    anyhow::ensure!(!slug.is_empty(), "slug is empty");
    validate_chars(slug)
}

pub fn validate_chars(slug: &str) -> Result<()> {
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

// ── Path-traversal guard ──────────────────────────────────────────────────────

/// Verify that `path` is strictly inside `base`.
pub fn path_traversal_guard(base: &PathBuf, path: &PathBuf) -> Result<()> {
    let canonical_base = std::fs::canonicalize(base)
        .with_context(|| format!("canonicalizing base dir {}", base.display()))?;

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
