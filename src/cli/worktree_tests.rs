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
