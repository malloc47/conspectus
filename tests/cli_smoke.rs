use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

/// Builds a `conspectus` binary command isolated from the host environment:
/// HOME is pinned to an empty directory (so the harness adapters see no real
/// `~/.codex` etc.) and tmux discovery is disabled. Tests that want harness or
/// tmux discovery override these env vars explicitly.
fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

#[test]
fn help_prints_usage() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("conspectus"))
        .stdout(predicate::str::contains("graph"))
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn version_prints_package_version() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("conspectus"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn graph_json_prints_empty_graph_document() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json output");

    assert_eq!(json["nodes"], serde_json::json!([]));
    assert_eq!(json["candidate_links"], serde_json::json!([]));
    assert_eq!(json["resolved_relationships"], serde_json::json!([]));
    assert_eq!(json["diagnostics"], serde_json::json!([]));
}

#[test]
fn graph_json_output_is_deterministic() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let first_output = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_output = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(first_output, second_output);
}

#[test]
fn graph_rejects_invalid_format() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("graph")
        .arg("--format")
        .arg("table")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn graph_json_discovers_plain_repo_from_scan_root() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();

    let assert = isolated_cmd(home.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json output");

    assert!(
        json["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| node["type"] == "repo")
    );
    assert!(
        json["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| node["type"] == "worktree")
    );
    assert!(
        json["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| node["type"] == "branch")
    );
}

#[test]
fn graph_json_rejects_missing_scan_root() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let missing = temp.path().join("missing");
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("graph")
        .arg("--format")
        .arg("json")
        .arg("--scan-root")
        .arg(missing)
        .assert()
        .failure()
        .stderr(predicate::str::contains("scan root does not exist"));
}

#[test]
fn graph_json_emits_agent_sessions_from_env_state_root() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    fs::write(
        codex_state.join("rollout-cli-test.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"cli-test\",\"cwd\":\"/work/x\"}}\n",
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json output");

    let sessions: Vec<_> = json["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|node| node["type"] == "agent_session")
        .collect();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["harness_key"], "codex");
    assert_eq!(sessions[0]["cwd"], "/work/x");
}

#[test]
fn graph_json_emits_no_mux_nodes_when_tmux_disabled() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");

    let assert = isolated_cmd(home.path())
        .current_dir(scan_root.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json output");

    let mux_nodes: Vec<_> = json["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|node| node["type"] == "mux_session")
        .collect();
    assert!(
        mux_nodes.is_empty(),
        "tmux discovery should be skipped when CONSPECTUS_DISABLE_TMUX is set"
    );
}

fn temp_git_repo() -> tempfile::TempDir {
    let temp = tempfile::TempDir::new().expect("temp dir");
    git(temp.path(), &["init", "--initial-branch", "main"]);
    git(temp.path(), &["config", "user.name", "Conspectus Test"]);
    git(
        temp.path(),
        &["config", "user.email", "conspectus@example.invalid"],
    );
    fs::write(temp.path().join("README.md"), "fixture\n").expect("write fixture");
    git(temp.path(), &["add", "README.md"]);
    git(temp.path(), &["commit", "-m", "initial"]);
    temp
}

fn git(root: &Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
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
