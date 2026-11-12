//! The live state each row carries for its entity (ADR 0205): a
//! session's effective state, a mux's counts over the agents it hosts.
//!
//! One pass ([`annotate`]) fills every row of every view after the view's
//! builder has laid its rows out, the way the content search's hits are
//! filled, so the same entity carries the same value wherever it is
//! listed and no builder works it out for itself. The values are read
//! from a live table (the snapshot's own `node_live`, or the one the
//! TUI bound from the store since) at one moment: an effective state
//! depends on the time it is read at.
//!
//! The rows carry the values; the left pane draws them
//! ([`super::LiveSign`]) and no builder reads them.

use std::collections::BTreeMap;

use crate::model::live::{
    LiveBinding, LiveConfidence, LiveCounts, LiveDetail, LiveObservation, LiveState,
};
use crate::model::{GraphNode, GraphSnapshot, LiveTables, MuxSessionId, NodeId};

use super::{RowKind, RowTree};

/// What a session's row knows of its live state: the effective state at
/// the time the row was filled, and the observation's facts beside it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCell {
    pub state: LiveState,
    pub detail: Option<LiveDetail>,
    /// Whether the evidence is fresh, quiet, or too old to trust.
    pub confidence: LiveConfidence,
    /// When the observed state began, Unix epoch milliseconds.
    pub since_ms: i64,
    /// Active child agents.
    pub subagents: u16,
}

impl LiveCell {
    /// The cell for `binding` read at `now_ms`.
    pub fn of(binding: &LiveBinding, now_ms: i64) -> Self {
        let effective = binding.effective(now_ms);
        let LiveObservation {
            since_ms,
            subagents,
            ..
        } = binding.observation;
        Self {
            state: effective.state,
            detail: effective.detail,
            confidence: effective.confidence,
            since_ms,
            subagents,
        }
    }
}

/// The live state of a row that can stand for an entity of any kind (the
/// Pinned and History rows, a stopped mux): a session's cell, a mux's
/// counts, or nothing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MixedLive {
    /// Not a session or a mux, or no evidence for it.
    #[default]
    None,
    Session(LiveCell),
    /// Counts over the mux's agents; empty when it has no evidence.
    Mux(LiveCounts),
}

/// The live values of one snapshot's entities at one moment: a cell for
/// each entry of the live table and counts for each mux an entry counts
/// toward.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveValues {
    cells: BTreeMap<NodeId, LiveCell>,
    counts: BTreeMap<MuxSessionId, LiveCounts>,
}

impl LiveValues {
    /// `tables`, bound to `snapshot`, read at `now_ms`. A mux counts by
    /// the rule of [`GraphSnapshot::live_counts_of_mux`].
    pub(crate) fn at(snapshot: &GraphSnapshot, tables: LiveTables<'_>, now_ms: i64) -> Self {
        Self {
            cells: cells_at(tables.nodes, now_ms),
            counts: snapshot.live_counts_by_mux(tables, now_ms),
        }
    }

    /// The cell of a session, or `None` without evidence.
    pub(crate) fn session(&self, id: &NodeId) -> Option<LiveCell> {
        self.cells.get(id).copied()
    }

    /// A mux's counts; empty without evidence.
    pub(crate) fn mux(&self, id: &MuxSessionId) -> LiveCounts {
        self.counts.get(id).copied().unwrap_or_default()
    }

    /// The value a row standing for `node` carries.
    pub(crate) fn of_node(&self, node: &NodeId) -> MixedLive {
        match node {
            NodeId::AgentSession(_) => self
                .session(node)
                .map_or(MixedLive::None, MixedLive::Session),
            NodeId::MuxSession(mux) => MixedLive::Mux(self.mux(mux)),
            _ => MixedLive::None,
        }
    }
}

/// The cell of every entry of `table` at `now_ms`. The values change with
/// time only through these, so comparing two of them tells whether time
/// alone moved what the rows show.
pub(crate) fn cells_at<K: Clone + Ord>(
    table: &BTreeMap<K, LiveBinding>,
    now_ms: i64,
) -> BTreeMap<K, LiveCell> {
    table
        .iter()
        .map(|(key, binding)| (key.clone(), LiveCell::of(binding, now_ms)))
        .collect()
}

/// Fill in the live value of every session, mux, and mixed row of
/// `tree` from `values`. A pin's row carries the value of the entity it
/// points at. Rows of other kinds carry none.
pub(crate) fn annotate(tree: &mut RowTree, snapshot: &GraphSnapshot, values: &LiveValues) {
    for row in &mut tree.rows {
        match &mut row.kind {
            RowKind::AgentSession(session) => {
                session.live = values.session(&session.primary_node);
            }
            RowKind::MuxSession(mux) => {
                mux.live = values.mux(&mux.mux);
            }
            RowKind::Mixed(mixed) => {
                mixed.live = match &mixed.primary_node {
                    NodeId::Pin(_) => pin_target(snapshot, &mixed.primary_node)
                        .map_or(MixedLive::None, |target| values.of_node(target)),
                    node => values.of_node(node),
                };
            }
            _ => {}
        }
    }
}

/// The entity the pin node `pin` points at.
fn pin_target<'a>(snapshot: &'a GraphSnapshot, pin: &NodeId) -> Option<&'a NodeId> {
    match snapshot.find_node(pin)? {
        GraphNode::Pin(pin) => pin.target_node.as_ref(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
