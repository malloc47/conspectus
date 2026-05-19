//! Core library for the Conspectus CLI.

pub mod api;
pub mod config;
pub mod declared;
pub mod discovery;
pub mod model;
pub mod output;
pub mod resolve;
pub mod tui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
