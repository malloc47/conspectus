use super::*;
use crate::model::{
    AgentSessionNode, GraphSnapshot, MuxSessionId, MuxSessionNode, PinMuxRef, Provenance,
};

fn mux_node(name: &str) -> GraphNode {
    // Production tmux discovery encodes the *id* as the
    // prefixed form `tmux:<name>` but stores the bare tmux
    // session name in `native_id` (see
    // `discovery/tmux/mod.rs:1038`). Test fixtures must match
    // so the resolver's `mux_index` can reconstruct the
    // prefixed key — otherwise default-socket pins never bind.
    GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new(format!("tmux:{name}")),
            "tmux".to_string(),
            name.to_string(),
        )
        .with_cwd("/home/me/work/repo".to_string()),
    )
}

fn agent_session_node(harness: &str, session_key: &str, cwd: &str) -> GraphNode {
    let id = AgentSessionId::new(
        harness.to_string(),
        format!("/state/{harness}"),
        session_key.to_string(),
    );
    GraphNode::AgentSession(AgentSessionNode {
        id,
        harness_key: harness.to_string(),
        cwd: Some(cwd.to_string()),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn linked_to_mux(
    link_id: &str,
    harness: &str,
    session_key: &str,
    mux_name: &str,
    provenance: Provenance,
) -> GraphLink {
    GraphLink {
        id: link_id.to_string(),
        source: NodeId::AgentSession(AgentSessionId::new(
            harness.to_string(),
            format!("/state/{harness}"),
            session_key.to_string(),
        )),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new(format!("tmux:{mux_name}"))),
        },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn pin_candidate(
    id: &str,
    harness: &str,
    cwd: &str,
    mux_name: &str,
    provenance: Provenance,
) -> PinCandidate {
    PinCandidate {
        id: id.to_string(),
        display_name: id.to_string(),
        harness: harness.to_string(),
        cwd: cwd.to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: mux_name.to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance,
        store_path: "/tmp/conspectus.toml".to_string(),
        binding: None,
    }
}

fn empty_snapshot_with_pin(pin: PinCandidate) -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    snap.pins.push(pin);
    snap
}

#[test]
fn unbound_when_mux_is_missing() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    assert_eq!(snap.pins[0].binding, Some(PinBinding::Unbound));
    assert_eq!(snap.candidate_links.len(), 1);
    let link = &snap.candidate_links[0];
    assert_eq!(link.source, NodeId::Pin(PinId::new("ingest")));
    assert_eq!(link.relation, RelationKind::PinTargetsMux);
    match &link.target {
        LinkEndpoint::Unresolved { evidence } => {
            assert_eq!(evidence.node_type, "mux_session");
            assert_eq!(evidence.native_id.as_deref(), Some("tmux:ingest"));
        }
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved mux target, got {other:?}")
        }
    }
    assert!(snap.aliases.is_empty());
    assert_eq!(diagnostics.len(), 1);
    match &diagnostics[0] {
        Diagnostic::PinUnbound {
            pin_id,
            expected_mux_native_id,
            last_session,
        } => {
            assert_eq!(pin_id, "ingest");
            assert_eq!(expected_mux_native_id, "tmux:ingest");
            assert!(last_session.is_none());
        }
        other => panic!("expected PinUnbound, got {other:?}"),
    }
}

#[test]
fn stale_mux_when_no_harness_session_attributed() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("ingest"));

    let diagnostics = apply_pin_bindings(&mut snap);

    match &snap.pins[0].binding {
        Some(PinBinding::StaleMux { mux }) => {
            assert_eq!(mux.native_id, "tmux:ingest");
        }
        other => panic!("expected StaleMux, got {other:?}"),
    }
    assert!(snap.aliases.is_empty());
    assert!(
        !snap
            .candidate_links
            .iter()
            .any(|l| l.id.starts_with("pin:")),
        "no synthesized pin link on stale-mux"
    );
    assert!(matches!(
        diagnostics.as_slice(),
        [Diagnostic::PinStaleMux { .. }]
    ));
}

#[test]
fn bind_when_exactly_one_harness_session_attributed() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("ingest"));
    snap.nodes
        .push(agent_session_node("codex", "alpha", "/home/me/work/repo"));
    snap.candidate_links.push(linked_to_mux(
        "discovered-alpha",
        "codex",
        "alpha",
        "ingest",
        Provenance::Discovered,
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    // No pin-specific diagnostics on a clean bind.
    assert!(diagnostics.is_empty(), "got {diagnostics:?}");
    match &snap.pins[0].binding {
        Some(PinBinding::Bound { mux, session }) => {
            assert_eq!(mux.native_id, "tmux:ingest");
            assert_eq!(session.session_key, "alpha");
        }
        other => panic!("expected Bound, got {other:?}"),
    }

    let synthesized = snap
        .candidate_links
        .iter()
        .find(|l| l.id == "pin:local_pin:ingest")
        .expect("synthesized link");
    assert_eq!(synthesized.relation, RelationKind::LinkedToMux);
    assert_eq!(synthesized.provenance, Provenance::LocalPin);
    assert!(matches!(synthesized.target, LinkEndpoint::Node { .. }));
    let session_id = AgentSessionId::new(
        "codex".to_string(),
        "/state/codex".to_string(),
        "alpha".to_string(),
    );
    assert_eq!(
        snap.aliases.get(&NodeId::AgentSession(session_id)),
        Some("ingest")
    );
}

#[test]
fn ambiguous_picks_highest_provenance_and_emits_diagnostic() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("ingest"));
    snap.nodes
        .push(agent_session_node("codex", "weak", "/home/me/work/repo"));
    snap.nodes
        .push(agent_session_node("codex", "strong", "/home/me/work/repo"));
    snap.candidate_links.push(linked_to_mux(
        "discovered-weak",
        "codex",
        "weak",
        "ingest",
        Provenance::Discovered,
    ));
    snap.candidate_links.push(linked_to_mux(
        "discovered-strong",
        "codex",
        "strong",
        "ingest",
        Provenance::StrongDiscovered,
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    match &snap.pins[0].binding {
        Some(PinBinding::Bound { session, .. }) => {
            assert_eq!(session.session_key, "strong");
        }
        other => panic!("expected Bound, got {other:?}"),
    }

    let ambig = diagnostics
        .iter()
        .find(|d| matches!(d, Diagnostic::PinAmbiguous { .. }))
        .expect("ambiguous diagnostic");
    match ambig {
        Diagnostic::PinAmbiguous {
            chosen, competing, ..
        } => {
            assert_eq!(chosen.session_key, "strong");
            assert_eq!(competing.len(), 1);
            assert_eq!(competing[0].session_key, "weak");
        }
        _ => unreachable!(),
    }
}

#[test]
fn drift_advisory_when_session_cwd_differs() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("ingest"));
    snap.nodes
        .push(agent_session_node("codex", "alpha", "/home/me/elsewhere"));
    snap.candidate_links.push(linked_to_mux(
        "discovered-alpha",
        "codex",
        "alpha",
        "ingest",
        Provenance::Discovered,
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    assert!(matches!(
        snap.pins[0].binding,
        Some(PinBinding::Bound { .. })
    ));
    let drift = diagnostics
        .iter()
        .find_map(|d| match d {
            Diagnostic::PinDrift {
                declared_cwd,
                observed_cwd,
                ..
            } => Some((declared_cwd.clone(), observed_cwd.clone())),
            _ => None,
        })
        .expect("drift diagnostic");
    assert_eq!(drift.0, "/home/me/work/repo");
    assert_eq!(drift.1, "/home/me/elsewhere");
}

#[test]
fn ignores_inactive_linked_to_mux_when_attributing() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "ingest",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("ingest"));
    snap.nodes
        .push(agent_session_node("codex", "ignored", "/home/me/work/repo"));
    let mut ignored = linked_to_mux(
        "ignored-link",
        "codex",
        "ignored",
        "ingest",
        Provenance::Discovered,
    );
    ignored.state = LinkState::Ignored { reason: None };
    snap.candidate_links.push(ignored);

    let diagnostics = apply_pin_bindings(&mut snap);

    // Ignored attribution is invisible to the pin; the mux is
    // therefore stale, not bound.
    assert!(matches!(
        snap.pins[0].binding,
        Some(PinBinding::StaleMux { .. })
    ));
    assert!(matches!(
        diagnostics.as_slice(),
        [Diagnostic::PinStaleMux { .. }]
    ));
}

#[test]
fn non_default_socket_lookup_uses_full_native_id() {
    let mut pin = pin_candidate(
        "scratch",
        "codex",
        "/home/me/work/repo",
        "ingest",
        Provenance::LocalPin,
    );
    pin.mux.socket_name = Some("scratch".to_string());
    let mut snap = empty_snapshot_with_pin(pin);

    // A default-socket mux with bare `tmux:ingest` should NOT
    // satisfy the pin; only the encoded
    // `tmux:scratch:ingest` mux does.
    snap.nodes.push(mux_node("ingest"));

    let diagnostics = apply_pin_bindings(&mut snap);
    assert!(matches!(snap.pins[0].binding, Some(PinBinding::Unbound)));
    assert!(matches!(
        diagnostics.as_slice(),
        [Diagnostic::PinUnbound { .. }]
    ));

    // Add the correctly-encoded non-default-socket mux and rerun.
    // Convention: `id` is the prefixed form `tmux:<socket>:<name>`,
    // `native_id` holds the post-backend portion (`<socket>:<name>`
    // for non-default sockets, just `<name>` for default). The
    // resolver's `mux_index` reconstructs the full lookup key
    // as `format!("{}:{}", backend, native_id)`.
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new("tmux:scratch:ingest"),
            "tmux".to_string(),
            "scratch:ingest".to_string(),
        )
        .with_cwd("/home/me/work/repo".to_string()),
    ));
    snap.nodes
        .push(agent_session_node("codex", "alpha", "/home/me/work/repo"));
    snap.candidate_links.push(linked_to_mux(
        "discovered-alpha",
        "codex",
        "alpha",
        "scratch:ingest",
        Provenance::Discovered,
    ));
    // Reset the pin's binding so the second pass observes fresh
    // state (the resolver is normally invoked once per snapshot).
    snap.pins[0].binding = None;

    let diagnostics = apply_pin_bindings(&mut snap);
    assert!(matches!(
        snap.pins[0].binding,
        Some(PinBinding::Bound { .. })
    ));
    assert!(diagnostics.is_empty());
}

#[test]
fn production_style_mux_node_binds_default_socket_pin() {
    // Regression for the inconsistency between resolver test
    // fixtures (which used to set `native_id` to the prefixed
    // `tmux:<name>` form) and production discovery (which sets
    // `native_id` to the bare `<name>` per
    // `discovery/tmux/mod.rs:1038`). Before the
    // `mux_index` reconstruction-key fix, default-socket pins
    // could never bind against a production-shaped snapshot —
    // the lookup key (`pin.mux.native_id()` = "tmux:editor")
    // never matched the index key (mux.native_id = "editor").
    // This test exercises the binding path with the production
    // convention to lock the fix in.
    use crate::model::PinMuxRef;

    let mut snap = GraphSnapshot::empty();
    // Production-shaped mux node: prefixed id, bare native_id.
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new("tmux:editor"),
            "tmux".to_string(),
            "editor".to_string(),
        )
        .with_cwd("/home/op/work".to_string()),
    ));
    snap.nodes
        .push(agent_session_node("codex", "alpha", "/home/op/work"));
    snap.candidate_links.push(linked_to_mux(
        "discovered-alpha",
        "codex",
        "alpha",
        "editor",
        Provenance::Discovered,
    ));
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/home/op/work".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "editor".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/home/op/work/.conspectus.toml".to_string(),
        binding: None,
    });

    let diagnostics = apply_pin_bindings(&mut snap);

    assert!(
        matches!(snap.pins[0].binding, Some(PinBinding::Bound { .. })),
        "expected Bound binding, got {:?}",
        snap.pins[0].binding,
    );
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics on a clean bind, got {diagnostics:?}",
    );
}

#[test]
fn local_pin_synthesized_link_outranks_discovered_in_precedence() {
    // The synthesized LocalPin link's `provenance.precedence()`
    // should sit above StrongDiscovered/Discovered/Cached and below
    // LocalDeclared, matching ADR 0057 §Resolver Binding Semantics
    // step 5.
    assert!(Provenance::LocalPin.precedence() > Provenance::StrongDiscovered.precedence());
    assert!(Provenance::LocalPin.precedence() > Provenance::GlobalDeclared.precedence());
    assert!(Provenance::LocalPin.precedence() < Provenance::LocalDeclared.precedence());
    assert!(Provenance::GlobalPin.precedence() > Provenance::StrongDiscovered.precedence());
    assert!(Provenance::GlobalPin.precedence() < Provenance::GlobalDeclared.precedence());
}

fn linked_to_mux_with_evidence(
    link_id: &str,
    session_key: &str,
    mux_name: &str,
    match_kind: &str,
) -> GraphLink {
    let mut link = linked_to_mux(
        link_id,
        "claude-code",
        session_key,
        mux_name,
        Provenance::StrongDiscovered,
    );
    link.confidence = Confidence::High;
    link.source_metadata.fields.insert(
        "match_kind".to_string(),
        Value::String(match_kind.to_string()),
    );
    link
}

fn two_pins_one_session_snapshot() -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    for name in ["agent", "agent3"] {
        snap.pins.push(pin_candidate(
            name,
            "claude-code",
            "/home/me/work/repo",
            name,
            Provenance::LocalPin,
        ));
        snap.nodes.push(mux_node(name));
    }
    snap.nodes.push(agent_session_node(
        "claude-code",
        "shared",
        "/home/me/work/repo",
    ));
    // Listed weaker-first so the outcome can't come from link order.
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "activity-agent3",
        "shared",
        "agent3",
        "session_file_activity_match",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "hook-agent",
        "shared",
        "agent",
        "hook_session_path_match",
    ));
    snap
}

#[test]
fn session_realizes_at_most_one_pin_and_stronger_evidence_keeps_it() {
    let mut snap = two_pins_one_session_snapshot();

    let diagnostics = apply_pin_bindings(&mut snap);

    match &snap.pins[0].binding {
        Some(PinBinding::Bound { session, .. }) => assert_eq!(session.session_key, "shared"),
        other => panic!("expected agent Bound, got {other:?}"),
    }
    match &snap.pins[1].binding {
        Some(PinBinding::StaleMux { mux }) => assert_eq!(mux.native_id, "tmux:agent3"),
        other => panic!("expected agent3 StaleMux, got {other:?}"),
    }
    let stale = diagnostics
        .iter()
        .find_map(|d| match d {
            Diagnostic::PinStaleMux {
                pin_id,
                claimed_elsewhere,
                ..
            } if pin_id == "agent3" => Some(claimed_elsewhere),
            _ => None,
        })
        .expect("agent3 stale diagnostic");
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].session.session_key, "shared");
    assert_eq!(stale[0].claimed_by_pin, "agent");
    // Only the winning pin synthesizes session links.
    let realized: Vec<&str> = snap
        .candidate_links
        .iter()
        .filter(|l| l.relation == RelationKind::PinRealizedBySession)
        .map(|l| l.id.as_str())
        .collect();
    assert_eq!(realized, ["pin-node:local_pin:agent:realized-by-session"]);
}

#[test]
fn pin_that_loses_a_shared_session_falls_back_to_its_next_candidate() {
    let mut snap = two_pins_one_session_snapshot();
    snap.nodes.push(agent_session_node(
        "claude-code",
        "own",
        "/home/me/work/repo",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "cwd-agent3",
        "own",
        "agent3",
        "exact_cwd_match",
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    match &snap.pins[1].binding {
        Some(PinBinding::Bound { session, .. }) => assert_eq!(session.session_key, "own"),
        other => panic!("expected agent3 bound to its own session, got {other:?}"),
    }
    // The session taken by `agent` is not reported as competition.
    assert!(
        !diagnostics
            .iter()
            .any(|d| matches!(d, Diagnostic::PinAmbiguous { .. })),
        "got {diagnostics:?}"
    );
}

#[test]
fn re_resolve_drops_previous_pin_links_so_a_wrong_binding_heals() {
    let mut snap = two_pins_one_session_snapshot();
    // Left over from an earlier pass that bound `shared` to agent3.
    // Its `LocalPin` provenance would outrank every discovered link.
    let mut stale = linked_to_mux(
        "pin:local_pin:agent3",
        "claude-code",
        "shared",
        "agent3",
        Provenance::LocalPin,
    );
    stale.source_metadata.adapter = "pin".to_string();
    snap.candidate_links.push(stale);

    apply_pin_bindings(&mut snap);
    let first = snap.pins.clone();
    apply_pin_bindings(&mut snap);

    assert!(matches!(
        &snap.pins[0].binding,
        Some(PinBinding::Bound { session, .. }) if session.session_key == "shared"
    ));
    assert!(matches!(
        &snap.pins[1].binding,
        Some(PinBinding::StaleMux { .. })
    ));
    assert_eq!(snap.pins, first, "second pass is a fixed point");
    assert_eq!(
        snap.candidate_links
            .iter()
            .filter(|l| l.id == "pin:local_pin:agent")
            .count(),
        1,
        "synthesized links are replaced, not duplicated"
    );
}

#[test]
fn duplicate_links_for_one_session_are_not_ambiguity() {
    let mut snap = empty_snapshot_with_pin(pin_candidate(
        "agent",
        "claude-code",
        "/home/me/work/repo",
        "agent",
        Provenance::LocalPin,
    ));
    snap.nodes.push(mux_node("agent"));
    snap.nodes.push(agent_session_node(
        "claude-code",
        "only",
        "/home/me/work/repo",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "hook",
        "only",
        "agent",
        "hook_session_path_match",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "activity",
        "only",
        "agent",
        "session_file_activity_match",
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    assert!(diagnostics.is_empty(), "got {diagnostics:?}");
}

fn mux_with_process(harness_key: Option<&str>) -> GraphSnapshot {
    use crate::model::{RuntimeProcessId, RuntimeProcessNode};
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(mux_node("agent"));
    let key = "mux_session:tmux:agent:root:1:pid:1";
    snap.nodes
        .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: RuntimeProcessId::new(key),
            observation_key: key.to_string(),
            pid: Some(1),
            parent_pid: None,
            root_pane_pid: Some(1),
            command: Some("claude".to_string()),
            cwd: None,
            harness_key: harness_key.map(str::to_string),
            role: None,
            depth: Some(0),
            observed_epoch: None,
        }));
    snap.candidate_links.push(GraphLink {
        id: "contains".to_string(),
        source: NodeId::MuxSession(MuxSessionId::new("tmux:agent")),
        target: LinkEndpoint::Node {
            id: NodeId::RuntimeProcess(RuntimeProcessId::new(key)),
        },
        relation: RelationKind::MuxContainsProcess,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    snap
}

#[test]
fn mux_hosts_harness_reads_pane_process_evidence() {
    let mux = MuxSessionId::new("tmux:agent");
    let running = mux_with_process(Some("claude-code"));
    assert!(mux_hosts_harness(&running, &mux, "claude-code"));
    assert!(!mux_hosts_harness(&running, &mux, "codex"));
    let shell = mux_with_process(None);
    assert!(!mux_hosts_harness(&shell, &mux, "claude-code"));
    let other_mux = MuxSessionId::new("tmux:elsewhere");
    assert!(!mux_hosts_harness(&running, &other_mux, "claude-code"));
}

/// Snapshot after a previous pass bound `mine` to pin `main`, where
/// the time-based evidence on `main`'s mux has since drifted to
/// `theirs`, which pin `worker` holds on equal evidence.
fn drifted_evidence_snapshot() -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    for name in ["main", "worker"] {
        snap.pins.push(pin_candidate(
            name,
            "claude-code",
            "/home/me/work/repo",
            name,
            Provenance::LocalPin,
        ));
        snap.nodes.push(mux_node(name));
    }
    for key in ["mine", "theirs"] {
        snap.nodes
            .push(agent_session_node("claude-code", key, "/home/me/work/repo"));
    }
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "hook-worker",
        "theirs",
        "worker",
        "hook_session_path_match",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "activity-main",
        "theirs",
        "main",
        "session_file_activity_match",
    ));
    let mut prior = linked_to_mux(
        "pin:local_pin:main",
        "claude-code",
        "mine",
        "main",
        Provenance::LocalPin,
    );
    prior.source_metadata.adapter = "pin".to_string();
    snap.candidate_links.push(prior);
    snap
}

fn bound_session_key(pin: &PinCandidate) -> Option<&str> {
    match &pin.binding {
        Some(PinBinding::Bound { session, .. }) => Some(session.session_key.as_str()),
        _ => None,
    }
}

#[test]
fn previous_binding_holds_when_evidence_drifts_to_a_claimed_session() {
    let mut snap = drifted_evidence_snapshot();

    let diagnostics = apply_pin_bindings(&mut snap);

    assert_eq!(bound_session_key(&snap.pins[0]), Some("mine"));
    assert_eq!(bound_session_key(&snap.pins[1]), Some("theirs"));
    assert!(
        !diagnostics
            .iter()
            .any(|d| matches!(d, Diagnostic::PinAmbiguous { .. })),
        "got {diagnostics:?}"
    );
}

#[test]
fn current_evidence_beats_the_previous_binding() {
    let mut snap = drifted_evidence_snapshot();
    snap.nodes.push(agent_session_node(
        "claude-code",
        "fresh",
        "/home/me/work/repo",
    ));
    snap.candidate_links.push(linked_to_mux_with_evidence(
        "hook-main",
        "fresh",
        "main",
        "hook_session_path_match",
    ));

    let diagnostics = apply_pin_bindings(&mut snap);

    assert_eq!(bound_session_key(&snap.pins[0]), Some("fresh"));
    // The superseded binding is history, not a competing candidate.
    assert!(
        !diagnostics
            .iter()
            .any(|d| matches!(d, Diagnostic::PinAmbiguous { .. })),
        "got {diagnostics:?}"
    );
}

#[test]
fn previous_binding_is_dropped_when_the_mux_was_recreated() {
    let mut snap = drifted_evidence_snapshot();
    for node in &mut snap.nodes {
        match node {
            GraphNode::MuxSession(mux) if mux.native_id == "main" => {
                mux.created_epoch = Some(2_000);
            }
            GraphNode::AgentSession(session) if session.id.session_key == "mine" => {
                session.last_active_epoch = Some(1_000);
            }
            _ => {}
        }
    }

    apply_pin_bindings(&mut snap);

    assert!(matches!(
        &snap.pins[0].binding,
        Some(PinBinding::StaleMux { .. })
    ));
}

#[test]
fn previous_binding_is_dropped_when_its_session_is_gone() {
    let mut snap = drifted_evidence_snapshot();
    snap.nodes
        .retain(|node| !matches!(node, GraphNode::AgentSession(s) if s.id.session_key == "mine"));

    apply_pin_bindings(&mut snap);

    assert!(matches!(
        &snap.pins[0].binding,
        Some(PinBinding::StaleMux { .. })
    ));
}
