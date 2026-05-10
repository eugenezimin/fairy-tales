//! `SessionStore` trait — the interface for admin session management.
//!
//! `auth.rs` depends only on this trait. Swapping the in-memory store for
//! Redis or a database requires changing only `app.rs` and the impl module.

use std::time::Instant;

pub trait SessionStore: Send + Sync {
    /// Record that a rate-limit attempt just happened.
    /// Returns the time of the *previous* attempt if one exists.
    fn last_auth_attempt(&self) -> Option<Instant>;
    fn record_auth_attempt(&self);

    /// Activate a new session with the given token.
    fn activate(&self, token: String);

    /// Check whether `cookie_token` matches an active, unexpired session.
    fn is_active(&self, cookie_token: &str, session_minutes: u64) -> bool;

    /// Invalidate the current session.
    fn clear(&self);
}
