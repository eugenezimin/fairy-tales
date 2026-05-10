//! Application bootstrap.

use anyhow::Result;
use std::sync::Arc;

use crate::config::Config;
use crate::render::AskamaRenderer;
use crate::repository::fs::FsArticleRepository;
use crate::server::session_memory::InMemorySessionStore;
use crate::server::{self, AppState};

pub struct Application {
    state: AppState,
}

impl Application {
    pub fn bootstrap(config_path: std::path::PathBuf) -> Result<Self> {
        tracing::info!(path = %config_path.display(), "loading config");
        let config = Config::load(&config_path)?;

        tracing::info!(
            content_dir = %config.content.dir.display(),
            theme = %config.theme.name,
            "config loaded"
        );

        // ── Swap this block to use a different backend ──────────────────────
        let repo = FsArticleRepository::new(config.content.dir.clone())?;
        let repo = Arc::new(repo);
        // ───────────────────────────────────────────────────────────────────

        match config.server.resolved_static_source() {
            crate::config::StaticSource::Local { source } => {
                tracing::info!(dir = %source.display(), "static assets: local directory");
            }
            crate::config::StaticSource::Github { source } => {
                tracing::info!(url = %source, "static assets: GitHub repository");
            }
        }

        let state = AppState {
            config: Arc::new(config),
            repo,
            renderer: Arc::new(AskamaRenderer),
            // ── Swap this line to use a different session backend ───────────
            sessions: Arc::new(InMemorySessionStore::new()),
            // ─────────────────────────────────────────────────────────────────
        };

        Ok(Self { state })
    }

    pub async fn run(self) -> Result<()> {
        server::run(self.state).await
    }
}
