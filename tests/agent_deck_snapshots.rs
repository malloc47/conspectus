//! Integration coverage for the agent-deck multi-repo workspace adapter.
//!
//! Per-provider behavior is unit-tested in
//! `src/discovery/agent_deck.rs`. This file exercises the wired
//! pipeline: `discover_local_with` with `agent_deck_root` set picks
//! up the fixture, emits a `Workspace` node carrying the provider
//! tag, and produces two active `WorkspaceContainsRepo` candidate
//! links. The agent-table rendering surface is covered separately
//! by `output::table::tests::sessions_projection_workspace_*`.

use std::fs;
use std::path::Path;
use std::process::Command;

use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::model::{GraphNode, RelationKind};
use tempfile::TempDir;

#[test]
fn discover_local_with_picks_up_agent_deck_workspace_from_configured_root() {
    let workspace_temp = TempDir::new().expect("workspace temp");
    let external = TempDir::new().expect("external temp");
    let worktrees_root = workspace_temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("multi-task");
    fs::create_dir_all(&workspace_id_dir).expect("workspace id dir");

    let repo_a = init_repo(external.path(), "atelier");
    let repo_b = init_repo(external.path(), "conspectus");
    symlink_dir(&repo_a, &workspace_id_dir.join("atelier"));
    symlink_dir(&repo_b, &workspace_id_dir.join("conspectus"));

    // Scan root is an unrelated empty temp tree — the adapter must
    // pick up agent-deck via its own configured root, not via the
    // user's scan path.
    let scan_root = TempDir::new().expect("scan temp");
    let config = LocalDiscoveryConfig::empty().with_agent_deck_root(&worktrees_root);
    let snapshot = discover_local_with([scan_root.path()], config).expect("discover");

    let workspace = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::Workspace(w) => Some(w),
            _ => None,
        })
        .expect("workspace node emitted");
    assert_eq!(workspace.provider.as_deref(), Some("agent-deck"));
    assert_eq!(workspace.name.as_deref(), Some("multi-task"));

    let member_count = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .count();
    assert_eq!(member_count, 2);
}

#[test]
fn discover_local_with_no_agent_deck_root_emits_no_workspace() {
    let scan_root = TempDir::new().expect("scan temp");
    let config = LocalDiscoveryConfig::empty();
    let snapshot = discover_local_with([scan_root.path()], config).expect("discover");

    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| !matches!(node, GraphNode::Workspace(_))),
        "agent-deck discovery should be a no-op when no root is configured"
    );
}

fn init_repo(parent: &Path, name: &str) -> std::path::PathBuf {
    let root = parent.join(name);
    fs::create_dir(&root).expect("create repo dir");
    git(&root, &["init", "--initial-branch", "main"]);
    git(&root, &["config", "user.name", "Conspectus Test"]);
    git(
        &root,
        &["config", "user.email", "conspectus@example.invalid"],
    );
    fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-m", "initial"]);
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create symlink");
}

#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("create symlink");
}
