//! CLI integration coverage for `conspectus query` (P9-004).
//!
//! Validates that the subcommand wires through end-to-end against a
//! cold-discovery fallback in an isolated `HOME`: the binary builds an
//! empty in-memory graph, applies the schema, runs the supplied SQL,
//! and prints the rendered output.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::tempdir;

fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    // Point XDG_DATA_HOME at an empty directory so the runner falls
    // into the cold-discovery path instead of opening a stray
    // `~/.local/share/conspectus/graph.sqlite` on the developer's host.
    cmd.env("XDG_DATA_HOME", home.join("data"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

#[test]
fn query_renders_table_with_header_separator_and_row() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "SELECT COUNT(*) AS n FROM v_nodes"])
        .current_dir(home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("n"))
        .stdout(predicate::str::contains("-"))
        .stdout(predicate::str::contains("0"));
}

#[test]
fn query_json_format_emits_object_per_row() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "--format", "json", "SELECT 1 AS x"])
        .current_dir(home.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{\"x\":1}"));
}

#[test]
fn query_rejects_insert_with_readonly_error() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args([
            "query",
            "INSERT INTO node_repos (node_id, common_dir) VALUES ('x', '/x')",
        ])
        .current_dir(home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("readonly").or(predicate::str::contains("read-only")));
}

#[test]
fn query_rejects_create_with_readonly_error() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "CREATE TABLE evil (x INTEGER)"])
        .current_dir(home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("readonly").or(predicate::str::contains("read-only")));
}
