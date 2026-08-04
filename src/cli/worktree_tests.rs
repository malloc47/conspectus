use super::*;
use conspectus::model::{CheckoutId, CheckoutNode, WorktreeMeta};

fn checkout(repo: &str, path: &str, branch: Option<&str>, meta: WorktreeMeta) -> GraphNode {
    let repo_id = RepoId::new(repo);
    let mut node =
        CheckoutNode::new(CheckoutId::new(repo_id.clone(), path), path).with_worktree(meta);
    node.current_branch = branch.map(|b| conspectus::model::BranchId::new(repo_id, b));
    GraphNode::Checkout(node)
}

#[test]
fn collect_skips_checkouts_without_worktree_metadata() {
    let mut snap = GraphSnapshot::empty();
    // A plain checkout (no worktree meta) is ignored.
    snap.nodes.push(GraphNode::Checkout(CheckoutNode::new(
        CheckoutId::new(RepoId::new("/r/.git"), "/r"),
        "/r",
    )));
    assert!(collect_worktrees(&snap).is_empty());
}

#[test]
fn collect_groups_by_repo_primary_first() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/src/wt/feature",
        Some("refs/heads/feature"),
        WorktreeMeta::linked(),
    ));
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/src/app",
        Some("refs/heads/main"),
        WorktreeMeta::primary(),
    ));

    let by_repo = collect_worktrees(&snap);
    assert_eq!(by_repo.len(), 1);
    let rows = by_repo.values().next().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].kind, WorktreeKind::Primary, "primary sorts first");
    assert_eq!(rows[0].branch.as_deref(), Some("main"));
    assert_eq!(rows[1].branch.as_deref(), Some("feature"));
}

#[test]
fn render_marks_kind_and_lock_and_prune() {
    let mut snap = GraphSnapshot::empty();
    let mut locked = WorktreeMeta::linked();
    locked.locked = Some("agent".to_string());
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/src/app",
        Some("refs/heads/main"),
        WorktreeMeta::primary(),
    ));
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/src/wt/bugfix",
        Some("refs/heads/bugfix"),
        locked,
    ));

    let out = render_worktrees(&collect_worktrees(&snap));
    assert!(out.contains("app\n"), "repo header basename: {out}");
    assert!(out.contains("primary"), "{out}");
    assert!(out.contains("linked locked"), "lock flag rendered: {out}");
}

#[test]
fn render_empty_reports_no_worktrees() {
    let empty = collect_worktrees(&GraphSnapshot::empty());
    assert_eq!(render_worktrees(&empty), "no worktrees found\n");
}

#[test]
fn detached_worktree_renders_placeholder_branch() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/src/app",
        None,
        WorktreeMeta::primary(),
    ));
    let out = render_worktrees(&collect_worktrees(&snap));
    assert!(out.contains("(detached)"), "{out}");
}

#[test]
fn repo_display_name_strips_dot_git() {
    assert_eq!(
        repo_display_name(&RepoId::new("/src/conspectus/.git")),
        "conspectus"
    );
    assert_eq!(repo_display_name(&RepoId::new("/src/bare.git")), "bare");
    assert_eq!(repo_display_name(&RepoId::new("/src/plain")), "plain");
}

// ---- H-WT-004a: rm guard helpers ----

use conspectus::model::{AgentSessionId, AgentSessionNode, MuxSessionId, MuxSessionNode};

fn snapshot_with_worktree_and_sessions() -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    // A linked worktree at /wt/feature checking out `feature`.
    snap.nodes.push(checkout(
        "/src/app/.git",
        "/wt/feature",
        Some("refs/heads/feature"),
        WorktreeMeta::linked(),
    ));
    // An agent session whose cwd is inside the worktree.
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(AgentSessionId::new("codex", "/state", "s1"), "codex")
            .with_cwd("/wt/feature/src".to_string()),
    ));
    // A mux session whose active pane is inside the worktree.
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(MuxSessionId::new("tmux:feat"), "tmux", "feat")
            .with_active_pane_current_path("/wt/feature".to_string()),
    ));
    // A session elsewhere — must NOT count.
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(AgentSessionId::new("codex", "/state", "other"), "codex")
            .with_cwd("/somewhere/else".to_string()),
    ));
    snap
}

#[test]
fn worktree_path_for_branch_resolves_short_name() {
    let snap = snapshot_with_worktree_and_sessions();
    assert_eq!(
        worktree_path_for_branch(&snap, "feature").as_deref(),
        Some("/wt/feature"),
    );
    assert_eq!(worktree_path_for_branch(&snap, "nonexistent"), None);
}

#[test]
fn live_sessions_in_worktree_finds_agent_and_mux_inside() {
    let snap = snapshot_with_worktree_and_sessions();
    let sessions = live_sessions_in_worktree(&snap, "/wt/feature");
    assert_eq!(
        sessions.len(),
        2,
        "agent + mux inside, other excluded: {sessions:?}"
    );
    assert!(
        sessions
            .iter()
            .any(|s| s.contains("codex") && s.contains("agent"))
    );
    assert!(
        sessions
            .iter()
            .any(|s| s.contains("feat") && s.contains("mux"))
    );
}

#[test]
fn live_sessions_in_worktree_empty_when_none_inside() {
    let snap = snapshot_with_worktree_and_sessions();
    assert!(live_sessions_in_worktree(&snap, "/wt/unused").is_empty());
}
