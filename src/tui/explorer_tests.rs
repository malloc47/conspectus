use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, ForgePrId, ForgePrNode,
    GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId,
    RepoNode, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SourceMetadata,
    UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
};
use crate::resolve::resolve_snapshot;
use std::path::PathBuf;

#[test]
fn directional_verb_catalog_is_exhaustive_and_disambiguates_inverses() {
    // ADR 0074 §2: every RelationKind has a verb pair, and for
    // every asymmetric relation the two verbs differ. The
    // exhaustive match in `directional_verb` keeps this catalog
    // honest at compile time; the assertions below pin a few
    // anchor cases so a future verb rewrite doesn't accidentally
    // collapse the direction signal.
    let pairs = [
        (RelationKind::WorkspaceContainsRepo, "contains", "member of"),
        (RelationKind::LinkedToMux, "attached to", "attached session"),
        (RelationKind::ParentFork, "forked from", "forked by"),
        (RelationKind::ChildSession, "child of", "parent of"),
        (RelationKind::ParentSession, "parent of", "child of"),
        (
            RelationKind::MuxContainsProcess,
            "contains process",
            "in mux",
        ),
        (RelationKind::BranchHasForgePr, "has PR", "for branch"),
    ];
    for (rel, out, inn) in pairs {
        assert_eq!(directional_verb(&rel, Direction::Downstream), out);
        assert_eq!(directional_verb(&rel, Direction::Upstream), inn);
        assert_ne!(
            directional_verb(&rel, Direction::Downstream),
            directional_verb(&rel, Direction::Upstream),
            "asymmetric relation {rel:?} should have distinct verbs per direction",
        );
    }
    // `AssociatedWith` is the documented symmetric relation; the
    // ADR explicitly accepts the same verb in both directions
    // until a real operator confusion materializes.
    assert_eq!(
        directional_verb(&RelationKind::AssociatedWith, Direction::Downstream),
        directional_verb(&RelationKind::AssociatedWith, Direction::Upstream),
    );
}

#[test]
fn flat_rows_splits_validated_and_other_zones() {
    // ADR 0074 §3: the validated zone holds every `Resolves` row
    // in a flat list; alternates, conflicts, and unresolved
    // stubs sit under one collapsible `Other` header below.
    // This test exercises the split via a multi-candidate
    // LinkedToMux setup: the resolver picks one winner, the
    // other candidate becomes an Other row.
    let mut snapshot = GraphSnapshot::empty();
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
    snapshot
        .nodes
        .push(agent("claude", "abc", Some("/x"), None));
    snapshot.nodes.push(mux("tmux", "primary", Some("/x")));
    snapshot.nodes.push(mux("tmux", "secondary", Some("/x")));
    let cwd_link = |id: &str, target: NodeId| {
        let mut l = link(id, session_id.clone(), target, RelationKind::LinkedToMux);
        l.confidence = Confidence::High;
        l.source_metadata.evidence = Some("exact_cwd_match".to_string());
        l
    };
    snapshot.candidate_links.push(cwd_link(
        "to-primary",
        NodeId::MuxSession(MuxSessionId::new("primary")),
    ));
    snapshot.candidate_links.push(cwd_link(
        "to-secondary",
        NodeId::MuxSession(MuxSessionId::new("secondary")),
    ));
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &session_id, Some(home().as_path()));
    let counts = view.relationship_counts();
    assert!(
        counts.validated >= 1,
        "at least one resolver winner expected: {counts:?}"
    );
    assert!(
        counts.other >= 1,
        "non-winning candidate should land in the Other zone: {counts:?}"
    );

    // Collapsed Other: only the validated rows + the header
    // appear; expanding adds the Other children.
    let collapsed = view.flat_rows(false, false);
    let validated_in_collapsed = collapsed
        .iter()
        .filter(|r| matches!(r, ExplorerRow::ValidatedLink { .. }))
        .count();
    let other_header_in_collapsed = collapsed
        .iter()
        .filter(|r| matches!(r, ExplorerRow::OtherHeader { .. }))
        .count();
    let other_children_in_collapsed = collapsed
        .iter()
        .filter(|r| {
            matches!(
                r,
                ExplorerRow::OtherLink { .. } | ExplorerRow::OtherUnresolved { .. }
            )
        })
        .count();
    assert_eq!(validated_in_collapsed, counts.validated);
    assert_eq!(other_header_in_collapsed, 1);
    assert_eq!(
        other_children_in_collapsed, 0,
        "Other children should not appear while the zone is collapsed"
    );

    let expanded = view.flat_rows(true, false);
    let other_children_in_expanded = expanded
        .iter()
        .filter(|r| {
            matches!(
                r,
                ExplorerRow::OtherLink { .. } | ExplorerRow::OtherUnresolved { .. }
            )
        })
        .count();
    assert_eq!(other_children_in_expanded, counts.other);
}

#[test]
fn relationship_groups_sort_by_kind_then_verb_then_label() {
    // ADR 0074 §4: rows in the Related zone scan top-to-bottom
    // as kind → verb → neighbor label. The merged-group sort in
    // `sort_relationship_groups` is the source of truth; this
    // test pins the contract by checking the kind ordinals are
    // monotonically non-decreasing across the merged list.
    let mut snapshot = GraphSnapshot::empty();
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
    let workspace_id = NodeId::Workspace(WorkspaceId::new("/ws"));
    let repo_id = NodeId::Repo(RepoId::new("/r/.git"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    snapshot
        .nodes
        .push(agent("claude", "abc", Some("/r"), None));
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/ws"),
        root: "/ws".to_string(),
        provider: None,
        name: None,
    }));
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))));
    snapshot.nodes.push(mux("tmux", "editor", Some("/r")));
    snapshot.candidate_links.push(link(
        "to-workspace",
        session_id.clone(),
        workspace_id,
        RelationKind::AssociatedWith,
    ));
    snapshot.candidate_links.push(link(
        "to-repo",
        session_id.clone(),
        repo_id,
        RelationKind::AssociatedWith,
    ));
    snapshot.candidate_links.push(link(
        "to-mux",
        session_id.clone(),
        mux_id,
        RelationKind::LinkedToMux,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &session_id, Some(home().as_path()));
    let ordinals: Vec<usize> = view
        .relationships
        .groups
        .iter()
        .map(|g| {
            crate::tui::icons::NodeKind::from_snake_case(&g.neighbor_kind)
                .map_or(usize::MAX, super::super::icons::NodeKind::ordinal)
        })
        .collect();
    for pair in ordinals.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "kind ordinals should not decrease across the sorted group list: {ordinals:?}"
        );
    }
    // Workspace (ordinal 0) sorts before Repo (1) sorts before
    // MuxSession (4).
    let kinds: Vec<&str> = view
        .relationships
        .groups
        .iter()
        .map(|g| g.neighbor_kind.as_str())
        .collect();
    let ws = kinds.iter().position(|k| *k == "workspace");
    let repo = kinds.iter().position(|k| *k == "repo");
    let mux = kinds.iter().position(|k| *k == "mux_session");
    if let (Some(ws), Some(repo)) = (ws, repo) {
        assert!(ws < repo, "workspace should sort before repo: {kinds:?}");
    }
    if let (Some(repo), Some(mux)) = (repo, mux) {
        assert!(repo < mux, "repo should sort before mux: {kinds:?}");
    }
}

#[test]
fn build_node_view_merges_directions_into_single_relationships() {
    // ADR 0074 pass 1: the data shape collapses to one
    // `relationships` field on NodeView, with each group
    // carrying its own direction. The `upstream()` /
    // `downstream()` helpers project filtered views off the
    // combined list for transitional consumers (passes 2 / 3
    // remove these helpers entirely). This test pins both
    // halves to prove the collapse + projection round-trip.
    let mut snapshot = GraphSnapshot::empty();
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
    snapshot
        .nodes
        .push(agent("claude", "abc", Some("/x"), None));
    snapshot.nodes.push(mux("tmux", "editor", Some("/x")));
    snapshot.candidate_links.push(link(
        "session->mux",
        session_id.clone(),
        NodeId::MuxSession(MuxSessionId::new("editor")),
        RelationKind::LinkedToMux,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &session_id, Some(home().as_path()));
    // Combined list carries the LinkedToMux group with
    // direction marked as Downstream (focus = source).
    assert_eq!(view.relationships.groups.len(), 1);
    assert_eq!(
        view.relationships.groups[0].direction,
        Direction::Downstream
    );
    assert_eq!(
        view.relationships.groups[0].relation,
        RelationKind::LinkedToMux
    );

    // Direction filtering on the combined list still works.
    let upstream_count = view
        .relationships
        .groups
        .iter()
        .filter(|g| g.direction == Direction::Upstream)
        .count();
    let downstream_count = view
        .relationships
        .groups
        .iter()
        .filter(|g| g.direction == Direction::Downstream)
        .count();
    assert_eq!(upstream_count, 0);
    assert_eq!(downstream_count, 1);
}

fn home() -> PathBuf {
    PathBuf::from("/home/op")
}

/// Test helper that re-projects the combined `relationships`
/// list back to a single-direction `RelationshipExplorer` so
/// the legacy upstream/downstream assertions in this file keep
/// reading naturally without recomputing the filter at every
/// call site.
fn filter_direction(view: &NodeView, direction: Direction) -> RelationshipExplorer {
    RelationshipExplorer {
        groups: view
            .relationships
            .groups
            .iter()
            .filter(|g| g.direction == direction)
            .cloned()
            .collect(),
    }
}

fn agent(harness: &str, key: &str, cwd: Option<&str>, title: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, "/state", key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: title.map(str::to_string),
        last_message_preview: None,
        last_active_epoch: Some(1_700_000_000),
        session_kind: None,
    })
}

fn mux(backend: &str, native: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(native),
        backend: backend.to_string(),
        native_id: native.to_string(),
        cwd: cwd.map(str::to_string),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: Some(true),
        activity_epoch: Some(1_700_000_005),
        created_epoch: Some(1_699_000_000),
        last_attached_epoch: None,
    })
}

fn process(observation_key: &str, pid: i64, command: &str) -> GraphNode {
    GraphNode::RuntimeProcess(RuntimeProcessNode {
        id: RuntimeProcessId::new(observation_key),
        observation_key: observation_key.to_string(),
        pid: Some(pid),
        parent_pid: Some(1),
        root_pane_pid: Some(pid),
        command: Some(command.to_string()),
        cwd: Some("/home/op/src/x".to_string()),
        harness_key: Some("claude-code".to_string()),
        role: Some(RuntimeProcessRole::HumanAgent),
        depth: Some(0),
        observed_epoch: Some(1_700_000_002),
    })
}

fn link(id: &str, source: NodeId, target: NodeId, relation: RelationKind) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn unresolved_link(
    id: &str,
    source: NodeId,
    evidence: UnresolvedEndpoint,
    relation: RelationKind,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source,
        target: LinkEndpoint::Unresolved { evidence },
        relation,
        provenance: Provenance::Discovered,
        confidence: Confidence::Low,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

/// Fixed clock for explorer views: one hour after the fixtures'
/// `1_700_000_000` session activity.
const TEST_NOW: i64 = 1_700_003_600;

fn build(snapshot: &GraphSnapshot, target: &NodeId, home: Option<&Path>) -> NodeView {
    build_node_view(ExplorerInputs {
        snapshot,
        target,
        home,
        now: Some(TEST_NOW),
    })
    .expect("view exists")
}

#[test]
fn unknown_node_returns_none() {
    let snapshot = GraphSnapshot::empty();
    let phantom = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "nope"));
    assert!(
        build_node_view(ExplorerInputs {
            snapshot: &snapshot,
            target: &phantom,
            home: None,
            now: Some(TEST_NOW),
        })
        .is_none()
    );
}

#[test]
fn sparse_agent_session_renders_core_and_empty_explorers() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));

    let view = build(&snapshot, &target, Some(home().as_path()));
    assert_eq!(view.kind, crate::model::NodeKind::AgentSession);
    assert_eq!(view.title_line, "claude-code:abc");
    let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
    assert_eq!(labels, vec!["id", "harness", "alias", "cwd", "status"]);
    let id = view
        .core_fields
        .iter()
        .find(|f| f.label == "id")
        .expect("id field");
    assert_eq!(id.value, "abc");
    assert!(view.relationships.groups.is_empty());
}

#[test]
fn agent_session_core_id_shows_full_external_session_key() {
    let long_id = "ffffffff-1111-2222-3333-444444444444";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("opencode", long_id, Some("/home/op/src/x"), None));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", long_id));

    let view = build(&snapshot, &target, Some(home().as_path()));
    let id = view
        .core_fields
        .iter()
        .find(|f| f.label == "id")
        .expect("id field");
    assert_eq!(id.value, long_id);
}

#[test]
fn agent_session_title_is_not_labeled_as_alias_in_core_fields() {
    let long_title = "The conspectus TUI, fashioned after a long prompt, should remain a title";
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent(
        "codex",
        "abc",
        Some("/home/op/src/x"),
        Some(long_title),
    ));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

    let view = build(&snapshot, &target, Some(home().as_path()));
    let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
    assert_eq!(labels, vec!["id", "harness", "title", "cwd", "status"]);
    let title = view
        .core_fields
        .iter()
        .find(|f| f.label == "title")
        .expect("title field");
    assert!(title.value.contains('…'));
    assert_eq!(title.long_value.as_deref(), Some(long_title));
}

#[test]
fn agent_session_groups_link_to_mux_downstream() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
    snapshot.nodes.push(mux("tmux", "work-claude", None));
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("work-claude"));
    snapshot.candidate_links.push(link(
        "l1",
        session_id.clone(),
        mux_id,
        RelationKind::LinkedToMux,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let view = build(&snapshot, &session_id, Some(home().as_path()));
    let downstream = filter_direction(&view, Direction::Downstream);
    assert_eq!(downstream.groups.len(), 1);
    let group = &downstream.groups[0];
    assert_eq!(group.relation, RelationKind::LinkedToMux);
    assert_eq!(group.neighbor_kind, "mux_session");
    assert!(group.is_single());
    assert_eq!(group.links.len(), 1);
    assert!(group.links[0].resolved_winner);
    assert_eq!(group.links[0].edge_state, EdgeStateLabel::Resolves);
    // Preview carries mux core fields. The `id` field shows the
    // external mux name and `backend` carries the backend label
    // (e.g. `tmux`), mirroring the agent-session detail layout.
    let labels: Vec<&str> = group.links[0].preview.iter().map(|f| f.label).collect();
    assert_eq!(
        labels,
        vec!["id", "backend", "cwd", "attached", "last_active"]
    );
    let id_field = group.links[0]
        .preview
        .iter()
        .find(|f| f.label == "id")
        .expect("id field");
    let backend_field = group.links[0]
        .preview
        .iter()
        .find(|f| f.label == "backend")
        .expect("backend field");
    assert_eq!(id_field.value, "work-claude");
    assert_eq!(backend_field.value, "tmux");
}

#[test]
fn linked_to_mux_suppressed_slot_surfaces_as_no_winner_ambiguous_group() {
    // H-UI-006 (ADR 0077) retires the H-UI-007 candidate-fan-out
    // fallback: the resolver now preserves the suppressed
    // `LinkedToMux` slot with `selected_link_id = None` and the
    // candidate set rolled into `competing_link_ids`. The
    // explorer reads ambiguity directly off the slot now —
    // every candidate row drops into the Other zone, the group
    // is marked ambiguous, and no validated row exists.
    let mut snapshot = GraphSnapshot::empty();
    let cwd = Some("/home/op/src/x");
    snapshot
        .nodes
        .push(agent("claude-code", "focused", cwd, None));
    snapshot
        .nodes
        .push(agent("claude-code", "other", cwd, None));
    snapshot.nodes.push(mux("tmux", "project", cwd));
    snapshot.nodes.push(mux("tmux", "ambiguous", cwd));

    let focused_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "focused"));
    let other_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "other"));
    let mux_a = NodeId::MuxSession(MuxSessionId::new("project"));
    let mux_b = NodeId::MuxSession(MuxSessionId::new("ambiguous"));

    let cwd_link = |id: &str, source: NodeId, target: NodeId| {
        let mut l = link(id, source, target, RelationKind::LinkedToMux);
        // `suppress_ambiguous_cwd_mux_links` keys off
        // `match_kind == exact_cwd_match` to recognize cwd
        // evidence; both `fields` and `evidence` are checked.
        l.source_metadata.evidence = Some("exact_cwd_match".to_string());
        l
    };

    snapshot
        .candidate_links
        .push(cwd_link("focused-a", focused_id.clone(), mux_a.clone()));
    snapshot
        .candidate_links
        .push(cwd_link("focused-b", focused_id.clone(), mux_b.clone()));
    snapshot
        .candidate_links
        .push(cwd_link("other-a", other_id.clone(), mux_a));
    snapshot
        .candidate_links
        .push(cwd_link("other-b", other_id, mux_b));

    let snapshot = resolve_snapshot(snapshot);

    // Precondition (ADR 0077): the resolver preserves the
    // `LinkedToMux` slot for the focused session but flips
    // `selected_link_id` to `None`. The slot survives so
    // downstream consumers can read ambiguity off the model.
    let focused_mux_slot = snapshot
        .resolved_relationships
        .iter()
        .find(|r| r.source == focused_id && r.relation == RelationKind::LinkedToMux)
        .expect("suppression preserves the LinkedToMux slot for the focused session");
    assert!(
        focused_mux_slot.selected_link_id.is_none(),
        "suppressed slot must carry no winner: {focused_mux_slot:?}",
    );
    assert!(
        focused_mux_slot.competing_link_ids.len() >= 2,
        "the candidate set rolls into competing_link_ids: {focused_mux_slot:?}",
    );

    let view = build(&snapshot, &focused_id, Some(home().as_path()));
    let group = filter_direction(&view, Direction::Downstream)
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::LinkedToMux)
        .expect("LinkedToMux group should render against the preserved slot")
        .clone();
    assert!(
        group.ambiguous,
        "no-winner slot must mark the group ambiguous",
    );
    assert!(
        group.links.len() >= 2,
        "both candidate targets should still appear as rows: {:?}",
        group.links,
    );
    assert!(
        group
            .links
            .iter()
            .all(|l| !matches!(l.edge_state, EdgeStateLabel::Resolves)),
        "no candidate is a winner inside a no-winner slot: {:?}",
        group.links,
    );
}

#[test]
fn multi_process_candidates_flag_ambiguity() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
    snapshot
        .nodes
        .push(process("obs:1", 100, "/usr/bin/claude"));
    snapshot
        .nodes
        .push(process("obs:2", 200, "/usr/bin/claude-sub"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let proc1 = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:1"));
    let proc2 = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:2"));
    let mut high = link(
        "p1",
        proc1,
        session_id.clone(),
        RelationKind::ProcessIdentifiesSession,
    );
    high.provenance = Provenance::StrongDiscovered;
    let mut low = link(
        "p2",
        proc2,
        session_id.clone(),
        RelationKind::ProcessCandidatesSession,
    );
    low.provenance = Provenance::Discovered;
    low.confidence = Confidence::Medium;
    snapshot.candidate_links.push(high);
    snapshot.candidate_links.push(low);
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &session_id, Some(home().as_path()));
    // Both groups are upstream because processes point at sessions.
    let upstream = filter_direction(&view, Direction::Upstream);
    assert_eq!(upstream.groups.len(), 2);
    let identifies = upstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::ProcessIdentifiesSession)
        .expect("identifies group");
    assert!(identifies.is_single());
    assert!(identifies.links[0].resolved_winner);
    assert_eq!(identifies.links[0].edge_state, EdgeStateLabel::Resolves);

    let candidates = upstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::ProcessCandidatesSession)
        .expect("candidates group");
    assert!(candidates.is_single());
    // `process_candidates` shows a runner-up — its slot is its own
    // resolved relationship since nothing else competes for it.
    assert_eq!(candidates.links[0].edge_state, EdgeStateLabel::Resolves);
}

#[test]
fn workspace_contains_multiple_repos_all_validated() {
    // Regression: the resolver keys `WorkspaceContainsRepo` by
    // `(source, relation, target)` so a workspace with three
    // distinct repo edges yields three independent winners.
    // The detail-pane explorer used to collapse them under a
    // single `resolved_for` lookup, leaving only one repo in the
    // validated zone and demoting the other two to the `Other`
    // chevron. Every per-target winner must land in the
    // validated zone with `EdgeStateLabel::Resolves`.
    let mut snapshot = GraphSnapshot::empty();
    let workspace_id = WorkspaceId::new("/home/op/work/multi");
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id.clone(),
        root: "/home/op/work/multi".to_string(),
        provider: None,
        name: Some("multi".to_string()),
    }));
    let repos = ["/srv/git/a.git", "/srv/git/b.git", "/srv/git/c.git"];
    for common_dir in repos {
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: RepoId::new(common_dir),
            common_dir: common_dir.to_string(),
            source_paths: Vec::new(),
            remotes: Vec::new(),
        }));
    }
    let workspace = NodeId::Workspace(workspace_id);
    for (idx, common_dir) in repos.iter().enumerate() {
        snapshot.candidate_links.push(link(
            &format!("l{idx}"),
            workspace.clone(),
            NodeId::Repo(RepoId::new(*common_dir)),
            RelationKind::WorkspaceContainsRepo,
        ));
    }
    let snapshot = resolve_snapshot(snapshot);

    // Sanity: the resolver emitted one slot per repo.
    let workspace_slots = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| r.source == workspace && r.relation == RelationKind::WorkspaceContainsRepo)
        .count();
    assert_eq!(
        workspace_slots, 3,
        "resolver should emit one WorkspaceContainsRepo slot per target repo",
    );

    let view = build(&snapshot, &workspace, Some(home().as_path()));
    let downstream = filter_direction(&view, Direction::Downstream);
    let group = downstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::WorkspaceContainsRepo)
        .expect("WorkspaceContainsRepo group");
    assert_eq!(group.links.len(), 3);
    for row in &group.links {
        assert!(
            row.resolved_winner,
            "every per-target winner must surface as resolved: {row:?}"
        );
        assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
    }
    assert!(
        !view.has_other_rows(),
        "no candidate should fall into the Other zone when every slot has a winner",
    );
    assert!(
        !group.ambiguous,
        "distinct per-target winners are not ambiguous",
    );
    let counts = view.relationship_counts();
    assert_eq!(counts.validated, 3);
    assert_eq!(counts.other, 0);
}

#[test]
fn workspace_associated_with_multiple_repos_all_validated() {
    // Symmetric coverage for `AssociatedWith`: a workspace that
    // declares an association with several repos should show
    // every repo as validated rather than demoting all but one
    // to `Other`. `AssociatedWith` is also part of the
    // `multi_target_relation` set on the resolver side.
    let mut snapshot = GraphSnapshot::empty();
    let workspace_id = WorkspaceId::new("/home/op/work/assoc");
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id.clone(),
        root: "/home/op/work/assoc".to_string(),
        provider: None,
        name: Some("assoc".to_string()),
    }));
    let repos = ["/srv/git/x.git", "/srv/git/y.git"];
    for common_dir in repos {
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: RepoId::new(common_dir),
            common_dir: common_dir.to_string(),
            source_paths: Vec::new(),
            remotes: Vec::new(),
        }));
    }
    let workspace = NodeId::Workspace(workspace_id);
    for (idx, common_dir) in repos.iter().enumerate() {
        snapshot.candidate_links.push(link(
            &format!("a{idx}"),
            workspace.clone(),
            NodeId::Repo(RepoId::new(*common_dir)),
            RelationKind::AssociatedWith,
        ));
    }
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &workspace, Some(home().as_path()));
    let downstream = filter_direction(&view, Direction::Downstream);
    let group = downstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::AssociatedWith)
        .expect("AssociatedWith downstream group");
    assert_eq!(group.links.len(), 2);
    for row in &group.links {
        assert!(row.resolved_winner);
        assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
    }
    assert!(!view.has_other_rows());
}

#[test]
fn repo_associated_with_multiple_workspaces_all_validated() {
    // Mirror of the bug report from the workspace's vantage:
    // when a single repo participates in several workspaces via
    // `AssociatedWith`, focusing the *repo* should surface every
    // workspace as validated (upstream direction). Each
    // `(workspace, AssociatedWith, repo)` slot is owned by its
    // workspace, so resolved_relationships contain three
    // independent winners, all of which the repo's detail pane
    // observes upstream.
    let mut snapshot = GraphSnapshot::empty();
    let repo_id = RepoId::new("/srv/git/shared.git");
    snapshot.nodes.push(GraphNode::Repo(RepoNode {
        id: repo_id.clone(),
        common_dir: "/srv/git/shared.git".to_string(),
        source_paths: Vec::new(),
        remotes: Vec::new(),
    }));
    let workspace_roots = [
        "/home/op/work/one",
        "/home/op/work/two",
        "/home/op/work/three",
    ];
    for root in workspace_roots {
        snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: None,
            name: None,
        }));
    }
    let repo = NodeId::Repo(repo_id);
    for (idx, root) in workspace_roots.iter().enumerate() {
        snapshot.candidate_links.push(link(
            &format!("w{idx}"),
            NodeId::Workspace(WorkspaceId::new(*root)),
            repo.clone(),
            RelationKind::AssociatedWith,
        ));
    }
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &repo, Some(home().as_path()));
    let upstream = filter_direction(&view, Direction::Upstream);
    let group = upstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::AssociatedWith)
        .expect("AssociatedWith upstream group on the repo");
    assert_eq!(group.links.len(), 3);
    for row in &group.links {
        assert!(row.resolved_winner);
        assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
    }
    assert!(!view.has_other_rows());
}

#[test]
fn child_session_group_multi_link_keeps_winner_first() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "parent", Some("/home/op/src/x"), None));
    snapshot.nodes.push(agent(
        "claude-code",
        "child-a",
        Some("/home/op/src/y"),
        None,
    ));
    snapshot.nodes.push(agent(
        "claude-code",
        "child-b",
        Some("/home/op/src/z"),
        None,
    ));
    let parent = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "parent"));
    let child_a = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child-a"));
    let child_b = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child-b"));
    snapshot.candidate_links.push(link(
        "ca",
        child_a,
        parent.clone(),
        RelationKind::ParentSession,
    ));
    snapshot.candidate_links.push(link(
        "cb",
        child_b,
        parent.clone(),
        RelationKind::ParentSession,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &parent, Some(home().as_path()));
    let upstream = filter_direction(&view, Direction::Upstream);
    let group = upstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::ParentSession)
        .expect("parent_session group on parent side");
    assert!(!group.is_single());
    assert_eq!(group.links.len(), 2);
}

#[test]
fn unresolved_endpoint_renders_as_unresolved_row() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "child", Some("/home/op/src/x"), None));
    let child = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child"));
    let evidence = UnresolvedEndpoint {
        node_type: "agent_session".to_string(),
        harness_key: Some("claude-code".to_string()),
        native_id: Some("9a1f".to_string()),
        state_scope: None,
        path: None,
        metadata: crate::model::Metadata::default(),
    };
    snapshot.candidate_links.push(unresolved_link(
        "u1",
        child.clone(),
        evidence,
        RelationKind::ParentSession,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let view = build(&snapshot, &child, Some(home().as_path()));
    let downstream = filter_direction(&view, Direction::Downstream);
    let group = downstream
        .groups
        .iter()
        .find(|g| g.relation == RelationKind::ParentSession)
        .expect("parent_session downstream");
    assert_eq!(group.links.len(), 0);
    assert_eq!(group.unresolved_count, 1);
    assert_eq!(group.unresolved[0].node_type, "agent_session");
    assert_eq!(
        group.unresolved[0].evidence.native_id.as_deref(),
        Some("9a1f")
    );
    assert!(group.is_single());
}

#[test]
fn long_command_truncates_with_full_value_available() {
    let mut snapshot = GraphSnapshot::empty();
    let long_command = "/usr/bin/claude --resume 7f3c2a917b8c4d556e6f7a8b9c0d1e2f3a4b5c6d --extra";
    snapshot.nodes.push(process("obs:1", 82310, long_command));
    let proc_id = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:1"));
    let snapshot = resolve_snapshot(snapshot);
    let view = build(&snapshot, &proc_id, Some(home().as_path()));
    let command_field = view
        .core_fields
        .iter()
        .find(|f| f.label == "command")
        .expect("command field");
    assert!(command_field.value.contains('…'));
    assert_eq!(command_field.long_value.as_deref(), Some(long_command));
}

#[test]
fn full_id_visible_in_all_fields_but_not_core() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent(
        "claude-code",
        "abc",
        Some("/home/op/src/x"),
        Some("title"),
    ));
    let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let snapshot = resolve_snapshot(snapshot);
    let view = build(&snapshot, &target, Some(home().as_path()));
    let core_labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
    assert!(!core_labels.contains(&"full_id"));
    let all_labels: Vec<&str> = view.all_fields.iter().map(|f| f.label).collect();
    assert!(all_labels.contains(&"full_id"));
    assert!(all_labels.contains(&"state_scope"));
    assert!(all_labels.contains(&"session_key"));
    // No duplication of Core entries.
    for label in &core_labels {
        let count = all_labels.iter().filter(|l| *l == label).count();
        assert_eq!(count, 1, "duplicate label {label} in all_fields");
    }
}

#[test]
fn checkout_repo_branch_fork_have_top5_only() {
    let repo = RepoNode {
        id: RepoId::new("/srv/git/conspectus.git"),
        common_dir: "/srv/git/conspectus.git".to_string(),
        source_paths: vec!["/home/op/src/conspectus".to_string()],
        remotes: vec!["git@github.com:malloc47/conspectus.git".to_string()],
    };
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Repo(repo));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::Repo(RepoId::new("/srv/git/conspectus.git"));
    let view = build(&snapshot, &target, Some(home().as_path()));
    assert_eq!(view.kind, crate::model::NodeKind::Repo);
    let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
    assert_eq!(
        labels,
        vec!["id", "common_dir", "remotes", "source_paths", "full_id"]
    );
}

#[test]
fn detail_timestamps_render_as_relative_ages() {
    let pr = ForgePrNode {
        id: ForgePrId::new("github", "github.com", "owner", "repo", 7),
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        number: 7,
        state: Some("open".to_string()),
        url: None,
        updated_epoch: Some(TEST_NOW - 2 * 86_400),
        is_draft: false,
    };
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", "s1", Some("/home/op/src/x"), None));
    snapshot.nodes.push(mux("tmux", "editor", None));
    snapshot.nodes.push(process("proc-1", 42, "codex"));
    snapshot.nodes.push(GraphNode::ForgePr(pr.clone()));
    let snapshot = resolve_snapshot(snapshot);
    let field = |target: NodeId, label: &str| -> String {
        let view = build(&snapshot, &target, None);
        view.all_fields
            .iter()
            .find(|f| f.label == label)
            .unwrap_or_else(|| panic!("{label} field"))
            .value
            .clone()
    };

    assert_eq!(
        field(
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "s1")),
            "status"
        ),
        "active · last 1h ago"
    );
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    assert_eq!(field(mux_id.clone(), "last_active"), "59m ago");
    assert_eq!(field(mux_id, "created"), "11d ago");
    assert_eq!(
        field(
            NodeId::RuntimeProcess(RuntimeProcessId::new("proc-1")),
            "observed"
        ),
        "59m ago"
    );
    assert_eq!(field(NodeId::ForgePr(pr.id), "updated"), "2d ago");
}

#[test]
fn forge_pr_core_renders_composite_label() {
    let pr = ForgePrNode {
        id: ForgePrId::new("github", "github.com", "malloc47", "conspectus", 42),
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: "malloc47".to_string(),
        repo: "conspectus".to_string(),
        number: 42,
        state: Some("open".to_string()),
        url: Some("https://github.com/malloc47/conspectus/pull/42".to_string()),
        updated_epoch: Some(1_700_000_100),
        is_draft: false,
    };
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::ForgePr(pr.clone()));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::ForgePr(pr.id);
    let view = build(&snapshot, &target, Some(home().as_path()));
    let pr_field = view
        .core_fields
        .iter()
        .find(|f| f.label == "pr")
        .expect("pr field");
    assert_eq!(pr_field.value, "malloc47/conspectus#42 (open)");
    // Extras include state and provider.
    let extra_labels: Vec<&str> = view.all_fields.iter().map(|f| f.label).collect();
    assert!(extra_labels.contains(&"provider"));
    assert!(extra_labels.contains(&"state"));
}

#[test]
fn short_node_label_renders_kind_short_tag_per_node() {
    // Every node kind should produce a `kind:short_tag`
    // label suitable for breadcrumb hops.
    assert_eq!(
        short_node_label(&agent(
            "claude-code",
            "session-abcdef0123456789",
            None,
            None,
        )),
        "session:23456789",
    );
    assert_eq!(short_node_label(&mux("tmux", "editor", None)), "mux:editor");
    assert_eq!(
        short_node_label(&process("obs:1", 82310, "/usr/bin/claude --resume aaa")),
        "proc:claude",
    );
    let pr = GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", "octo", "repo", 7),
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: "octo".to_string(),
        repo: "repo".to_string(),
        number: 7,
        state: None,
        url: None,
        updated_epoch: None,
        is_draft: false,
    });
    assert_eq!(short_node_label(&pr), "pr:octo/repo#7");
    let repo = RepoNode {
        id: RepoId::new("/srv/git/conspectus.git"),
        common_dir: "/srv/git/conspectus.git".to_string(),
        source_paths: vec![],
        remotes: vec![],
    };
    assert_eq!(
        short_node_label(&GraphNode::Repo(repo)),
        "repo:conspectus.git"
    );
}

#[test]
fn render_breadcrumb_chain_joins_hops_with_separator() {
    let theme = Theme::default();
    let hops = vec![
        breadcrumb_hop(
            "session:abc",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
        ),
        breadcrumb_hop(
            "mux:editor",
            NodeId::MuxSession(MuxSessionId::new("editor")),
        ),
        breadcrumb_hop(
            "proc:claude",
            NodeId::RuntimeProcess(RuntimeProcessId::new("proc:1")),
        ),
    ];
    let rendered = render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty");
    // Flatten the line back to plain text. Each hop renders as
    // `<glyph> <tag>` (no `kind:` prefix; the glyph carries
    // the kind identity now).
    let plain = breadcrumb_plain(&rendered);
    assert_eq!(
        plain,
        format!(
            "{} abc › {} editor › {} claude",
            NodeKind::AgentSession.default_glyph(),
            NodeKind::MuxSession.default_glyph(),
            NodeKind::RuntimeProcess.default_glyph(),
        ),
    );
}

#[test]
fn render_breadcrumb_chain_uses_kind_color_per_glyph_span() {
    let theme = Theme::default();
    let hops = vec![
        breadcrumb_hop(
            "session:abc",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
        ),
        breadcrumb_hop(
            "mux:editor",
            NodeId::MuxSession(MuxSessionId::new("editor")),
        ),
    ];
    let rendered = render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty");
    let session_glyph = NodeKind::AgentSession.default_glyph();
    let mux_glyph = NodeKind::MuxSession.default_glyph();
    let session_span = rendered
        .spans
        .iter()
        .find(|s| s.content == session_glyph)
        .expect("session glyph span");
    let mux_span = rendered
        .spans
        .iter()
        .find(|s| s.content == mux_glyph)
        .expect("mux glyph span");
    assert_eq!(session_span.style.fg, Some(theme.node_agent_session));
    assert_eq!(mux_span.style.fg, Some(theme.node_mux_session));
}

#[test]
fn render_breadcrumb_chain_returns_none_when_empty() {
    let theme = Theme::default();
    assert!(render_breadcrumb_chain(&[], &theme, 80).is_none());
}

#[test]
fn render_breadcrumb_chain_elides_middle_when_too_long() {
    let theme = Theme::default();
    // Four hops; budget only fits `first … last`.
    let hops = vec![
        breadcrumb_hop(
            "session:abcdefgh",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "agent-1")),
        ),
        breadcrumb_hop(
            "mux:editor-east",
            NodeId::MuxSession(MuxSessionId::new("editor-east")),
        ),
        breadcrumb_hop(
            "proc:claude-helper",
            NodeId::RuntimeProcess(RuntimeProcessId::new("proc:helper")),
        ),
        breadcrumb_hop(
            "session:xyzlast",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "agent-2")),
        ),
    ];
    let plain = breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 40).expect("non-empty"));
    let session_glyph = NodeKind::AgentSession.default_glyph();
    assert!(
        plain.starts_with(&format!("{session_glyph} abcdefgh")),
        "expected leading first hop in {plain:?}",
    );
    assert!(
        plain.ends_with(&format!("{session_glyph} xyzlast")),
        "expected trailing last hop in {plain:?}",
    );
    assert!(plain.contains('…'), "expected elision marker in {plain:?}");
}

#[test]
fn render_breadcrumb_chain_falls_back_to_last_hop_when_extremely_narrow() {
    let theme = Theme::default();
    let hops = vec![
        breadcrumb_hop(
            "session:abc",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
        ),
        breadcrumb_hop(
            "mux:editor",
            NodeId::MuxSession(MuxSessionId::new("editor")),
        ),
        breadcrumb_hop(
            "proc:claude",
            NodeId::RuntimeProcess(RuntimeProcessId::new("proc:1")),
        ),
    ];
    // Budget only fits the last hop.
    let plain = breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 5).expect("non-empty"));
    let proc_glyph = NodeKind::RuntimeProcess.default_glyph();
    assert_eq!(plain, format!("{proc_glyph} claude"));
}

#[test]
fn render_breadcrumb_chain_disambiguates_colliding_short_labels() {
    let theme = Theme::default();
    // Two `session:abc` hops should pick up a `·last4` tail
    // so the operator can tell which is which.
    let hops = vec![
        breadcrumb_hop(
            "session:abc",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "session-1234")),
        ),
        breadcrumb_hop(
            "mux:editor",
            NodeId::MuxSession(MuxSessionId::new("editor")),
        ),
        breadcrumb_hop(
            "session:abc",
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "session-5678")),
        ),
    ];
    let plain = breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty"));
    // Both colliding hops should carry a `·` disambiguator
    // (`abc·1234`, `abc·5678`).
    let session_segments: Vec<&str> = plain.split(" › ").filter(|s| s.contains("abc")).collect();
    assert_eq!(session_segments.len(), 2);
    assert!(
        session_segments.iter().all(|s| s.contains('·')),
        "colliding hops should be disambiguated: {plain}",
    );
}

fn breadcrumb_hop(short_label: &str, focused: NodeId) -> BreadcrumbHop {
    BreadcrumbHop {
        focused,
        short_label: short_label.to_string(),
        cursor_key: None,
        other_expanded: false,
        full_detail_expanded: false,
        left_pane_selection: None,
    }
}

fn breadcrumb_plain(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn cwd_owner_kind_resolves_to_checkout_workspace_then_repo() {
    // When the cwd of a session matches a Checkout in
    // the snapshot, surface `checkout`; otherwise Workspace,
    // then Repo (`common_dir` or any `source_paths` entry).
    let mut snapshot = GraphSnapshot::empty();
    let repo_id = RepoId::new("/srv/git/conspectus.git");
    snapshot.nodes.push(GraphNode::Repo(RepoNode {
        id: repo_id.clone(),
        common_dir: "/srv/git/conspectus.git".to_string(),
        source_paths: vec!["/home/op/src/conspectus".to_string()],
        remotes: vec![],
    }));
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/home/op/atelier/demo"),
        root: "/home/op/atelier/demo".to_string(),
        provider: None,
        name: Some("demo".to_string()),
    }));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo_id, "/home/op/src/conspectus"),
        root: "/home/op/src/conspectus".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    // Checkout wins over Repo for the same path.
    assert_eq!(
        cwd_owner_kind(&snapshot, "/home/op/src/conspectus"),
        Some(crate::model::NodeKind::Checkout)
    );
    // Workspace beats Repo when no checkout matches.
    assert_eq!(
        cwd_owner_kind(&snapshot, "/home/op/atelier/demo"),
        Some(crate::model::NodeKind::Workspace)
    );
    // Repo by common_dir.
    assert_eq!(
        cwd_owner_kind(&snapshot, "/srv/git/conspectus.git"),
        Some(crate::model::NodeKind::Repo)
    );
    // No match anywhere → bare cwd.
    assert_eq!(cwd_owner_kind(&snapshot, "/tmp/scratch"), None);
    // Empty/whitespace inputs don't claim a match.
    assert_eq!(cwd_owner_kind(&snapshot, ""), None);
}

#[test]
fn agent_session_cwd_field_carries_kind_chip_when_resolved() {
    // The session's `cwd` field should pick up the
    // owning-node kind chip when the path resolves in the graph.
    let mut snapshot = GraphSnapshot::empty();
    let repo_id = RepoId::new("/srv/git/conspectus.git");
    snapshot.nodes.push(GraphNode::Repo(RepoNode {
        id: repo_id.clone(),
        common_dir: "/srv/git/conspectus.git".to_string(),
        source_paths: vec!["/home/op/src/conspectus".to_string()],
        remotes: vec![],
    }));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo_id, "/home/op/src/conspectus"),
        root: "/home/op/src/conspectus".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snapshot.nodes.push(agent(
        "claude-code",
        "abc",
        Some("/home/op/src/conspectus"),
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let view = build(&snapshot, &target, Some(home().as_path()));
    let cwd = view
        .core_fields
        .iter()
        .find(|f| f.label == "cwd")
        .expect("cwd field present");
    assert_eq!(cwd.kind_chip, Some(crate::model::NodeKind::Checkout));
}

#[test]
fn agent_session_cwd_has_no_kind_chip_when_path_does_not_resolve() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("claude-code", "abc", Some("/tmp/scratch"), None));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let view = build(&snapshot, &target, Some(home().as_path()));
    let cwd = view
        .core_fields
        .iter()
        .find(|f| f.label == "cwd")
        .expect("cwd field present");
    assert!(
        cwd.kind_chip.is_none(),
        "unresolved cwd should leave kind_chip empty: {cwd:?}",
    );
}
