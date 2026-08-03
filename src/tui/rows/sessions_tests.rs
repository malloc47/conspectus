// Extracted from sessions.rs H-HYG-011 rolling wave via #[path = "sessions_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RelationKind,
    RepoId, RepoNode, SessionKind, WorkspaceId, WorkspaceNode,
};
use crate::resolve::resolve_snapshot;
use std::path::PathBuf;

fn home() -> PathBuf {
    PathBuf::from("/home/op")
}

fn repo(common_dir: &str) -> GraphNode {
    GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
}

fn repo_with_source(common_dir: &str, source_path: &str) -> GraphNode {
    let mut repo = RepoNode::new(RepoId::new(common_dir));
    repo.source_paths.push(source_path.to_string());
    GraphNode::Repo(repo)
}

fn worktree(repo_common: &str, root: &str) -> GraphNode {
    GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(repo_common), root.to_string()),
        root: root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    })
}

fn workspace(root: &str) -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(root),
        root: root.to_string(),
        provider: None,
        name: None,
    })
}

fn agent_session(
    harness: &str,
    scope: &str,
    key: &str,
    cwd: Option<&str>,
    title: Option<&str>,
    preview: Option<&str>,
) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, scope, key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: title.map(str::to_string),
        last_message_preview: preview.map(str::to_string),
        last_active_epoch: None,
        session_kind: None,
    })
}

fn agent_session_with_activity(
    harness: &str,
    scope: &str,
    key: &str,
    cwd: Option<&str>,
    activity_epoch: i64,
) -> GraphNode {
    let mut node = agent_session(harness, scope, key, cwd, None, None);
    if let GraphNode::AgentSession(session) = &mut node {
        session.last_active_epoch = Some(activity_epoch);
    }
    node
}

fn mux_node(backend: &str, native_id: &str) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode::new(
        MuxSessionId::new(native_id),
        backend.to_string(),
        native_id.to_string(),
    ))
}

fn linked_to_mux(
    session: &NodeId,
    mux: &NodeId,
    provenance: Provenance,
    suffix: &str,
) -> GraphLink {
    GraphLink {
        id: format!("session-mux-{suffix}"),
        source: session.clone(),
        target: LinkEndpoint::Node { id: mux.clone() },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn associated_with_workspace(session: &NodeId, root: &str) -> GraphLink {
    let workspace = NodeId::Workspace(WorkspaceId::new(root));
    GraphLink {
        id: format!("test:{session}:associated_with:{workspace}"),
        source: session.clone(),
        target: LinkEndpoint::Node { id: workspace },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn workspace_with_name(root: &str, name: &str) -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(root),
        root: root.to_string(),
        provider: None,
        name: Some(name.to_string()),
    })
}

fn workspace_contains_repo_link(workspace_root: &str, repo_common_dir: &str) -> GraphLink {
    GraphLink {
        id: format!("test:wcr:{workspace_root}:{repo_common_dir}"),
        source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
        target: LinkEndpoint::Node {
            id: NodeId::Repo(RepoId::new(repo_common_dir)),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn build(inputs: SessionsBuildInputs<'_>) -> RowTree {
    build_sessions_tree(inputs)
}

#[test]
fn empty_snapshot_produces_empty_tree() {
    let snapshot = GraphSnapshot::empty();
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    assert!(tree.rows.is_empty());
    assert_eq!(tree.view, ViewLabel::Sessions);
}

#[test]
fn single_session_with_one_worktree_collapses_worktree_level() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        Some("first message"),
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    // Expect: repo row, then session row. No worktree row.
    assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
    assert!(matches!(tree.rows[0].kind, RowKind::Group(_)));
    let group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(group.display_path, "~/src/proj");
    assert_eq!(tree.rows[0].depth, 0);

    assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
    assert_eq!(tree.rows[1].depth, 1);
}

#[test]
fn none_grouping_emits_flat_recency_sorted_session_rows_with_project_labels() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/alpha/.git",
        "/home/op/src/alpha",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/alpha/.git", "/home/op/src/alpha"));
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/beta/.git",
        "/home/op/src/beta",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/beta/.git", "/home/op/src/beta"));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "older",
        Some("/home/op/src/alpha"),
        100,
    ));
    snapshot.nodes.push(agent_session_with_activity(
        "claude-code",
        "/state",
        "newer",
        Some("/home/op/src/beta"),
        200,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::None,
        home: Some(home().as_path()),
        now: Some(300),
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
    assert!(tree.rows.iter().all(|row| row.depth == 0));
    assert!(
        tree.rows
            .iter()
            .all(|row| matches!(row.kind, RowKind::AgentSession(_)))
    );
    let first = match &tree.rows[0].kind {
        RowKind::AgentSession(session) => session,
        other => panic!("expected session row, got {other:?}"),
    };
    let second = match &tree.rows[1].kind {
        RowKind::AgentSession(session) => session,
        other => panic!("expected session row, got {other:?}"),
    };
    assert_eq!(first.session.session_key, "newer");
    assert_eq!(first.project_display.as_deref(), Some("beta"));
    assert_eq!(second.session.session_key, "older");
    assert_eq!(second.project_display.as_deref(), Some("alpha"));
}

#[test]
fn repo_group_prefers_source_path_over_git_common_dir() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/proj/.git",
        "/home/op/src/proj",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: Some(Path::new("/home/op/src/proj")),
        filter: RowFilter::default(),
    });

    let group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(group.display_path, "~/src/proj");
    assert!(group.is_launch_context);
}

#[test]
fn repo_group_strips_git_suffix_when_source_path_is_missing() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj/.git"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: Some(Path::new("/home/op/src/proj")),
        filter: RowFilter::default(),
    });

    let group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(group.display_path, "~/src/proj");
    assert!(group.is_launch_context);
}

#[test]
fn repo_group_prefers_canonical_over_non_canonical_source_path() {
    // Regression: when the only known `source_path` is a
    // non-canonical worktree (e.g. an agent-deck multi-repo
    // checkout that was probed before the canonical clone), the
    // repo row should still label with the canonical path derived
    // from the git common dir — otherwise the canonical
    // checkout appears visually nested *under* the agent-deck
    // path when both worktrees fan out as group rows.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/proj/.git",
        "/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj",
    ));
    snapshot.nodes.push(worktree(
        "/home/op/src/proj/.git",
        "/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "a",
        Some("/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(agent_session(
        "claude-code",
        "/state",
        "b",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let repo_row = tree
            .rows
            .iter()
            .find(|row| matches!(&row.kind, RowKind::Group(g) if matches!(&g.primary_node, Some(NodeId::Repo(_)))))
            .expect("repo group row");
    let group = match &repo_row.kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(group.display_path, "~/src/proj");
}

#[test]
fn session_nested_inside_checkout_groups_under_checkout() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/proj/.git",
        "/home/op/src/proj",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj/crates/core"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
    let group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(group.display_path, "~/src/proj");
    assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
}

#[test]
fn graph_grouping_uses_session_workspace_context() {
    // Hybrid Graph (ADR 0064): A-class sessions sit directly
    // beneath their workspace at depth 1 — no intermediate repo
    // level. The repo is still implicit (a session always lives
    // in a checkout), but the workspace is the dominant
    // top-level entity for any session carrying an
    // `AssociatedWith Workspace` edge.
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(workspace("/home/op/ws"));
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/proj/.git",
        "/home/op/src/proj",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj/crates/core"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&session_id, "/home/op/ws"));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
    let workspace_group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert!(matches!(
        workspace_group.primary_node,
        Some(NodeId::Workspace(_))
    ));
    assert_eq!(tree.rows[0].depth, 0);
    assert_eq!(
        tree.rows[1].depth, 1,
        "session sits directly under workspace — no repo intermediate",
    );
}

#[test]
fn repo_grouping_excludes_workspace_context() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(workspace("/home/op/ws"));
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/proj/.git",
        "/home/op/src/proj",
    ));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj/crates/core"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&session_id, "/home/op/ws"));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Repo,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
    let group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => unreachable!(),
    };
    assert!(matches!(group.primary_node, Some(NodeId::Repo(_))));
    assert_eq!(tree.rows[0].depth, 0);
    assert_eq!(tree.rows[1].depth, 1);
}

// -----------------------------------------------------------------
// H-WS-001: strict workspace grouping + weak-membership chip
// -----------------------------------------------------------------
//
// Helper that pulls the session row out of a tree so the chip and
// grouping depth can be asserted without repeating the matching
// boilerplate. The session-of-interest in these tests is always
// the lone AgentSession row.
fn first_session_row(tree: &RowTree) -> (&Row, &AgentSessionRow) {
    for row in &tree.rows {
        if let RowKind::AgentSession(s) = &row.kind {
            return (row, s);
        }
    }
    panic!("no AgentSession row in tree:\n{:#?}", tree.rows);
}

fn find_session_row<'a>(tree: &'a RowTree, key: &str) -> (&'a Row, &'a AgentSessionRow) {
    for row in &tree.rows {
        if let RowKind::AgentSession(s) = &row.kind
            && s.session.session_key == key
        {
            return (row, s);
        }
    }
    panic!("no AgentSession {key:?} in tree:\n{:#?}", tree.rows);
}

#[test]
fn workspace_rooted_session_nests_directly_under_workspace() {
    // Hybrid Graph (ADR 0064): A-class session sits at depth 1
    // directly under the workspace, no repo intermediate.
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
    snapshot.nodes.push(repo_with_source(
        "/home/op/atelier/conspectus/.git",
        "/home/op/atelier/conspectus",
    ));
    snapshot.nodes.push(worktree(
        "/home/op/atelier/conspectus/.git",
        "/home/op/atelier/conspectus",
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/atelier/conspectus/crates/core"),
        None,
        None,
    ));
    snapshot.candidate_links.push(workspace_contains_repo_link(
        "/home/op/atelier",
        "/home/op/atelier/conspectus/.git",
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&session_id, "/home/op/atelier"));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let (row, _) = first_session_row(&tree);
    assert_eq!(
        row.depth, 1,
        "workspace-rooted session sits directly under workspace at depth 1:\n{:#?}",
        tree.rows
    );
    // Workspace header uses the shared format_workspace_display
    // helper: name + member list + provider chip.
    let ws_group = match &tree.rows[0].kind {
        RowKind::Group(g) => g,
        _ => panic!("expected workspace group row at index 0:\n{:#?}", tree.rows),
    };
    assert!(
        ws_group.display_path.contains("atelier-ws"),
        "workspace header should include the workspace name `atelier-ws`, got `{}`",
        ws_group.display_path,
    );
}

#[test]
fn repo_shared_session_stays_at_repo_level() {
    // Case B — session has no AssociatedWith but its repo is a
    // workspace member. Strict grouping (H-WS-001) puts it under
    // repo, not workspace. The fixture includes an (A)-class
    // session at the workspace root so both classes are present
    // in the tree; we assert each lands at the right depth.
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
    snapshot.nodes.push(repo_with_source(
        "/home/op/src/conspectus/.git",
        "/home/op/src/conspectus",
    ));
    snapshot.nodes.push(worktree(
        "/home/op/src/conspectus/.git",
        "/home/op/src/conspectus",
    ));
    let active_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "active"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "active",
        Some("/home/op/atelier"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&active_id, "/home/op/atelier"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "shared",
        Some("/home/op/src/conspectus/crates/core"),
        None,
        None,
    ));
    snapshot.candidate_links.push(workspace_contains_repo_link(
        "/home/op/atelier",
        "/home/op/src/conspectus/.git",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let (shared_row, _) = find_session_row(&tree, "shared");
    assert_eq!(
        shared_row.depth, 1,
        "(B)-class session should sit under repo, not workspace:\n{:#?}",
        tree.rows
    );

    let (active_row, _) = find_session_row(&tree, "active");
    assert!(
        active_row.depth >= 1,
        "(A)-class session should sit under workspace header:\n{:#?}",
        tree.rows
    );
}

#[test]
fn workspace_root_cwd_groups_under_workspace_without_checkout() {
    // ADR 0064 motivation: agent-deck launches the harness with
    // cwd at the workspace composite directory itself, not
    // inside a member subdir. That cwd has no checkout, so the
    // legacy resolve_group_key (which required a checkout)
    // dropped these sessions into the ungrouped bucket. The
    // hybrid path looks up the workspace edge first and groups
    // the session directly under the workspace.
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "wsroot"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(workspace_with_name(
        "/home/op/.agent-deck/multi-repo-worktrees/abc",
        "abc",
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "wsroot",
        Some("/home/op/.agent-deck/multi-repo-worktrees/abc"),
        None,
        None,
    ));
    snapshot.candidate_links.push(associated_with_workspace(
        &session_id,
        "/home/op/.agent-deck/multi-repo-worktrees/abc",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(
        tree.rows.len(),
        2,
        "workspace header + session, no ungrouped bucket:\n{:#?}",
        tree.rows
    );
    assert!(
        matches!(&tree.rows[0].kind, RowKind::Group(g)
                if matches!(g.primary_node, Some(NodeId::Workspace(_)))),
        "row 0 must be a workspace group, got:\n{:#?}",
        tree.rows[0],
    );
    assert_eq!(tree.rows[0].depth, 0);
    assert!(matches!(&tree.rows[1].kind, RowKind::AgentSession(_)));
    assert_eq!(tree.rows[1].depth, 1);
}

#[test]
fn hybrid_emits_workspace_and_repo_buckets_as_peer_top_level_parents() {
    // ADR 0064 shape: one A-class session whose workspace edge
    // determines a workspace bucket, plus one B-class session in
    // an unrelated repo. The Graph view should emit both at
    // depth 0, with sessions at depth 1 under each, and the
    // workspace bucket sorted before the repo bucket.
    let a_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "a",
        Some("/home/op/atelier"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&a_id, "/home/op/atelier"));

    snapshot.nodes.push(repo_with_source(
        "/home/op/src/standalone/.git",
        "/home/op/src/standalone",
    ));
    snapshot.nodes.push(worktree(
        "/home/op/src/standalone/.git",
        "/home/op/src/standalone",
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "b",
        Some("/home/op/src/standalone"),
        None,
        None,
    ));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    // Workspace bucket first, repo bucket second. Each bucket
    // contributes one header row at depth 0 plus one session
    // row at depth 1.
    let top_level: Vec<_> = tree.rows.iter().filter(|r| r.depth == 0).collect();
    assert_eq!(
        top_level.len(),
        2,
        "expected 2 top-level parent rows:\n{:#?}",
        tree.rows
    );

    let first = match &top_level[0].kind {
        RowKind::Group(g) => g,
        _ => panic!("expected Group row at top of tree:\n{:#?}", tree.rows),
    };
    assert!(
        matches!(first.primary_node, Some(NodeId::Workspace(_))),
        "workspace bucket should sort before repo bucket, got:\n{:#?}",
        tree.rows,
    );

    let second = match &top_level[1].kind {
        RowKind::Group(g) => g,
        _ => panic!("expected Group row for repo bucket:\n{:#?}", tree.rows),
    };
    assert!(
        matches!(second.primary_node, Some(NodeId::Repo(_))),
        "repo bucket follows workspace bucket:\n{:#?}",
        tree.rows,
    );

    let session_rows: Vec<_> = tree
        .rows
        .iter()
        .filter(|r| matches!(&r.kind, RowKind::AgentSession(_)))
        .collect();
    assert_eq!(
        session_rows.len(),
        2,
        "two sessions, one under each parent:\n{:#?}",
        tree.rows
    );
    for row in &session_rows {
        assert_eq!(
            row.depth, 1,
            "session sits one level under its parent (no repo intermediate under workspace, no checkout-fanout for single-worktree repo):\n{:#?}",
            tree.rows
        );
    }
}

#[test]
fn two_worktrees_in_same_repo_show_worktree_level() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/wt/featx"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "def",
        Some("/home/op/wt/featx"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    // Expect a single repo row at depth 0, followed by two
    // worktree rows at depth 1 each owning their session rows
    // at depth 2. The dedup pass collapses what would otherwise
    // be a per-bucket repo header repeat (was T8-001).
    let kinds: Vec<&RowKind> = tree.rows.iter().map(|r| &r.kind).collect();
    let repo_count = kinds
        .iter()
        .filter(|k| {
            matches!(
                k,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Repo(_)),
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        repo_count, 1,
        "expected exactly one repo group row, kinds={kinds:#?}"
    );
    let worktree_count = kinds
        .iter()
        .filter(|k| {
            matches!(
                k,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Checkout(_)),
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        worktree_count, 2,
        "expected two worktree group rows, kinds={kinds:#?}"
    );
    // Repo row sits above the two worktree subtrees.
    let repo_pos = tree
        .rows
        .iter()
        .position(|r| {
            matches!(
                &r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Repo(_)),
                    ..
                })
            )
        })
        .unwrap();
    let first_worktree_pos = tree
        .rows
        .iter()
        .position(|r| {
            matches!(
                &r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Checkout(_)),
                    ..
                })
            )
        })
        .unwrap();
    assert!(repo_pos < first_worktree_pos);
}

#[test]
fn workspace_attributed_worktree_does_not_force_repo_checkout_fanout() {
    // Mirror the snapshot that surfaced this bug: a repo with two
    // discovered checkouts — its canonical checkout and a worktree
    // that lives inside an agent-deck workspace — where only the
    // canonical checkout has a (B)-class session. Sessions whose
    // cwd is the workspace root are (A)-class and group under the
    // workspace, not under the repo. The worktree fanout decision
    // must look at session-bearing repo buckets only, so the repo
    // collapses its single contributing worktree the same way a
    // repo with one discovered checkout does.
    let ws_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "ws"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/config"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/config", "/home/op/src/config"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/config", "/home/op/ws/abc/config"));
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/ws/abc", "abc"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "ws",
        Some("/home/op/ws/abc"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&ws_id, "/home/op/ws/abc"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "canon",
        Some("/home/op/src/config"),
        None,
        None,
    ));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let kinds: Vec<&RowKind> = tree.rows.iter().map(|r| &r.kind).collect();
    let checkout_rows = kinds
        .iter()
        .filter(|k| {
            matches!(
                k,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Checkout(_)),
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        checkout_rows, 0,
        "single session-bearing worktree must collapse its checkout level even when a second worktree exists under a workspace:\n{:#?}",
        tree.rows
    );

    let (canon_row, _) = find_session_row(&tree, "canon");
    assert_eq!(
        canon_row.depth, 1,
        "(B)-class session sits directly under its repo:\n{:#?}",
        tree.rows
    );
    let (ws_row, _) = find_session_row(&tree, "ws");
    assert_eq!(
        ws_row.depth, 1,
        "(A)-class session sits directly under its workspace:\n{:#?}",
        tree.rows
    );
}

#[test]
fn workspace_grouping_emits_header_for_every_workspace_even_without_sessions() {
    // ADR 0065: Sessions/Workspace must surface idle workspaces
    // the same way the dropped View::Workspaces did. A workspace
    // node with no (A)-class sessions still gets a header at
    // top level so operators see what workspaces exist on the
    // machine without flipping views.
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/ws/idle", "idle-ws"));
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/ws/busy", "busy-ws"));
    let busy_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "busy"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "busy",
        Some("/home/op/ws/busy"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&busy_id, "/home/op/ws/busy"));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Workspace,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let workspace_headers: Vec<&Row> = tree
        .rows
        .iter()
        .filter(|r| {
            matches!(
                &r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Workspace(_)),
                    ..
                })
            )
        })
        .collect();
    assert_eq!(
        workspace_headers.len(),
        2,
        "expected one header per workspace node:\n{:#?}",
        tree.rows
    );
    assert_eq!(workspace_headers[0].depth, 0);
    assert_eq!(workspace_headers[1].depth, 0);
}

#[test]
fn workspace_grouping_drops_b_class_session_into_ungrouped() {
    // ADR 0065: a session whose cwd is inside a workspace
    // member's checkout but not associated with the workspace
    // (no AssociatedWith link) is (B)-class. In Workspace mode
    // there are no repo buckets, so it goes to Ungrouped.
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/ws/abc", "abc"));
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "b",
        Some("/home/op/src/proj"),
        None,
        None,
    ));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Workspace,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let ungrouped_header = tree.rows.iter().find(|r| {
        matches!(
            &r.kind,
            RowKind::Group(GroupRow { display_path, primary_node: None, .. })
            if display_path == "Ungrouped"
        )
    });
    assert!(
        ungrouped_header.is_some(),
        "B-class session must land under an Ungrouped header in Workspace mode:\n{:#?}",
        tree.rows
    );

    let repo_rows = tree
        .rows
        .iter()
        .filter(|r| {
            matches!(
                &r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Repo(_)),
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        repo_rows, 0,
        "no repo buckets render in Workspace grouping:\n{:#?}",
        tree.rows
    );

    let (b_row, _) = find_session_row(&tree, "b");
    assert_eq!(
        b_row.depth, 1,
        "B-class session sits under the Ungrouped header at depth 1:\n{:#?}",
        tree.rows
    );
}

#[test]
fn workspace_grouping_groups_a_class_session_under_workspace_header() {
    // ADR 0065 happy path: (A)-class session with an
    // AssociatedWith Workspace edge nests directly under its
    // workspace header at depth 1, mirroring Sessions/Graph.
    let ws_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "ws"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_with_name("/home/op/ws/abc", "abc"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "ws",
        Some("/home/op/ws/abc"),
        None,
        None,
    ));
    snapshot
        .candidate_links
        .push(associated_with_workspace(&ws_id, "/home/op/ws/abc"));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Workspace,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let (ws_row, _) = find_session_row(&tree, "ws");
    assert_eq!(
        ws_row.depth, 1,
        "(A)-class session sits at depth 1 under its workspace:\n{:#?}",
        tree.rows
    );
}

#[test]
fn unmuxed_session_has_unmuxed_indicator_and_is_a_leaf() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_row = tree
        .rows
        .iter()
        .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
        .expect("session row present");
    match &session_row.kind {
        RowKind::AgentSession(row) => {
            assert_eq!(row.mux_state, MuxIndicator::Unmuxed);
        }
        _ => unreachable!(),
    }
    assert!(!session_row.expandable);
}

#[test]
fn session_row_uses_agent_activity_epoch_for_recency() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        1_000_000 - 120,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_row = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session),
            _ => None,
        })
        .expect("session row present");
    assert_eq!(session_row.activity_epoch, Some(1_000_000 - 120));
    assert_eq!(session_row.recency.as_deref(), Some("2m"));
}

#[test]
fn sessions_sort_by_most_recent_activity_within_group() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "old",
        Some("/home/op/src/proj"),
        1_000,
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "missing",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "new",
        Some("/home/op/src/proj"),
        2_000,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(2_500),
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_keys: Vec<&str> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(session_keys, vec!["new", "old", "missing"]);
}

#[test]
fn float_muxed_sessions_top_lifts_attached_above_unmuxed() {
    let muxed_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "muxed-old"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    // Newest session is unmuxed — it should drop below the older
    // muxed session when the bool is set, but stay on top within
    // its own (unmuxed) half of the split.
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "unmuxed-new",
        Some("/home/op/src/proj"),
        2_000,
    ));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "muxed-old",
        Some("/home/op/src/proj"),
        1_000,
    ));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "unmuxed-old",
        Some("/home/op/src/proj"),
        500,
    ));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.candidate_links.push(linked_to_mux(
        &muxed_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));
    let snapshot = resolve_snapshot(snapshot);

    let baseline = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(3_000),
        cwd: None,
        filter: RowFilter::default(),
    });
    let baseline_keys: Vec<&str> = baseline
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        baseline_keys,
        vec!["unmuxed-new", "muxed-old", "unmuxed-old"],
        "baseline sort is recency desc"
    );

    let floated = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(3_000),
        cwd: None,
        filter: RowFilter {
            float_muxed_sessions_top: true,
            ..RowFilter::default()
        },
    });
    let floated_keys: Vec<&str> = floated
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        floated_keys,
        vec!["muxed-old", "unmuxed-new", "unmuxed-old"],
        "muxed session rises above the newer unmuxed ones, \
             and within the unmuxed half the recency order is preserved"
    );
}

#[test]
fn single_mux_link_yields_attached_indicator() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let row = tree
        .rows
        .iter()
        .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
        .expect("session row");
    match &row.kind {
        RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
        _ => unreachable!(),
    }
    assert!(!row.expandable);
}

#[test]
fn two_mux_links_with_distinct_provenance_resolve_to_one_attached_mux() {
    // H-UI-008: the sessions tree consumes resolver winners,
    // not raw candidate links. With two `LinkedToMux` candidates
    // pointing at different muxes, the resolver picks the
    // higher-provenance candidate; the tree should reflect that
    // single winner as `Attached` rather than presenting both
    // candidates as an ambiguity. Pre-H-UI-008 the tree raised
    // a false-positive `Ambiguous` here because it grouped by
    // candidate target instead of consulting the resolver.
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
    let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.nodes.push(mux_node("tmux", "scratch"));
    // editor is StrongDiscovered (winner); scratch is
    // Discovered (loses the LinkedToMux slot).
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &editor,
        Provenance::StrongDiscovered,
        "1",
    ));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &scratch,
        Provenance::Discovered,
        "2",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_row = tree
        .rows
        .iter()
        .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
        .expect("session row");
    match &session_row.kind {
        RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
        _ => unreachable!(),
    }
    assert!(!session_row.expandable);
    assert!(
        !tree
            .rows
            .iter()
            .any(|r| matches!(r.kind, RowKind::AgentSessionMuxCandidate(_))),
        "no candidate child rows when the resolver has a single winner",
    );
}

#[test]
fn duplicate_mux_target_links_do_not_make_session_ambiguous() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "s1"));
    let mux = NodeId::MuxSession(MuxSessionId::new("editor"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent_session(
        "claude-code",
        "/state",
        "s1",
        Some("/repo"),
        None,
        None,
    ));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux,
        Provenance::StrongDiscovered,
        "hook",
    ));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux,
        Provenance::Discovered,
        "cwd",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_row = tree
        .rows
        .iter()
        .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
        .expect("session row");
    match &session_row.kind {
        RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
        _ => unreachable!(),
    }
    assert!(!session_row.expandable);
    assert!(
        !tree
            .rows
            .iter()
            .any(|row| matches!(row.kind, RowKind::AgentSessionMuxCandidate(_)))
    );
}

#[test]
fn orphan_session_lands_in_ungrouped_bucket() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent_session("codex", "/state", "abc", None, None, None));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    assert_eq!(tree.rows.len(), 2);
    match &tree.rows[0].kind {
        RowKind::Group(g) => assert_eq!(g.display_path, "Ungrouped"),
        _ => panic!("first row should be the Ungrouped group"),
    }
    assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
}

#[test]
fn session_row_carries_title_preview_and_short_id() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "claude-code",
        "/state",
        "xyz",
        Some("/home/op/src/proj"),
        Some("Phase 8 mockup"),
        Some("could you give me a bit more co…"),
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let row = tree
        .rows
        .iter()
        .find_map(|r| match &r.kind {
            RowKind::AgentSession(s) => Some(s),
            _ => None,
        })
        .expect("session row");
    assert_eq!(row.harness_label, "claude");
    assert_eq!(row.title.as_deref(), Some("Phase 8 mockup"));
    assert_eq!(
        row.preview.as_deref(),
        Some("could you give me a bit more co…")
    );
    assert_eq!(row.cwd_display.as_deref(), Some("~/src/proj"));
    assert_eq!(row.short_id.len(), 6, "short id floor at 6 chars");
}

#[test]
fn session_row_alias_overrides_title_in_display_label() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        None,
        Some("harness title"),
        None,
    ));
    let id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    snapshot.aliases.insert(id, "ingest-refactor".to_string());

    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let row = tree
        .rows
        .iter()
        .find_map(|r| match &r.kind {
            RowKind::AgentSession(s) => Some(s),
            _ => None,
        })
        .expect("session row");
    assert_eq!(row.alias.as_deref(), Some("ingest-refactor"));
    assert_eq!(row.title.as_deref(), Some("harness title"));
    assert_eq!(row.display_label(), Some("ingest-refactor"));
}

#[test]
fn worktree_grouping_always_shows_worktree_level() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "abc",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Checkout,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    // With explicit worktree grouping, the worktree row is
    // always present even though there's a single worktree.
    let has_worktree_group = tree.rows.iter().any(|r| {
        matches!(
            r.kind,
            RowKind::Group(GroupRow {
                primary_node: Some(NodeId::Checkout(_)),
                ..
            })
        )
    });
    assert!(
        has_worktree_group,
        "explicit worktree grouping should show the worktree level"
    );
}

#[test]
fn mark_launch_context_marks_deepest_ancestor_group() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/wt/featx"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "a",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "b",
        Some("/home/op/wt/featx"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        // Launching from inside the featx worktree should mark
        // the featx worktree row (more specific than the repo
        // row, which is also an ancestor).
        cwd: Some(std::path::Path::new("/home/op/wt/featx/src")),
        filter: RowFilter::default(),
    });

    let marked: Vec<&GroupRow> = tree
        .rows
        .iter()
        .filter_map(|r| match &r.kind {
            RowKind::Group(g) if g.is_launch_context => Some(g),
            _ => None,
        })
        .collect();
    assert_eq!(marked.len(), 1, "exactly one row should be marked");
    let marked = marked[0];
    match &marked.primary_node {
        Some(NodeId::Checkout(wt)) => assert_eq!(wt.root, "/home/op/wt/featx"),
        other => panic!("expected featx worktree marked, got {other:?}"),
    }
}

#[test]
fn mark_launch_context_with_no_match_leaves_all_rows_unmarked() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "a",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: Some(std::path::Path::new("/tmp/elsewhere")),
        filter: RowFilter::default(),
    });
    for row in &tree.rows {
        if let RowKind::Group(g) = &row.kind {
            assert!(
                !g.is_launch_context,
                "no row should be marked when cwd is unrelated; got marked: {g:?}"
            );
        }
    }
}

#[test]
fn mark_launch_context_disabled_when_cwd_is_none() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "a",
        Some("/home/op/src/proj"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    for row in &tree.rows {
        if let RowKind::Group(g) = &row.kind {
            assert!(!g.is_launch_context);
        }
    }
}

#[test]
fn path_is_ancestor_of_respects_component_boundaries() {
    use std::path::Path;
    assert!(path_is_ancestor_of(Path::new("/a/b"), Path::new("/a/b")));
    assert!(path_is_ancestor_of(Path::new("/a/b"), Path::new("/a/b/c")));
    assert!(!path_is_ancestor_of(
        Path::new("/a/b"),
        Path::new("/a/barbecue")
    ));
    assert!(!path_is_ancestor_of(Path::new("/x"), Path::new("/y")));
}

// ---- ADR 0031: filter predicate integration ----

fn three_harness_snapshot() -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/proj"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
    // claude session, recent
    snapshot.nodes.push(agent_session_with_activity(
        "claude-code",
        "/state",
        "c1",
        Some("/home/op/src/proj"),
        1_000_000,
    ));
    // codex session, stale (8 days old vs `now = 1_000_000`)
    let eight_days = 8 * 24 * 60 * 60;
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "x1",
        Some("/home/op/src/proj"),
        1_000_000 - eight_days,
    ));
    // opencode session, recent
    snapshot.nodes.push(agent_session_with_activity(
        "opencode",
        "/state",
        "o1",
        Some("/home/op/src/proj"),
        1_000_000 - 600,
    ));
    resolve_snapshot(snapshot)
}

fn session_rows(tree: &RowTree) -> Vec<&AgentSessionRow> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[test]
fn filter_harness_narrows_to_matching_sessions() {
    let snapshot = three_harness_snapshot();
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..RowFilter::default()
        },
    });
    let sessions = session_rows(&tree);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].harness_label, "claude");
}

#[test]
fn filter_max_age_drops_stale_sessions() {
    let snapshot = three_harness_snapshot();
    let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            max_age: Some(week),
            ..RowFilter::default()
        },
    });
    // codex (8d old) drops; claude and opencode remain.
    let labels: Vec<&str> = session_rows(&tree)
        .iter()
        .map(|s| s.harness_label.as_str())
        .collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.contains(&"claude"));
    assert!(labels.contains(&"opencode"));
    assert!(!labels.contains(&"codex"));
}

#[test]
fn filter_mux_state_unmuxed_admits_unmuxed_sessions_only() {
    let snapshot = three_harness_snapshot();
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            mux_state: Some(crate::filter::MuxStateFilter::from_values([
                crate::filter::MuxStateKey::Unmuxed,
            ])),
            ..RowFilter::default()
        },
    });
    // None of the fixture sessions have mux links, so all three pass.
    assert_eq!(session_rows(&tree).len(), 3);

    // Asking for attached-only drops all three.
    let attached_only = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            mux_state: Some(crate::filter::MuxStateFilter::from_values([
                crate::filter::MuxStateKey::Attached,
            ])),
            ..RowFilter::default()
        },
    });
    assert!(session_rows(&attached_only).is_empty());
}

#[test]
fn filter_intersection_of_all_dimensions() {
    let snapshot = three_harness_snapshot();
    let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values([
                "claude-code",
                "codex",
            ])),
            max_age: Some(week),
            mux_state: Some(crate::filter::MuxStateFilter::from_values([
                crate::filter::MuxStateKey::Unmuxed,
            ])),
            ..RowFilter::default()
        },
    });
    // claude-code passes harness+age+mux; codex fails the age cut.
    let sessions = session_rows(&tree);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].harness_label, "claude");
}

#[test]
fn filter_emptying_set_drops_entire_tree() {
    let snapshot = three_harness_snapshot();
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: Some(1_000_000),
        cwd: None,
        filter: RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["aider"])),
            ..RowFilter::default()
        },
    });
    // No matching session means no rows at all — group headers
    // skip emission when their bucket is empty.
    assert!(tree.rows.is_empty(), "{:#?}", tree.rows);
}

#[test]
fn filter_skips_empty_groups_so_no_orphan_headers_render() {
    // Two repos, but the filter only matches a session in repo A.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo("/home/op/src/projA"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/projA", "/home/op/src/projA"));
    snapshot.nodes.push(repo("/home/op/src/projB"));
    snapshot
        .nodes
        .push(worktree("/home/op/src/projB", "/home/op/src/projB"));
    snapshot.nodes.push(agent_session(
        "claude-code",
        "/state",
        "a",
        Some("/home/op/src/projA"),
        None,
        None,
    ));
    snapshot.nodes.push(agent_session(
        "codex",
        "/state",
        "b",
        Some("/home/op/src/projB"),
        None,
        None,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..RowFilter::default()
        },
    });
    // Only one group header should render (projA), not both.
    let group_count = tree
        .rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Group(_)))
        .count();
    assert_eq!(group_count, 1, "{:#?}", tree.rows);
    assert_eq!(session_rows(&tree).len(), 1);
}

// --- Subagent nesting tests ---

fn agent_session_with_kind(
    harness: &str,
    scope: &str,
    key: &str,
    cwd: Option<&str>,
    title: Option<&str>,
    session_kind: Option<SessionKind>,
) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, scope, key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: title.map(str::to_string),
        last_message_preview: None,
        last_active_epoch: None,
        session_kind,
    })
}

fn parent_session_link(child: &NodeId, parent: &NodeId) -> GraphLink {
    GraphLink {
        id: format!("test:lineage:{child}:parent_session:{parent}"),
        source: child.clone(),
        target: LinkEndpoint::Node { id: parent.clone() },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata {
            adapter: "opencode".to_string(),
            evidence: Some("opencode session.parent_id unknown".to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn checkout_node(repo_common_dir: &str, root: &str) -> GraphNode {
    let repo_id = RepoId {
        common_dir: repo_common_dir.to_string(),
    };
    GraphNode::Checkout(CheckoutNode {
        id: CheckoutId {
            repo: repo_id,
            root: root.to_string(),
        },
        root: root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    })
}

#[test]
fn subagent_sessions_are_nested_under_parent_in_row_tree() {
    let parent = agent_session_with_kind(
        "opencode",
        "/state",
        "parent",
        Some("/work/repo"),
        Some("Parent session"),
        None,
    );
    let subagent = agent_session_with_kind(
        "opencode",
        "/state",
        "sub",
        Some("/work/repo"),
        Some("(@explore subagent) Find files"),
        Some(SessionKind::Subagent),
    );
    let parent_id = parent.id();
    let sub_id = subagent.id();

    let checkout = checkout_node("/work/repo/.git", "/work/repo");
    let resolves = crate::resolve::resolve_snapshot;

    let snapshot = resolves(GraphSnapshot {
        nodes: vec![parent, subagent, checkout],
        candidate_links: vec![parent_session_link(&sub_id, &parent_id)],
        ..GraphSnapshot::empty()
    });

    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    // Parent should be at depth 2 (workspace → repo → agent)
    // Subagent should be at depth 3 (under parent, but since no mux,
    // it's just depth+1)
    let parent_row = tree
        .rows
        .iter()
        .find(|r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "parent"));
    let subagent_row = tree
        .rows
        .iter()
        .find(|r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "sub"));

    assert!(parent_row.is_some(), "parent session should appear in tree");
    assert!(subagent_row.is_some(), "subagent should appear in tree");

    let parent_row = parent_row.unwrap();
    let subagent_row = subagent_row.unwrap();

    assert!(
        parent_row.expandable,
        "parent should be expandable (has subagent child)"
    );
    assert!(
        subagent_row.depth > parent_row.depth,
        "subagent depth {subagent_depth} should be > parent depth {parent_depth}, rows: {rows:#?}",
        subagent_depth = subagent_row.depth,
        parent_depth = parent_row.depth,
        rows = tree.rows
    );
}

#[test]
fn graph_grouping_nests_resolved_lineage_for_regular_sessions() {
    let parent =
        agent_session_with_kind("codex", "/state", "parent", Some("/work/repo"), None, None);
    let child = agent_session_with_kind("codex", "/state", "child", Some("/work/repo"), None, None);
    let parent_id = parent.id();
    let child_id = child.id();
    let checkout = checkout_node("/work/repo/.git", "/work/repo");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![parent, child, checkout],
        candidate_links: vec![parent_session_link(&child_id, &parent_id)],
        ..GraphSnapshot::empty()
    });

    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let parent_row = tree
        .rows
        .iter()
        .find(|r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "parent"));
    let child_row = tree
        .rows
        .iter()
        .find(|r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "child"));

    let parent_row = parent_row.expect("parent session row");
    let child_row = child_row.expect("child session row");
    assert!(parent_row.expandable, "{:#?}", tree.rows);
    assert!(
        child_row.depth > parent_row.depth,
        "child should nest under parent in graph grouping: {:#?}",
        tree.rows
    );
}

#[test]
fn repo_grouping_keeps_regular_lineage_sessions_flat_by_location() {
    let parent = agent_session_with_kind(
        "claude-code",
        "/state",
        "parent",
        Some("/work/repo"),
        None,
        None,
    );
    let child = agent_session_with_kind(
        "claude-code",
        "/state",
        "child",
        Some("/work/repo"),
        None,
        None,
    );
    let parent_id = parent.id();
    let child_id = child.id();
    let checkout = checkout_node("/work/repo/.git", "/work/repo");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![parent, child, checkout],
        candidate_links: vec![parent_session_link(&child_id, &parent_id)],
        ..GraphSnapshot::empty()
    });

    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Repo,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let session_rows: Vec<_> = tree
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .collect();
    assert_eq!(session_rows.len(), 2, "{:#?}", tree.rows);
    assert_eq!(session_rows[0].depth, session_rows[1].depth);
    assert!(
        session_rows.iter().all(|row| !row.expandable),
        "repo grouping should not expose lineage disclosure rows: {:#?}",
        tree.rows
    );
}

// ---- ADR 0057 / H-PIN-016 pin row integration ---------------

fn pin_candidate(
    id: &str,
    harness: &str,
    cwd: &str,
    mux_name: &str,
    provenance: Provenance,
    binding: Option<PinBinding>,
) -> crate::model::PinCandidate {
    crate::model::PinCandidate {
        id: id.to_string(),
        display_name: id.to_string(),
        harness: harness.to_string(),
        cwd: cwd.to_string(),
        mux: crate::model::PinMuxRef {
            backend: "tmux".to_string(),
            name: mux_name.to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance,
        store_path: "/tmp/conspectus.toml".to_string(),
        binding,
    }
}

fn build_tree(snapshot: &GraphSnapshot) -> RowTree {
    build(SessionsBuildInputs {
        snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    })
}

#[test]
fn none_grouping_floats_bound_pinned_session_rows_to_top() {
    let session_id = AgentSessionId::new("codex", "/state", "older");
    let mux_id = MuxSessionId::new("tmux:older");
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "older",
        Some("/home/op/work/repo"),
        100,
    ));
    snapshot.nodes.push(agent_session_with_activity(
        "codex",
        "/state",
        "newer",
        Some("/home/op/work/repo"),
        200,
    ));
    snapshot.pins.push(pin_candidate(
        "older-pin",
        "codex",
        "/home/op/work/repo",
        "older",
        Provenance::LocalPin,
        Some(PinBinding::Bound {
            mux: mux_id,
            session: session_id,
        }),
    ));

    let tree = build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::None,
        home: Some(home().as_path()),
        now: Some(300),
        cwd: None,
        filter: RowFilter::default(),
    });

    let sessions: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 2, "{:#?}", tree.rows);
    assert_eq!(sessions[0].session.session_key, "older");
    assert_eq!(sessions[0].pin_id.as_deref(), Some("older-pin"));
    assert_eq!(sessions[1].session.session_key, "newer");
}

#[test]
fn unbound_pin_emits_synthetic_pins_group_and_placeholder_session_row() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::Unbound),
    ));

    let tree = build_tree(&snapshot);

    // Synthetic group + the one pin row.
    let pin_group_idx = tree
        .rows
        .iter()
        .position(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"))
        .expect("pins group emitted");
    assert!(matches!(&tree.rows[pin_group_idx].kind, RowKind::Group(_)));

    let pin_row = tree
        .rows
        .iter()
        .find(|row| matches!(&row.id, RowId::Pin { pin_id } if pin_id == "ingest"))
        .expect("pin row emitted");
    match &pin_row.kind {
        RowKind::AgentSession(row) => {
            assert_eq!(row.session.session_key, "ingest");
            assert_eq!(row.harness_label, "codex");
            assert_eq!(row.cwd_display.as_deref(), Some("~/work/repo"));
            assert_eq!(row.preview.as_deref(), Some("~/work/repo"));
            assert_eq!(row.title, None);
            assert_eq!(row.alias.as_deref(), Some("ingest"));
            assert_eq!(row.pin_id.as_deref(), Some("ingest"));
            assert!(matches!(&row.primary_node, NodeId::Pin(pin) if pin.id == "ingest"));
        }
        other => panic!("expected placeholder AgentSession row, got {other:?}"),
    }
    assert_eq!(pin_row.depth, 1);
}

#[test]
fn unbound_pin_placeholder_also_emits_in_matching_repo_bucket() {
    // When a pin's cwd lives under a known checkout, the
    // placeholder row should appear under the project bucket in
    // addition to the synthetic Pins group at top — mirroring how
    // bound pinned sessions show up in both contexts. The pin's
    // cwd here matches the repo's checkout root.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            repo("/home/op/work/repo/.git"),
            worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::Unbound),
    ));

    let tree = build_tree(&snapshot);

    let pin_rows: Vec<_> = tree
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| matches!(&row.id, RowId::Pin { pin_id } if pin_id == "ingest"))
        .collect();
    assert_eq!(
        pin_rows.len(),
        2,
        "placeholder should appear in both Pins group and repo bucket: {:#?}",
        tree.rows,
    );

    let pin_group_idx = tree
        .rows
        .iter()
        .position(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"))
        .expect("Pins group emitted");
    let repo_group_idx = tree
        .rows
        .iter()
        .position(|row| matches!(&row.id, RowId::Group(NodeId::Repo(_))))
        .expect("repo group emitted");
    assert!(
        pin_group_idx < repo_group_idx,
        "Pins group should precede the repo bucket: pins={pin_group_idx} repo={repo_group_idx}",
    );

    let (first_idx, _) = pin_rows[0];
    let (second_idx, _) = pin_rows[1];
    assert!(
        first_idx > pin_group_idx && first_idx < repo_group_idx,
        "first placeholder belongs to the Pins group: idx={first_idx}",
    );
    assert!(
        second_idx > repo_group_idx,
        "second placeholder belongs to the repo bucket: idx={second_idx}",
    );
}

#[test]
fn unbound_pin_placeholder_seeds_empty_repo_bucket() {
    // A pin whose cwd maps to a known checkout, but that
    // checkout has no live sessions, should still cause the
    // project header to render so the operator sees the pin
    // alongside its declared cwd. Without the pre-seed, the
    // bucket map would be empty for that key and no header
    // would emit.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            repo("/home/op/work/repo/.git"),
            worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::Unbound),
    ));

    let tree = build_tree(&snapshot);
    let repo_group = tree
        .rows
        .iter()
        .find(|row| matches!(&row.id, RowId::Group(NodeId::Repo(_))));
    assert!(
        repo_group.is_some(),
        "repo bucket header should emit even with no sessions: {:#?}",
        tree.rows,
    );
}

#[test]
fn unbound_pin_with_unknown_cwd_appears_only_in_pins_group() {
    // No checkout covers the pin's cwd, so the bucket lookup
    // returns None and the placeholder only surfaces in the
    // synthetic Pins group — same as before this slice.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/elsewhere/no-checkout",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::Unbound),
    ));

    let tree = build_tree(&snapshot);
    let pin_rows = tree
        .rows
        .iter()
        .filter(|row| matches!(&row.id, RowId::Pin { pin_id } if pin_id == "ingest"))
        .count();
    assert_eq!(pin_rows, 1, "{:#?}", tree.rows);
}

#[test]
fn stale_mux_pin_emits_placeholder_session_row() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::StaleMux {
            mux: MuxSessionId::new("tmux:ingest"),
        }),
    ));

    let tree = build_tree(&snapshot);
    let row = tree
        .rows
        .iter()
        .find_map(|r| match &r.kind {
            RowKind::AgentSession(session) if session.pin_id.as_deref() == Some("ingest") => {
                Some(session.clone())
            }
            _ => None,
        })
        .expect("placeholder session row");
    assert_eq!(row.preview.as_deref(), Some("~/work/repo"));
    assert_eq!(row.title, None);
    assert!(matches!(&row.primary_node, NodeId::Pin(pin) if pin.id == "ingest"));
}

#[test]
fn bound_pin_appears_in_pins_group_and_marks_agent_session_row() {
    // The resolver synthesizes a LinkedToMux candidate when a pin
    // binds; we mimic that here by:
    // - adding the session and mux nodes
    // - adding a LinkedToMux candidate (the discovered one)
    // - declaring the pin with `binding = Bound`
    // Bound pins appear as agent-session rows in BOTH the
    // synthetic "Pins" group (top of view) AND the regular
    // project group so the operator sees the same session shape
    // in both contexts.
    let session_id = AgentSessionId::new("codex", "/state", "alpha");
    let mux_id = MuxSessionId::new("tmux:ingest");
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            repo("/home/op/work/repo/.git"),
            worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
            agent_session(
                "codex",
                "/state",
                "alpha",
                Some("/home/op/work/repo"),
                None,
                None,
            ),
            mux_node("tmux", "tmux:ingest"),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.candidate_links.push(linked_to_mux(
        &NodeId::AgentSession(session_id.clone()),
        &NodeId::MuxSession(mux_id.clone()),
        Provenance::StrongDiscovered,
        "discovered",
    ));
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "ingest",
        Provenance::LocalPin,
        Some(PinBinding::Bound {
            mux: mux_id,
            session: session_id.clone(),
        }),
    ));

    let tree = build_tree(&snapshot);

    // Bound pin appears in the synthetic Pins group as the
    // realized session row, not as a pin config row.
    let pin_group_idx = tree
        .rows
        .iter()
        .position(|r| matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins"))
        .expect("Pins group present");
    let pinned_session_row = tree
        .rows
        .iter()
        .skip(pin_group_idx + 1)
        .take_while(|r| r.depth > 0)
        .find_map(|r| match &r.kind {
            RowKind::AgentSession(s) if s.session == session_id => Some(s.clone()),
            _ => None,
        })
        .expect("session row in synthetic Pins group");
    assert_eq!(pinned_session_row.pin_id.as_deref(), Some("ingest"));

    // The Pins group must come BEFORE the regular session
    // groups so it's the first thing the operator sees.
    let first_non_pin_group_idx = tree.rows.iter().position(|r| {
        matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins" || *tag == "ungrouped")
            .then_some(false)
            .unwrap_or(
                matches!(&r.kind, RowKind::Group(_))
                    && !matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins"),
            )
    });
    if let Some(idx) = first_non_pin_group_idx {
        assert!(
            pin_group_idx < idx,
            "Pins group should precede other groups (Pins at {pin_group_idx}, first other at {idx})",
        );
    }

    // The natural agent-session row still carries the pin
    // marker so the operator sees the pin in-place too.
    let session_row = tree
        .rows
        .iter()
        .skip(pin_group_idx + 2)
        .find_map(|r| match &r.kind {
            RowKind::AgentSession(s) if s.session == session_id => Some(s.clone()),
            _ => None,
        })
        .expect("agent session row emitted");
    assert_eq!(session_row.pin_id.as_deref(), Some("ingest"));
}

#[test]
fn mixed_pin_states_all_emit_in_synthetic_group() {
    // All declared pins surface in the Pins group regardless
    // of binding state. Bound pins use their existing session
    // row shape; unbound pins use placeholder session rows
    // because no live session exists yet.
    let session_id = AgentSessionId::new("codex", "/state", "alpha");
    let mux_id = MuxSessionId::new("tmux:bound");
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            repo("/home/op/work/repo/.git"),
            worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
            agent_session(
                "codex",
                "/state",
                "alpha",
                Some("/home/op/work/repo"),
                None,
                None,
            ),
            mux_node("tmux", "tmux:bound"),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.candidate_links.push(linked_to_mux(
        &NodeId::AgentSession(session_id.clone()),
        &NodeId::MuxSession(mux_id.clone()),
        Provenance::StrongDiscovered,
        "discovered",
    ));
    snapshot.pins.extend([
        pin_candidate(
            "bound-one",
            "codex",
            "/home/op/work/repo",
            "bound",
            Provenance::LocalPin,
            Some(PinBinding::Bound {
                mux: mux_id,
                session: session_id,
            }),
        ),
        pin_candidate(
            "free-one",
            "codex",
            "/home/op/other",
            "free-one",
            Provenance::LocalPin,
            Some(PinBinding::Unbound),
        ),
        pin_candidate(
            "free-two",
            "claude-code",
            "/home/op/another",
            "free-two",
            Provenance::GlobalPin,
            Some(PinBinding::Unbound),
        ),
    ]);

    let tree = build_tree(&snapshot);

    let pins_group_idx = tree
        .rows
        .iter()
        .position(|r| matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins"))
        .expect("Pins group present");
    let pins_group_rows: Vec<_> = tree
        .rows
        .iter()
        .skip(pins_group_idx + 1)
        .take_while(|r| r.depth > 0)
        .filter_map(|r| match &r.kind {
            RowKind::AgentSession(s) => s.pin_id.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(pins_group_rows, vec!["bound-one", "free-one", "free-two"]);

    let bound_marker = tree.rows.iter().any(|row| {
        matches!(
            &row.kind,
            RowKind::AgentSession(s) if s.pin_id.as_deref() == Some("bound-one")
        )
    });
    assert!(bound_marker, "bound pin marker missing from session row");
}

#[test]
fn resolver_drives_pin_rows_end_to_end() {
    // Smoke test that wires discovery + resolver together: an
    // unbound pin (no matching mux in the snapshot) should
    // surface as a placeholder `RowKind::AgentSession` after
    // `resolve_snapshot` runs.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(pin_candidate(
        "ingest",
        "codex",
        "/home/op/work/repo",
        "missing-mux",
        Provenance::LocalPin,
        None,
    ));

    let resolved = resolve_snapshot(snapshot);
    // Sanity: the resolver populated the binding to Unbound.
    assert!(matches!(
        resolved.pins[0].binding,
        Some(PinBinding::Unbound)
    ));

    let tree = build(SessionsBuildInputs {
        snapshot: &resolved,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let has_pin_row = tree.rows.iter().any(|r| {
        matches!(
            &r.kind,
            RowKind::AgentSession(session)
                if session.pin_id.as_deref() == Some("ingest")
                    && matches!(&session.primary_node, NodeId::Pin(_))
        )
    });
    assert!(
        has_pin_row,
        "expected a pin row after resolver run: {:#?}",
        tree.rows
    );
}

// ----- P8-015: title-disambiguation in the row tree ----------------

fn session_keys_with_disambiguating_titles(tree: &RowTree) -> Vec<String> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(s) if s.title_disambiguates => {
                Some(s.session.session_key.clone())
            }
            _ => None,
        })
        .collect()
}

fn session_keys_in_order(tree: &RowTree) -> Vec<String> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(s) => Some(s.session.session_key.clone()),
            _ => None,
        })
        .collect()
}

fn p8_015_snapshot_with_sessions(cwd: &str, specs: &[(&str, &str, Option<&str>)]) -> RowTree {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(repo(cwd));
    snapshot.nodes.push(worktree(cwd, cwd));
    for (harness, key, title) in specs {
        snapshot.nodes.push(agent_session(
            harness,
            "/state",
            key,
            Some(cwd),
            *title,
            None,
        ));
    }
    let snapshot = resolve_snapshot(snapshot);
    build(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(home().as_path()),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    })
}

#[test]
fn title_disambiguation_off_for_single_session_per_harness() {
    // Case 1: a project group with one codex and one opencode
    // session — both already disambiguated by harness label, so
    // neither row should incorporate its title.
    let tree = p8_015_snapshot_with_sessions(
        "/home/op/src/proj",
        &[
            ("codex", "alpha", Some("scratch draft")),
            ("opencode", "beta", Some("review pass")),
        ],
    );
    let flagged = session_keys_with_disambiguating_titles(&tree);
    assert!(
        flagged.is_empty(),
        "no row should be flagged when harness already disambiguates: {flagged:?}"
    );
}

#[test]
fn title_disambiguation_on_for_same_harness_siblings_with_distinct_titles() {
    // Case 2: two codex sessions in the same project group, both
    // carrying distinct titles — both rows are flagged.
    let tree = p8_015_snapshot_with_sessions(
        "/home/op/src/proj",
        &[
            ("codex", "alpha", Some("scratch draft")),
            ("codex", "beta", Some("review pass")),
        ],
    );
    let mut flagged = session_keys_with_disambiguating_titles(&tree);
    flagged.sort();
    assert_eq!(
        flagged,
        vec!["alpha".to_string(), "beta".to_string()],
        "both same-harness siblings should be flagged"
    );
}

#[test]
fn title_disambiguation_only_flags_the_session_with_a_title() {
    // Case 3: two codex sessions in the same project group, but
    // only one has a title — only the titled row is flagged. The
    // untitled row stays clean (no `tree_label`).
    let tree = p8_015_snapshot_with_sessions(
        "/home/op/src/proj",
        &[
            ("codex", "alpha", Some("scratch draft")),
            ("codex", "beta", None),
        ],
    );
    let flagged = session_keys_with_disambiguating_titles(&tree);
    assert_eq!(flagged, vec!["alpha".to_string()]);
    // The untitled row still emits a normal AgentSessionRow — it
    // just doesn't gain a tree label.
    let alpha = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(s) if s.session.session_key == "alpha" => Some(s),
            _ => None,
        })
        .expect("alpha row present");
    let beta = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(s) if s.session.session_key == "beta" => Some(s),
            _ => None,
        })
        .expect("beta row present");
    assert_eq!(alpha.tree_label(), Some("scratch draft"));
    assert_eq!(beta.tree_label(), None);
}

#[test]
fn title_disambiguation_flips_deterministically_on_refresh_without_reordering() {
    // Case 4: a project starts with one codex session whose title
    // is hidden (no collision). A refresh adds a second codex
    // session with a different title; the previously-clean row
    // gains its title and the new row arrives with a title too,
    // both in the same deterministic position the row tree built
    // them in. Sort order across the refresh is preserved (the
    // collision flag never re-keys the sort).
    let first = p8_015_snapshot_with_sessions(
        "/home/op/src/proj",
        &[("codex", "alpha", Some("scratch draft"))],
    );
    assert!(
        session_keys_with_disambiguating_titles(&first).is_empty(),
        "single-session group should not flag titles"
    );
    let pre_order = session_keys_in_order(&first);
    assert_eq!(pre_order, vec!["alpha".to_string()]);

    let second = p8_015_snapshot_with_sessions(
        "/home/op/src/proj",
        &[
            ("codex", "alpha", Some("scratch draft")),
            ("codex", "beta", Some("review pass")),
        ],
    );
    let mut flagged = session_keys_with_disambiguating_titles(&second);
    flagged.sort();
    assert_eq!(
        flagged,
        vec!["alpha".to_string(), "beta".to_string()],
        "both rows should gain the disambiguation flag once a sibling appears"
    );
    let post_order = session_keys_in_order(&second);
    // `alpha` retains its slot; `beta` appends. The session-key
    // sort order is alphabetical, so this is the stable shape.
    assert_eq!(
        post_order,
        vec!["alpha".to_string(), "beta".to_string()],
        "row order must stay deterministic across the refresh"
    );
}
