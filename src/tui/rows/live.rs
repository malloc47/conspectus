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
//! A ready turn's cell also says whether the operator has seen it
//! ([`super::seen`]), which the pass judges from the snapshot and the
//! TUI's memory of the turns it found seen.
//!
//! The rows carry the values, and the tree keeps the whole set it filled
//! them from ([`RowTree::live`]), which the detail pane reads, so a
//! session's fields there say what its row's mark says; the left pane
//! draws them ([`super::LiveSign`]) and no builder reads them.

use std::collections::BTreeMap;

use crate::model::live::{
    LiveBinding, LiveConfidence, LiveCounts, LiveDetail, LiveObservation, LiveSource, LiveState,
};
use crate::model::{GraphNode, GraphSnapshot, LivePane, LiveTables, MuxSessionId, NodeId};

use super::seen::{self, Agent, SeenContext};
use super::{LiveMark, LiveSign, RowKind, RowTree};

/// What a session's row knows of its live state: the effective state at
/// the time the row was filled, and the observation's facts beside it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCell {
    pub state: LiveState,
    pub detail: Option<LiveDetail>,
    /// Whether the evidence is fresh, quiet, or too old to trust.
    pub confidence: LiveConfidence,
    /// When the state shown began, Unix epoch milliseconds: the recorded
    /// start, or the latest evidence when the read rules put another
    /// state in the recorded one's place, as `conspectus node show` says.
    pub since_ms: i64,
    /// Active child agents.
    pub subagents: u16,
    /// What reported the state.
    pub source: LiveSource,
    /// A `ready` turn the operator has not seen yet (ADR 0214); always
    /// `false` in any other state.
    pub unseen: bool,
}

impl LiveCell {
    /// The cell for `binding` read at `now_ms`.
    pub fn of(binding: &LiveBinding, now_ms: i64) -> Self {
        let effective = binding.effective(now_ms);
        let LiveObservation {
            subagents, source, ..
        } = binding.observation;
        Self {
            state: effective.state,
            detail: effective.detail,
            confidence: effective.confidence,
            since_ms: crate::output::live::shown_since_ms(binding, &effective),
            subagents,
            source,
            unseen: false,
        }
    }

    /// The child agents worth showing beside the state: none once the
    /// session reads exited or unknown, whatever count was last recorded,
    /// as `conspectus table` shows them.
    pub fn running_subagents(&self) -> u16 {
        if self.state.shows_subagents() {
            self.subagents
        } else {
            0
        }
    }
}

/// What a mux's row knows of its agents' live state: how many are in
/// each state, and how many of its ready ones ended a turn the operator
/// has not seen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MuxLive {
    pub counts: LiveCounts,
    pub unseen: u32,
}

impl MuxLive {
    /// Whether no agent of the mux has live state.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

impl From<LiveCounts> for MuxLive {
    /// Counts with no unseen turn among them.
    fn from(counts: LiveCounts) -> Self {
        Self { counts, unseen: 0 }
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
    Mux(MuxLive),
}

/// The live values of one snapshot's entities at one moment: a cell for
/// each entry of the live tables (a session's, or a pane's no session
/// matches) and counts for each mux an entry counts toward.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveValues {
    cells: BTreeMap<NodeId, LiveCell>,
    panes: BTreeMap<LivePane, LiveCell>,
    counts: BTreeMap<MuxSessionId, LiveCounts>,
    /// For each mux, how many ready agents counted on it ended a turn the
    /// operator has not seen.
    unseen: BTreeMap<MuxSessionId, u32>,
    /// Every agent of the tables waiting on the operator.
    waiting: usize,
}

impl LiveValues {
    /// `tables`, bound to `snapshot`, read at `now_ms`, each ready turn
    /// judged seen or not by `seen`. A mux counts by the rule of
    /// [`GraphSnapshot::live_counts_of_mux`].
    pub(crate) fn at(
        snapshot: &GraphSnapshot,
        tables: LiveTables<'_>,
        now_ms: i64,
        seen: SeenContext<'_>,
    ) -> Self {
        let mut cells = cells_at(tables.nodes, now_ms);
        let mut panes = cells_at(tables.panes, now_ms);
        let mut unseen: BTreeMap<MuxSessionId, u32> = BTreeMap::new();
        let ready = cells
            .iter_mut()
            .map(|(node, cell)| (Agent::Session(node.clone()), cell))
            .chain(
                panes
                    .iter_mut()
                    .map(|(pane, cell)| (Agent::Pane(pane.clone()), cell)),
            )
            .filter(|(_, cell)| cell.state == LiveState::Ready);
        for (agent, cell) in ready {
            let mux = agent.mux(snapshot);
            cell.unseen = seen::unseen(snapshot, &agent, mux.as_ref(), cell.since_ms, seen);
            if let Some(mux) = mux.filter(|_| cell.unseen) {
                *unseen.entry(mux).or_default() += 1;
            }
        }
        let waiting = cells
            .values()
            .chain(panes.values())
            .filter(|cell| LiveSign::of_session(Some(**cell)).mark == LiveMark::Waiting)
            .count();
        Self {
            cells,
            panes,
            counts: snapshot.live_counts_by_mux(tables, now_ms),
            unseen,
            waiting,
        }
    }

    /// Every ready turn these values found seen, with when it ended, for
    /// the memory to keep ([`seen::SeenTurns::see`]).
    pub(crate) fn seen_turns(&self) -> impl Iterator<Item = (Agent, i64)> + '_ {
        let seen = |cell: &LiveCell| cell.state == LiveState::Ready && !cell.unseen;
        let sessions = self
            .cells
            .iter()
            .filter(move |(_, cell)| seen(cell))
            .map(|(node, cell)| (Agent::Session(node.clone()), cell.since_ms));
        let panes = self
            .panes
            .iter()
            .filter(move |(_, cell)| seen(cell))
            .map(|(pane, cell)| (Agent::Pane(pane.clone()), cell.since_ms));
        sessions.chain(panes)
    }

    /// Whether any agent is ready, so judging its turn again could
    /// change what the rows show.
    pub(crate) fn any_ready(&self) -> bool {
        self.cells
            .values()
            .chain(self.panes.values())
            .any(|cell| cell.state == LiveState::Ready)
    }

    /// How many agents anywhere wait on the operator: every entry of the
    /// tables, a session's or a pane's no session matches, whichever rows
    /// a view lists.
    pub(crate) fn waiting(&self) -> usize {
        self.waiting
    }

    /// The cell of a session, or `None` without evidence.
    pub(crate) fn session(&self, id: &NodeId) -> Option<LiveCell> {
        self.cells.get(id).copied()
    }

    /// A mux's counts and its unseen turns; empty without evidence.
    pub(crate) fn mux(&self, id: &MuxSessionId) -> MuxLive {
        MuxLive {
            counts: self.counts.get(id).copied().unwrap_or_default(),
            unseen: self.unseen.get(id).copied().unwrap_or_default(),
        }
    }

    /// The agents seen in panes of `mux` that no session matches, each
    /// with the pane it runs in, in pane order.
    pub(crate) fn panes_of<'a>(
        &'a self,
        mux: &'a MuxSessionId,
    ) -> impl Iterator<Item = (&'a LivePane, LiveCell)> + 'a {
        self.panes
            .iter()
            .filter(move |(pane, _)| pane.mux == *mux)
            .map(|(pane, cell)| (pane, *cell))
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
/// `tree` from `values`, the tree's count of the agents anywhere that
/// wait, and the values themselves, which the detail pane reads. A pin's
/// row carries the value of the entity it points at. Rows of other kinds
/// carry none.
pub(crate) fn annotate(tree: &mut RowTree, snapshot: &GraphSnapshot, values: &LiveValues) {
    tree.waiting_anywhere = values.waiting();
    tree.live = values.clone();
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
pub(crate) fn pin_target<'a>(snapshot: &'a GraphSnapshot, pin: &NodeId) -> Option<&'a NodeId> {
    match snapshot.find_node(pin)? {
        GraphNode::Pin(pin) => pin.target_node.as_ref(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
