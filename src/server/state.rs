//! Shared application state threaded through every Axum handler.

use std::sync::Arc;

use crate::config::Config;
use crate::render::Renderer;
use crate::repository::ArticleRepository;
use crate::server::session::SessionStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub repo: Arc<dyn ArticleRepository>,
    pub renderer: Arc<dyn Renderer>,
    pub sessions: Arc<dyn SessionStore>,
}
