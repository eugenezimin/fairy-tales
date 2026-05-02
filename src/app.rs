//! Application orchestration.
//!
//! Wires the modules together: load config, load content, build state,
//! start the server. This is the single place that knows about all
//! the pieces — `main` just calls `Application::run()`.

use anyhow::Result;
use std::sync::Arc;

use crate::config::Config;
use crate::content;
use crate::server::{self, AppState};

pub struct Application {
    state: AppState,
}

impl Application {
    /// Build the application from a config path. Loads config and all
    /// content eagerly so that any error surfaces *before* binding the port.
    pub fn bootstrap(config_path: std::path::PathBuf) -> Result<Self> {
        tracing::info!(path = %config_path.display(), "loading config");
        let config = Config::load(&config_path)?;

        tracing::info!(
            content_dir = %config.content.dir.display(),
            "loading content"
        );
        let bundle = content::load(&config.content)?;

        tracing::info!(
            sections = bundle.article.sections.len(),
            stories = bundle.stories.len(),
            theme = %config.theme.name,
            "content loaded"
        );

        let state = AppState {
            config: Arc::new(config),
            content: Arc::new(bundle),
        };

        Ok(Self { state })
    }

    /// Hand off to the server. Returns once the server stops.
    pub async fn run(self) -> Result<()> {
        server::run(self.state).await
    }
}
