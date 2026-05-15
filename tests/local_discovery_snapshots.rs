use std::fs;
use std::path::Path;
use std::process::Command;

use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::output::render_graph_json;
use conspectus::resolve::resolve_snapshot;

#[test]
fn plain_repo_local_discovery_snapshot() {
    let fixture = LocalFixture::new();
    fixture.init_repo("repo");

    assert_local_snapshot(
        "plain_repo_local_discovery",
        fixture.root().join("repo"),
        &fixture,
    );
}

#[test]
fn linked_worktree_local_discovery_snapshot() {
    let fixture = LocalFixture::new();
    let repo = fixture.init_repo("repo");
    let linked = fixture.root().join("repo-linked");
    git(
        &repo,
        &["worktree", "add", "-b", "linked", path_str(&linked)],
    );

    assert_local_snapshot("linked_worktree_local_discovery", linked, &fixture);
}

#[test]
fn generic_workspace_local_discovery_snapshot() {
    let fixture = LocalFixture::new();
    fixture.init_repo("repo-a");
    fixture.init_repo("repo-b");

    assert_local_snapshot(
        "generic_workspace_local_discovery",
        fixture.root(),
        &fixture,
    );
}

#[test]
fn atelier_workspace_without_forks_local_discovery_snapshot() {
    let fixture = LocalFixture::new();
    fixture.init_repo("repo-a");
    fixture.write_atelier_config(
        r#"
[workspace]
name = "atelier-demo"

[[repos]]
name = "repo-a"
path = "/sources/repo-a"
"#,
    );

    assert_local_snapshot(
        "atelier_workspace_without_forks_local_discovery",
        fixture.root(),
        &fixture,
    );
}

#[test]
fn atelier_workspace_with_forks_local_discovery_snapshot() {
    let fixture = LocalFixture::new();
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

[[forks]]
name = "beta"
parent = "alpha"
created-epoch = 2
mode = "selected"
root = ".atelier/forks/beta"

[[forks.repos]]
name = "repo-b"
source = "/sources/repo-b"
parent-worktree = "repo-b"
link = true

[[forks]]
name = "research"
created-epoch = 3
mode = "research"
root = ".atelier/forks/research"
"#,
    );

    assert_local_snapshot(
        "atelier_workspace_with_forks_local_discovery",
        fixture.root(),
        &fixture,
    );
}

fn assert_local_snapshot(name: &str, root: impl AsRef<Path>, fixture: &LocalFixture) {
    let snapshot = discover_local_with([root.as_ref()], LocalDiscoveryConfig::empty())
        .expect("local discovery succeeds");
    let rendered = render_graph_json(&resolve_snapshot(snapshot)).expect("render graph");
    let normalized = normalize_fixture_paths(&rendered, fixture.root());

    insta::assert_snapshot!(name, normalized);
}

fn normalize_fixture_paths(rendered: &str, root: &Path) -> String {
    let normalized = rendered.replace(&root.to_string_lossy().to_string(), "/fixture");
    match root.file_name() {
        Some(name) => normalized.replace(&name.to_string_lossy().to_string(), "fixture"),
        None => normalized,
    }
}

struct LocalFixture {
    temp: tempfile::TempDir,
}

impl LocalFixture {
    fn new() -> Self {
        Self {
            temp: tempfile::TempDir::new().expect("temp dir"),
        }
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn init_repo(&self, name: &str) -> std::path::PathBuf {
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

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}
