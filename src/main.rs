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

fn load_dotenv() {
    let Ok(contents) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            if std::env::var(k).is_err() {
                unsafe { std::env::set_var(k, v) };
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
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
