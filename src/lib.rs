//! Core library for the Conspectus CLI.

pub mod aliases;
pub mod api;
pub mod config;
pub mod declared;
#[cfg(any(test, debug_assertions))]
pub mod dev_scenarios;
pub mod discovery;
pub mod filter;
pub mod hook;
pub mod model;
pub mod output;
pub mod pin_bindings;
pub mod pins;
#[cfg(feature = "query")]
pub mod query;
pub mod rename;
pub mod resolve;
pub mod tui;
pub mod tui_state;
pub mod viewer;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
