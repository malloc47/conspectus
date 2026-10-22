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

use crate::model::{FieldValue, PinId, PinKind, PinNode, SlotFields, SlotStatus};

/// A mux slot pointing at `mux_id` with a `name` override.
fn pin_slot_for(slot_id: &str, mux_id: &MuxSessionId, name: Option<&str>) -> GraphNode {
    GraphNode::Pin(PinNode {
        id: PinId::new(slot_id),
        kind: PinKind::Mux,
        label: slot_id.to_string(),
        target: Some("history:h_1".to_string()),
        target_node: Some(crate::model::NodeId::MuxSession(mux_id.clone())),
        status: SlotStatus::Live,
        overrides: name
            .map(|name| SlotFields::from([("name".to_string(), FieldValue::Text(name.into()))]))
            .unwrap_or_default(),
        snapshot: SlotFields::new(),
        effective: Vec::new(),
        order: 0,
        legacy: false,
        pinned_at: None,
    })
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
    assert!(plan.pin_slot_updates.is_empty());
}

#[test]
fn plan_mux_rename_updates_the_name_override_of_slots_pointing_at_the_mux() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot
        .nodes
        .push(pin_slot_for("ingest-a", &mux_id, Some("editor")));
    snapshot
        .nodes
        .push(pin_slot_for("ingest-b", &mux_id, Some("editor")));
    // No name override: the record follows the rename by itself.
    snapshot.nodes.push(pin_slot_for("plain", &mux_id, None));
    // Another name: not this mux's.
    snapshot
        .nodes
        .push(pin_slot_for("other", &mux_id, Some("elsewhere")));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");
    let updates: Vec<_> = plan
        .pin_slot_updates
        .iter()
        .map(|u| (u.slot_id.as_str(), u.new_name.as_str()))
        .collect();
    assert_eq!(
        updates,
        [("ingest-a", "workshop"), ("ingest-b", "workshop")]
    );
}

#[test]
fn plan_mux_rename_ignores_slots_pointing_at_a_different_mux() {
    let mut snapshot = GraphSnapshot::empty();
    let m = mux("editor");
    let mux_id = m.id.clone();
    let other = MuxSessionId::new("tmux:other");
    snapshot.nodes.push(GraphNode::MuxSession(m));
    snapshot
        .nodes
        .push(pin_slot_for("theirs", &other, Some("editor")));

    let plan = plan_mux_rename(&snapshot, &mux_id, "workshop".to_string()).expect("plan");
    assert!(plan.pin_slot_updates.is_empty());
}

#[test]
fn applying_slot_updates_changes_the_override_and_reports_failures() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::pin_slots::SlotStore::at(dir.path().join("pins.toml"));
    store
        .add(crate::model::PinSlot {
            id: "ingest".into(),
            kind: PinKind::Mux,
            target: None,
            pinned_at: None,
            overrides: SlotFields::from([("name".to_string(), FieldValue::Text("editor".into()))]),
            snapshot: SlotFields::new(),
            order: 0,
            legacy: false,
        })
        .unwrap();
    let updates = [
        PinSlotNameUpdate {
            slot_id: "ingest".into(),
            new_name: "workshop".into(),
        },
        PinSlotNameUpdate {
            slot_id: "absent".into(),
            new_name: "workshop".into(),
        },
    ];
    let results = apply_pin_slot_updates(&store, &updates);
    assert_eq!(results[0], ("ingest".to_string(), Ok(true)));
    assert!(
        results[1]
            .1
            .as_ref()
            .unwrap_err()
            .contains("no pin slot named `absent`")
    );
    assert_eq!(
        store.load().slots[0].overrides["name"],
        FieldValue::Text("workshop".into())
    );
    // Again: nothing left to change.
    assert_eq!(
        apply_pin_slot_updates(&store, &updates[..1])[0].1,
        Ok(false)
    );
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

/// A mux node shaped the way discovery emits it: the id carries the
/// backend prefix, `native_id` does not.
fn discovered_mux(backend: &str, name: &str) -> MuxSessionNode {
    MuxSessionNode::new(
        MuxSessionId::new(format!("{backend}:{name}")),
        backend.to_string(),
        name.to_string(),
    )
}

#[test]
fn lockstep_rename_targets_the_bare_native_name_on_the_muxs_backend() {
    for backend in ["tmux", "zellij"] {
        let mut snapshot = GraphSnapshot::empty();
        let node = session("codex", "alpha");
        let id = node.id.clone();
        let mux_node = discovered_mux(backend, "editor");
        let mux_id = mux_node.id.clone();
        snapshot.nodes.push(GraphNode::AgentSession(node));
        snapshot.nodes.push(GraphNode::MuxSession(mux_node));
        snapshot
            .candidate_links
            .push(linked_to_mux("link", &id, &mux_id));

        let plan =
            plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");
        let rename = plan.mux_native_rename.expect("lockstep rename");

        assert_eq!(rename.mux, mux_id);
        assert_eq!(rename.backend, backend);
        assert_eq!(
            rename.target, "editor",
            "never the prefixed `{backend}:editor`"
        );
        assert_eq!(rename.socket, None);
    }
}

#[test]
fn lockstep_rename_skips_a_mux_discovery_no_longer_sees() {
    let mut snapshot = GraphSnapshot::empty();
    let node = session("codex", "alpha");
    let id = node.id.clone();
    snapshot.nodes.push(GraphNode::AgentSession(node));
    snapshot
        .candidate_links
        .push(linked_to_mux("link", &id, &MuxSessionId::new("tmux:gone")));

    let plan =
        plan_session_rename(&snapshot, &id, Some("ingest".to_string()), false).expect("plan");

    assert_eq!(
        plan.agent_alias_write.display_name.as_deref(),
        Some("ingest")
    );
    assert!(plan.mux_native_rename.is_none());
}

#[test]
fn plan_mux_rename_carries_the_muxs_backend_and_bare_name() {
    let mut snapshot = GraphSnapshot::empty();
    let mux_node = discovered_mux("zellij", "notes");
    let mux_id = mux_node.id.clone();
    snapshot.nodes.push(GraphNode::MuxSession(mux_node));

    let plan = plan_mux_rename(&snapshot, &mux_id, "notes-2".to_string()).expect("plan");

    assert_eq!(plan.mux_rename.backend, "zellij");
    assert_eq!(plan.mux_rename.target, "notes");
}
