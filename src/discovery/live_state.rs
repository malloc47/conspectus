//! Live-state binding (ADR 0205): the entries of the live-state store
//! (ADR 0204) put on the graph nodes they describe, as a snapshot's
//! `node_live` table.
//!
//! [`bind_live`] is pure and has two callers: the `live_state` pass
//! ([`apply_live_state`]), which every discovery runs after the harness
//! providers and the hook sidecar, and the TUI, which binds the store
//! again against the snapshot it holds whenever the store changes.
//!
//! An entry binds by the first of these rules that fits:
//!
//! 1. to the agent session whose harness and native session id are the
//!    entry's, exactly;
//! 2. otherwise, when the entry names a tmux server socket and session,
//!    to the tmux mux of that name on that server: an agent seen in one
//!    of the mux's panes that no session matches yet, such as a new
//!    session with no transcript. Once its session appears, the next
//!    bind moves the state there;
//! 3. otherwise to nothing. The entry stays in the store and shows
//!    nowhere.
//!
//! Binding never creates a node, and a node that exists only in history
//! (gone or unknown, ADR 0140) takes no live state. The graph has no pane
//! node, so a node holds one entry: when several bind to it (agents in
//! two panes of one mux that no session matches, or a session its
//! process left beside the one it moved to), it keeps the one the store
//! wrote last, by the highest entry `rev`, then the newest evidence, then
//! one that has not exited, then the lowest entry key.
//!
//! Each binding records whether the entry's process ran when it was
//! bound, from the recorded pid and start time checked through a
//! [`PidProbe`], so readers apply the time rules alone. The pass reads
//! one small file and checks one pid per bound entry; it starts no
//! process and walks no process tree, and it runs after the process-tree
//! fingerprint is taken, so a state change never reopens that walk (ADR
//! 0091).

use std::collections::BTreeMap;
use std::path::Path;

use crate::live_store::{LiveEntry, LiveFile, LiveStore, PidProbe};
use crate::model::live::{LiveBinding, LiveState};
use crate::model::{GraphNode, GraphSnapshot, NodeId, effective_socket};

/// The store in `root` bound to `snapshot`, replacing its `node_live`
/// table. A missing, unreadable, corrupt, or newer-schema store binds
/// nothing, so the table ends empty and discovery carries on.
pub fn apply_live_state(snapshot: &mut GraphSnapshot, root: &Path, probe: &dyn PidProbe) {
    let file = LiveStore::at(root).read();
    snapshot.node_live = bind_live(snapshot, &file, probe);
}

/// The `node_live` table for `snapshot` from the store's entries, by the
/// rules in the module documentation, with each entry's process checked
/// through `probe`.
pub fn bind_live(
    snapshot: &GraphSnapshot,
    file: &LiveFile,
    probe: &dyn PidProbe,
) -> BTreeMap<NodeId, LiveBinding> {
    let targets = Targets::of(snapshot);
    let mut winners: BTreeMap<NodeId, &LiveEntry> = BTreeMap::new();
    // Entries come in key order, and a later one replaces the winner
    // only when it ranks strictly higher, so equal ranks keep the lowest
    // key.
    for entry in file.entries.values() {
        let Some(node) = targets.node_for(entry) else {
            continue;
        };
        match winners.get(node) {
            Some(winner) if rank(winner) >= rank(entry) => {}
            _ => {
                winners.insert(node.clone(), entry);
            }
        }
    }
    winners
        .into_iter()
        .map(|(node, entry)| {
            let liveness = probe.liveness(&entry.observation);
            (node, LiveBinding::new(entry.observation, liveness))
        })
        .collect()
}

/// Which of two entries bound to one node it keeps: the one written last
/// (`rev`), then the newer evidence, then one still running. A process
/// that moves to a new session ends its old entry in the same write, so
/// the two share a `rev` and an observed time.
fn rank(entry: &LiveEntry) -> (u64, i64, bool) {
    (
        entry.rev,
        entry.observation.observed_ms,
        entry.observation.state != LiveState::Exited,
    )
}

/// The nodes an entry can bind to, indexed once per bind.
struct Targets {
    /// Live agent sessions by harness and native session id. Two nodes
    /// for one session (seen under two state scopes) keep the lower id.
    sessions: BTreeMap<(String, String), NodeId>,
    /// Live tmux muxes by server and session name; the default server is
    /// `None`, as a mux node records it.
    muxes: BTreeMap<(Option<String>, String), NodeId>,
}

impl Targets {
    fn of(snapshot: &GraphSnapshot) -> Self {
        let mut sessions: BTreeMap<(String, String), NodeId> = BTreeMap::new();
        let mut muxes = BTreeMap::new();
        for node in snapshot.live_nodes() {
            match node {
                GraphNode::AgentSession(session) => {
                    let id = node.id();
                    sessions
                        .entry((session.harness_key.clone(), session.id.session_key.clone()))
                        .and_modify(|kept| {
                            if id < *kept {
                                *kept = id.clone();
                            }
                        })
                        .or_insert(id);
                }
                GraphNode::MuxSession(mux)
                    if mux.backend == crate::discovery::tmux::TMUX_BACKEND =>
                {
                    let server = effective_socket(mux.socket.as_deref()).map(str::to_string);
                    muxes.insert((server, mux.session_name().to_string()), node.id());
                }
                _ => {}
            }
        }
        Self { sessions, muxes }
    }

    fn node_for(&self, entry: &LiveEntry) -> Option<&NodeId> {
        let session = (entry.harness.clone(), entry.session_id.clone());
        if let Some(node) = self.sessions.get(&session) {
            return Some(node);
        }
        let tmux = entry.tmux.as_ref()?;
        let server = server_of(tmux.socket.as_deref()?)?.map(str::to_string);
        self.muxes.get(&(server, tmux.session.clone()?))
    }
}

/// The tmux server a socket path belongs to, by the name tmux gives it
/// (`-L <name>` puts the socket at `<dir>/<name>`): `Some(None)` for the
/// default server, `None` for a path that names no socket.
fn server_of(socket_path: &str) -> Option<Option<&str>> {
    let name = Path::new(socket_path).file_name()?.to_str()?;
    Some(effective_socket(Some(name)))
}

#[cfg(test)]
#[path = "live_state_tests.rs"]
mod tests;
