//! HTTP server.
//!
//! Builds the Axum router from `AppState` and runs it. Content is loaded
//! fresh on every request (directory rescan + single article parse) so new
//! files are picked up without a restart and RAM stays minimal.

use anyhow::{Context, Result};
use axum::response::Redirect;
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::content;
use crate::render;

const ADMIN_COOKIE: &str = "admin_session";
const SESSION_MINUTES: u64 = 30;

/// Shared, read-only application state passed to every handler.
/// Content is intentionally absent — it is loaded per-request.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub admin_session: Arc<Mutex<Option<Instant>>>,
}

pub fn build_router(state: AppState) -> Router {
    let static_dir = state.config.server.static_dir.clone();

    Router::new()
        .route("/", get(index_handler))
        .route("/:token", get(activate_admin_handler)) // <-- new
        .route("/article/:slug", get(article_handler))
        .route("/admin/article", post(upload_article_handler))
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

async fn index_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> impl IntoResponse {
    serve_page(state, headers, jar, None).await
}

async fn article_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    serve_page(state, headers, jar, Some(slug)).await
}

async fn serve_page(
    state: AppState,
    headers: HeaderMap,
    jar: CookieJar,
    slug: Option<String>,
) -> impl IntoResponse {
    let is_mobile = detect_mobile(&headers);
    let is_admin = is_admin_session(&state, &jar);

    let bundle = match content::load(&state.config.content, slug.as_deref()) {
        Ok(b) => b,
        Err(err) => {
            tracing::error!(error = ?err, "failed to load content");
            return (StatusCode::NOT_FOUND, "article not found").into_response();
        }
    };

    match render::render_index(
        &state.config.site,
        &state.config.theme,
        &bundle,
        is_mobile,
        is_admin,
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

async fn upload_article_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    body: String,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let slug = extract_slug_from_raw(&body);
    let filename = format!("{}.md", slug);
    let path = state.config.content.dir.join(&filename);

    // Path traversal guard
    let canonical_content = std::fs::canonicalize(&state.config.content.dir).unwrap();
    let canonical_path = std::fs::canonicalize(path.parent().unwrap_or(&path)).unwrap_or_default();
    if !canonical_path.starts_with(&canonical_content) {
        return (StatusCode::BAD_REQUEST, "invalid slug").into_response();
    }

    match std::fs::write(&path, body.as_bytes()) {
        Ok(_) => (StatusCode::CREATED, slug).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "failed to write article");
            (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response()
        }
    }
}

fn extract_slug_from_raw(raw: &str) -> String {
    let raw = raw.trim_start_matches('\n');
    if raw.starts_with("+++") {
        if let Some(after) = raw.get(3..) {
            let after = after.trim_start_matches('\n');
            if let Some(close) = after.find("\n+++") {
                let front = &after[..close];
                for line in front.lines() {
                    let line = line.trim();
                    if let Some(eq) = line.find('=') {
                        if line[..eq].trim() == "slug" {
                            return line[eq + 1..].trim().trim_matches('"').to_string();
                        }
                    }
                }
            }
        }
    }
    // fallback: timestamp
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "article-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    )
}

async fn activate_admin_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
) -> impl IntoResponse {
    if token != state.config.admin.token {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    // Record activation time
    *state.admin_session.lock().unwrap() = Some(Instant::now());

    let cookie = Cookie::build((ADMIN_COOKIE, "1"))
        .path("/")
        .max_age(time::Duration::minutes(SESSION_MINUTES as i64))
        .http_only(true)
        .build();

    (jar.add(cookie), Redirect::to("/")).into_response()
}

fn is_admin_session(state: &AppState, jar: &CookieJar) -> bool {
    if jar.get(ADMIN_COOKIE).is_none() {
        return false;
    }
    match *state.admin_session.lock().unwrap() {
        Some(activated_at) => activated_at.elapsed().as_secs() < SESSION_MINUTES * 60,
        None => false,
    }
}

// ---------- Mobile detection ----------

fn detect_mobile(headers: &HeaderMap) -> bool {
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if ua.is_empty() {
        return false;
    }

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
