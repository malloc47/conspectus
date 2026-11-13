//! Live-state binding (ADR 0205): the entries of the live-state store
//! (ADR 0204) put on the graph nodes they describe, as a snapshot's
//! `node_live` and `pane_live` tables.
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
//!    to the pane it names on the tmux mux of that name on that server:
//!    an agent seen in one of the mux's panes that no session matches
//!    yet, such as a new session with no transcript. Once its session
//!    appears, the next bind moves the state there. The server is the
//!    one whose socket path the mux's probe read, exactly; a probe that
//!    read none falls back to the path's last component, the name `tmux
//!    -L` gives the server;
//! 3. otherwise to nothing. The entry stays in the store and shows
//!    nowhere.
//!
//! Rule 1 fills the snapshot's `node_live` table, one entry per session;
//! rule 2 fills `pane_live`, one entry per pane of a mux (the graph has
//! no pane node), so two agents in two panes of one mux each count on it.
//! Binding never creates a node, and a node that exists only in history
//! (gone or unknown, ADR 0140) takes no live state. When several entries
//! bind to one session or one pane (a session its process left beside
//! the one it moved to, or an agent that replaced another in its pane
//! before either had a session), it keeps the one the store wrote last,
//! by the highest entry `rev`, then the newest evidence, then one that
//! has not exited, then the lowest entry key. Entries that name no pane
//! share one slot on their mux.
//!
//! Then the marks the mux probes read from the titles of agents' panes
//! (ADR 0217) fill in where the store's entries say nothing, or nothing
//! fresh, about a pane's agent, and clear a wait Codex's hooks cannot
//! report the end of (the `title` module). Last, the snapshot's latest
//! poll of a harness's own list of its sessions (ADR 0218) repairs a
//! `working` or `waiting` entry of that harness whose hooks have been
//! silent since before the poll, and shows a running agent of that
//! harness that no entry reports on its session (ADR 0222; the `poll`
//! module); the pass runs the polls it is given before it binds
//! ([`apply_live_state`]).
//!
//! Each binding records whether the entry's process ran when it was
//! bound, from the recorded pid and start time checked through a
//! [`PidProbe`], so readers apply the time rules alone. The pass reads
//! one small file and checks one pid per bound entry; the only process it
//! starts is a poll its caller gave it, at most one per poller every
//! thirty seconds, and binding itself starts none. It walks no process
//! tree, and it runs after the process-tree fingerprint is taken, so a
//! state change never reopens that walk (ADR 0091).

use std::collections::BTreeMap;
use std::path::Path;

use crate::discovery::live_poller::LivePoller;
use crate::live_store::{LiveEntry, LiveFile, LiveStore, PidProbe};
use crate::model::live::{LiveBinding, LiveState};
use crate::model::{
    GraphNode, GraphSnapshot, LivePane, LiveTables, MuxSessionId, MuxSessionNode, NodeId,
    effective_socket, probe_key,
};

/// The store in `root` bound to `snapshot`, replacing its `node_live`,
/// `pane_live`, and `live_polls` tables. A missing, unreadable, corrupt,
/// or newer-schema store binds nothing, so the tables end empty and
/// discovery carries on.
///
/// Each of `pollers` is first handed its harness's bound hook entries
/// that a poll might repair and how many of its running agents no entry
/// reports, runs its poll when they call for one (ADRs 0218, 0222), and
/// puts its latest poll in `live_polls` for the binding to fold in. With
/// no pollers, as in a one-shot command, `live_polls` ends empty.
pub fn apply_live_state(
    snapshot: &mut GraphSnapshot,
    root: &Path,
    probe: &dyn PidProbe,
    pollers: &[std::sync::Arc<LivePoller>],
) {
    let file = LiveStore::at(root).read();
    snapshot.live_polls = BTreeMap::new();
    for poller in pollers {
        let candidates = poll::candidates(snapshot, &file, probe, poller.harness());
        let unreported = poll::unreported(snapshot, &file, probe, poller.harness());
        if let Some(read) = poller.poll(&candidates, unreported.len(), probe) {
            snapshot
                .live_polls
                .insert(poller.harness().to_string(), read);
        }
    }
    let bound = bind_live(snapshot, &file, probe);
    snapshot.node_live = bound.nodes;
    snapshot.pane_live = bound.panes;
}

/// The store's entries bound to one snapshot's nodes: the tables a
/// snapshot carries as `node_live` and `pane_live`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoundLive {
    /// Entries bound to the agent session they describe.
    pub nodes: BTreeMap<NodeId, LiveBinding>,
    /// Entries of agents seen in a mux's panes that no session matched.
    pub panes: BTreeMap<LivePane, LiveBinding>,
}

impl BoundLive {
    /// Both tables, read together.
    pub fn tables(&self) -> LiveTables<'_> {
        LiveTables {
            nodes: &self.nodes,
            panes: &self.panes,
        }
    }

    /// Whether no entry bound anywhere.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.panes.is_empty()
    }

    /// Every binding of both tables, to update in place.
    pub fn bindings_mut(&mut self) -> impl Iterator<Item = &mut LiveBinding> {
        self.nodes.values_mut().chain(self.panes.values_mut())
    }
}

/// The live tables for `snapshot` from the store's entries, by the rules
/// in the module documentation, with each entry's process checked
/// through `probe`.
pub fn bind_live(snapshot: &GraphSnapshot, file: &LiveFile, probe: &dyn PidProbe) -> BoundLive {
    let Winners { sessions, panes } = Winners::of(snapshot, file);
    let bind =
        |entry: &LiveEntry| LiveBinding::new(entry.observation, probe.liveness(&entry.observation));
    let mut bound = BoundLive {
        nodes: sessions
            .iter()
            .map(|(node, entry)| (node.clone(), bind(entry)))
            .collect(),
        panes: panes
            .iter()
            .map(|(pane, entry)| (pane.clone(), bind(entry)))
            .collect(),
    };
    title::fold_titles(snapshot, &sessions, &panes, &mut bound);
    poll::fold_polls(snapshot, &sessions, &panes, &mut bound);
    poll::fold_poll_only(snapshot, file, probe, &mut bound);
    bound
}

/// The store entry that binds to each session and to each pane: of the
/// entries that bind to one slot, the one [`rank`] puts first.
struct Winners<'a> {
    sessions: BTreeMap<NodeId, &'a LiveEntry>,
    panes: BTreeMap<LivePane, &'a LiveEntry>,
}

impl<'a> Winners<'a> {
    fn of(snapshot: &GraphSnapshot, file: &'a LiveFile) -> Self {
        let targets = Targets::of(snapshot);
        let mut sessions: BTreeMap<NodeId, &LiveEntry> = BTreeMap::new();
        let mut panes: BTreeMap<LivePane, &LiveEntry> = BTreeMap::new();
        // Entries come in key order, and a later one replaces the winner
        // only when it ranks strictly higher, so equal ranks keep the
        // lowest key.
        for entry in file.entries.values() {
            match targets.target_for(entry) {
                Some(Target::Session(node)) => keep_last(&mut sessions, node.clone(), entry),
                Some(Target::Pane(pane)) => keep_last(&mut panes, pane, entry),
                None => {}
            }
        }
        Self { sessions, panes }
    }

    /// Every winning entry, sessions' first.
    fn entries(&self) -> impl Iterator<Item = &'a LiveEntry> + '_ {
        self.sessions.values().chain(self.panes.values()).copied()
    }
}

/// Put `entry` in `key`'s slot unless the entry already there ranks at
/// least as high.
fn keep_last<'a, K: Ord>(winners: &mut BTreeMap<K, &'a LiveEntry>, key: K, entry: &'a LiveEntry) {
    match winners.get(&key) {
        Some(winner) if rank(winner) >= rank(entry) => {}
        _ => {
            winners.insert(key, entry);
        }
    }
}

/// Which of two entries bound to one slot it keeps: the one written last
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

/// Where one entry binds.
enum Target<'a> {
    Session(&'a NodeId),
    Pane(LivePane),
}

/// The nodes an entry can bind to, indexed once per bind.
struct Targets<'a> {
    /// Live agent sessions by harness and native session id. Two nodes
    /// for one session (seen under two state scopes) keep the lower id.
    sessions: BTreeMap<(String, String), NodeId>,
    /// Live tmux muxes by server and session name; the default server is
    /// `None`, as a mux node records it.
    muxes: BTreeMap<(Option<String>, String), TmuxTarget<'a>>,
}

/// A live tmux mux an entry can bind to, with the socket path its
/// server's probe read, when it read one.
struct TmuxTarget<'a> {
    mux: MuxSessionId,
    probed: Option<&'a str>,
}

impl<'a> Targets<'a> {
    fn of(snapshot: &'a GraphSnapshot) -> Self {
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
                    muxes.insert(
                        (server, mux.session_name().to_string()),
                        TmuxTarget {
                            mux: mux.id.clone(),
                            probed: probed_socket(snapshot, mux),
                        },
                    );
                }
                _ => {}
            }
        }
        Self { sessions, muxes }
    }

    fn target_for(&self, entry: &LiveEntry) -> Option<Target<'_>> {
        let session = (entry.harness.clone(), entry.session_id.clone());
        if let Some(node) = self.sessions.get(&session) {
            return Some(Target::Session(node));
        }
        let tmux = entry.tmux.as_ref()?;
        let socket = tmux.socket.as_deref()?;
        let server = server_of(socket)?.map(str::to_string);
        let target = self.muxes.get(&(server, tmux.session.clone()?))?;
        if !names_server(socket, target.probed) {
            return None;
        }
        let pane = tmux.pane.clone().filter(|pane| !pane.is_empty());
        Some(Target::Pane(LivePane::new(target.mux.clone(), pane)))
    }
}

/// The tmux server a socket path belongs to, by the name tmux gives it
/// (`-L <name>` puts the socket at `<dir>/<name>`): `Some(None)` for the
/// default server, `None` for a path that names no socket.
fn server_of(socket_path: &str) -> Option<Option<&str>> {
    let name = Path::new(socket_path).file_name()?.to_str()?;
    Some(effective_socket(Some(name)))
}

/// The full path of the socket of `mux`'s server, as the probe of that
/// server read it, when it read one.
fn probed_socket<'a>(snapshot: &'a GraphSnapshot, mux: &MuxSessionNode) -> Option<&'a str> {
    snapshot
        .mux_probes
        .get(&probe_key(&mux.backend, mux.socket.as_deref()))?
        .socket_path
        .as_deref()
}

/// Whether the socket path `socket` an entry recorded is the socket of
/// a server whose name its last component already matched, given the
/// path that server's probe read. Two servers can share a name (a `tmux
/// -S /some/dir/work` socket and a `-L work` server, or the default
/// server under another `TMUX_TMPDIR` or uid), so a probed path must be
/// the entry's exactly; with none, the name is all there is to go by.
fn names_server(socket: &str, probed: Option<&str>) -> bool {
    probed.is_none_or(|probed| probed == socket)
}

#[path = "live_title.rs"]
mod title;

#[path = "live_poll.rs"]
mod poll;

pub use poll::{RunningAgent, running_agents};

#[cfg(test)]
#[path = "live_state_tests.rs"]
mod tests;
