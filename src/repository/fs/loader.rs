//! Filesystem scanning and cheap metadata extraction.
//!
//! These functions read the content directory to build `ArticleMeta` lists
//! without fully parsing every article. Full parsing is deferred to
//! `parser::parse_article` and only done for the article actually being served.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::domain::{Article, Block};
use crate::repository::ArticleMeta;
use crate::repository::fs::{front_matter, parser, util};

// ── Directory scan ────────────────────────────────────────────────────────────

/// Internal per-file descriptor used during scanning.
pub struct FileMeta {
    pub title: String,
    pub slug: String,
    pub path: PathBuf,
}

/// Read every `.md` file in `dir` just enough to extract title + slug.
///
/// Files without an H1 heading are skipped with a warning.
/// Results are sorted by slug for deterministic ordering.
pub fn scan(dir: &Path) -> Result<Vec<FileMeta>> {
    let mut metas = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading content directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.context("reading directory entry")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let (front, body) = front_matter::split(&raw);
        let mut slug = front_matter::value(front, "slug").unwrap_or_default();
        let title = parser::extract_title(body);

        if title.is_empty() {
            tracing::warn!(path = %path.display(), "skipping: no H1 title found");
            continue;
        }
        if slug.is_empty() {
            slug = util::slugify(&title);
        }

        metas.push(FileMeta { title, slug, path });
    }

    metas.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(metas)
}

/// Build `ArticleMeta` rows from a directory scan.
///
/// Each entry includes a cheap snippet (first non-empty body line) so callers
/// don't have to do a second pass.
pub fn scan_as_meta(dir: &Path) -> Result<Vec<ArticleMeta>> {
    let file_metas = scan(dir)?;
    Ok(file_metas
        .into_iter()
        .map(|fm| {
            let snippet = cheap_snippet(&fm.path);
            ArticleMeta {
                title: fm.title,
                slug: fm.slug,
                snippet,
            }
        })
        .collect())
}

// ── Snippet extraction ────────────────────────────────────────────────────────

/// Extract a short preview string from a file without fully parsing it.
///
/// Reads the raw text, skips front matter and the H1, and returns the first
/// meaningful non-heading, non-comment line (truncated to 120 chars).
pub fn cheap_snippet(path: &Path) -> String {
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    let (_, body) = front_matter::split(&raw);
    snippet_from_body(body)
}

fn snippet_from_body(body: &str) -> String {
    let mut past_h1 = false;
    for line in body.lines() {
        let t = line.trim();
        if !past_h1 {
            if t.starts_with("# ") {
                past_h1 = true;
            }
            continue;
        }
        if t.is_empty() || t.starts_with('#') || t.starts_with("<!--") || t.starts_with("+++") {
            continue;
        }
        let clean: String = t
            .chars()
            .filter(|c| !matches!(c, '*' | '_' | '`'))
            .collect();
        let clean = clean.trim().to_string();
        if clean.is_empty() {
            continue;
        }
        return if clean.len() > 120 {
            format!("{}…", clean.chars().take(120).collect::<String>())
        } else {
            clean
        };
    }
    String::new()
}

/// Extract a snippet from a fully-parsed `Article` (used after full parse).
pub fn snippet_from_article(article: &Article) -> String {
    for section in &article.sections {
        for block in &section.blocks {
            if let Block::Paragraph(inlines) = block {
                let text = parser::inlines_to_plain_text(inlines);
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return if trimmed.len() > 120 {
                        let cut = trimmed
                            .char_indices()
                            .map(|(i, _)| i)
                            .take(120)
                            .last()
                            .unwrap_or(0);
                        format!("{}…", &trimmed[..cut])
                    } else {
                        trimmed
                    };
                }
            }
        }
    }
    String::new()
}
