// Extracted from rename.rs H-HYG-011 rolling wave via #[path = "rename_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionNode, Confidence, Freshness, GraphNode, MuxSessionNode, Provenance, SourceMetadata,
};

fn session(harness: &str, key: &str) -> AgentSessionNode {
    AgentSessionNode::new(
        AgentSessionId::new(harness, "/state", key),
        harness.to_string(),
    )
}

fn mux(name: &str) -> MuxSessionNode {
    MuxSessionNode::new(
        MuxSessionId::new(name),
        "tmux".to_string(),
        name.to_string(),
    )
}

fn linked_to_mux(id: &str, source: &AgentSessionId, target: &MuxSessionId) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::AgentSession(source.clone()),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(target.clone()),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn rename_with_no_mux_link_writes_alias_only() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    snapshot.nodes.push(GraphNode::AgentSession(node));

    let plan =
        plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");

    assert_eq!(plan.agent_alias_write.session, id);
    assert_eq!(
        plan.agent_alias_write.display_name.as_deref(),
        Some("ingest")
    );
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn rename_with_single_mux_link_renames_both() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_node = mux("editor");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link-1", &id, &mux_node.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan =
        plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");

    let mux_rename = plan.mux_native_rename.expect("mux rename");
    assert_eq!(mux_rename.mux.native_id, "editor");
    assert_eq!(mux_rename.new_name, "ingest");
}

#[test]
fn ambiguous_mux_refuses_lockstep() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_a = mux("editor");
    let mux_b = mux("shell");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link-a", &id, &mux_a.id));
    snapshot
        .candidate_links
        .push(linked_to_mux("link-b", &id, &mux_b.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_a));
    snapshot.nodes.push(GraphNode::MuxSession(mux_b));

    let err = plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false)
        .expect_err("ambiguous");
    assert_eq!(err, RenamePlanError::AmbiguousMux { candidate_count: 2 });
}

#[test]
fn ambiguous_mux_with_no_mux_flag_skips_mux_rename() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_a = mux("editor");
    let mux_b = mux("shell");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link-a", &id, &mux_a.id));
    snapshot
        .candidate_links
        .push(linked_to_mux("link-b", &id, &mux_b.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_a));
    snapshot.nodes.push(GraphNode::MuxSession(mux_b));

    let plan = plan_session_rename(&snapshot, &id, Some("ingest".to_string()), true).expect("plan");
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn no_mux_flag_suppresses_lockstep_even_with_single_candidate() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_node = mux("editor");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link", &id, &mux_node.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan = plan_session_rename(&snapshot, &id, Some("ingest".to_string()), true).expect("plan");
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn clearing_alias_skips_mux_rename_even_with_single_candidate() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_node = mux("editor");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link", &id, &mux_node.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan = plan_session_rename(&snapshot, &id, None, false).expect("plan");
    assert!(plan.agent_alias_write.display_name.is_none());
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn duplicate_linked_to_mux_candidates_collapse_to_one_mux() {
    // Multiple evidence rows pointing at the same mux are
    // resolved-single from the operator's perspective; they
    // should not trigger ambiguity.
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_node = mux("editor");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("evidence-1", &id, &mux_node.id));
    snapshot
        .candidate_links
        .push(linked_to_mux("evidence-2", &id, &mux_node.id));
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan =
        plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");
    let mux_rename = plan.mux_native_rename.expect("mux rename");
    assert_eq!(mux_rename.mux.native_id, "editor");
}

#[test]
fn ignored_linked_to_mux_candidates_are_skipped() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    let mux_node = mux("editor");
    snapshot.nodes.push(GraphNode::AgentSession(node));
    let mut link = linked_to_mux("ignored", &id, &mux_node.id);
    link.state = LinkState::Ignored { reason: None };
    snapshot.candidate_links.push(link);
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan =
        plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn empty_display_name_is_rejected() {
    let snapshot = GraphSnapshot::empty();
    let id = AgentSessionId::new("codex", "/state", "alpha");
    let err =
        plan_session_rename(&snapshot, &id, Some("   ".to_string()), false).expect_err("empty");
    assert_eq!(err, RenamePlanError::EmptyDisplayName);
}

// --- H-RENAME-MUX: graph-aware mux rename tests -----------------

use crate::model::PinCandidate;

fn pin_bound_to(pin_id: &str, mux_name: &str, socket: Option<&str>) -> PinCandidate {
    PinCandidate {
        id: pin_id.to_string(),
        display_name: pin_id.to_string(),
        harness: "codex".to_string(),
        cwd: "/workspace".to_string(),
        mux: crate::model::PinMuxRef {
            backend: "tmux".to_string(),
            name: mux_name.to_string(),
            socket_name: socket.map(str::to_string),
        },
        launch_argv: None,
        reason: None,
        provenance: crate::model::Provenance::LocalPin,
        store_path: format!("/workspace/.conspectus.toml.{pin_id}"),
        binding: None,
    }
}

#[test]
fn plan_mux_rename_with_no_pins_writes_only_the_mux() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");

    assert_eq!(plan.mux_rename.mux, mux_id);
    assert_eq!(plan.mux_rename.new_name, "workshop");
    assert!(plan.pin_mux_name_updates.is_empty());
}

#[test]
fn plan_mux_rename_cascades_to_matching_pin() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot.pins.push(pin_bound_to("ingest", "editor", None));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");

    assert_eq!(plan.pin_mux_name_updates.len(), 1);
    let update = &plan.pin_mux_name_updates[0];
    assert_eq!(update.pin_id, "ingest");
    assert_eq!(update.new_mux_name, "workshop");
    assert_eq!(update.store_path, "/workspace/.conspectus.toml.ingest");
}

#[test]
fn plan_mux_rename_ignores_pins_pointed_at_a_different_mux() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot.pins.push(pin_bound_to("other", "workshop", None));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");
    assert!(plan.pin_mux_name_updates.is_empty());
}

#[test]
fn plan_mux_rename_cascades_to_every_matching_pin() {
    // ADR 0057 discourages multiple pins on the same mux but the
    // plan handles it gracefully — every pin whose mux triple
    // matches picks up an update entry.
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot.pins.push(pin_bound_to("ingest-a", "editor", None));
    snapshot.pins.push(pin_bound_to("ingest-b", "editor", None));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");
    let ids: Vec<_> = plan
        .pin_mux_name_updates
        .iter()
        .map(|u| u.pin_id.clone())
        .collect();
    assert_eq!(ids, vec!["ingest-a".to_string(), "ingest-b".to_string()]);
}

#[test]
fn plan_mux_rename_rejects_empty_name() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));

    let err =
        plan_mux_rename(&snapshot, &mux_id, "   ".to_string()).expect_err("expected empty error");
    assert_eq!(err, MuxRenamePlanError::EmptyName);
}

#[test]
fn plan_mux_rename_rejects_noop() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));

    let err = plan_mux_rename(&snapshot, &mux_id, "editor".to_string()).expect_err("expected noop");
    assert_eq!(err, MuxRenamePlanError::NoOp);
}

#[test]
fn plan_mux_rename_rejects_missing_mux() {
    let snapshot = GraphSnapshot::empty();
    let ghost = MuxSessionId::new("phantom");

    let err =
        plan_mux_rename(&snapshot, &ghost, "workshop".to_string()).expect_err("expected not-found");
    assert_eq!(err, MuxRenamePlanError::MuxNotFound);
}

#[test]
fn plan_mux_rename_socket_mismatch_skips_pin() {
    // Pin declares socket="scratch"; mux is a default-socket
    // (parsed socket = None). The socket-effective comparison
    // guards against cross-socket cascade.
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot
        .pins
        .push(pin_bound_to("ingest", "editor", Some("scratch")));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");
    assert!(
        plan.pin_mux_name_updates.is_empty(),
        "pin on a different socket must not cascade"
    );
}
