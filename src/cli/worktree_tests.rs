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

// ---- close-down helpers (H-WT-006) ----

fn pin_node(id: &str, cwd: &str, store: &str) -> GraphNode {
    use conspectus::model::{PinId, PinMuxRef, PinNode, Provenance};
    GraphNode::Pin(PinNode {
        id: PinId::new(id),
        display_name: id.to_string(),
        harness: "codex".to_string(),
        cwd: cwd.to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: id.to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: store.to_string(),
        binding: None,
    })
}

#[test]
fn live_mux_teardown_targets_carries_native_id_and_pane_pid() {
    let mut snap = snapshot_with_worktree_and_sessions();
    // Give the in-worktree mux a pane pid so the graceful phase has a
    // target.
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(MuxSessionId::new("tmux:feat2"), "tmux", "feat2")
            .with_active_pane_current_path("/wt/feature/deep".to_string())
            .with_active_pane_pid(4242),
    ));
    let targets = live_mux_teardown_targets(&snap, "/wt/feature");
    // The base snapshot's `feat` mux plus the `feat2` we added.
    assert_eq!(targets.len(), 2, "{targets:?}");
    let feat2 = targets.iter().find(|t| t.native_id == "feat2").unwrap();
    assert_eq!(feat2.pane_pid, Some(4242));
    assert_eq!(feat2.socket_name, None);
    // Sessions outside the worktree are excluded.
    assert!(live_mux_teardown_targets(&snap, "/wt/unused").is_empty());
}

#[test]
fn pins_rooted_in_selects_pins_under_the_worktree() {
    let mut snap = snapshot_with_worktree_and_sessions();
    snap.nodes.push(pin_node(
        "inside",
        "/wt/feature/src",
        "/wt/feature/.conspectus.toml",
    ));
    snap.nodes.push(pin_node(
        "outside",
        "/elsewhere",
        "/elsewhere/.conspectus.toml",
    ));
    let pins = pins_rooted_in(&snap, "/wt/feature");
    assert_eq!(pins.len(), 1, "{pins:?}");
    assert_eq!(pins[0].1, "inside");
    assert_eq!(pins[0].0, "/wt/feature/.conspectus.toml");
}

#[test]
fn should_confirm_honors_policy_and_live_state() {
    use conspectus::config::TeardownConfirm;
    assert!(should_confirm(TeardownConfirm::Always, false));
    assert!(should_confirm(TeardownConfirm::Always, true));
    assert!(!should_confirm(TeardownConfirm::Live, false));
    assert!(should_confirm(TeardownConfirm::Live, true));
    assert!(!should_confirm(TeardownConfirm::Never, true));
    assert!(!should_confirm(TeardownConfirm::Never, false));
}
