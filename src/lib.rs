//! Core library for the Conspectus CLI.
//!
//! The library contract (ADR 0015) is the [`api`] facade plus the
//! [`model`], [`discovery`], [`resolve`], [`output`], [`config`],
//! [`declared`], [`aliases`], and [`rename`] modules. Modules hidden
//! from these docs (`cli`, `tui`, `snapshot`, `hook`, `filter`, and
//! `dev_scenarios`) stay reachable for the binary, integration tests,
//! and the widget preview example, but are not part of that contract.

pub mod aliases;
pub mod api;
#[doc(hidden)]
pub mod cli;
pub mod config;
pub mod declared;
#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
pub mod dev_scenarios;
pub mod discovery;
#[doc(hidden)]
pub mod filter;
#[doc(hidden)]
pub mod hook;
pub mod model;
pub mod output;
pub(crate) mod pin_bindings;
pub(crate) mod pin_store_registry;
pub(crate) mod pins;
pub mod rename;
pub mod resolve;
pub(crate) mod server;
#[doc(hidden)]
pub mod snapshot;
#[doc(hidden)]
pub mod tui;
pub(crate) mod tui_state;
pub(crate) mod viewer;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
