//! Representative fixture for Atelier delegation work.
//!
//! The fixture combines Atelier workspace metadata, fork metadata, a fake
//! codex session, and a fake tmux session, then snapshots both the full graph
//! JSON and all three session table projections. Atelier-side delegation can
//! use this as a concrete comparison target.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use conspectus::api::{
    GraphNode, LocalDiscoveryConfig, Projection, discover_local_with, render_graph_json,
    resolve_snapshot, table,
};
use conspectus::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use conspectus::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};
use conspectus::discovery::tmux::FakeTmux;

#[test]
fn atelier_delegation_comparison_fixture_snapshot() {
    let fixture = AtelierDelegationFixture::new();
    fixture.init_repo("repo-a");
    fixture.init_repo("repo-b");
    fixture.write_atelier_config(
        r#"
[workspace]
name = "atelier-demo"

[[repos]]
name = "repo-a"
path = "/sources/repo-a"

[[repos]]
name = "repo-b"
path = "/sources/repo-b"
"#,
    );
    fixture.write_fork_index(
        r#"
[[forks]]
name = "alpha"
created-epoch = 1
mode = "worktree"
root = ".atelier/forks/alpha"
state = "isolated"

[[forks.repos]]
name = "repo-a"
source = "/sources/repo-a"
parent-worktree = "repo-a"
fork-worktree = ".atelier/forks/alpha/repo-a"
branch = "fork/alpha/repo-a"
forked = true

[[forks.harness]]
key = "codex"
source-session = "parent-session"
fork-session = "child-session"
capability = "native"
"#,
    );

    let fork_cwd = fixture.root().join(".atelier/forks/alpha/repo-a");
    fs::create_dir_all(&fork_cwd).expect("fork cwd");
    fixture.write_codex_session("session-alpha", &fork_cwd);

    let tmux_stdout = format!(
        "alpha\t{cwd}\t1700000500\t1700000000\n",
        cwd = fork_cwd.display()
    );
    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root())
        .with_tmux_runner(FakeTmux::with_sessions(tmux_stdout));

    let snapshot = discover_local_with([fixture.root()], config).expect("discover");
    let snapshot = resolve_snapshot(snapshot);
    let mut table_snapshot = snapshot.clone();
    fixture.normalize_snapshot_paths(&mut table_snapshot);

    let graph = render_graph_json(&snapshot).expect("render graph");
    insta::assert_snapshot!(
        "atelier_delegation_comparison_graph_json",
        fixture.normalize(&graph)
    );
    insta::assert_snapshot!(
        "atelier_delegation_comparison_agent_table",
        table::render(&table_snapshot, Projection::Agent)
    );
    insta::assert_snapshot!(
        "atelier_delegation_comparison_mux_table",
        table::render(&table_snapshot, Projection::Mux)
    );
    insta::assert_snapshot!(
        "atelier_delegation_comparison_union_table",
        table::render(&table_snapshot, Projection::Union)
    );
}

struct AtelierDelegationFixture {
    temp: tempfile::TempDir,
    state_dir: PathBuf,
}

impl AtelierDelegationFixture {
    fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("temp");
        let state_dir = temp.path().join(".state");
        fs::create_dir_all(&state_dir).expect("state dir");
        Self { temp, state_dir }
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn codex_state_root(&self) -> PathBuf {
        self.state_dir.join("codex")
    }

    fn init_repo(&self, name: &str) -> PathBuf {
        let root = self.root().join(name);
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

    fn write_atelier_config(&self, text: &str) {
        fs::write(self.root().join("atelier.toml"), text).expect("write atelier config");
    }

    fn write_fork_index(&self, text: &str) {
        let path = self.root().join(".atelier/forks/index.toml");
        fs::create_dir_all(path.parent().expect("index parent")).expect("create fork dir");
        fs::write(path, text).expect("write fork index");
    }

    fn write_codex_session(&self, id: &str, cwd: &Path) {
        let record = CodexSessionRecord::new(id).with_cwd(cwd.to_str().expect("utf8 cwd"));
        HarnessFixture::at(&self.state_dir)
            .write_codex_session(&record)
            .expect("write codex session");
    }

    fn normalize(&self, rendered: &str) -> String {
        let canonical = self
            .root()
            .canonicalize()
            .unwrap_or_else(|_| self.root().to_path_buf());
        let mut out = rendered.replace(&canonical.to_string_lossy().to_string(), "/fixture");
        out = out.replace(&self.root().to_string_lossy().to_string(), "/fixture");
        if let Some(name) = self.root().file_name().and_then(|s| s.to_str()) {
            out = out.replace(name, "fixture");
        }
        out
    }

    fn normalize_snapshot_paths(&self, snapshot: &mut conspectus::api::GraphSnapshot) {
        for node in &mut snapshot.nodes {
            match node {
                GraphNode::Repo(node) => {
                    node.common_dir = self.normalize(&node.common_dir);
                    node.source_paths = node
                        .source_paths
                        .iter()
                        .map(|path| self.normalize(path))
                        .collect();
                }
                GraphNode::Worktree(node) => {
                    node.root = self.normalize(&node.root);
                    node.git_dir = node.git_dir.as_ref().map(|path| self.normalize(path));
                }
                GraphNode::Workspace(node) => {
                    node.root = self.normalize(&node.root);
                }
                GraphNode::AgentSession(node) => {
                    node.cwd = node.cwd.as_ref().map(|path| self.normalize(path));
                }
                GraphNode::MuxSession(node) => {
                    node.cwd = node.cwd.as_ref().map(|path| self.normalize(path));
                }
                GraphNode::Branch(_) | GraphNode::Fork(_) | GraphNode::ForgePr(_) => {}
            }
        }
    }
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
