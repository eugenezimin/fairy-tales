//! Content loading.
//!
//! Domain types representing what an article and a story header look like,
//! plus a loader that reads them from disk based on the `ContentConfig`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::ContentConfig;

// ---------- Domain types ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    pub id: String,
    pub heading: String,
    pub paragraphs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Article {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoryHeader {
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StoriesFile {
    stories: Vec<StoryHeader>,
}

/// Aggregate of everything loaded from disk for the index page.
#[derive(Debug, Clone)]
pub struct ContentBundle {
    pub article: Article,
    pub stories: Vec<StoryHeader>,
}

// ---------- Loader ----------

pub fn load(cfg: &ContentConfig) -> Result<ContentBundle> {
    let article = load_article(&cfg.article_path())?;
    let stories = load_stories(&cfg.stories_path())?;
    Ok(ContentBundle { article, stories })
}

fn load_article(path: &Path) -> Result<Article> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading article file at {}", path.display()))?;
    let article: Article = toml::from_str(&raw)
        .with_context(|| format!("parsing article TOML at {}", path.display()))?;
    anyhow::ensure!(
        !article.sections.is_empty(),
        "article has no sections: {}",
        path.display()
    );
    Ok(article)
}

fn load_stories(path: &Path) -> Result<Vec<StoryHeader>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading stories file at {}", path.display()))?;
    let parsed: StoriesFile = toml::from_str(&raw)
        .with_context(|| format!("parsing stories TOML at {}", path.display()))?;
    Ok(parsed.stories)
}
