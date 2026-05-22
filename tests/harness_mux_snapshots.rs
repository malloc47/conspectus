//! End-to-end JSON snapshots for harness and tmux discovery wired into
//! `discover_local_with`. Each test sets up a single temp directory, points
//! every harness state root at it, optionally injects a `FakeTmux` runner, and
//! snapshots the rendered JSON after path normalization.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use conspectus::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use conspectus::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};
use conspectus::discovery::tmux::{FakeTmux, UnavailableReason};
use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::model::GraphNode;
use conspectus::output::render_graph_json;
use conspectus::resolve::resolve_snapshot;

#[test]
fn orphan_harness_session_snapshot() {
    let fixture = ScenarioFixture::new();
    fixture.write_codex_session("orphan-session", None);

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root());

    assert_snapshot(&fixture, "orphan_harness_session", config);
}

#[test]
fn mux_only_with_unavailable_harness_snapshot() {
    let fixture = ScenarioFixture::new();
    let config = LocalDiscoveryConfig::empty().with_tmux_runner(FakeTmux::with_sessions(
        "solo\t/fixture/work\t1700000500\t1700000000\n",
    ));

    assert_snapshot(&fixture, "mux_only_with_unavailable_harness", config);
}

#[test]
fn unavailable_tmux_yields_no_mux_nodes_snapshot() {
    let fixture = ScenarioFixture::new();
    let config = LocalDiscoveryConfig::empty()
        .with_tmux_runner(FakeTmux::unavailable(UnavailableReason::NoServer));

    assert_snapshot(&fixture, "unavailable_tmux_yields_no_mux_nodes", config);
}

#[test]
fn session_and_tmux_cwd_match_emits_linked_to_mux_snapshot() {
    let fixture = ScenarioFixture::new();
    let work = fixture.path().join("work");
    fs::create_dir_all(&work).expect("work dir");
    fixture.write_codex_session("session-x", Some(work.to_str().expect("utf8")));
    let stdout = format!("alpha\t{}\t1700000500\t1700000000\n", work.display());

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root())
        .with_tmux_runner(FakeTmux::with_sessions(stdout));

    assert_snapshot(&fixture, "session_and_tmux_cwd_match", config);
}

#[test]
fn observed_session_cwd_backfills_git_context_outside_scan_roots() {
    let fixture = ScenarioFixture::new();
    fixture.init_repo("work");
    let scan_root = fixture.path().join("scan");
    let nested_cwd = fixture.path().join("work/nested");
    fs::create_dir_all(&scan_root).expect("scan root");
    fs::create_dir_all(&nested_cwd).expect("nested cwd");
    fixture.write_codex_session("session-x", Some(nested_cwd.to_str().expect("utf8")));

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root());

    let snapshot = discover_local_with([scan_root], config).expect("discover");

    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Repo(repo) if repo.common_dir.ends_with("/work/.git")
        )),
        "observed cwd should backfill the repo node: {:#?}",
        snapshot.nodes
    );
    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Checkout(worktree) if worktree.root.ends_with("/work")
        )),
        "observed cwd should backfill the checkout/worktree node: {:#?}",
        snapshot.nodes
    );
    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Branch(branch) if branch.refname == "refs/heads/main"
        )),
        "observed cwd should backfill the branch node: {:#?}",
        snapshot.nodes
    );
}

#[test]
fn observed_linked_worktree_cwd_backfills_git_context_outside_scan_roots() {
    let fixture = ScenarioFixture::new();
    fixture.init_repo("repo");
    let linked = fixture.add_worktree("repo", "linked", "feature/linked");
    let scan_root = fixture.mkdir("scan");
    fixture.write_codex_session("session-linked", Some(path_str(&linked)));

    let snapshot = fixture.discover_from(scan_root, LocalDiscoveryConfig::empty());

    assert_worktree_root(&snapshot, &linked);
    assert_branch_ref(&snapshot, "refs/heads/feature/linked");
}

#[test]
fn observed_bare_repo_worktree_cwd_backfills_git_context_outside_scan_roots() {
    let fixture = ScenarioFixture::new();
    fixture.init_repo("seed");
    let bare = fixture.clone_bare("seed", "repo.git");
    let linked = fixture.add_bare_worktree(&bare, "bare-linked", "feature/bare-linked");
    let scan_root = fixture.mkdir("scan");
    fixture.write_codex_session("session-bare", Some(path_str(&linked)));

    let snapshot = fixture.discover_from(scan_root, LocalDiscoveryConfig::empty());

    assert_worktree_root(&snapshot, &linked);
    assert_branch_ref(&snapshot, "refs/heads/feature/bare-linked");
    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Repo(repo) if repo.common_dir == path_str(&bare)
        )),
        "bare common-dir should identify the repo: {:#?}",
        snapshot.nodes
    );
}

#[test]
fn missing_observed_session_cwd_does_not_create_git_context() {
    let fixture = ScenarioFixture::new();
    let scan_root = fixture.mkdir("scan");
    let missing = fixture.path().join("missing");
    fixture.write_codex_session("session-missing", Some(path_str(&missing)));

    let snapshot = fixture.discover_from(scan_root, LocalDiscoveryConfig::empty());

    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| !matches!(node, GraphNode::Repo(_) | GraphNode::Checkout(_))),
        "missing cwd should not create git context: {:#?}",
        snapshot.nodes
    );
}

#[test]
fn observed_mux_cwd_backfills_git_context_outside_scan_roots() {
    let fixture = ScenarioFixture::new();
    fixture.init_repo("mux-work");
    let scan_root = fixture.mkdir("scan");
    let work = fixture.path().join("mux-work");
    let stdout = format!("muxed\t{}\t1700000500\t1700000000\n", work.display());

    let snapshot = fixture.discover_from(
        scan_root,
        LocalDiscoveryConfig::empty().with_tmux_runner(FakeTmux::with_sessions(stdout)),
    );

    assert_worktree_root(&snapshot, &work);
    assert_branch_ref(&snapshot, "refs/heads/main");
}

#[test]
fn one_to_many_mux_candidates_preserved_snapshot() {
    let fixture = ScenarioFixture::new();
    let work = fixture.path().join("work");
    fs::create_dir_all(&work).expect("work dir");
    fixture.write_codex_session("session-x", Some(work.to_str().expect("utf8")));
    let stdout = format!(
        "alpha\t{cwd}\t1700000100\t1700000000\nbeta\t{cwd}\t1700000900\t1700000000\n",
        cwd = work.display()
    );

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root())
        .with_tmux_runner(FakeTmux::with_sessions(stdout));

    assert_snapshot(&fixture, "one_to_many_mux_candidates_preserved", config);
}

#[test]
fn fork_associated_session_and_unresolved_lineage_snapshot() {
    let fixture = ScenarioFixture::new();
    fixture.write_atelier_config(
        r#"
[workspace]
name = "atelier-demo"

[[repos]]
name = "repo-a"
path = "/sources/repo-a"
"#,
    );
    fixture.init_repo("repo-a");
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
    let fork_cwd = fixture.path().join(".atelier/forks/alpha/repo-a");
    fs::create_dir_all(&fork_cwd).expect("fork cwd");
    fixture.write_codex_session("session-x", Some(fork_cwd.to_str().expect("utf8")));

    let config = LocalDiscoveryConfig::empty()
        .with_harness_state_root(CODEX_HARNESS_KEY, fixture.codex_state_root());

    assert_snapshot(
        &fixture,
        "fork_associated_session_and_unresolved_lineage",
        config,
    );
}

fn assert_snapshot(fixture: &ScenarioFixture, name: &str, config: LocalDiscoveryConfig) {
    let snapshot = discover_local_with([fixture.path()], config).expect("discover");
    let rendered = render_graph_json(&resolve_snapshot(snapshot)).expect("render");
    let normalized = fixture.normalize(&rendered);
    insta::assert_snapshot!(name, normalized);
}

struct ScenarioFixture {
    temp: tempfile::TempDir,
    state_dir: std::path::PathBuf,
}

impl ScenarioFixture {
    fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("temp");
        let state_dir = temp.path().join(".state");
        fs::create_dir_all(&state_dir).expect("state dir");
        Self { temp, state_dir }
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }

    fn mkdir(&self, name: &str) -> PathBuf {
        let path = self.temp.path().join(name);
        fs::create_dir_all(&path).expect("create dir");
        path
    }

    fn codex_state_root(&self) -> std::path::PathBuf {
        self.state_dir.join("codex")
    }

    fn write_codex_session(&self, id: &str, cwd: Option<&str>) {
        let record = match cwd {
            Some(cwd) => CodexSessionRecord::new(id).with_cwd(cwd),
            None => CodexSessionRecord::new(id),
        };
        HarnessFixture::at(&self.state_dir)
            .write_codex_session(&record)
            .expect("write codex session");
    }

    fn init_repo(&self, name: &str) {
        let root = self.temp.path().join(name);
        fs::create_dir_all(&root).expect("create repo dir");
        git_in(&root, &["init", "--initial-branch", "main"]);
        git_in(&root, &["config", "user.name", "Conspectus Test"]);
        git_in(
            &root,
            &["config", "user.email", "conspectus@example.invalid"],
        );
        fs::write(root.join("README.md"), "fixture\n").expect("write readme");
        git_in(&root, &["add", "README.md"]);
        git_in(&root, &["commit", "-m", "initial"]);
    }

    fn add_worktree(&self, repo: &str, name: &str, branch: &str) -> PathBuf {
        let repo_root = self.temp.path().join(repo);
        let linked = self.temp.path().join(name);
        git_in(
            &repo_root,
            &["worktree", "add", "-b", branch, path_str(&linked)],
        );
        linked
    }

    fn clone_bare(&self, source_repo: &str, bare_name: &str) -> PathBuf {
        let bare = self.temp.path().join(bare_name);
        let source = self.temp.path().join(source_repo);
        let out = Command::new("git")
            .args(["clone", "--bare", path_str(&source), path_str(&bare)])
            .output()
            .expect("git clone --bare");
        assert!(
            out.status.success(),
            "git clone --bare failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        bare
    }

    fn add_bare_worktree(&self, bare: &Path, name: &str, branch: &str) -> PathBuf {
        let linked = self.temp.path().join(name);
        let git_dir = format!("--git-dir={}", path_str(bare));
        let out = Command::new("git")
            .args([
                git_dir.as_str(),
                "worktree",
                "add",
                "-b",
                branch,
                path_str(&linked),
            ])
            .output()
            .expect("git bare worktree add");
        assert!(
            out.status.success(),
            "git bare worktree add failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        linked
    }

    fn discover_from(
        &self,
        root: PathBuf,
        config: LocalDiscoveryConfig,
    ) -> conspectus::model::GraphSnapshot {
        let config = config.with_harness_state_root(CODEX_HARNESS_KEY, self.codex_state_root());
        discover_local_with([root], config).expect("discover")
    }

    fn write_atelier_config(&self, text: &str) {
        fs::write(self.temp.path().join("atelier.toml"), text).expect("write atelier config");
    }

    fn write_fork_index(&self, text: &str) {
        let path = self.temp.path().join(".atelier/forks/index.toml");
        fs::create_dir_all(path.parent().expect("parent")).expect("create fork dir");
        fs::write(path, text).expect("write fork index");
    }

    fn normalize(&self, rendered: &str) -> String {
        let mut out = rendered.replace(&self.temp.path().to_string_lossy().to_string(), "/fixture");
        if let Some(name) = self.temp.path().file_name().and_then(|s| s.to_str()) {
            out = out.replace(name, "fixture");
        }
        out
    }
}

fn assert_worktree_root(snapshot: &conspectus::model::GraphSnapshot, root: &Path) {
    let root = path_str(root);
    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Checkout(worktree) if worktree.root == root
        )),
        "expected worktree root {root}: {:#?}",
        snapshot.nodes
    );
}

fn assert_branch_ref(snapshot: &conspectus::model::GraphSnapshot, refname: &str) {
    assert!(
        snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::Branch(branch) if branch.refname == refname
        )),
        "expected branch {refname}: {:#?}",
        snapshot.nodes
    );
}

fn git_in(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}
