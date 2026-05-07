//! Application bootstrap.
//!
//! The only job of this module is to load config, build the concrete
//! repository, assemble `AppState`, and hand off to the server.
//!
//! To swap in a different storage backend, change the two lines inside
//! `bootstrap` that construct `repo` — nothing else needs to change.

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::repository::fs::FsArticleRepository;
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

        let state = AppState {
            config: Arc::new(config),
            repo,
            admin_session: Arc::new(Mutex::new(None)),
            last_auth_attempt: Arc::new(Mutex::new(None)),
        };

        Ok(Self { state })
    }

    pub async fn run(self) -> Result<()> {
        server::run(self.state).await
    }
}
