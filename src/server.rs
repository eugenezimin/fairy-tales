//! HTTP server.
//!
//! Builds the Axum router from `AppState` and runs it. Handlers are kept
//! deliberately thin — they pull from state and delegate to `render`.

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::content::ContentBundle;
use crate::render;

/// Shared, read-only application state passed to every handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub content: Arc<ContentBundle>,
}

pub fn build_router(state: AppState) -> Router {
    let static_dir = state.config.server.static_dir.clone();

    Router::new()
        .route("/", get(index_handler))
        .route("/healthz", get(health_handler))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn run(state: AppState) -> Result<()> {
    let addr = state.config.server.socket_addr();
    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;

    tracing::info!("listening on http://{addr}");
    axum::serve(listener, router)
        .await
        .context("axum server error")?;
    Ok(())
}

// ---------- Handlers ----------

async fn index_handler(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let is_mobile = detect_mobile(&headers);

    match render::render_index(
        &state.config.site,
        &state.config.theme,
        &state.content,
        is_mobile,
    ) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "failed to render index");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

// ---------- Mobile detection ----------

/// Returns `true` when the `User-Agent` header looks like a mobile device.
///
/// Detection criteria (any one match → mobile):
/// - Contains "Mobile" (covers Android Chrome, Firefox for Android, etc.)
/// - Contains "Android" without "Tablet"
/// - Contains "iPhone" or "iPod"
/// - Contains "BlackBerry", "IEMobile", or "Opera Mini"
///
/// Tablets (iPad, Android Tablet) are treated as desktop by default so they
/// get the full three-column layout; the client-side JS will switch them to
/// mobile if their viewport is actually narrow.
fn detect_mobile(headers: &HeaderMap) -> bool {
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if ua.is_empty() {
        return false;
    }

    // Fast path: the word "Mobile" is the most reliable signal.
    // Android tablets send "Android" but NOT "Mobile"; phones send both.
    let is_mobile_keyword = ua.contains("Mobile")
        || ua.contains("iPhone")
        || ua.contains("iPod")
        || ua.contains("BlackBerry")
        || ua.contains("IEMobile")
        || ua.contains("Opera Mini");

    if is_mobile_keyword {
        tracing::debug!(ua = %ua, "mobile UA detected");
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ua_headers(ua: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("user-agent", HeaderValue::from_str(ua).unwrap());
        h
    }

    #[test]
    fn test_iphone_is_mobile() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
        assert!(detect_mobile(&ua_headers(ua)));
    }

    #[test]
    fn test_android_phone_is_mobile() {
        let ua = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Mobile Safari/537.36";
        assert!(detect_mobile(&ua_headers(ua)));
    }

    #[test]
    fn test_android_tablet_is_desktop() {
        // Android tablets omit "Mobile" from their UA string
        let ua = "Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";
        assert!(!detect_mobile(&ua_headers(ua)));
    }

    #[test]
    fn test_ipad_is_desktop() {
        let ua = "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/604.1";
        assert!(!detect_mobile(&ua_headers(ua)));
    }

    #[test]
    fn test_desktop_chrome_is_not_mobile() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";
        assert!(!detect_mobile(&ua_headers(ua)));
    }

    #[test]
    fn test_empty_ua_is_not_mobile() {
        assert!(!detect_mobile(&HeaderMap::new()));
    }
}
