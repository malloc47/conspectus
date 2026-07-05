//! Plain-text table renderer over a [`GraphSnapshot`].
//!
//! See ADR 0006 for the projection vocabulary. Three projections are
//! supported:
//!
//! - [`Projection::Agent`] — one row per `AgentSession`, showing the
//!   preferred mux and preferred PR.
//! - [`Projection::Mux`] — one row per `MuxSession`, showing every
//!   attached agent session.
//! - [`Projection::Union`] — combined view with one row per node,
//!   preserving relationship status.
//!
//! Every projection shares one compact indicator format:
//!
//! - Provenance codes: `LD` (local declared), `GD` (global declared),
//!   `SD` (strong discovered), `D` (discovered), `C` (convention),
//!   `$` (cached).
//! - Confidence codes: `H` / `M` / `L`.
//! - An ambiguity marker `*` follows the cell when the resolver chose
//!   among multiple plausible candidates for that source/relation.
//!
//! Shared rendering primitives — [`RenderOptions`], the column
//! registry, [`render_rows`], the ADR 0022 color palette — live in
//! [`super::render`] (P10-003 / ADR 0043). This module re-exports the
//! public ones so external callers (`cli`, `node_show`, `tui`) keep
//! compiling against the same surface.

use crate::model::{Confidence, GraphSnapshot, NodeId, Provenance};

// Re-exports of the backend-agnostic surface that external callers
// reach for through `output::table::*`. New code should prefer
// `output::render::*` directly.
pub use super::render::{
    COLUMN_GAP, COLUMN_GAP_WIDTH, ColumnSpec, ColumnsError, Layout, MIN_COLUMN_BUDGET, Projection,
    RenderOptions, SHORT_ID_FLOOR, columns_for, current_epoch, default_columns, display_width,
    fit_to_width, format_relative_age, header_label, header_style, natural_widths,
    node_short_id_from_display, parse_columns_spec, push_styled, render_columns_listing,
    render_rows, resolve_explicit_columns, strip_branch_prefix, truncate_to_width,
    unique_prefix_len,
};

/// FNV-1a 64-bit hash of a [`NodeId`]'s `Display` form. Used to derive
/// a stable short row identifier for table output (H-TBL-002).
pub fn node_short_id(node_id: &NodeId) -> String {
    node_short_id_from_display(&node_id.to_string())
}

/// Render `snapshot` as an untruncated plain-text table using `projection`.
pub fn render(snapshot: &GraphSnapshot, projection: Projection) -> String {
    render_with(snapshot, projection, &RenderOptions::wide())
}

/// Render `snapshot` as a plain-text table using `projection` and `options`.
pub fn render_with(
    snapshot: &GraphSnapshot,
    projection: Projection,
    options: &RenderOptions,
) -> String {
    let columns: Vec<&'static str> = options
        .columns
        .clone()
        .unwrap_or_else(|| default_columns(projection));
    let rows = match projection {
        Projection::Agent => {
            super::agent::build_agent_rows_from_snapshot(snapshot, &columns, options)
        }
        Projection::Mux => super::mux::build_mux_rows_from_snapshot(snapshot, &columns, options),
        Projection::Union => {
            super::union::build_union_rows_from_snapshot(snapshot, &columns, options)
        }
        Projection::Pr => super::prs::build_pr_rows_from_snapshot(snapshot, &columns, options),
        Projection::Fork => {
            super::forks::build_fork_rows_from_snapshot(snapshot, &columns, options)
        }
    };
    render_rows(rows, &columns, options)
}

/// Compact `provenance/confidence[*]` cell, e.g. `LD/H*`. Used in
/// every projection so the cells are easy to scan.
pub fn indicator(provenance: Provenance, confidence: Confidence, ambiguous: bool) -> String {
    let mut buf = String::with_capacity(6);
    buf.push_str(provenance_code(provenance));
    buf.push('/');
    buf.push_str(confidence_code(confidence));
    if ambiguous {
        buf.push('*');
    }
    buf
}

fn provenance_code(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "LD",
        Provenance::LocalPin => "LP",
        Provenance::GlobalDeclared => "GD",
        Provenance::GlobalPin => "GP",
        Provenance::StrongDiscovered => "SD",
        Provenance::Discovered => "D",
        Provenance::Convention => "C",
        Provenance::Cached => "$",
    }
}

fn confidence_code(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::High => "H",
        Confidence::Medium => "M",
        Confidence::Low => "L",
    }
}

// -----------------------------------------------------------------------------
// Per-row-type column extractors
// -----------------------------------------------------------------------------

/// Truncation threshold for AGENT-label session keys. UUIDs (32
/// hex chars + 4 dashes = 36) sit above this and collapse via
/// [`short_session_id`]; anything ≤ 32 chars renders verbatim so
/// human-readable session keys aren't truncated unnecessarily.
pub(crate) fn agent_session_key_for_label(key: &str) -> String {
    const UUID_THRESHOLD: usize = 32;
    if key.chars().count() <= UUID_THRESHOLD {
        key.to_string()
    } else {
        short_session_id(key)
    }
}

/// Shorten a session id for human display: keep short ids whole, abbreviate
/// long ones (typical UUIDs) to a `…<last-8>` suffix so adjacent rows stay
/// distinguishable without dominating the table width.
pub(crate) fn short_session_id(key: &str) -> String {
    const FULL_MAX: usize = 12;
    const TAIL: usize = 8;

    if key.chars().count() <= FULL_MAX {
        key.to_string()
    } else {
        let chars: Vec<char> = key.chars().collect();
        let tail: String = chars[chars.len() - TAIL..].iter().collect();
        format!("…{tail}")
    }
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
