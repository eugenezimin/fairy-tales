//! Config loading and validation.
//!
//! All I/O, environment reads, and cross-field checks live here.
//! The types they operate on are in `types.rs`.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::{Config, StaticSource};

impl Config {
    /// Load config from `path`, overlay env vars, load the strings file,
    /// then validate.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file at {}", path.display()))?;

        let mut cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing TOML config at {}", path.display()))?;

        if let Ok(tok) = std::env::var("APP_ADMIN_TOKEN") {
            cfg.admin.token = Some(tok);
        }

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

        validate(&cfg)?;
        Ok(cfg)
    }

    /// Resolve the config file path, in priority order:
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
}

fn validate(cfg: &Config) -> Result<()> {
    if let StaticSource::Local { source } = &cfg.server.static_source {
        anyhow::ensure!(
            source.is_dir(),
            "static dir does not exist or is not a directory: {}",
            source.display()
        );
    }
    anyhow::ensure!(
        cfg.content.dir.is_dir(),
        "content.dir does not exist or is not a directory: {}",
        cfg.content.dir.display()
    );
    anyhow::ensure!(
        !cfg.theme.name.trim().is_empty(),
        "theme.name must not be empty"
    );
    Ok(())
}
