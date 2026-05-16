//! Core library for the Conspectus CLI.

pub mod config;
pub mod discovery;
pub mod model;
pub mod output;
pub mod resolve;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
