//! Whether the operator has seen a finished turn (ADR 0214).
//!
//! A `ready` turn is unseen until the operator has looked at it, and that
//! is derived, read-only, from what the TUI already knows. For an agent
//! in a mux (a session's preferred mux, or the mux of the pane an agent no
//! session matches runs in), a tmux client attached to that mux after the
//! turn ended: the mux's last attach time is at or after it, or a snapshot
//! loaded after it shows a client attached, or the operator's client came
//! back to the TUI's tmux session from that mux after the turn ended
//! ([`ClientReturn`]). For a session with no mux, the cursor resting on
//! one of its rows in this TUI for [`SEEN_REST`].
//!
//! A stop found seen stays seen, in this TUI's memory ([`SeenTurns`]),
//! until a later stop begins: a refresh that no longer shows the client
//! attached can't make it unseen again. The marks of agent sessions are
//! kept in the TUI's state file and loaded back at the next start
//! (ADR 0245); a pane's agent has no mark that outlives the TUI.
//!
//! The model's word is unseen and the operator's is unread: the same
//! judgment, for a waiting, ready, or failed agent alike.

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

/// The operator's tmux client came back to the TUI's own session (the
/// one the TUI runs in) from `session`, so it was on that session until
/// `at_ms` (ADR 0231). tmux moves a session's last attach time when a
/// client attaches or switches to it, not while one stays, so this is
/// what says a turn that ended while the client watched the session was
/// seen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientReturn {
    /// The socket path of the tmux server both sessions live on, as
    /// `$TMUX` names it.
    pub socket: String,
    /// The session the client came back from, by its tmux name.
    pub session: String,
    /// The last moment the client was known to be away from the TUI's
    /// session, in Unix epoch milliseconds.
    pub at_ms: i64,
}

/// The finished turns this TUI found seen: for each agent, when the
/// latest of them ended; and the sessions its client came back from, for
/// the turns it watched there. In memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SeenTurns {
    turns: BTreeMap<Agent, i64>,
    /// The latest return from each session, by server socket and name.
    returns: BTreeMap<(String, String), i64>,
    /// Whether a session's mark moved since the state file last took it
    /// ([`Self::take_changed`]).
    changed: bool,
    /// Whether the marks live in this TUI's memory alone
    /// ([`Self::in_memory_only`]).
    memory_only: bool,
}

/// No turn seen, for readers with no memory of their own.
#[cfg(test)]
static NOTHING_SEEN: SeenTurns = SeenTurns {
    turns: BTreeMap::new(),
    returns: BTreeMap::new(),
    changed: false,
    memory_only: false,
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
        let session = matches!(agent, Agent::Session(_));
        let seen = self.turns.entry(agent).or_insert(i64::MIN);
        if *seen >= ended_ms {
            return false;
        }
        *seen = ended_ms;
        self.changed |= session && !self.memory_only;
        true
    }

    /// Keep every mark in this TUI's memory: none is taken from the state
    /// file or offered to it. For a TUI that shows a world of its own
    /// (a fixture or a scenario), whose sessions the operator's state
    /// file knows nothing of.
    pub(crate) fn in_memory_only(&mut self) {
        self.memory_only = true;
    }

    /// The marks of the agent sessions, for the state file: each
    /// session's [`mark_key`](crate::tui_state::mark_key) and when its
    /// latest read stop began. A pane's agent is left out.
    pub(crate) fn session_marks(&self) -> BTreeMap<String, i64> {
        if self.memory_only {
            return BTreeMap::new();
        }
        self.turns
            .iter()
            .filter_map(|(agent, at_ms)| match agent {
                Agent::Session(node) => Some((crate::tui_state::mark_key(node)?, *at_ms)),
                Agent::Pane(_) => None,
            })
            .collect()
    }

    /// Take the marks a state file holds: a key that is no session's
    /// node id is skipped, and nothing taken here is news to write back.
    pub(crate) fn load_marks(&mut self, marks: &BTreeMap<String, i64>) {
        if self.memory_only {
            return;
        }
        for (key, at_ms) in marks {
            let Some(node @ NodeId::AgentSession(_)) = NodeId::parse(key) else {
                continue;
            };
            let seen = self.turns.entry(Agent::Session(node)).or_insert(i64::MIN);
            *seen = (*seen).max(*at_ms);
        }
    }

    /// Whether a session's mark moved since the last call, which is when
    /// the state file is due a write.
    pub(crate) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Remember that the client came back from a session. Says whether
    /// that is news: an older return from the same session is not.
    pub(crate) fn note_return(&mut self, back: ClientReturn) -> bool {
        let at = self
            .returns
            .entry((back.socket, back.session))
            .or_insert(i64::MIN);
        if *at >= back.at_ms {
            return false;
        }
        *at = back.at_ms;
        true
    }

    /// Whether the client came back from `mux`'s session after the turn
    /// that ended at `ended_ms`: the return is on `mux`'s server, by the
    /// rule the live-state binding uses, and names its session.
    fn returned_from(&self, snapshot: &GraphSnapshot, mux: &MuxSessionNode, ended_ms: i64) -> bool {
        self.returns.iter().any(|((socket, session), at_ms)| {
            ended_ms <= *at_ms
                && session == mux.session_name()
                && crate::discovery::live_state::on_server(snapshot, mux, socket)
        })
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
        Some(mux) => {
            !attached_since(mux, ended_ms, context.loaded_ms)
                && !context.memory.returned_from(snapshot, mux, ended_ms)
        }
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
