use assert_cmd::Command;
use predicates::prelude::*;

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
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    let assert = cmd
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
    let mut first = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    let mut second = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    let first_output = first
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_output = second
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
