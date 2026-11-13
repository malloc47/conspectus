//! Whether the operator has seen a finished turn (ADR 0214).
//!
//! A `ready` turn is unseen until the operator has looked at it, and that
//! is derived, read-only, from what the TUI already knows. For an agent
//! in a mux (a session's preferred mux, or the mux of the pane an agent no
//! session matches runs in), a tmux client attached to that mux after the
//! turn ended: the mux's last attach time is at or after it, or a snapshot
//! loaded after it shows a client attached. For a session with no mux, the
//! cursor resting on one of its rows in this TUI for [`SEEN_REST`].
//!
//! A turn found seen stays seen, in this TUI's memory ([`SeenTurns`]),
//! until a later turn ends: a refresh that no longer shows the client
//! attached can't make it unseen again. Nothing is written anywhere, and
//! the memory starts empty with each TUI.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::model::{GraphNode, GraphSnapshot, LivePane, MuxSessionId, MuxSessionNode, NodeId};

/// How long the cursor rests on a row standing for a session with no mux
/// before that session's finished turn counts as seen. A cursor that
/// passes the row sooner, while the operator scrolls, does not count.
pub(crate) const SEEN_REST: Duration = Duration::from_secs(1);

/// An agent whose turns can be seen: a session, or an agent in a pane
/// that no session matches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Agent {
    Session(NodeId),
    Pane(LivePane),
}

impl Agent {
    /// The mux the agent runs in, when it has one: a session's preferred
    /// mux (the one its live state counts toward), a pane's own.
    pub(crate) fn mux(&self, snapshot: &GraphSnapshot) -> Option<MuxSessionId> {
        match self {
            // An entry put on a mux by hand counts toward that mux.
            Self::Session(NodeId::MuxSession(mux)) => Some(mux.clone()),
            Self::Session(node) => snapshot.preferred_mux_for(node),
            Self::Pane(pane) => Some(pane.mux.clone()),
        }
    }
}

/// The finished turns this TUI found seen: for each agent, when the
/// latest of them ended. In memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SeenTurns {
    turns: BTreeMap<Agent, i64>,
}

/// No turn seen, for readers with no memory of their own.
#[cfg(test)]
static NOTHING_SEEN: SeenTurns = SeenTurns {
    turns: BTreeMap::new(),
};

impl SeenTurns {
    /// Whether the turn of `agent` that ended at `ended_ms` was found
    /// seen. A turn that ended later was not.
    pub(crate) fn has_seen(&self, agent: &Agent, ended_ms: i64) -> bool {
        self.turns.get(agent).is_some_and(|seen| *seen >= ended_ms)
    }

    /// Remember that the turn of `agent` that ended at `ended_ms` was
    /// seen. Says whether that is news.
    pub(crate) fn see(&mut self, agent: Agent, ended_ms: i64) -> bool {
        let seen = self.turns.entry(agent).or_insert(i64::MIN);
        if *seen >= ended_ms {
            return false;
        }
        *seen = ended_ms;
        true
    }
}

/// What judging a turn reads besides the snapshot: the memory, and when
/// the snapshot was loaded (Unix epoch milliseconds), which dates the
/// clients its tmux listing shows attached.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeenContext<'a> {
    pub(crate) memory: &'a SeenTurns,
    pub(crate) loaded_ms: i64,
}

#[cfg(test)]
impl SeenContext<'static> {
    /// No memory and no load time: only a mux's last attach time can say
    /// a turn was seen.
    pub(crate) fn nothing() -> Self {
        Self {
            memory: &NOTHING_SEEN,
            loaded_ms: i64::MIN,
        }
    }
}

/// Whether the turn of `agent` that ended at `ended_ms` (when its ready
/// state began) is one the operator has not seen, by `snapshot` and
/// `context`; `mux` is the agent's ([`Agent::mux`]). An agent whose mux
/// the snapshot does not list counts as having none.
pub(crate) fn unseen(
    snapshot: &GraphSnapshot,
    agent: &Agent,
    mux: Option<&MuxSessionId>,
    ended_ms: i64,
    context: SeenContext<'_>,
) -> bool {
    if context.memory.has_seen(agent, ended_ms) {
        return false;
    }
    match mux.and_then(|mux| mux_node(snapshot, mux)) {
        Some(mux) => !attached_since(mux, ended_ms, context.loaded_ms),
        None => true,
    }
}

/// Whether `agent` runs in a mux the snapshot lists: its turns are seen
/// by attaching to it, not by resting the cursor on its row.
pub(crate) fn has_mux(snapshot: &GraphSnapshot, agent: &Agent) -> bool {
    agent
        .mux(snapshot)
        .is_some_and(|mux| mux_node(snapshot, &mux).is_some())
}

fn mux_node<'a>(snapshot: &'a GraphSnapshot, mux: &MuxSessionId) -> Option<&'a MuxSessionNode> {
    match snapshot.find_node(&NodeId::MuxSession(mux.clone()))? {
        GraphNode::MuxSession(node) => Some(node),
        _ => None,
    }
}

/// Whether a client attached to `mux` after the turn that ended at
/// `ended_ms`: its last attach was at or after the second the turn
/// ended, or a client was attached when a snapshot loaded at `loaded_ms`,
/// after the turn ended, listed it. tmux keeps attach times to the
/// second, so an attach in the turn's own last second counts.
fn attached_since(mux: &MuxSessionNode, ended_ms: i64, loaded_ms: i64) -> bool {
    let attached_after = mux
        .last_attached_epoch
        .is_some_and(|epoch| epoch >= ended_ms.div_euclid(1000));
    let attached_at_load = mux.client_attached == Some(true) && ended_ms <= loaded_ms;
    attached_after || attached_at_load
}

#[cfg(test)]
#[path = "seen_tests.rs"]
mod tests;
