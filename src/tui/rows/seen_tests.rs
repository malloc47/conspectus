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

const SOCKET: &str = "/tmp/tmux-1000/default";

fn came_back(session: &str, at_ms: i64) -> ClientReturn {
    ClientReturn {
        socket: SOCKET.to_string(),
        session: session.to_string(),
        at_ms,
    }
}

/// Memory holding one return of the client to the TUI's own session.
fn returned(session: &str, at_ms: i64) -> SeenTurns {
    let mut memory = SeenTurns::default();
    memory.note_return(came_back(session, at_ms));
    memory
}

#[test]
fn a_turn_that_ended_before_the_client_left_its_mux_is_seen_on_the_return() {
    let hosted = Agent::Session(session("hosted"));
    let snapshot = world(false, Some(ENDED_S - 600));
    let loaded = ENDED_MS + 60_000;
    for (left_at, unseen) in [
        // The client left the mux after the turn ended: it watched it.
        (ENDED_MS + 5_000, false),
        (ENDED_MS, false),
        // The turn ended after the client was already gone.
        (ENDED_MS - 1, true),
    ] {
        assert_eq!(
            judged(&snapshot, &hosted, &returned("work", left_at), loaded),
            unseen,
            "client left at {left_at}"
        );
    }
}

#[test]
fn a_return_from_another_session_sees_nothing_of_this_mux() {
    let hosted = Agent::Session(session("hosted"));
    let snapshot = world(false, Some(ENDED_S - 600));
    assert!(
        judged(
            &snapshot,
            &hosted,
            &returned("elsewhere", ENDED_MS + 5_000),
            ENDED_MS + 60_000
        ),
        "the client came back from `elsewhere`, not from the mux"
    );
}

#[test]
fn a_return_is_matched_by_server_as_well_as_by_name() {
    let hosted = Agent::Session(session("hosted"));
    // The mux lives on the `-L side` server, as a session named `work`.
    let mut snapshot = world(false, Some(ENDED_S - 600));
    for node in &mut snapshot.nodes {
        if let GraphNode::MuxSession(mux) = node {
            mux.socket = Some("side".to_string());
            mux.native_id = "side:work".to_string();
        }
    }
    let loaded = ENDED_MS + 60_000;
    assert!(
        judged(
            &snapshot,
            &hosted,
            &returned("work", ENDED_MS + 5_000),
            loaded
        ),
        "a `work` on the default server is not the mux on `side`"
    );
    let mut memory = SeenTurns::default();
    memory.note_return(ClientReturn {
        socket: "/tmp/tmux-1000/side".to_string(),
        session: "work".to_string(),
        at_ms: ENDED_MS + 5_000,
    });
    assert!(
        !judged(&snapshot, &hosted, &memory, loaded),
        "a return on the `side` server is"
    );
}

#[test]
fn a_return_sees_a_pane_agents_turn_and_never_one_with_no_mux() {
    let pane = Agent::Pane(LivePane::new(work(), Some("%3".into())));
    let loose = Agent::Session(session("loose"));
    let snapshot = world(false, None);
    let memory = returned("work", ENDED_MS + 5_000);
    let loaded = ENDED_MS + 60_000;
    assert!(!judged(&snapshot, &pane, &memory, loaded));
    assert!(
        judged(&snapshot, &loose, &memory, loaded),
        "a session in no mux is seen by resting on it"
    );
}

#[test]
fn only_the_latest_return_from_a_session_is_kept() {
    let mut memory = SeenTurns::default();
    assert!(memory.note_return(came_back("work", 2_000)));
    assert!(
        !memory.note_return(came_back("work", 1_000)),
        "an older one is no news"
    );
    assert!(
        memory.note_return(came_back("work", 3_000)),
        "a later one is"
    );
    assert!(
        memory.note_return(came_back("other", 1_000)),
        "another session has its own"
    );
}
