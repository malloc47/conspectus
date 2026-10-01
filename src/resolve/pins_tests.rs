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
