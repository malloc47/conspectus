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
