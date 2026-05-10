//! Route handlers.
//!
//! Each handler is a thin adapter: extract what Axum gives us, call domain /
//! repository / render code, return a response. No business logic lives here.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;

use crate::domain::ContentBundle;
use crate::render::{self, views::AdminArticleEntry};
use crate::server::auth::is_admin_session;
use crate::server::mobile;
use crate::server::state::AppState;

// ── Public pages ──────────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Default)]
pub(crate) struct PageQuery {
    page: Option<usize>,
}

pub async fn index_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    axum::extract::Query(pq): axum::extract::Query<PageQuery>,
) -> impl IntoResponse {
    serve_page(state, headers, jar, None, pq.page.unwrap_or(1)).await
}

pub async fn article_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Path(slug): Path<String>,
    axum::extract::Query(pq): axum::extract::Query<PageQuery>,
) -> impl IntoResponse {
    serve_page(state, headers, jar, Some(slug), pq.page.unwrap_or(1)).await
}
async fn serve_page(
    state: AppState,
    headers: HeaderMap,
    jar: CookieJar,
    slug: Option<String>,
    page: usize,
) -> impl IntoResponse {
    let is_mobile = mobile::detect(&headers);
    let is_admin = is_admin_session(&state, &jar);

    let slug = match slug {
        Some(s) => s,
        None => match state.repo.random_slug() {
            Ok(s) => s,
            Err(_) => {
                let view = render::empty_view(
                    &state.config.site,
                    &state.config.theme,
                    is_mobile,
                    is_admin,
                    &resolve_static_base(&state),
                    state.config.strings.clone(),
                );
                return match state.renderer.render(view) {
                    Ok(html) => Html(html).into_response(),
                    Err(err) => {
                        tracing::error!(error = ?err, "failed to render empty state");
                        (StatusCode::NOT_FOUND, "no articles found").into_response()
                    }
                };
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
    let view = render::article_view(
        &state.config.site,
        &state.config.theme,
        &bundle,
        is_mobile,
        is_admin,
        &resolve_static_base(&state),
        state.config.strings.clone(),
        &state.config.pagination,
        page,
    );

    match state.renderer.render(view) {
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
    headers: HeaderMap,
    jar: CookieJar,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let is_mobile = mobile::detect(&headers);

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

    let view = render::admin_view(
        &state.config.site,
        &state.config.theme,
        is_mobile,
        articles,
        &resolve_static_base(&state),
        state.config.strings.clone(),
    );

    match state.renderer.render(view) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "admin list render failed");
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

// ── Admin: image upload ───────────────────────────────────────────────────────

pub async fn upload_image_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if !is_admin_session(&state, &jar) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let orig_name = headers
        .get("x-image-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image.jpg")
        .to_string();

    let slug = headers
        .get("x-article-slug")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("article")
        .to_string();

    let index: u32 = headers
        .get("x-image-index")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let ext = std::path::Path::new(&orig_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();

    if !matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "avif"
    ) {
        return (StatusCode::BAD_REQUEST, "unsupported image type").into_response();
    }

    let new_name = format!("{}-{}.{}", slug, index, ext);
    let img_dir = std::path::Path::new("static").join("img");

    if let Err(e) = std::fs::create_dir_all(&img_dir) {
        tracing::error!(error = ?e, "failed to create static/img directory");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create image directory",
        )
            .into_response();
    }

    let dest = img_dir.join(&new_name);
    if let Err(e) = std::fs::write(&dest, &body) {
        tracing::error!(error = ?e, path = %dest.display(), "failed to write image");
        return (StatusCode::INTERNAL_SERVER_ERROR, "failed to save image").into_response();
    }

    tracing::info!(file = %new_name, "image uploaded");
    (StatusCode::OK, new_name).into_response()
}

// ── Misc ──────────────────────────────────────────────────────────────────────

pub async fn health_handler() -> &'static str {
    "ok"
}

fn resolve_static_base(state: &AppState) -> String {
    state
        .config
        .server
        .static_source
        .github_raw_base()
        .unwrap_or_default()
}
