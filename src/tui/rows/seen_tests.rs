use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, LinkEndpoint, LinkState, Provenance, RelationKind,
};

/// When the turns end, in Unix epoch milliseconds.
const ENDED_MS: i64 = 1_700_000_000_500;
const ENDED_S: i64 = 1_700_000_000;

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

fn work() -> MuxSessionId {
    MuxSessionId::new("tmux:work")
}

/// The session `hosted` in the mux `work`, whose client is attached or not
/// and was last attached at `last_attached` (Unix seconds), and the
/// session `loose` in no mux.
fn world(attached: bool, last_attached: Option<i64>) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for key in ["hosted", "loose"] {
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", key),
                "claude-code",
            )));
    }
    let mut mux = MuxSessionNode::new(work(), "tmux", "work");
    mux.client_attached = Some(attached);
    mux.last_attached_epoch = last_attached;
    snapshot.nodes.push(GraphNode::MuxSession(mux));
    let mut link = GraphLink::new(
        "fixture:hosted-in-work".to_string(),
        session("hosted"),
        LinkEndpoint::Node {
            id: NodeId::MuxSession(work()),
        },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    link.state = LinkState::Active;
    snapshot.candidate_links.push(link);
    snapshot
}

/// Whether the turn of `agent` that ended at [`ENDED_MS`] is unseen.
fn judged(snapshot: &GraphSnapshot, agent: &Agent, memory: &SeenTurns, loaded_ms: i64) -> bool {
    let mux = agent.mux(snapshot);
    unseen(
        snapshot,
        agent,
        mux.as_ref(),
        ENDED_MS,
        SeenContext { memory, loaded_ms },
    )
}

#[test]
fn a_turn_in_a_mux_is_seen_once_a_client_attached_after_it_ended() {
    let hosted = Agent::Session(session("hosted"));
    let none = SeenTurns::default();
    let loaded = ENDED_MS + 60_000;
    for (last_attached, unseen) in [
        (None, true),
        (Some(ENDED_S - 1), true),
        // tmux keeps the second, so an attach in the turn's last second
        // counts.
        (Some(ENDED_S), false),
        (Some(ENDED_S + 30), false),
    ] {
        assert_eq!(
            judged(&world(false, last_attached), &hosted, &none, loaded),
            unseen,
            "last attached at {last_attached:?}"
        );
    }
}

#[test]
fn a_client_attached_in_a_snapshot_loaded_after_the_turn_ended_sees_it() {
    let hosted = Agent::Session(session("hosted"));
    let none = SeenTurns::default();
    let attached = world(true, Some(ENDED_S - 600));
    assert!(
        !judged(&attached, &hosted, &none, ENDED_MS + 1),
        "a client was on the mux after the turn ended"
    );
    assert!(
        judged(&attached, &hosted, &none, ENDED_MS - 1),
        "the listing is older than the turn's end, so it can't say"
    );
    assert!(
        judged(
            &world(false, Some(ENDED_S - 600)),
            &hosted,
            &none,
            ENDED_MS + 1
        ),
        "no client then, and none attached since"
    );
}

#[test]
fn a_session_with_no_mux_is_unseen_until_remembered_and_a_later_turn_again() {
    let snapshot = world(true, Some(ENDED_S + 30));
    let loose = Agent::Session(session("loose"));
    assert!(!has_mux(&snapshot, &loose));
    let mut memory = SeenTurns::default();
    assert!(
        judged(&snapshot, &loose, &memory, ENDED_MS + 1),
        "another mux's client says nothing about it"
    );
    assert!(
        memory.see(loose.clone(), ENDED_MS),
        "the first look is news"
    );
    assert!(!judged(&snapshot, &loose, &memory, ENDED_MS + 1));
    assert!(!memory.see(loose.clone(), ENDED_MS), "looking again is not");
    assert!(
        !memory.see(loose.clone(), ENDED_MS - 5_000),
        "an earlier turn is seen with it"
    );
    assert!(memory.has_seen(&loose, ENDED_MS));
    assert!(
        !memory.has_seen(&loose, ENDED_MS + 1),
        "a turn that ends later is unseen again"
    );
}

#[test]
fn the_memory_keeps_a_turn_seen_after_the_evidence_is_gone() {
    let hosted = Agent::Session(session("hosted"));
    let mut memory = SeenTurns::default();
    memory.see(hosted.clone(), ENDED_MS);
    // The client left, and the mux was last attached before the turn
    // ended: only the memory says it was seen.
    assert!(!judged(
        &world(false, Some(ENDED_S - 600)),
        &hosted,
        &memory,
        ENDED_MS + 60_000
    ));
}

#[test]
fn a_pane_agent_is_seen_through_its_panes_mux() {
    let pane = Agent::Pane(LivePane::new(work(), Some("%3".into())));
    assert_eq!(pane.mux(&world(false, None)), Some(work()));
    assert!(has_mux(&world(false, None), &pane));
    let none = SeenTurns::default();
    assert!(judged(&world(false, None), &pane, &none, ENDED_MS + 1));
    assert!(!judged(
        &world(false, Some(ENDED_S + 1)),
        &pane,
        &none,
        ENDED_MS + 1
    ));
}

#[test]
fn an_agent_whose_mux_is_not_listed_has_none() {
    let mut snapshot = world(true, Some(ENDED_S + 30));
    snapshot
        .nodes
        .retain(|node| !matches!(node, GraphNode::MuxSession(_)));
    let hosted = Agent::Session(session("hosted"));
    assert_eq!(
        hosted.mux(&snapshot),
        Some(work()),
        "its link still names it"
    );
    assert!(
        !has_mux(&snapshot, &hosted),
        "but the snapshot lists no such mux"
    );
    assert!(judged(
        &snapshot,
        &hosted,
        &SeenTurns::default(),
        ENDED_MS + 1
    ));
}
