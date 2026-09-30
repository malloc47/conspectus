//! Native multi-harness session transcript viewer (ADR 0052).
//!
//! This module is designed to be **extractable** as a standalone
//! crate. It MUST NOT import from `crate::model`, `crate::resolve`,
//! `crate::discovery`, or `crate::tui::*` outside its own subtree.
//! (`crate::tui::theme` is the one tracked
//! exception, see ADR 0052; until extraction it's wrapped here in
//! [`theme`].) Adding a new external crate dep without updating
//! [`ALLOWED_EXTERNAL_DEPS`] and `docs/transcript-viewer-deps.md`
//! is a review blocker.
//!
//! Per ADR 0052 the viewer is the **default** target of the `T`
//! keybind on `AgentSession` rows. The bridge that hooks the TUI
//! into this module lives in `src/tui/viewer_bridge.rs` (to keep
//! `crate::tui::*` imports outside this subtree).
//!
//! # Dependency surface
//!
//! [`ALLOWED_EXTERNAL_DEPS`] is the machine-readable mirror of the
//! "Direct dependencies" table in `docs/transcript-viewer-deps.md`.
//! The [`tests::dep_surface_matches_doc_manifest`] test asserts the
//! two agree at compile time; diverging them is a review blocker.

pub mod input;
pub mod model;
pub mod parser;
pub mod render;
pub mod state;
pub mod table;
pub mod theme;
pub mod widget;

/// Crates the viewer module is permitted to depend on (library
/// surface). Mirrors `docs/transcript-viewer-deps.md`. Add to both
/// when an entry changes.
///
/// Entries are crate names as they appear at the *use-statement*
/// level (e.g. `ratatui`, not `ratatui-core`). Feature flags and
/// versions are tracked in `Cargo.toml` and the manifest doc; this
/// list is just the names.
pub const ALLOWED_EXTERNAL_DEPS: &[&str] = &[
    "ansi-to-tui",
    "anyhow",
    "chrono",
    "comfy-table",
    "crossterm",
    "ratatui",
    "rusqlite",
    "serde",
    "serde_json",
    "syntect",
    "thiserror",
    "tui-markdown",
    "unicode-width",
];

/// Binary-only deps used by the extracted-crate `[[bin]]` wrapper.
/// Not imported by the library surface.
pub const ALLOWED_BINARY_DEPS: &[&str] = &["clap"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Read the `docs/transcript-viewer-deps.md` manifest at compile
    /// time and assert the crate names in the "Direct dependencies"
    /// table match [`ALLOWED_EXTERNAL_DEPS`].
    ///
    /// Failure means the doc and the code disagree about what the
    /// viewer is allowed to import. Update both in the same review.
    #[test]
    fn dep_surface_matches_doc_manifest() {
        const DOC: &str = include_str!("../../docs/transcript-viewer-deps.md");

        let library_deps = extract_deps_under_heading(DOC, "## Direct dependencies");
        assert_eq!(
            sorted(library_deps),
            sorted(
                ALLOWED_EXTERNAL_DEPS
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect()
            ),
            "src/viewer/mod.rs ALLOWED_EXTERNAL_DEPS and docs/transcript-viewer-deps.md \
             disagree on the viewer's permitted direct dependencies. Update both."
        );

        let binary_deps = extract_deps_under_heading(DOC, "## Binary-only dependencies");
        assert_eq!(
            sorted(binary_deps),
            sorted(
                ALLOWED_BINARY_DEPS
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect()
            ),
            "src/viewer/mod.rs ALLOWED_BINARY_DEPS and docs/transcript-viewer-deps.md \
             disagree on the viewer's binary-only dependencies. Update both."
        );
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    /// Parse the doc's Markdown tables for crate names in the first
    /// column. Looks for the next `| name | ... |` rows after
    /// `heading`, stopping at the next `##` heading.
    fn extract_deps_under_heading(doc: &str, heading: &str) -> Vec<String> {
        let mut deps = Vec::new();
        let mut in_section = false;
        for line in doc.lines() {
            if line.starts_with(heading) {
                in_section = true;
                continue;
            }
            if in_section && line.starts_with("## ") {
                break;
            }
            if !in_section {
                continue;
            }
            let Some(name) = first_table_cell_crate_name(line) else {
                continue;
            };
            deps.push(name);
        }
        deps
    }

    /// Pull the first `| `Crate` |` cell name out of a Markdown
    /// table row. Returns `None` for non-table lines, the header
    /// row, the separator row, and rows whose first cell is not a
    /// backticked identifier.
    fn first_table_cell_crate_name(line: &str) -> Option<String> {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            return None;
        }
        let first_cell = trimmed.split('|').nth(1)?.trim();
        // Header row says "Crate"; separator row is dashes.
        if first_cell == "Crate" || first_cell.chars().all(|c| c == '-' || c == ':') {
            return None;
        }
        let backticked = first_cell.strip_prefix('`')?.strip_suffix('`')?;
        Some(backticked.to_string())
    }
}
