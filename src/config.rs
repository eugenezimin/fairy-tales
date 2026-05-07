//! Configuration loading and validation.
//!
//! The application is driven entirely by a TOML config file. The path
//! defaults to `config.toml` in the working directory, but can be
//! overridden via the `APP_CONFIG` environment variable or a CLI arg.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub site: SiteConfig,
    pub content: ContentConfig,
    pub theme: ThemeConfig,
    pub admin: AdminConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    /// Path to serve static assets from (CSS, images, etc.)
    pub static_dir: PathBuf,
}

impl ServerConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub page_title: String,
    pub footer_year: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentConfig {
    /// Directory holding article TOML files (one file = one article).
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfig {
    /// Active theme name; corresponds to `static/css/theme-{name}.css`.
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub token: Option<String>,
}

impl Config {
    /// Load config from a path, parse TOML, and validate referenced files exist.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file at {}", path.display()))?;

        let mut cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing TOML config at {}", path.display()))?;

        if let Ok(contents) = std::fs::read_to_string(".env") {
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"');
                    if std::env::var(k).is_err() {
                        unsafe {
                            std::env::set_var(k, v);
                        }
                    }
                }
            }
        }

        if let Ok(tok) = std::env::var("APP_ADMIN_TOKEN") {
            cfg.admin.token = Some(tok);
        }

        anyhow::ensure!(
            cfg.admin.token.as_deref().is_some_and(|t| !t.is_empty()),
            "admin.token must be set in config.toml or APP_ADMIN_TOKEN env var"
        );

        cfg.validate()?;
        Ok(cfg)
    }

    /// Resolve which config file to load, in priority order:
    /// 1. explicit CLI argument
    /// 2. `APP_CONFIG` environment variable
    /// 3. `./config.toml`
    pub fn resolve_path(cli_arg: Option<String>) -> PathBuf {
        if let Some(p) = cli_arg {
            return PathBuf::from(p);
        }
        if let Ok(p) = std::env::var("APP_CONFIG") {
            return PathBuf::from(p);
        }
        PathBuf::from("config.toml")
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.server.static_dir.is_dir(),
            "server.static_dir does not exist or is not a directory: {}",
            self.server.static_dir.display()
        );
        anyhow::ensure!(
            self.content.dir.is_dir(),
            "content.dir does not exist or is not a directory: {}",
            self.content.dir.display()
        );
        anyhow::ensure!(
            !self.theme.name.trim().is_empty(),
            "theme.name must not be empty"
        );
        Ok(())
    }
}
