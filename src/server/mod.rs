//! HTTP server assembly.
//!
//! Wires routes, middleware, and static-file serving together.
//! The only exported symbols needed by `app.rs` are `AppState` (re-exported
//! from `state`) and `run`.

pub mod auth;
pub mod handlers;
pub mod middleware;
pub mod mobile;
pub mod state;

pub use state::AppState;

use anyhow::{Context, Result};
use axum::{
    Router,
    http::{HeaderValue, header},
    middleware as axum_middleware,
    routing::{delete, get, post},
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};

use crate::server::{
    auth::{activate_handler, logout_handler},
    handlers::{
        admin_list_handler, article_handler, delete_article_handler, health_handler, index_handler,
        list_articles_redirect_handler, upload_article_handler, upload_image_handler,
    },
    middleware::security_headers,
};

use axum::extract::DefaultBodyLimit;

// ── Router ────────────────────────────────────────────────────────────────────

pub fn build_router(state: AppState) -> Router {
    use crate::config::StaticSource;

    tracing::info!("registering routes");

    let mut router = Router::new()
        // Public
        .route("/", get(index_handler))
        .route("/article/:slug", get(article_handler))
        .route("/healthz", get(health_handler))
        // Auth
        .route("/auth/:token", get(activate_handler))
        .route("/admin/logout", get(logout_handler))
        // Admin
        .route("/admin", get(admin_list_handler))
        .route("/admin/articles", get(list_articles_redirect_handler))
        .route(
            "/admin/article",
            post(upload_article_handler).layer(DefaultBodyLimit::max(512 * 1024)),
        )
        .route(
            "/admin/image",
            post(upload_image_handler).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/admin/article/:slug", delete(delete_article_handler));

    // Static files — only wired for local sources.
    if let StaticSource::Local { source } = state.config.server.resolved_static_source() {
        router = router.nest_service(
            "/static",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new(source)),
        );
    }

    // Always serve uploaded images from the local filesystem,
    // even when CSS/JS come from GitHub.
    router = router.nest_service(
        "/static/img",
        tower::ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::if_not_present(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=86400"),
            ))
            .service(ServeDir::new("static/img")),
    );

    router
        // Catch-all
        .fallback(|req: axum::http::Request<axum::body::Body>| async move {
            tracing::warn!(
                method = %req.method(),
                path   = %req.uri().path(),
                "no route matched"
            );
            (
                axum::http::StatusCode::NOT_FOUND,
                format!("404 — no route for {} {}", req.method(), req.uri().path()),
            )
        })
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
                let path = request.uri().path();
                let logged_path = if path.starts_with("/auth/") {
                    "/auth/[REDACTED]"
                } else {
                    path
                };
                tracing::info_span!("http", method = %request.method(), path = logged_path)
            }),
        )
        .layer(axum_middleware::from_fn(security_headers))
        .with_state(state)
}

// ── Server lifecycle ──────────────────────────────────────────────────────────

pub async fn run(state: AppState) -> Result<()> {
    let addr = state.config.server.socket_addr();
    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;

    tracing::info!("listening on http://{addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server error")?;
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async { signal::ctrl_c().await.expect("failed to listen for ctrl_c") };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("shutdown signal received");
}
