use super::*;
use crate::model::live::{LiveDetail, LiveObservation, LiveSource, Liveness};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, GraphNode, LinkEndpoint, MuxSessionNode,
    Provenance, RelationKind,
};

/// The read time of every test: 2026-09-30T12:00:00Z.
const NOW_MS: i64 = 1_790_769_600_000;
const MINUTE_MS: i64 = 60_000;

fn session_id(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

fn mux_id() -> MuxSessionId {
    MuxSessionId::new("tmux:editor")
}

/// Sessions `keys`, each hosted by the mux `tmux:editor`.
fn snapshot(keys: &[&str]) -> GraphSnapshot {
    let mut nodes: Vec<GraphNode> = keys
        .iter()
        .map(|key| {
            GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", *key),
                "claude-code".to_string(),
            ))
        })
        .collect();
    nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        mux_id(),
        "tmux".to_string(),
        "editor".to_string(),
    )));
    let candidate_links = keys
        .iter()
        .map(|key| {
            GraphLink::new(
                format!("mux-{key}"),
                session_id(key),
                LinkEndpoint::Node {
                    id: NodeId::MuxSession(mux_id()),
                },
                RelationKind::LinkedToMux,
                Provenance::StrongDiscovered,
            )
        })
        .collect();
    GraphSnapshot {
        nodes,
        candidate_links,
        ..GraphSnapshot::empty()
    }
}

fn bind(
    snapshot: &mut GraphSnapshot,
    id: NodeId,
    observation: LiveObservation,
    liveness: Liveness,
) {
    snapshot
        .node_live
        .insert(id, LiveBinding::new(observation, liveness));
}

/// An observation of `state` whose latest evidence is `silent_min`
/// minutes old.
fn observed(state: LiveState, silent_min: i64) -> LiveObservation {
    LiveObservation::new(state, LiveSource::Hook, NOW_MS - silent_min * MINUTE_MS)
}

#[test]
fn a_pinned_epoch_is_the_read_time_and_none_reads_the_clock() {
    assert_eq!(read_time_ms(Some(1_790_769_600)), NOW_MS);
    let before = crate::live_store::now_ms();
    let read = read_time_ms(None);
    assert!(read >= before, "{read} < {before}");
}

#[test]
fn a_session_cell_shows_the_state_its_detail_subagents_and_age() {
    let mut graph = snapshot(&["waiter"]);
    bind(
        &mut graph,
        session_id("waiter"),
        observed(LiveState::Waiting, 1)
            .with_detail(LiveDetail::Permission)
            .with_since_ms(NOW_MS - 3 * MINUTE_MS)
            .with_subagents(2)
            .with_pid(4242, Some(7)),
        Liveness::Alive,
    );
    assert_eq!(
        session_cell(&graph, &session_id("waiter"), NOW_MS).as_deref(),
        Some("waiting:permission +2 3m")
    );
}

#[test]
fn a_session_without_live_state_has_no_cell() {
    let graph = snapshot(&["silent"]);
    assert_eq!(session_cell(&graph, &session_id("silent"), NOW_MS), None);
}

#[test]
fn a_long_silent_working_session_is_marked_quiet_and_keeps_its_state() {
    let mut graph = snapshot(&["builder"]);
    bind(
        &mut graph,
        session_id("builder"),
        observed(LiveState::Working, 12).with_detail(LiveDetail::Tool),
        Liveness::Alive,
    );
    assert_eq!(
        session_cell(&graph, &session_id("builder"), NOW_MS).as_deref(),
        Some("working:tool quiet 12m")
    );
}

#[test]
fn a_derived_state_counts_its_age_from_the_latest_evidence() {
    let mut graph = snapshot(&["lost", "gone"]);
    // Nobody could check the process, and its evidence is 20 minutes old.
    bind(
        &mut graph,
        session_id("lost"),
        observed(LiveState::Working, 20)
            .with_since_ms(NOW_MS - 60 * MINUTE_MS)
            .with_subagents(1),
        Liveness::Unknown,
    );
    // The process is gone: the turn it was in started an hour ago, and it
    // was last heard from four minutes ago.
    bind(
        &mut graph,
        session_id("gone"),
        observed(LiveState::Working, 4)
            .with_since_ms(NOW_MS - 60 * MINUTE_MS)
            .with_subagents(1),
        Liveness::Dead,
    );
    assert_eq!(
        session_cell(&graph, &session_id("lost"), NOW_MS).as_deref(),
        Some("unknown:stale 20m")
    );
    assert_eq!(
        session_cell(&graph, &session_id("gone"), NOW_MS).as_deref(),
        Some("exited:killed 4m")
    );
}

#[test]
fn counts_list_the_counted_states_most_urgent_first() {
    let counts: LiveCounts = [
        LiveState::Working,
        LiveState::Working,
        LiveState::Exited,
        LiveState::Waiting,
        LiveState::Ready,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        counts_text(&counts).as_deref(),
        Some("1 waiting, 1 ready, 2 working, 1 exited")
    );
    assert_eq!(counts_text(&LiveCounts::default()), None);
}

#[test]
fn a_mux_cell_counts_its_sessions_and_its_pane_agent() {
    let mut graph = snapshot(&["waiter", "builder", "silent"]);
    bind(
        &mut graph,
        session_id("waiter"),
        observed(LiveState::Waiting, 1).with_detail(LiveDetail::Question),
        Liveness::Alive,
    );
    bind(
        &mut graph,
        session_id("builder"),
        observed(LiveState::Working, 1),
        Liveness::Alive,
    );
    bind(
        &mut graph,
        NodeId::MuxSession(mux_id()),
        observed(LiveState::Working, 1),
        Liveness::Alive,
    );
    assert_eq!(
        mux_cell(&graph, &mux_id(), NOW_MS).as_deref(),
        Some("1 waiting, 2 working")
    );
    assert_eq!(mux_cell(&snapshot(&["silent"]), &mux_id(), NOW_MS), None);
}

#[test]
fn node_show_lists_every_field_of_a_session_entry() {
    let mut graph = snapshot(&["waiter"]);
    let mut observation = observed(LiveState::Waiting, 2)
        .with_detail(LiveDetail::Permission)
        .with_since_ms(NOW_MS - 11 * MINUTE_MS)
        .with_subagents(1)
        .with_pid(4242, Some(7));
    observation.via_subagent = true;
    bind(
        &mut graph,
        session_id("waiter"),
        observation,
        Liveness::Alive,
    );

    assert_eq!(
        node_show_lines(&graph, &session_id("waiter"), NOW_MS),
        vec![
            "  state:      waiting",
            "  detail:     permission",
            "  via:        a subagent",
            "  since:      2026-09-30T11:49:00Z",
            "  age:        11m",
            "  last event: 2026-09-30T11:58:00Z",
            "  confidence: fresh",
            "  subagents:  1",
            "  source:     hook",
            "  process:    pid 4242, alive",
        ]
    );
}

#[test]
fn node_show_names_the_recorded_state_the_read_rules_replaced() {
    let mut graph = snapshot(&["gone"]);
    bind(
        &mut graph,
        session_id("gone"),
        observed(LiveState::Working, 4)
            .with_detail(LiveDetail::Tool)
            .with_since_ms(NOW_MS - 60 * MINUTE_MS),
        Liveness::Dead,
    );
    let lines = node_show_lines(&graph, &session_id("gone"), NOW_MS);
    assert_eq!(
        lines[..4],
        [
            "  state:      exited",
            "  detail:     killed",
            "  recorded:   working:tool",
            "  since:      2026-09-30T11:56:00Z",
        ]
    );
    assert_eq!(lines.last().map(String::as_str), Some("  process:    dead"));
}

#[test]
fn node_show_lists_a_mux_s_counts_and_its_pane_agent() {
    let mut graph = snapshot(&["waiter"]);
    bind(
        &mut graph,
        session_id("waiter"),
        observed(LiveState::Waiting, 1),
        Liveness::Alive,
    );
    bind(
        &mut graph,
        NodeId::MuxSession(mux_id()),
        observed(LiveState::Starting, 1),
        Liveness::Unknown,
    );
    let lines = node_show_lines(&graph, &NodeId::MuxSession(mux_id()), NOW_MS);
    assert_eq!(
        lines[..3],
        [
            "  counts:     1 waiting, 1 starting",
            "  pane agent with no session:",
            "    state:      starting",
        ]
    );
    assert_eq!(
        lines.last().map(String::as_str),
        Some("    process:    unknown")
    );
}

#[test]
fn node_show_lists_nothing_without_live_state() {
    let graph = snapshot(&["silent"]);
    assert!(node_show_lines(&graph, &session_id("silent"), NOW_MS).is_empty());
    assert!(node_show_lines(&graph, &NodeId::MuxSession(mux_id()), NOW_MS).is_empty());
}
