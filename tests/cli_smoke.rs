use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

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
    let temp = tempfile::TempDir::new().expect("temp dir");
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    let assert = cmd
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
    let temp = tempfile::TempDir::new().expect("temp dir");
    let mut first = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    let mut second = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    let first_output = first
        .current_dir(temp.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_output = second
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
    let repo = temp_git_repo();
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    let assert = cmd
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
