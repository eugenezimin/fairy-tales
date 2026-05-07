//! Mobile user-agent detection.

use axum::http::HeaderMap;

/// Returns `true` when the request's `User-Agent` header indicates a phone.
///
/// Tablets (iPad, Android without "Mobile") are treated as desktop so they
/// receive the full three-column layout.
pub fn detect(headers: &HeaderMap) -> bool {
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if ua.is_empty() {
        return false;
    }

    let mobile = ua.contains("Mobile")
        || ua.contains("iPhone")
        || ua.contains("iPod")
        || ua.contains("BlackBerry")
        || ua.contains("IEMobile")
        || ua.contains("Opera Mini");

    if mobile {
        tracing::debug!(ua = %ua, "mobile UA detected");
    }

    mobile
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ua(s: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("user-agent", HeaderValue::from_str(s).unwrap());
        h
    }

    #[test]
    fn iphone_is_mobile() {
        assert!(detect(&ua(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148 Safari/604.1"
        )));
    }

    #[test]
    fn android_phone_is_mobile() {
        assert!(detect(&ua(
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/123.0.0.0 Mobile Safari/537.36"
        )));
    }

    #[test]
    fn android_tablet_is_desktop() {
        assert!(!detect(&ua(
            "Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 Chrome/123.0.0.0 Safari/537.36"
        )));
    }

    #[test]
    fn ipad_is_desktop() {
        assert!(!detect(&ua(
            "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Version/17.0 Safari/604.1"
        )));
    }

    #[test]
    fn desktop_chrome_not_mobile() {
        assert!(!detect(&ua(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/123.0.0.0 Safari/537.36"
        )));
    }

    #[test]
    fn empty_ua_not_mobile() {
        assert!(!detect(&HeaderMap::new()));
    }
}
