//! In-memory implementation of `SessionStore`.
//!
//! State lives in two `Mutex`-wrapped `Option`s — the same data that used to
//! sit directly on `AppState`. Thread-safe, zero dependencies.

use std::sync::Mutex;
use std::time::Instant;

use super::session::SessionStore;

pub struct InMemorySessionStore {
    session: Mutex<Option<(String, Instant)>>,
    last_attempt: Mutex<Option<Instant>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            last_attempt: Mutex::new(None),
        }
    }
}

impl SessionStore for InMemorySessionStore {
    fn last_auth_attempt(&self) -> Option<Instant> {
        *self.last_attempt.lock().unwrap()
    }

    fn record_auth_attempt(&self) {
        *self.last_attempt.lock().unwrap() = Some(Instant::now());
    }

    fn activate(&self, token: String) {
        *self.session.lock().unwrap() = Some((token, Instant::now()));
    }

    fn is_active(&self, cookie_token: &str, session_minutes: u64) -> bool {
        match self.session.lock().unwrap().as_ref() {
            Some((token, activated_at)) => {
                activated_at.elapsed().as_secs() < session_minutes * 60
                    && super::auth::constant_time_eq(cookie_token.as_bytes(), token.as_bytes())
            }
            None => false,
        }
    }

    fn clear(&self) {
        *self.session.lock().unwrap() = None;
    }
}
