//! Native multi-harness session transcript viewer (ADR 0052).
//!
//! This module is designed to be **extractable** as a standalone
//! crate. It MUST NOT import from `crate::model`, `crate::resolve`,
//! `crate::discovery`, or `crate::tui::*` outside its own subtree.
//! (`crate::tui::theme` is the one tracked
//! exception, see ADR 0052; until extraction it's wrapped here in
//! [`theme`].) Adding a new external crate dep without updating
//! `ALLOWED_EXTERNAL_DEPS` (in this module's tests) and
//! `docs/transcript-viewer-deps.md` is a review blocker.
//!
//! Per ADR 0052 the viewer is the **default** target of the `T`
//! keybind on `AgentSession` rows. The bridge that hooks the TUI
//! into this module lives in `src/tui/viewer_bridge.rs` (to keep
//! `crate::tui::*` imports outside this subtree).
//!
//! # Dependency surface
//!
//! The tests' `ALLOWED_EXTERNAL_DEPS` is the machine-readable mirror of the
//! "Direct dependencies" table in `docs/transcript-viewer-deps.md`.
//! The `tests::dep_surface_matches_doc_manifest` test asserts the
//! two agree at compile time; diverging them is a review blocker.

pub mod help;
pub mod input;
pub mod model;
pub mod parser;
pub mod render;
pub mod search;
pub mod state;
pub mod table;
pub mod theme;
pub mod widget;

#[cfg(test)]
#[path = "viewer_tests.rs"]
mod tests;
