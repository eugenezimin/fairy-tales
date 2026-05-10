//! The `Renderer` trait — the interface between handlers and any HTML backend.
//!
//! Handlers depend only on this trait. Swapping Askama for Tera (or a JSON
//! serialiser, or a test stub) requires changing only `app.rs` and the impl
//! module; handler code is untouched.

use anyhow::Result;

use crate::render::views::PageView;

pub trait Renderer: Send + Sync {
    fn render(&self, view: PageView) -> Result<String>;
}
