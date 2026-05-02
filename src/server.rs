//! HTTP server.
//!
//! Builds the Axum router from `AppState` and runs it. Handlers are kept
//! deliberately thin — they pull from state and delegate to `render`.

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
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

async fn index_handler(State(state): State<AppState>) -> impl IntoResponse {
    match render::render_index(&state.config.site, &state.config.theme, &state.content) {
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
