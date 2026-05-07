//! Route handlers.
//!
//! Each handler is a thin adapter: extract what Axum gives us, call domain /
//! repository / render code, return a response. No business logic lives here.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;

use crate::domain::ContentBundle;
use crate::render;
use crate::server::auth::is_admin_session;
use crate::server::mobile;
use crate::server::state::AppState;

// ── Public pages ──────────────────────────────────────────────────────────────

pub async fn index_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> impl IntoResponse {
    serve_page(state, headers, jar, None).await
}

pub async fn article_handler(
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
    let is_mobile = mobile::detect(&headers);
    let is_admin = is_admin_session(&state, &jar);

    // Resolve the slug to render (random when None).
    let slug = match slug {
        Some(s) => s,
        None => match state.repo.random_slug() {
            Ok(s) => s,
            Err(err) => {
                tracing::error!(error = ?err, "failed to pick random article");
                return (StatusCode::NOT_FOUND, "no articles found").into_response();
            }
        },
    };

    let article = match state.repo.get(&slug) {
        Ok(a) => a,
        Err(err) => {
            tracing::error!(error = ?err, slug = %slug, "failed to load article");
            return (StatusCode::NOT_FOUND, "article not found").into_response();
        }
    };

    let stories = match state.repo.sidebar_stories(&article.slug, 4) {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(error = ?err, "failed to load sidebar stories");
            vec![]
        }
    };

    let bundle = ContentBundle { article, stories };

    match render::render_index(
        &state.config.site,
        &state.config.theme,
        &bundle,
        is_mobile,
        is_admin,
    ) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "failed to render page");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

// ── Admin: article list ───────────────────────────────────────────────────────

pub async fn admin_list_handler(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    #[derive(Template)]
    #[template(path = "admin.html")]
    struct AdminView {
        site_title: String,
        theme: String,
        year: u16,
        articles: Vec<AdminArticleEntry>,
    }

    #[derive(Clone)]
    struct AdminArticleEntry {
        slug: String,
        title: String,
        preview: String,
    }

    let metas = match state.repo.list() {
        Ok(m) => m,
        Err(err) => {
            tracing::error!(error = ?err, "failed to list articles");
            return (StatusCode::INTERNAL_SERVER_ERROR, "failed to list articles").into_response();
        }
    };

    let articles: Vec<AdminArticleEntry> = metas
        .into_iter()
        .map(|m| AdminArticleEntry {
            slug: m.slug,
            title: m.title,
            preview: m.snippet,
        })
        .collect();

    match (AdminView {
        site_title: state.config.site.title.clone(),
        theme: state.config.theme.name.clone(),
        year: state.config.site.footer_year,
        articles,
    }
    .render())
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "admin list render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "render error").into_response()
        }
    }
}

// ── Admin: upload ─────────────────────────────────────────────────────────────

pub async fn upload_article_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    body: String,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    match state.repo.save(&body) {
        Ok(slug) => (StatusCode::CREATED, slug).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "failed to save article");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

// ── Admin: delete ─────────────────────────────────────────────────────────────

pub async fn delete_article_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    match state.repo.delete(&slug) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            tracing::error!(error = ?err, slug = %slug, "delete failed");
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

// ── Admin: redirect alias ─────────────────────────────────────────────────────

pub async fn list_articles_redirect_handler(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    Redirect::to("/admin").into_response()
}

// ── Misc ──────────────────────────────────────────────────────────────────────

pub async fn health_handler() -> &'static str {
    "ok"
}
