//! Projection dispatch for plain-text table output over a
//! [`GraphSnapshot`] (H-HYG-010, ADR 0006).
//!
//! This module owns the per-projection dispatch — the small
//! surface that takes a `(snapshot, projection, options)`
//! triple and picks the right per-projection row builder.
//! It sits **outside** the [`super::render`] substrate because
//! the substrate is model-free by contract
//! (`substrate_has_no_model_deps` test enforces the invariant)
//! and this module by design touches typed `GraphSnapshot` /
//! `Provenance` / `Confidence` / `NodeId`.
//!
//! Supported projections (ADR 0006):
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
//! Shared rendering primitives — [`super::render::RenderOptions`],
//! the column registry, [`super::render::render_rows`], the
//! ADR 0022 color palette — live in [`super::render`] and are
//! reached directly. Pre-H-HYG-010 this module re-exported them;
//! callers now go through `output::render::*` for those items
//! and through `output::table::*` only for the projection-
//! dispatch entrypoints below.

use crate::model::{Confidence, GraphSnapshot, NodeId, Provenance};

use super::render::{
    Projection, RenderOptions, default_columns, node_short_id_from_display, render_rows,
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
