//! Pure configuration data types.
//!
//! All structs and enums are plain data — no I/O, no validation logic.
//! They are deserialized from TOML by `loader.rs`.

use serde::Deserialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub site: SiteConfig,
    pub content: ContentConfig,
    pub theme: ThemeConfig,
    pub admin: AdminConfig,
    #[serde(skip)]
    pub strings: HashMap<String, String>,
    #[serde(default)]
    pub pagination: PaginationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub static_source: StaticSource,
    pub secure_cookies: bool,
}

impl ServerConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    pub fn resolved_static_source(&self) -> &StaticSource {
        &self.static_source
    }
}

/// Where static assets come from.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StaticSource {
    Local { source: PathBuf },
    Github { source: String },
}

impl StaticSource {
    pub fn local_dir(&self) -> Option<&PathBuf> {
        match self {
            StaticSource::Local { source } => Some(source),
            StaticSource::Github { .. } => None,
        }
    }

    pub fn github_raw_base(&self) -> Option<String> {
        match self {
            StaticSource::Github { source } => Some(github_tree_to_raw(source)),
            StaticSource::Local { .. } => None,
        }
    }
}

/// Convert a GitHub tree URL to a jsDelivr CDN base URL.
///
/// `https://github.com/user/repo/tree/main/static`
/// → `https://cdn.jsdelivr.net/gh/user/repo@main/static`
fn github_tree_to_raw(tree_url: &str) -> String {
    let without_gh = tree_url.trim_start_matches("https://github.com/");
    if let Some((repo_part, rest)) = without_gh.split_once("/tree/") {
        if let Some((branch, path)) = rest.split_once('/') {
            return format!("https://cdn.jsdelivr.net/gh/{repo_part}@{branch}/{path}");
        }
    }
    tree_url
        .replace("https://github.com/", "https://raw.githubusercontent.com/")
        .replace("/tree/", "/")
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub page_title: String,
    pub footer_year: u16,
    pub language: String,
    #[serde(default)]
    pub footer: FooterConfig,
    pub base_url: String,
}

/// All fields optional — unset means "use the hardcoded default".
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FooterConfig {
    pub text: Option<String>,
    pub copyright: Option<String>,
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
    /// Directory holding article Markdown files (one file = one article).
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
/// Pagination feature. Off by default — add `[pagination]` to config.toml to enable.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaginationConfig {
    /// Master switch. When `false` (default), all other fields are ignored.
    #[serde(default)]
    pub enabled: bool,
    /// Maximum plain-text symbols per page. Defaults to 10 000.
    #[serde(default = "PaginationConfig::default_limit")]
    pub symbol_limit: usize,
}

impl PaginationConfig {
    fn default_limit() -> usize {
        10_000
    }
}
