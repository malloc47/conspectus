use std::collections::BTreeMap;

use super::*;
use crate::live_store::{LiveTmux, PidStatus};
use crate::model::live::{
    EffectiveLive, LiveConfidence, LiveCounts, LiveDetail, LiveObservation, LiveSource, Liveness,
};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, LinkEndpoint, MuxSessionId, MuxSessionNode,
    NodePresence, Provenance, RelationKind,
};

const NOW_MS: i64 = 1_790_769_600_000;
const DEFAULT_SOCKET: &str = "/tmp/tmux-1000/default";
const SCRATCH_SOCKET: &str = "/tmp/tmux-1000/scratch";

/// A process table that holds exactly the pids given, with their start
/// times.
struct FakePids(BTreeMap<i64, u64>);

impl FakePids {
    fn none() -> Self {
        Self(BTreeMap::new())
    }

    fn running(pids: &[(i64, u64)]) -> Self {
        Self(pids.iter().copied().collect())
    }
}

impl PidProbe for FakePids {
    fn status(&self, pid: i64) -> PidStatus {
        match self.0.get(&pid) {
            Some(start) => PidStatus::Running {
                start: Some(*start),
            },
            None => PidStatus::Gone,
        }
    }
}

fn session_id(harness: &str, key: &str) -> AgentSessionId {
    AgentSessionId::new(harness, format!("/home/me/.{harness}"), key)
}

fn session_node(harness: &str, key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(session_id(harness, key), harness))
}

fn session(harness: &str, key: &str) -> NodeId {
    NodeId::AgentSession(session_id(harness, key))
}

/// A tmux mux named `name` on the default server, or on the `-L`
/// server `socket`.
fn mux_node(name: &str, socket: Option<&str>) -> GraphNode {
    let native = match socket {
        Some(socket) => format!("{socket}:{name}"),
        None => name.to_string(),
    };
    let mut node = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{native}")), "tmux", native);
    node.socket = socket.map(str::to_string);
    GraphNode::MuxSession(node)
}

fn mux_id(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

fn mux(name: &str) -> NodeId {
    NodeId::MuxSession(mux_id(name))
}

fn snapshot(nodes: Vec<GraphNode>) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        ..GraphSnapshot::empty()
    }
}

/// `harness:key` in `state`, written at revision `rev`.
fn entry(harness: &str, key: &str, state: LiveState, rev: u64) -> LiveEntry {
    let mut entry = LiveEntry::new(harness, key, NOW_MS);
    entry.observation = LiveObservation::new(state, LiveSource::Hook, NOW_MS);
    entry.rev = rev;
    entry
}

/// `entry` seen in pane `pane` of tmux session `name` on the server at
/// `socket`.
fn in_pane(mut entry: LiveEntry, socket: &str, name: &str, pane: &str) -> LiveEntry {
    entry.tmux = Some(LiveTmux {
        socket: Some(socket.to_string()),
        session: Some(name.to_string()),
        pane: Some(pane.to_string()),
    });
    entry
}

fn with_pid(mut entry: LiveEntry, pid: i64, start: u64) -> LiveEntry {
    entry.observation = entry.observation.with_pid(pid, Some(start));
    entry
}

fn file(entries: impl IntoIterator<Item = LiveEntry>) -> LiveFile {
    let mut file = LiveFile::default();
    for entry in entries {
        file.rev = file.rev.max(entry.rev);
        file.entries.insert(entry.key(), entry);
    }
    file
}

fn states(table: &BTreeMap<NodeId, LiveBinding>) -> Vec<(NodeId, LiveState)> {
    table
        .iter()
        .map(|(id, binding)| (id.clone(), binding.observation.state))
        .collect()
}

fn linked(snapshot: &mut GraphSnapshot, from: NodeId, to: &MuxSessionId) {
    snapshot.candidate_links.push(GraphLink::new(
        format!("link-{from}"),
        from,
        LinkEndpoint::Node {
            id: NodeId::MuxSession(to.clone()),
        },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    ));
}

// ---- the binding rules ----

#[test]
fn an_entry_binds_to_the_session_with_its_harness_and_native_id() {
    let graph = snapshot(vec![
        session_node("claude-code", "s1"),
        session_node("codex", "s1"),
        session_node("claude-code", "s2"),
    ]);
    let store = file([
        entry("claude-code", "s1", LiveState::Working, 1),
        entry("codex", "s2", LiveState::Waiting, 2),
    ]);

    let table = bind_live(&graph, &store, &FakePids::none());

    assert_eq!(
        states(&table),
        vec![(session("claude-code", "s1"), LiveState::Working)],
        "the match is exact on both harness and session id"
    );
}

#[test]
fn a_session_match_wins_over_the_pane_its_entry_names() {
    let mut graph = snapshot(vec![
        session_node("claude-code", "s1"),
        mux_node("work", None),
    ]);
    linked(&mut graph, session("claude-code", "s1"), &mux_id("work"));
    let store = file([in_pane(
        entry("claude-code", "s1", LiveState::Ready, 1),
        DEFAULT_SOCKET,
        "work",
        "%3",
    )]);

    graph.node_live = bind_live(&graph, &store, &FakePids::none());

    assert_eq!(
        states(&graph.node_live),
        vec![(session("claude-code", "s1"), LiveState::Ready)]
    );
    assert_eq!(
        graph.live_counts_of_mux(&mux_id("work"), NOW_MS),
        LiveCounts {
            ready: 1,
            ..LiveCounts::default()
        },
        "the mux counts it through the session it hosts"
    );
}

#[test]
fn a_pane_only_entry_binds_to_the_mux_on_its_server() {
    let graph = snapshot(vec![
        mux_node("work", None),
        mux_node("work", Some("scratch")),
        session_node("pi", "unrelated"),
    ]);
    let store = file([
        in_pane(
            entry("pi", "new-default", LiveState::Starting, 1),
            DEFAULT_SOCKET,
            "work",
            "%1",
        ),
        in_pane(
            entry("pi", "new-scratch", LiveState::Working, 2),
            SCRATCH_SOCKET,
            "work",
            "%2",
        ),
    ]);

    let table = bind_live(&graph, &store, &FakePids::none());

    assert_eq!(
        states(&table),
        vec![
            (mux("scratch:work"), LiveState::Working),
            (mux("work"), LiveState::Starting),
        ]
    );
}

#[test]
fn an_entry_that_names_no_known_node_stays_unbound() {
    let graph = snapshot(vec![mux_node("work", None), session_node("codex", "known")]);
    let mut no_socket = entry("codex", "a", LiveState::Working, 1);
    no_socket.tmux = Some(LiveTmux {
        socket: None,
        session: Some("work".to_string()),
        pane: Some("%1".to_string()),
    });
    let mut no_session = entry("codex", "b", LiveState::Working, 2);
    no_session.tmux = Some(LiveTmux {
        socket: Some(DEFAULT_SOCKET.to_string()),
        session: None,
        pane: Some("%1".to_string()),
    });
    let store = file([
        entry("codex", "outside-tmux", LiveState::Working, 3),
        no_socket,
        no_session,
        in_pane(
            entry("codex", "other-name", LiveState::Working, 4),
            DEFAULT_SOCKET,
            "play",
            "%1",
        ),
        in_pane(
            entry("codex", "other-server", LiveState::Working, 5),
            SCRATCH_SOCKET,
            "work",
            "%1",
        ),
        in_pane(
            entry("codex", "no-socket-name", LiveState::Working, 6),
            "/",
            "work",
            "%1",
        ),
    ]);

    let table = bind_live(&graph, &store, &FakePids::none());

    assert!(table.is_empty(), "{table:?}");
}

#[test]
fn only_tmux_muxes_take_pane_only_entries() {
    let mut zellij = MuxSessionNode::new(MuxSessionId::new("zellij:work"), "zellij", "work");
    zellij.socket = None;
    let graph = snapshot(vec![GraphNode::MuxSession(zellij)]);
    let store = file([in_pane(
        entry("pi", "new", LiveState::Working, 1),
        DEFAULT_SOCKET,
        "work",
        "%1",
    )]);

    assert!(bind_live(&graph, &store, &FakePids::none()).is_empty());
}

#[test]
fn nodes_that_exist_only_in_history_take_no_live_state() {
    let mut graph = snapshot(vec![mux_node("gone", None), session_node("codex", "gone")]);
    graph
        .node_presence
        .insert(mux("gone"), NodePresence::Gone { last_seen: 1 });
    graph.node_presence.insert(
        session("codex", "gone"),
        NodePresence::Unknown { last_seen: 1 },
    );
    let store = file([
        in_pane(
            entry("pi", "new", LiveState::Working, 1),
            DEFAULT_SOCKET,
            "gone",
            "%1",
        ),
        entry("codex", "gone", LiveState::Ready, 2),
    ]);

    assert!(bind_live(&graph, &store, &FakePids::none()).is_empty());
}

#[test]
fn binding_never_creates_a_node() {
    let mut graph = snapshot(vec![mux_node("work", None)]);
    let before = graph.nodes.clone();
    let root = tempfile::tempdir().expect("store dir");
    LiveStore::at(root.path())
        .apply_event(&crate::live_store::LiveEvent::new(
            "claude-code",
            "nowhere",
            NOW_MS,
            crate::live_store::LiveEventKind::TurnStarted,
        ))
        .expect("write the store");

    apply_live_state(&mut graph, root.path(), &FakePids::none());

    assert_eq!(graph.nodes, before);
    assert!(graph.node_live.is_empty());
}

// ---- a session that appears later ----

#[test]
fn a_session_that_appears_later_takes_the_state_from_its_mux() {
    let pane_entry = in_pane(
        with_pid(entry("pi", "fresh", LiveState::Working, 4), 4242, 77),
        DEFAULT_SOCKET,
        "work",
        "%1",
    );
    let store = file([pane_entry]);
    let probe = FakePids::running(&[(4242, 77)]);

    // No transcript yet: the agent shows on its mux.
    let mut before = snapshot(vec![mux_node("work", None)]);
    before.node_live = bind_live(&before, &store, &probe);
    assert_eq!(
        states(&before.node_live),
        vec![(mux("work"), LiveState::Working)]
    );
    let working = LiveCounts {
        working: 1,
        ..LiveCounts::default()
    };
    assert_eq!(before.live_counts_of_mux(&mux_id("work"), NOW_MS), working);

    // Its session appears: the next bind moves the state to it.
    let mut after = snapshot(vec![mux_node("work", None), session_node("pi", "fresh")]);
    linked(&mut after, session("pi", "fresh"), &mux_id("work"));
    after.node_live = bind_live(&after, &store, &probe);
    assert_eq!(
        states(&after.node_live),
        vec![(session("pi", "fresh"), LiveState::Working)]
    );
    assert_eq!(after.live_state_of(&mux("work"), NOW_MS), None);
    assert_eq!(
        after.live_counts_of_mux(&mux_id("work"), NOW_MS),
        working,
        "the mux still counts the agent, once, through its session"
    );
}

// ---- split panes ----

#[test]
fn two_agents_in_two_panes_of_one_mux_each_keep_their_own_state() {
    let mut graph = snapshot(vec![
        mux_node("work", None),
        session_node("claude-code", "left"),
        session_node("codex", "right"),
    ]);
    linked(&mut graph, session("claude-code", "left"), &mux_id("work"));
    linked(&mut graph, session("codex", "right"), &mux_id("work"));
    let mut waiting = in_pane(
        entry("claude-code", "left", LiveState::Waiting, 7),
        DEFAULT_SOCKET,
        "work",
        "%1",
    );
    waiting.observation = waiting.observation.with_detail(LiveDetail::Permission);
    let store = file([
        waiting,
        in_pane(
            entry("codex", "right", LiveState::Working, 9),
            DEFAULT_SOCKET,
            "work",
            "%2",
        ),
    ]);

    graph.node_live = bind_live(&graph, &store, &FakePids::none());

    assert_eq!(
        graph
            .live_state_of(&session("claude-code", "left"), NOW_MS)
            .map(|live| (live.state, live.detail)),
        Some((LiveState::Waiting, Some(LiveDetail::Permission)))
    );
    assert_eq!(
        graph
            .live_state_of(&session("codex", "right"), NOW_MS)
            .map(|live| live.state),
        Some(LiveState::Working)
    );
    assert_eq!(
        graph.live_counts_of_mux(&mux_id("work"), NOW_MS),
        LiveCounts {
            waiting: 1,
            working: 1,
            ..LiveCounts::default()
        }
    );
}

// ---- liveness ----

#[test]
fn liveness_comes_from_the_recorded_pid_and_start_time() {
    let graph = snapshot(vec![
        session_node("codex", "alive"),
        session_node("codex", "dead"),
        session_node("codex", "reused"),
        session_node("codex", "unchecked"),
    ]);
    let store = file([
        with_pid(entry("codex", "alive", LiveState::Waiting, 1), 100, 5),
        with_pid(entry("codex", "dead", LiveState::Working, 2), 200, 5),
        with_pid(entry("codex", "reused", LiveState::Ready, 3), 300, 5),
        entry("codex", "unchecked", LiveState::Working, 4),
    ]);
    // 300 runs again, but as a process that started later.
    let probe = FakePids::running(&[(100, 5), (300, 6)]);

    let mut graph = graph;
    graph.node_live = bind_live(&graph, &store, &probe);

    let liveness: Vec<(NodeId, Liveness)> = graph
        .node_live
        .iter()
        .map(|(id, binding)| (id.clone(), binding.liveness))
        .collect();
    assert_eq!(
        liveness,
        vec![
            (session("codex", "alive"), Liveness::Alive),
            (session("codex", "dead"), Liveness::Dead),
            (session("codex", "reused"), Liveness::Dead),
            (session("codex", "unchecked"), Liveness::Unknown),
        ]
    );
    let killed = Some(EffectiveLive {
        state: LiveState::Exited,
        detail: Some(LiveDetail::Killed),
        confidence: LiveConfidence::Fresh,
    });
    assert_eq!(
        graph.live_state_of(&session("codex", "dead"), NOW_MS),
        killed
    );
    assert_eq!(
        graph.live_state_of(&session("codex", "reused"), NOW_MS),
        killed
    );
    assert_eq!(
        graph
            .live_state_of(&session("codex", "alive"), NOW_MS)
            .map(|live| live.state),
        Some(LiveState::Waiting)
    );
}

// ---- several entries for one node ----

#[test]
fn a_node_keeps_the_entry_written_last() {
    let graph = snapshot(vec![mux_node("work", None)]);
    let in_work =
        |key: &str, state, rev| in_pane(entry("pi", key, state, rev), DEFAULT_SOCKET, "work", "%1");

    let newer = file([
        in_work("a", LiveState::Working, 5),
        in_work("b", LiveState::Waiting, 3),
    ]);
    assert_eq!(
        states(&bind_live(&graph, &newer, &FakePids::none())),
        vec![(mux("work"), LiveState::Working)],
        "the highest revision wins"
    );

    // A process that moved to a new session ends the old entry in the
    // same write: same revision, same time. The one still running wins.
    let moved = file([
        in_work("a-old", LiveState::Exited, 8),
        in_work("b-new", LiveState::Starting, 8),
    ]);
    assert_eq!(
        states(&bind_live(&graph, &moved, &FakePids::none())),
        vec![(mux("work"), LiveState::Starting)]
    );

    let mut later = in_work("z-later", LiveState::Ready, 8);
    later.observation.observed_ms += 1;
    let by_time = file([in_work("a-earlier", LiveState::Working, 8), later]);
    assert_eq!(
        states(&bind_live(&graph, &by_time, &FakePids::none())),
        vec![(mux("work"), LiveState::Ready)],
        "then the newest evidence"
    );

    let tied = file([
        in_work("b", LiveState::Waiting, 8),
        in_work("a", LiveState::Working, 8),
    ]);
    assert_eq!(
        states(&bind_live(&graph, &tied, &FakePids::none())),
        vec![(mux("work"), LiveState::Working)],
        "and the lowest key when nothing else tells them apart"
    );
}

#[test]
fn two_session_nodes_for_one_native_session_bind_the_lower_id() {
    let mut other_scope = AgentSessionNode::new(
        AgentSessionId::new("codex", "/elsewhere/.codex", "s1"),
        "codex",
    );
    other_scope.cwd = None;
    let graph = snapshot(vec![
        session_node("codex", "s1"),
        GraphNode::AgentSession(other_scope.clone()),
    ]);
    let store = file([entry("codex", "s1", LiveState::Working, 1)]);

    let table = bind_live(&graph, &store, &FakePids::none());

    let expected = session("codex", "s1").min(NodeId::AgentSession(other_scope.id));
    assert_eq!(states(&table), vec![(expected, LiveState::Working)]);
}

// ---- the pass ----

#[test]
fn a_missing_or_corrupt_store_leaves_an_empty_table() {
    let mut graph = snapshot(vec![session_node("codex", "s1")]);
    graph.node_live.insert(
        session("codex", "s1"),
        LiveBinding::new(
            LiveObservation::new(LiveState::Working, LiveSource::Hook, NOW_MS),
            Liveness::Alive,
        ),
    );
    let root = tempfile::tempdir().expect("store dir");

    apply_live_state(&mut graph, &root.path().join("absent"), &FakePids::none());
    assert!(graph.node_live.is_empty(), "a missing store binds nothing");

    std::fs::write(
        root.path().join(crate::live_store::LIVE_STATE_FILE),
        "{ not json",
    )
    .expect("corrupt store");
    apply_live_state(&mut graph, root.path(), &FakePids::none());
    assert!(graph.node_live.is_empty(), "a corrupt store binds nothing");
}

#[test]
fn the_pass_replaces_the_table_from_the_store() {
    let mut graph = snapshot(vec![
        session_node("codex", "s1"),
        session_node("codex", "s2"),
    ]);
    graph.node_live.insert(
        session("codex", "s2"),
        LiveBinding::new(
            LiveObservation::new(LiveState::Waiting, LiveSource::Hook, NOW_MS),
            Liveness::Alive,
        ),
    );
    let root = tempfile::tempdir().expect("store dir");
    LiveStore::at(root.path())
        .apply_event(&crate::live_store::LiveEvent::new(
            "codex",
            "s1",
            NOW_MS,
            crate::live_store::LiveEventKind::TurnStarted,
        ))
        .expect("write the store");

    apply_live_state(&mut graph, root.path(), &FakePids::none());

    assert_eq!(
        states(&graph.node_live),
        vec![(session("codex", "s1"), LiveState::Working)],
        "an entry the store no longer backs is gone"
    );
}
