//! Configuration.
//!
//! `types`  — pure data structures (structs, enums).
//! `loader` — I/O, env-var overlay, validation logic (`Config::load`, `Config::resolve_path`).

pub mod loader;
pub mod types;

pub use types::*;
