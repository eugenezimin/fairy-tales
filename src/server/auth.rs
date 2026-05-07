//! Admin authentication and session management.
//!
//! Authentication is token-based: the operator sets a secret token in config.
//! Visiting `/auth/<token>` activates a short-lived server-side session backed
//! by a signed cookie. No passwords, no user accounts.

use std::time::Instant;

use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use crate::server::state::AppState;

pub const ADMIN_COOKIE: &str = "admin_session";
pub const SESSION_MINUTES: u64 = 30;

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn activate_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
) -> impl IntoResponse {
    // Simple rate limit: one attempt every 2 seconds.
    {
        let mut last = state.last_auth_attempt.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed().as_secs() < 2 {
                return (StatusCode::TOO_MANY_REQUESTS, "slow down").into_response();
            }
        }
        *last = Some(Instant::now());
    }

    if !constant_time_eq(
        token.as_bytes(),
        state.config.admin.token.as_deref().unwrap_or("").as_bytes(),
    ) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let session_token = random_hex_token();
    *state.admin_session.lock().unwrap() = Some((session_token.clone(), Instant::now()));

    let cookie = Cookie::build((ADMIN_COOKIE, session_token))
        .path("/")
        .max_age(time::Duration::minutes(SESSION_MINUTES as i64))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.config.server.secure_cookies)
        .build();

    (jar.add(cookie), Redirect::to("/")).into_response()
}

pub async fn logout_handler(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    *state.admin_session.lock().unwrap() = None;
    let mut removal = Cookie::from(ADMIN_COOKIE);
    removal.set_path("/");
    (jar.remove(removal), Redirect::to("/")).into_response()
}

// ── Session check ─────────────────────────────────────────────────────────────

/// Returns `true` when the request carries a valid, unexpired admin session.
pub fn is_admin_session(state: &AppState, jar: &CookieJar) -> bool {
    let cookie_value = match jar.get(ADMIN_COOKIE) {
        Some(c) => c.value().to_string(),
        None => return false,
    };
    match state.admin_session.lock().unwrap().as_ref() {
        Some((token, activated_at)) => {
            activated_at.elapsed().as_secs() < SESSION_MINUTES * 60
                && constant_time_eq(cookie_value.as_bytes(), token.as_bytes())
        }
        None => false,
    }
}

// ── Crypto helpers ────────────────────────────────────────────────────────────

/// Constant-time byte-slice equality to resist timing attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Produce a pseudo-random 64-hex-char session token.
///
/// Uses two independent `DefaultHasher` seeds (wall clock + stack address).
/// Not cryptographically secure, but adequate for a single-operator admin
/// session on a trusted machine.
fn random_hex_token() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h1 = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h1);
    std::thread::current().id().hash(&mut h1);

    let mut h2 = DefaultHasher::new();
    let stack_addr: usize = &h2 as *const _ as usize;
    stack_addr.hash(&mut h2);
    std::time::Instant::now().hash(&mut h2);

    let a = h1.finish();
    let b = h2.finish();
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        a,
        b,
        a ^ b,
        a.wrapping_add(b)
    )
}
