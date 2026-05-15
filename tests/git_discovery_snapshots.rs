use std::path::PathBuf;

use conspectus::discovery::git::{GitProbeResult, GitRemote, fragment_from_probe};
use conspectus::output::render_graph_json;
use conspectus::resolve::resolve_snapshot;

#[test]
fn plain_repo_git_mapping_snapshot() {
    assert_probe_snapshot(
        "plain_repo_git_mapping",
        GitProbeResult {
            common_dir: PathBuf::from("/workspace/repo/.git"),
            worktree_root: PathBuf::from("/workspace/repo"),
            git_dir: PathBuf::from("/workspace/repo/.git"),
            branch_ref: Some("refs/heads/main".to_string()),
            upstream: Some("origin/main".to_string()),
            remotes: vec![GitRemote {
                name: "origin".to_string(),
                url: "git@example.com:owner/repo.git".to_string(),
            }],
        },
    );
}

#[test]
fn detached_worktree_git_mapping_snapshot() {
    assert_probe_snapshot(
        "detached_worktree_git_mapping",
        GitProbeResult {
            common_dir: PathBuf::from("/workspace/repo/.git"),
            worktree_root: PathBuf::from("/workspace/repo"),
            git_dir: PathBuf::from("/workspace/repo/.git"),
            branch_ref: None,
            upstream: None,
            remotes: Vec::new(),
        },
    );
}

#[test]
fn linked_worktree_git_mapping_snapshot() {
    assert_probe_snapshot(
        "linked_worktree_git_mapping",
        GitProbeResult {
            common_dir: PathBuf::from("/workspace/repo/.git"),
            worktree_root: PathBuf::from("/workspace/repo-linked"),
            git_dir: PathBuf::from("/workspace/repo/.git/worktrees/repo-linked"),
            branch_ref: Some("refs/heads/feature".to_string()),
            upstream: None,
            remotes: vec![GitRemote {
                name: "origin".to_string(),
                url: "git@example.com:owner/repo.git".to_string(),
            }],
        },
    );
}

fn assert_probe_snapshot(name: &str, probe: GitProbeResult) {
    let snapshot = resolve_snapshot(fragment_from_probe(&probe).into_snapshot());
    let rendered = render_graph_json(&snapshot).expect("render git graph");

    insta::assert_snapshot!(name, rendered);
}
