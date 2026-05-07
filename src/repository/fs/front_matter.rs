//! Front-matter parsing.
//!
//! Articles may optionally begin with a `+++`-fenced TOML-lite block:
//!
//! ```text
//! +++
//! slug      = "my-article"
//! author    = "Jane Doe"
//! published = "2026-04-01"
//! +++
//! ```
//!
//! This module is intentionally pure: no I/O, no external crates.

/// Split `+++…+++` front matter from the body.
///
/// Returns `("", raw)` when no front matter fence is found.
pub fn split(raw: &str) -> (&str, &str) {
    let raw = raw.trim_start_matches('\n');
    if !raw.starts_with("+++") {
        return ("", raw);
    }
    let after_open = raw[3..].trim_start_matches('\n');
    if let Some(close) = after_open.find("\n+++") {
        let front = &after_open[..close];
        let body = after_open[close + 4..].trim_start_matches('\n');
        (front, body)
    } else {
        ("", raw)
    }
}

/// Extract a single `key = "value"` (or `key = value`) pair from the
/// front-matter block returned by `split`.
///
/// Returns `None` when the key is absent.
pub fn value(front: &str, key: &str) -> Option<String> {
    for line in front.lines() {
        let line = line.trim();
        let eq = line.find('=')?;
        let k = line[..eq].trim();
        if k == key {
            let v = line[eq + 1..].trim().trim_matches('"');
            return Some(v.to_string());
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_front_matter() {
        let raw = "+++\nslug = \"hello\"\n+++\n# Title\n";
        let (front, body) = split(raw);
        assert_eq!(front, "slug = \"hello\"");
        assert_eq!(body, "# Title\n");
    }

    #[test]
    fn no_front_matter_returns_empty() {
        let raw = "# Title\nBody.";
        let (front, body) = split(raw);
        assert_eq!(front, "");
        assert_eq!(body, raw);
    }

    #[test]
    fn extracts_value() {
        let front = "slug = \"my-slug\"\nauthor = \"Jane\"";
        assert_eq!(value(front, "slug"), Some("my-slug".into()));
        assert_eq!(value(front, "author"), Some("Jane".into()));
        assert_eq!(value(front, "missing"), None);
    }
}
