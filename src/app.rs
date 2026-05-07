//! Application orchestration.
//!
//! Wires the modules together: load config, build state, start the server.
//! Content is no longer loaded at startup — it is scanned and parsed
//! fresh on every request so new articles appear without a restart.

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::server::{self, AppState};

pub struct Application {
    state: AppState,
}

impl Application {
    /// Build the application from a config path. Loads and validates config
    /// eagerly so any misconfiguration surfaces before the port is bound.
    pub fn bootstrap(config_path: std::path::PathBuf) -> Result<Self> {
        tracing::info!(path = %config_path.display(), "loading config");
        let config = Config::load(&config_path)?;

        tracing::info!(
            content_dir = %config.content.dir.display(),
            theme = %config.theme.name,
            "config loaded — content will be scanned per request"
        );

        let state = AppState {
            config: Arc::new(config),
            admin_session: Arc::new(Mutex::new(None::<(String, std::time::Instant)>)),
            last_auth_attempt: Arc::new(Mutex::new(None)),
        };

        Ok(Self { state })
    }

    /// Hand off to the server. Returns once the server stops.
    pub async fn run(self) -> Result<()> {
        server::run(self.state).await
    }
}
