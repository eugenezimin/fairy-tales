//! HTTP middleware.

use axum::{extract::Request, response::IntoResponse};

/// Inject security-related response headers on every request.
pub async fn security_headers(request: Request, next: axum::middleware::Next) -> impl IntoResponse {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    headers.insert(
    "Content-Security-Policy",
    "default-src 'self'; style-src 'self' 'unsafe-inline' https://raw.githubusercontent.com; script-src 'self' 'unsafe-inline' https://raw.githubusercontent.com; img-src 'self' data: https:"
        .parse()
        .unwrap(),
);
    response
}
