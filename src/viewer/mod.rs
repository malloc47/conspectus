//! Native multi-harness session transcript viewer (ADR 0052).
//!
//! This module is designed to be **extractable** as a standalone
//! crate. It MUST NOT import from `crate::model`, `crate::resolve`,
//! `crate::discovery`, or `crate::tui::*` outside its own subtree. It
//! draws from the framework theme (`tuikit::theme`); [`theme`] names
//! the styles it reads there. Adding a new external crate dep without
//! updating `ALLOWED_EXTERNAL_DEPS` (in this module's tests) and
//! `docs/transcript-viewer-deps.md` is a review blocker.
//!
//! Per ADR 0052 the viewer is the **default** target of the `T`
//! keybind on `AgentSession` rows. The bridge that hooks the TUI
//! into this module lives in `src/tui/viewer_bridge.rs` (to keep
//! `crate::tui::*` imports outside this subtree).
//!
//! # What is here, and what is the framework's
//!
//! The full-screen viewer is the framework's pager (`tuikit::pager`)
//! over a [`state::Transcript`], which implements its content trait
//! (ADR 0180): the pager owns scrolling, the find prompt and stepping,
//! the help page, the footer, and the chrome. This module owns what is
//! the transcript's: the normalized model ([`model`]) and the parsers
//! that build it ([`parser`]), the rendering of turns ([`render`],
//! [`table`]) and their layout as rows ([`widget`]), a find whose unit
//! is a turn and the count of matches the toggles hide, the four
//! toggles and their keys ([`keys`], [`help`]), opening at a search
//! result ([`find`]), the shared content matcher ([`search`]), and the
//! painter for a transcript's matches ([`highlight`]).
//!
//! # Dependency surface
//!
//! The tests' `ALLOWED_EXTERNAL_DEPS` is the machine-readable mirror of the
//! "Direct dependencies" table in `docs/transcript-viewer-deps.md`.
//! The `tests::dep_surface_matches_doc_manifest` test asserts the
//! two agree at compile time; diverging them is a review blocker.

pub mod find;
pub mod help;
pub mod highlight;
pub mod keys;
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
