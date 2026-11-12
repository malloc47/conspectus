use super::*;
use crate::model::live::{
    LiveBinding, LiveConfidence, LiveDetail, LiveObservation, LiveSource, LiveState, Liveness,
};

const NOW_MS: i64 = 1_790_769_600_000;
const MINUTE_MS: i64 = 60 * 1000;

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("codex", "/home/me/.codex", key))
}

fn mux(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

fn observed(state: LiveState, minutes_ago: i64) -> LiveObservation {
    LiveObservation::new(state, LiveSource::Hook, NOW_MS - minutes_ago * MINUTE_MS)
}

/// `observation` bound with a process found running.
fn alive(observation: LiveObservation) -> LiveBinding {
    LiveBinding::new(observation, Liveness::Alive)
}

/// A snapshot with a node for every session and mux named, and a
/// `linked_to_mux` candidate from each session to its mux with the
/// given provenance.
fn hosted(links: &[(&str, &str, Provenance)]) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    let mut seen = BTreeSet::new();
    for (index, (key, name, provenance)) in links.iter().enumerate() {
        let session_id = session(key);
        let mux_id = mux(name);
        if seen.insert(session_id.clone())
            && let NodeId::AgentSession(id) = &session_id
        {
            snapshot
                .nodes
                .push(GraphNode::AgentSession(AgentSessionNode::new(
                    id.clone(),
                    "codex",
                )));
        }
        if seen.insert(NodeId::MuxSession(mux_id.clone())) {
            snapshot
                .nodes
                .push(GraphNode::MuxSession(MuxSessionNode::new(
                    mux_id.clone(),
                    "tmux",
                    *name,
                )));
        }
        snapshot.candidate_links.push(GraphLink::new(
            format!("link-{index}"),
            session_id,
            LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
            RelationKind::LinkedToMux,
            *provenance,
        ));
    }
    snapshot.canonicalize();
    snapshot
}

// ---- serialization ----

#[test]
fn an_empty_live_table_leaves_the_json_as_it_was() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    snapshot.node_presence.insert(
        NodeId::MuxSession(mux("main")),
        NodePresence::Gone { last_seen: 1 },
    );
    let json = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
    assert!(!json.contains("node_live"), "{json}");

    let read: GraphSnapshot =
        serde_json::from_str(&json).expect("a snapshot without the table loads");
    assert_eq!(read, snapshot);
}

#[test]
fn a_live_table_serializes_as_a_list_beside_each_node_id() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.node_live.insert(
        session("a"),
        alive(
            observed(LiveState::Waiting, 1)
                .with_detail(LiveDetail::Permission)
                .with_pid(4242, Some(99)),
        ),
    );
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("main")),
        LiveBinding::new(observed(LiveState::Working, 0), Liveness::Unknown),
    );

    let value = serde_json::to_value(&snapshot).expect("serialize snapshot");
    let entries = value["node_live"].as_array().expect("node_live is a list");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0],
        serde_json::json!({
            "node_id": {
                "type": "agent_session",
                "harness_key": "codex",
                "state_scope": "/home/me/.codex",
                "session_key": "a",
            },
            "state": "waiting",
            "detail": "permission",
            "since_ms": NOW_MS - MINUTE_MS,
            "observed_ms": NOW_MS - MINUTE_MS,
            "source": "hook",
            "pid": 4242,
            "pid_start": 99,
            "liveness": "alive",
        })
    );
    assert_eq!(
        entries[1],
        serde_json::json!({
            "node_id": { "type": "mux_session", "native_id": "tmux:main" },
            "state": "working",
            "since_ms": NOW_MS,
            "observed_ms": NOW_MS,
            "source": "hook",
            "liveness": "unknown",
        })
    );
}

#[test]
fn a_live_table_round_trips_through_json() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    snapshot.node_live.insert(
        session("a"),
        alive(
            observed(LiveState::Working, 3)
                .with_detail(LiveDetail::Tool)
                .with_since_ms(NOW_MS - 20 * MINUTE_MS)
                .with_subagents(2)
                .with_pid(4242, Some(99))
                .with_heartbeat_s(30),
        ),
    );
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("main")),
        LiveBinding::new(
            observed(LiveState::Ready, 0).with_detail(LiveDetail::Finished),
            Liveness::Dead,
        ),
    );

    let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let read: GraphSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");
    assert_eq!(read, snapshot);
}

#[test]
fn a_live_table_round_trips_through_rkyv() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    snapshot.node_live.insert(
        session("a"),
        LiveBinding::new(
            observed(LiveState::Error, 2)
                .with_detail(LiveDetail::RateLimited)
                .with_pid(4242, None),
            Liveness::Dead,
        ),
    );
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("main")),
        alive(observed(LiveState::Starting, 1)),
    );

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot).expect("archive snapshot");
    let read = rkyv::from_bytes::<GraphSnapshot, rkyv::rancor::Error>(&bytes)
        .expect("deserialize snapshot");
    assert_eq!(read, snapshot);
}

#[test]
fn an_entry_with_names_this_version_does_not_know_still_loads() {
    let json = serde_json::json!({
        "nodes": [],
        "candidate_links": [],
        "resolved_relationships": [],
        "diagnostics": [],
        "node_live": [{
            "node_id": { "type": "mux_session", "native_id": "tmux:main" },
            "state": "dreaming",
            "detail": "lucid",
            "since_ms": 1,
            "observed_ms": 2,
            "source": "telepathy",
            "liveness": "undead",
        }, {
            "node_id": { "type": "mux_session", "native_id": "tmux:other" },
            "state": "ready",
            "since_ms": 1,
            "observed_ms": 2,
            "source": "hook",
        }],
    });
    let read: GraphSnapshot = serde_json::from_value(json).expect("deserialize snapshot");
    let binding = read.node_live[&NodeId::MuxSession(mux("main"))];
    assert_eq!(binding.liveness, Liveness::Unknown);
    let observation = binding.observation;
    assert_eq!(observation.state, LiveState::Unknown);
    assert_eq!(observation.detail, None);
    assert_eq!(observation.source, LiveSource::Unknown);
    assert_eq!((observation.since_ms, observation.observed_ms), (1, 2));
    assert_eq!(
        read.node_live[&NodeId::MuxSession(mux("other"))].liveness,
        Liveness::Unknown,
        "a binding without a liveness reads as unchecked"
    );
}

// ---- canonicalize and eviction ----

#[test]
fn canonicalize_keeps_the_live_table() {
    let mut snapshot = hosted(&[
        ("b", "main", Provenance::StrongDiscovered),
        ("a", "main", Provenance::StrongDiscovered),
    ]);
    snapshot
        .node_live
        .insert(session("b"), alive(observed(LiveState::Working, 0)));
    snapshot
        .node_live
        .insert(session("a"), alive(observed(LiveState::Waiting, 0)));
    let table = snapshot.node_live.clone();

    snapshot.canonicalize();

    assert_eq!(snapshot.node_live, table);
}

#[test]
fn evicting_a_provider_drops_the_live_state_of_its_nodes_only() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    let mux_id = NodeId::MuxSession(mux("main"));
    let unowned = session("unowned");
    for (id, provider) in [(&session("a"), "claude_code"), (&mux_id, "tmux")] {
        snapshot.node_provenance.insert(
            id.clone(),
            NodeProvenance {
                provider: provider.to_string(),
                freshness_epoch: None,
            },
        );
    }
    for id in [&session("a"), &mux_id, &unowned] {
        snapshot
            .node_live
            .insert(id.clone(), alive(observed(LiveState::Working, 0)));
    }

    snapshot.evict_provider("tmux");
    assert_eq!(
        snapshot.node_live.keys().cloned().collect::<Vec<_>>(),
        vec![session("a"), unowned.clone()],
        "the mux's pane-only entry goes with the mux"
    );

    snapshot.evict_provider("claude_code");
    assert_eq!(
        snapshot.node_live.keys().cloned().collect::<Vec<_>>(),
        vec![unowned],
        "an entry for a node no provider owns stays, as the node does"
    );
}

// ---- live_state_of ----

#[test]
fn live_state_of_is_none_without_an_entry() {
    let snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    assert_eq!(snapshot.live_state_of(&session("a"), NOW_MS), None);
}

#[test]
fn live_state_of_reads_the_effective_state_with_the_bound_liveness() {
    let waiting = observed(LiveState::Waiting, 120)
        .with_detail(LiveDetail::Question)
        .with_pid(4242, Some(7));
    let mut snapshot = GraphSnapshot::empty();
    for (key, liveness) in [
        ("alive", Liveness::Alive),
        ("dead", Liveness::Dead),
        ("unchecked", Liveness::Unknown),
    ] {
        snapshot
            .node_live
            .insert(session(key), LiveBinding::new(waiting, liveness));
    }
    snapshot
        .node_live
        .insert(session("working"), alive(observed(LiveState::Working, 11)));

    assert_eq!(
        snapshot.live_state_of(&session("alive"), NOW_MS),
        Some(EffectiveLive {
            state: LiveState::Waiting,
            detail: Some(LiveDetail::Question),
            confidence: LiveConfidence::Fresh,
        }),
        "a waiting agent whose process lives does not age"
    );
    assert_eq!(
        snapshot
            .live_state_of(&session("dead"), NOW_MS)
            .map(|live| (live.state, live.detail)),
        Some((LiveState::Exited, Some(LiveDetail::Killed)))
    );
    assert_eq!(
        snapshot.live_state_of(&session("unchecked"), NOW_MS),
        Some(EffectiveLive::STALE),
        "with no liveness evidence a two-hour-old state is stale"
    );
    assert_eq!(
        snapshot
            .live_state_of(&session("working"), NOW_MS)
            .map(|live| (live.state, live.confidence)),
        Some((LiveState::Working, LiveConfidence::Quiet))
    );
}

#[test]
fn live_state_of_reads_a_pane_only_entry_on_its_mux() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    let mux_id = NodeId::MuxSession(mux("main"));
    snapshot
        .node_live
        .insert(mux_id.clone(), alive(observed(LiveState::Starting, 0)));

    assert_eq!(
        snapshot
            .live_state_of(&mux_id, NOW_MS)
            .map(|live| live.state),
        Some(LiveState::Starting)
    );
    assert_eq!(snapshot.live_state_of(&session("a"), NOW_MS), None);
}

// ---- live_counts_of_mux ----

#[test]
fn a_mux_with_no_sessions_counts_nothing() {
    let mut snapshot = hosted(&[("a", "other", Provenance::StrongDiscovered)]);
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            mux("empty"),
            "tmux",
            "empty",
        )));
    snapshot
        .node_live
        .insert(session("a"), alive(observed(LiveState::Waiting, 0)));

    let counts = snapshot.live_counts_of_mux(&mux("empty"), NOW_MS);
    assert!(counts.is_empty(), "{counts:?}");
    assert_eq!(counts.dominant(), None);
}

#[test]
fn a_mux_counts_the_session_it_hosts() {
    let mut snapshot = hosted(&[("a", "main", Provenance::StrongDiscovered)]);
    snapshot
        .node_live
        .insert(session("a"), alive(observed(LiveState::Ready, 0)));

    let counts = snapshot.live_counts_of_mux(&mux("main"), NOW_MS);
    assert_eq!(
        counts,
        LiveCounts {
            ready: 1,
            ..LiveCounts::default()
        }
    );
}

#[test]
fn a_mux_counts_every_session_it_prefers_and_no_other() {
    let mut snapshot = hosted(&[
        ("a", "main", Provenance::StrongDiscovered),
        ("b", "main", Provenance::StrongDiscovered),
        ("c", "main", Provenance::Convention),
        ("elsewhere", "other", Provenance::StrongDiscovered),
        // Also a weaker candidate of `main`, but `other` is preferred.
        ("drifted", "other", Provenance::StrongDiscovered),
        ("drifted", "main", Provenance::Cached),
    ]);
    for (key, state) in [
        ("a", LiveState::Waiting),
        ("b", LiveState::Working),
        ("c", LiveState::Working),
        ("elsewhere", LiveState::Error),
        ("drifted", LiveState::Compacting),
    ] {
        snapshot
            .node_live
            .insert(session(key), alive(observed(state, 0)));
    }

    let counts = snapshot.live_counts_of_mux(&mux("main"), NOW_MS);
    assert_eq!(
        counts,
        LiveCounts {
            waiting: 1,
            working: 2,
            ..LiveCounts::default()
        }
    );
    assert_eq!(counts.dominant(), Some(LiveState::Waiting));

    let other = snapshot.live_counts_of_mux(&mux("other"), NOW_MS);
    assert_eq!(
        other,
        LiveCounts {
            error: 1,
            compacting: 1,
            ..LiveCounts::default()
        }
    );
}

#[test]
fn a_pane_only_entry_counts_on_its_mux_beside_its_sessions() {
    let mut snapshot = hosted(&[
        ("a", "main", Provenance::StrongDiscovered),
        ("b", "other", Provenance::StrongDiscovered),
    ]);
    snapshot
        .node_live
        .insert(session("a"), alive(observed(LiveState::Working, 0)));
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("main")),
        alive(observed(LiveState::Waiting, 0).with_detail(LiveDetail::Input)),
    );
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("other")),
        alive(observed(LiveState::Ready, 0)),
    );

    let counts = snapshot.live_counts_of_mux(&mux("main"), NOW_MS);
    assert_eq!(
        counts,
        LiveCounts {
            working: 1,
            waiting: 1,
            ..LiveCounts::default()
        }
    );
    assert_eq!(counts.total(), 2);
}

#[test]
fn a_hosted_session_without_an_entry_is_not_counted() {
    let mut snapshot = hosted(&[
        ("a", "main", Provenance::StrongDiscovered),
        ("silent", "main", Provenance::StrongDiscovered),
    ]);
    snapshot
        .node_live
        .insert(session("a"), alive(observed(LiveState::Working, 0)));

    let counts = snapshot.live_counts_of_mux(&mux("main"), NOW_MS);
    assert_eq!(
        counts,
        LiveCounts {
            working: 1,
            ..LiveCounts::default()
        },
        "no evidence is not `unknown` and not idle"
    );
}

#[test]
fn mux_counts_read_each_observation_with_its_own_liveness() {
    let mut snapshot = hosted(&[
        ("a", "main", Provenance::StrongDiscovered),
        ("b", "main", Provenance::StrongDiscovered),
        ("c", "main", Provenance::StrongDiscovered),
    ]);
    snapshot.node_live.insert(
        session("a"),
        LiveBinding::new(
            observed(LiveState::Working, 0).with_pid(1, None),
            Liveness::Dead,
        ),
    );
    snapshot.node_live.insert(
        session("b"),
        alive(observed(LiveState::Waiting, 0).with_pid(2, None)),
    );
    // Silent for twenty minutes with no pid to check.
    snapshot.node_live.insert(
        session("c"),
        LiveBinding::new(observed(LiveState::Ready, 20), Liveness::Unknown),
    );

    let counts = snapshot.live_counts_of_mux(&mux("main"), NOW_MS);
    assert_eq!(
        counts,
        LiveCounts {
            exited: 1,
            waiting: 1,
            unknown: 1,
            ..LiveCounts::default()
        }
    );
}

#[test]
fn counts_for_every_mux_at_once_agree_with_each_muxs_own() {
    let mut snapshot = hosted(&[
        ("a", "main", Provenance::StrongDiscovered),
        ("b", "main", Provenance::Convention),
        ("drifted", "other", Provenance::StrongDiscovered),
        ("drifted", "main", Provenance::Cached),
        ("idle", "quiet", Provenance::StrongDiscovered),
    ]);
    for (key, state) in [
        ("a", LiveState::Waiting),
        ("b", LiveState::Working),
        ("drifted", LiveState::Compacting),
    ] {
        snapshot
            .node_live
            .insert(session(key), alive(observed(state, 0)));
    }
    snapshot.node_live.insert(
        NodeId::MuxSession(mux("main")),
        alive(observed(LiveState::Ready, 0)),
    );
    // A session no mux hosts counts nowhere.
    snapshot
        .node_live
        .insert(session("loose"), alive(observed(LiveState::Error, 0)));

    let by_mux = snapshot.live_counts_by_mux(&snapshot.node_live, NOW_MS);
    for name in ["main", "other", "quiet"] {
        assert_eq!(
            by_mux.get(&mux(name)).copied().unwrap_or_default(),
            snapshot.live_counts_of_mux(&mux(name), NOW_MS),
            "{name}"
        );
    }
    assert!(
        !by_mux.contains_key(&mux("quiet")),
        "a mux nothing counts toward has no entry"
    );

    // Another table over the same snapshot reads the same way.
    let mut later = BTreeMap::new();
    later.insert(session("b"), alive(observed(LiveState::Ready, 0)));
    let by_mux = snapshot.live_counts_by_mux(&later, NOW_MS);
    assert_eq!(
        by_mux,
        BTreeMap::from([(
            mux("main"),
            LiveCounts {
                ready: 1,
                ..LiveCounts::default()
            }
        )])
    );
}
