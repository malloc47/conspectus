//! Palette wrapper. Until extraction this thin module re-exports
//! the conspectus [`crate::tui::theme::Theme`] so the rest of
//! `src/viewer/` doesn't import `crate::tui::theme` directly
//! everywhere — when the viewer is lifted into a standalone crate,
//! this is the only file that needs touching to swap in a
//! self-contained palette.
//!
//! ADR 0052 §"Extraction-ready boundary" rule 1: this module is
//! the tracked carve-out for the otherwise-forbidden import from
//! `crate::tui::*`.

pub use crate::tui::theme::Theme;
