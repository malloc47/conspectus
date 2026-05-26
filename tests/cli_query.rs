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

#[test]
fn query_csv_emits_header_and_crlf() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "--format", "csv", "SELECT 1 AS x, 'two' AS y"])
        .current_dir(home.path())
        .assert()
        .success()
        // CSV header line: `x,y` followed by CRLF; body row follows.
        .stdout(predicate::str::starts_with("x,y\r\n1,two\r\n"));
}

#[test]
fn query_tsv_emits_tab_separated_header() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "--format", "tsv", "SELECT 1 AS x, 'two' AS y"])
        .current_dir(home.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("x\ty\n1\ttwo\n"));
}

#[test]
fn query_table_width_truncates_long_cells_with_ellipsis() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args([
            "query",
            "--width",
            "20",
            "SELECT '012345678901234567890123' AS very_long_label",
        ])
        .current_dir(home.path())
        .assert()
        .success()
        // Ellipsis (`…`) appears somewhere when the cell exceeds the
        // budget. The exact column layout is covered by the runner's
        // unit tests.
        .stdout(predicate::str::contains("…"));
}

#[test]
fn query_list_views_prints_every_curated_view() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query", "--list-views"])
        .current_dir(home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("v_sessions_with_repo"))
        .stdout(predicate::str::contains("v_mux_attachments"))
        .stdout(predicate::str::contains("v_pr_by_branch"))
        .stdout(predicate::str::contains("v_fork_ancestry"))
        .stdout(predicate::str::contains("v_workspace_member_repos"));
}

#[test]
fn query_without_sql_or_list_views_fails_with_usage_error() {
    let home = tempdir().expect("temp HOME");
    isolated_cmd(home.path())
        .args(["query"])
        .current_dir(home.path())
        .assert()
        .failure()
        // clap exits with a usage message naming the missing
        // required argument.
        .stderr(predicate::str::contains("required"));
}
