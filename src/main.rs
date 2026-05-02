//! Entry point. Initializes logging, resolves the config path, hands
//! everything off to `Application`. Keep this thin.

mod app;
mod config;
mod content;
mod render;
mod server;

use anyhow::Result;

use crate::app::Application;
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli_arg = std::env::args().nth(1);
    let config_path = Config::resolve_path(cli_arg);

    let app = Application::bootstrap(config_path)?;
    app.run().await
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));

    fmt().with_env_filter(filter).init();
}
