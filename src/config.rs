//! Configuration loading and validation.
//!
//! The application is driven entirely by a TOML config file. The path
//! defaults to `config.toml` in the working directory, but can be
//! overridden via the `APP_CONFIG` environment variable or a CLI arg.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use tokio::join;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub site: SiteConfig,
    pub content: ContentConfig,
    pub theme: ThemeConfig,
    pub admin: AdminConfig,
    #[serde(skip)]
    pub strings: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub static_source: StaticSource,
    pub secure_cookies: bool,
}

/// Where static assets come from.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StaticSource {
    /// Serve files from a local directory (default).
    Local { source: PathBuf },
    /// Fetch assets from a GitHub repository tree URL, e.g.
    /// `"https://github.com/eugenezimin/fairy-tales/tree/main/static"`.
    /// The server rewrites this to the raw.githubusercontent.com CDN base.
    Github { source: String },
}

impl ServerConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Resolve the effective static source.
    pub fn resolved_static_source(&self) -> &StaticSource {
        &self.static_source
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub page_title: String,
    pub footer_year: u16,
    pub language: String,
    #[serde(default)]
    pub footer: FooterConfig,
}

/// All fields are optional — unset means "use the hardcoded default".
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FooterConfig {
    /// Replaces the entire footer text. Supports basic HTML.
    pub text: Option<String>,
    /// Overrides just the copyright line; ignored when `text` is set.
    pub copyright: Option<String>,
    /// Extra links rendered after the copyright line, e.g. `[{label="Privacy", href="/privacy"}]`.
    #[serde(default)]
    pub links: Vec<FooterLink>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FooterLink {
    pub label: String,
    pub href: String,
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

        if let Ok(tok) = std::env::var("APP_ADMIN_TOKEN") {
            cfg.admin.token = Some(tok);
        }

        // Derive the strings file path from the config file's directory
        // and the configured language, e.g. `strings.en`.
        let strings_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("./static")
            .join("int")
            .join(format!("strings.{}", cfg.site.language));
        let strings_raw = std::fs::read_to_string(&strings_path)
            .with_context(|| format!("reading strings file at {}", strings_path.display()))?;
        cfg.strings = toml::from_str::<HashMap<String, String>>(&strings_raw)
            .with_context(|| format!("parsing strings file at {}", strings_path.display()))?;

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
        if let StaticSource::Local { source } = &self.server.static_source {
            anyhow::ensure!(
                source.is_dir(),
                "static dir does not exist or is not a directory: {}",
                source.display()
            );
        }
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

impl StaticSource {
    /// For `Local`, return the directory. Panics on `Github` — callers
    /// must branch on the variant before asking for a local path.
    pub fn local_dir(&self) -> Option<&PathBuf> {
        match self {
            StaticSource::Local { source } => Some(source),
            StaticSource::Github { .. } => None,
        }
    }

    /// For `Github`, return the raw CDN base URL
    /// (`https://raw.githubusercontent.com/user/repo/ref/static`).
    /// Returns `None` for `Local`.
    pub fn github_raw_base(&self) -> Option<String> {
        match self {
            StaticSource::Github { source } => Some(github_tree_to_raw(source)),
            StaticSource::Local { .. } => None,
        }
    }
}

/// Convert a GitHub tree URL to a raw.githubusercontent.com prefix.
///
/// `https://github.com/eugenezimin/fairy-tales/tree/main/static`
/// → `https://raw.githubusercontent.com/eugenezimin/fairy-tales/main/static`
fn github_tree_to_raw(tree_url: &str) -> String {
    // raw.githubusercontent.com serves with Content-Type: text/plain, which
    // browsers reject for CSS/JS. jsDelivr mirrors GitHub and serves correct
    // MIME types.
    //
    // https://github.com/USER/REPO/tree/BRANCH/PATH
    // → https://cdn.jsdelivr.net/gh/USER/REPO@BRANCH/PATH
    let without_gh = tree_url.trim_start_matches("https://github.com/");

    // Split on "/tree/" to separate "user/repo" from "branch/path"
    if let Some((repo_part, rest)) = without_gh.split_once("/tree/") {
        // rest = "main/static" — split branch from subpath
        if let Some((branch, path)) = rest.split_once('/') {
            return format!("https://cdn.jsdelivr.net/gh/{repo_part}@{branch}/{path}");
        }
    }

    // Fallback: just strip the tree segment (may still have MIME issues)
    tree_url
        .replace("https://github.com/", "https://raw.githubusercontent.com/")
        .replace("/tree/", "/")
}
