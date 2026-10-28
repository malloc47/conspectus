//! A terminal UI framework built on ratatui.
//!
//! The crate covers two layers:
//!
//! - **Components and design tokens:** widgets, theme and palette, glyphs,
//!   chips, text and scroll helpers, and layout frames.
//! - **Interaction conventions:** key tables that generate footers and help,
//!   the overlay and modal-stack contract, focus rules, a feedback model, and
//!   a test kit.
//!
//! The application runtime stays with the host: the event loop, the effect
//! lifecycle, and terminal hand-off belong to the program that uses these
//! controls.
//!
//! The crate depends on nothing from the program that hosts it, spawns no
//! process, and writes no file. Host meaning enters through type parameters,
//! closures, traits, and a named-style registry. The guard tests in
//! `tests/independence.rs` enforce this and document how to request a
//! reviewed exception.
//!
//! This crate is not published: it carries no semver promise, and it can
//! move to its own repository as a directory lift.
