//! The rows a tmux hand-off froze, mux by mux (ADRs 0108, 0132, 0201).
//!
//! Returning from an attach freezes the mux and the agent sessions linked
//! to it, since the screen still shows them as they were before. Each mux
//! is released on its own: by the existence check, when the mux's server
//! still answers for the same incarnation, or by the first refresh the
//! runtime started after that mux's return. A later hand-off never holds
//! an earlier one's rows. The hand-off itself lasts until the newest of
//! those refreshes lands, so an entity to follow is looked up in current
//! data.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::model::{AgentSessionId, GraphNode, GraphSnapshot, MuxSessionId, NodeId};

/// Mux sessions the operator just returned from, plus the agent
/// sessions linked to them, whose rows still show the pre-hand-off
/// snapshot (ADR 0108). The renderer dims these rows and puts a
/// spinner in their attach-glyph cell until each mux is released.
#[derive(Debug, Clone)]
pub struct PendingHandoff {
    /// Each frozen mux and the agent sessions the snapshot linked to it
    /// when it froze.
    muxes: BTreeMap<MuxSessionId, BTreeSet<AgentSessionId>>,
    /// Muxes a launch froze whose attach has not returned yet.
    launching: BTreeSet<MuxSessionId>,
    pub started_at: Instant,
    /// Set by the reducer, taken by the runtime, which then spawns
    /// the background refresh that settles the hand-off.
    refresh_requested: bool,
    /// What that refresh settles, gathered since the runtime last took a
    /// request.
    request: HandoffRequest,
}

/// What the runtime's refresh after a hand-off settles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandoffRequest {
    /// The muxes returned from, in order.
    pub returned: Vec<ReturnedMux>,
    /// The TUI's pane came back into view: whatever the operator did in
    /// another session is not on screen yet.
    pub regained: bool,
}

/// A mux an attach returned from, and how its refresh can tell whether it
/// outlived the hand-off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedMux {
    pub mux: MuxSessionId,
    pub check: ReturnCheck,
}

/// How the refresh after a return settles one mux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnCheck {
    /// The screen showed the mux running as this incarnation and only an
    /// attach happened since: ask its server whether it still is.
    Incarnation(MuxIncarnationTarget),
    /// The screen showed it running, but it can't be asked: it has no
    /// recorded incarnation, or a launch changed what runs in it. Its
    /// refresh probes again while the listing still shows it.
    Listed,
    /// The screen didn't show it running (the hand-off created it), so
    /// there is nothing to wait out.
    New,
}

/// The incarnation a mux had before a hand-off, and where to ask about
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxIncarnationTarget {
    pub backend: String,
    /// The non-default server the mux lives on (`tmux -L`); `None` for
    /// the default one.
    pub socket: Option<String>,
    /// The backend's own id for the session (tmux `#{session_id}`).
    pub session_id: String,
    /// When the session was created (tmux `#{session_created}`).
    pub created_epoch: i64,
}

impl PendingHandoff {
    pub(super) fn new() -> Self {
        Self {
            muxes: BTreeMap::new(),
            launching: BTreeSet::new(),
            started_at: Instant::now(),
            refresh_requested: false,
            request: HandoffRequest::default(),
        }
    }

    /// The frozen muxes.
    pub fn muxes(&self) -> BTreeSet<MuxSessionId> {
        self.muxes.keys().cloned().collect()
    }

    /// The agent sessions frozen with their muxes.
    pub fn sessions(&self) -> BTreeSet<AgentSessionId> {
        self.muxes.values().flatten().cloned().collect()
    }

    /// Whether `mux`'s rows are frozen.
    pub fn holds_mux(&self, mux: &MuxSessionId) -> bool {
        self.muxes.contains_key(mux)
    }

    /// Whether `session`'s rows are frozen with a mux it is linked to.
    pub fn holds_session(&self, session: &AgentSessionId) -> bool {
        self.muxes
            .values()
            .any(|sessions| sessions.contains(session))
    }

    fn freeze(&mut self, mux: MuxSessionId, sessions: Vec<AgentSessionId>) {
        self.muxes.entry(mux).or_default().extend(sessions);
    }

    /// An attach to `mux` returned: freeze it and ask for the refresh,
    /// which settles it the way `check` says.
    pub(super) fn returned(
        &mut self,
        mux: MuxSessionId,
        sessions: Vec<AgentSessionId>,
        check: ReturnCheck,
    ) {
        self.launching.remove(&mux);
        self.freeze(mux.clone(), sessions);
        self.request.returned.push(ReturnedMux { mux, check });
        self.refresh_requested = true;
    }

    /// The TUI's pane came back into view: ask for the refresh without
    /// freezing any row.
    pub(super) fn regained(&mut self) {
        self.request.regained = true;
        self.refresh_requested = true;
    }

    /// A launch is starting `mux`: freeze it; the attach that follows
    /// asks for the refresh.
    pub(super) fn launch_started(&mut self, mux: MuxSessionId, sessions: Vec<AgentSessionId>) {
        self.launching.insert(mux.clone());
        self.freeze(mux, sessions);
    }

    /// Whether the launch of `mux` is still to attach.
    pub(super) fn is_launching(&self, mux: &MuxSessionId) -> bool {
        self.launching.contains(mux)
    }

    /// The launch of `mux` ended without an attach. Returns whether
    /// nothing is left for the hand-off to wait on.
    pub(super) fn launch_aborted(&mut self, mux: &MuxSessionId) -> bool {
        self.launching.remove(mux);
        self.muxes.remove(mux);
        self.muxes.is_empty() && !self.refresh_requested
    }

    /// `mux` shows current values again: its rows and its sessions'
    /// rows act normally. The hand-off itself waits for its refresh.
    pub(super) fn release(&mut self, mux: &MuxSessionId) {
        self.muxes.remove(mux);
    }

    /// Take the refresh request, if a return asked for one.
    pub(super) fn take_request(&mut self) -> Option<HandoffRequest> {
        std::mem::take(&mut self.refresh_requested).then(|| std::mem::take(&mut self.request))
    }

    pub(super) fn refresh_requested(&self) -> bool {
        self.refresh_requested
    }
}

/// How the refresh after a return from `mux` settles it, judged from
/// `snapshot`, the one the screen showed before the attach. `launched`
/// says a launch ran in the mux before the attach.
pub(super) fn return_check(
    snapshot: Option<&GraphSnapshot>,
    mux: &MuxSessionId,
    launched: bool,
) -> ReturnCheck {
    let id = NodeId::MuxSession(mux.clone());
    let node = snapshot
        .filter(|snapshot| snapshot.is_live(&id))
        .and_then(|snapshot| snapshot.find_node(&id));
    let Some(GraphNode::MuxSession(node)) = node else {
        return ReturnCheck::New;
    };
    if launched {
        return ReturnCheck::Listed;
    }
    match (&node.backend_session_id, node.created_epoch) {
        (Some(session_id), Some(created_epoch)) if !session_id.is_empty() => {
            ReturnCheck::Incarnation(MuxIncarnationTarget {
                backend: node.backend.clone(),
                socket: node.socket.clone(),
                session_id: session_id.clone(),
                created_epoch,
            })
        }
        _ => ReturnCheck::Listed,
    }
}

#[cfg(test)]
#[path = "handoff_tests.rs"]
mod tests;
