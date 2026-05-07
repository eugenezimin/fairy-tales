//! Shared application state threaded through every Axum handler.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;
use crate::repository::ArticleRepository;

/// Cloneable handle to all shared runtime state.
///
/// `repo` is the only content-aware field. Handlers call methods on the
/// trait object; they are oblivious to whether articles live on disk,
/// in a database, or in S3.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub repo: Arc<dyn ArticleRepository>,
    /// `Some((session_token, activated_at))` while a session is live.
    pub admin_session: Arc<Mutex<Option<(String, Instant)>>>,
    pub last_auth_attempt: Arc<Mutex<Option<Instant>>>,
}
