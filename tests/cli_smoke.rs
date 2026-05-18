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
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
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
fn declared_help_lists_subcommands() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("declared")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("override"));
}

#[test]
fn declared_create_rejects_invalid_relation() {
    let home = tempfile::TempDir::new().expect("home temp");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("alpha")
        .arg("--relation")
        .arg("not_a_relation")
        .arg("--source")
        .arg("mux_session:native_id=tmux:editor")
        .arg("--target")
        .arg("agent_session:harness_key=codex,state_scope=/state,session_key=s1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid relation"));
}

#[test]
fn declared_create_rejects_invalid_endpoint_syntax() {
    let home = tempfile::TempDir::new().expect("home temp");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("alpha")
        .arg("--relation")
        .arg("linked_to_mux")
        .arg("--source")
        .arg("not-an-endpoint")
        .arg("--target")
        .arg("mux_session:native_id=tmux:editor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid endpoint syntax"));
}

#[test]
fn declared_create_requires_core_arguments() {
    let home = tempfile::TempDir::new().expect("home temp");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("create")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn declared_list_empty_stores_prints_nothing() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("declared")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn declared_list_renders_project_declared_links() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    fs::write(repo.path().join(".conspectus.toml"), declared_config()).expect("write config");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("list")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "project\tlocal_declared\tactive\tdeclared-session-mux\tlinked_to_mux",
        ))
        .stdout(predicate::str::contains("agent_session:harness_key=codex"))
        .stdout(predicate::str::contains(
            "mux_session:native_id=tmux:missing",
        ));
}

#[test]
fn declared_list_renders_user_declared_links() {
    let home = tempfile::TempDir::new().expect("home temp");
    let user_config = home.path().join(".config/conspectus/config.toml");
    fs::create_dir_all(user_config.parent().expect("parent")).expect("parent");
    fs::write(&user_config, ignored_and_overridden_declared_config()).expect("write config");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("list")
        .arg("--store")
        .arg("user")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "user\tglobal_declared\tignored\tignored-link\tlinked_to_mux",
        ))
        .stdout(predicate::str::contains("stale"))
        .stdout(predicate::str::contains(
            "user\tglobal_declared\toverridden\told-link\tlinked_to_mux",
        ))
        .stdout(predicate::str::contains("replacement-link"));
}

#[test]
fn declared_list_renders_project_before_user_deterministically() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    fs::write(repo.path().join(".conspectus.toml"), declared_config()).expect("write project");
    let user_config = home.path().join(".config/conspectus/config.toml");
    fs::create_dir_all(user_config.parent().expect("parent")).expect("parent");
    fs::write(&user_config, ignored_and_overridden_declared_config()).expect("write user");

    let assert = isolated_cmd(home.path())
        .arg("declared")
        .arg("list")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let project_index = output.find("project\t").expect("project row");
    let user_index = output.find("user\t").expect("user row");
    assert!(
        project_index < user_index,
        "project rows should sort before user rows:\n{output}"
    );
}

#[test]
fn declared_list_reports_malformed_config_diagnostics() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    fs::write(repo.path().join(".conspectus.toml"), "[declared\n").expect("write bad config");

    isolated_cmd(home.path())
        .arg("declared")
        .arg("list")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("failed to parse declared config"));
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

#[test]
fn session_default_projection_renders_agent_table() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("AGENT"),
        "agent table should be the default projection; got:\n{output}",
    );
}

#[test]
fn session_projection_flag_switches_to_mux() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .arg("--projection")
        .arg("mux")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("MUX"),
        "mux projection should print MUX header; got:\n{output}",
    );
}

#[test]
fn session_width_flag_truncates_long_cells_within_target() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    let long_cwd = "/very/long/workspace/path/that/will/exceed/eighty/columns/easily";
    fs::write(
        codex_state.join("rollout-width-test.jsonl"),
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"width-test\",\"cwd\":\"{long_cwd}\"}}}}\n"
        ),
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("session")
        .arg("--width")
        .arg("80")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let body_rows: Vec<&str> = output.lines().skip(2).collect();
    assert!(!body_rows.is_empty(), "expected at least one body row");
    for line in output.lines() {
        // Each row must fit; the truncation algorithm settles at column
        // floors when the target is impossibly narrow, so use a small
        // slack ceiling rather than a hard 80.
        assert!(
            line.chars().count() <= 90,
            "row exceeded reasonable width with --width 80: {line:?}",
        );
    }
    assert!(
        output.contains('…'),
        "long cwd should have been truncated with an ellipsis in:\n{output}",
    );
}

#[test]
fn session_wide_flag_emits_untruncated_output() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    let long_cwd = "/very/long/workspace/path/that/will/exceed/eighty/columns/easily";
    fs::write(
        codex_state.join("rollout-wide-test.jsonl"),
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"wide-test\",\"cwd\":\"{long_cwd}\"}}}}\n"
        ),
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("session")
        .arg("--wide")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.contains(long_cwd),
        "--wide should emit the full cwd; got:\n{output}",
    );
    assert!(
        !output.contains('…'),
        "--wide should not truncate; got:\n{output}",
    );
}

#[test]
fn session_piped_output_defaults_to_wide() {
    // assert_cmd's captured stdout is never a TTY, so the default
    // behavior should leave output untruncated for grep/awk friendliness.
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    let long_cwd = "/very/long/workspace/path/that/will/exceed/eighty/columns/easily";
    fs::write(
        codex_state.join("rollout-pipe-test.jsonl"),
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"pipe-test\",\"cwd\":\"{long_cwd}\"}}}}\n"
        ),
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("session")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.contains(long_cwd),
        "piped output should not truncate by default; got:\n{output}",
    );
}

#[test]
fn session_wide_and_width_flags_conflict() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .arg("--wide")
        .arg("--width")
        .arg("80")
        .assert()
        .failure();
}

#[test]
fn session_projection_flag_switches_to_union() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .arg("--projection")
        .arg("union")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("KIND"),
        "union projection should print KIND header; got:\n{output}",
    );
}

#[test]
fn session_rejects_invalid_projection() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .arg("--projection")
        .arg("ledger")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn session_reads_default_projection_from_project_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = home.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    fs::write(
        project.join(".conspectus.toml"),
        "[session]\nprojection = \"union\"\n",
    )
    .expect("write project config");

    let assert = isolated_cmd(home.path())
        .current_dir(&project)
        .arg("session")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("KIND"),
        "project config should set default projection to union; got:\n{output}",
    );
}

#[test]
fn session_output_is_deterministic_across_runs() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let first = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("session")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(first, second);
}

#[test]
fn graph_and_session_do_not_create_config_files_in_clean_repo() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("session")
        .assert()
        .success();

    assert!(!repo.path().join(".conspectus.toml").exists());
    assert!(!home.path().join(".config/conspectus/config.toml").exists());
}

#[test]
fn declared_create_writes_project_config_for_repo_rooted_endpoint() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let repo_root = repo.path().canonicalize().expect("canonicalize");
    let common_dir = repo_root.join(".git");
    let worktree_source = format!(
        "worktree:repo_common_dir={},root={}",
        common_dir.display(),
        repo_root.display()
    );

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("worktree-belongs")
        .arg("--relation")
        .arg("belongs_to_repo")
        .arg("--source")
        .arg(&worktree_source)
        .arg("--target")
        .arg(format!("repo:common_dir={}", common_dir.display()))
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success();

    let config_path = repo.path().join(".conspectus.toml");
    let text = fs::read_to_string(&config_path).expect("config exists");
    assert!(text.contains("[declared]"), "config:\n{text}");
    assert!(text.contains("worktree-belongs"), "config:\n{text}");
    assert!(!home.path().join(".config/conspectus/config.toml").exists());
}

#[test]
fn declared_create_writes_user_config_for_orphan_endpoint() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("orphan-session-mux")
        .arg("--relation")
        .arg("linked_to_mux")
        .arg("--source")
        .arg("agent_session:harness_key=codex,state_scope=/state,session_key=alpha")
        .arg("--target")
        .arg("mux_session:native_id=tmux:editor")
        .assert()
        .success();

    let user_config = home.path().join(".config/conspectus/config.toml");
    let text = fs::read_to_string(&user_config).expect("user config exists");
    assert!(text.contains("orphan-session-mux"), "config:\n{text}");
    assert!(!cwd.path().join(".conspectus.toml").exists());
}

#[test]
fn declared_create_respects_explicit_store_user_override() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("explicit-user")
        .arg("--relation")
        .arg("linked_to_mux")
        .arg("--source")
        .arg("agent_session:harness_key=codex,state_scope=/state,session_key=alpha")
        .arg("--target")
        .arg("mux_session:native_id=tmux:editor")
        .arg("--store")
        .arg("user")
        .assert()
        .success();

    assert!(!repo.path().join(".conspectus.toml").exists());
    let user_config = home.path().join(".config/conspectus/config.toml");
    let text = fs::read_to_string(&user_config).expect("user config exists");
    assert!(text.contains("explicit-user"), "config:\n{text}");
}

#[test]
fn declared_create_rejects_store_all() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("invalid")
        .arg("--relation")
        .arg("linked_to_mux")
        .arg("--source")
        .arg("agent_session:harness_key=codex,state_scope=/state,session_key=alpha")
        .arg("--target")
        .arg("mux_session:native_id=tmux:editor")
        .arg("--store")
        .arg("all")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not valid for write commands"));
}

#[test]
fn declared_create_is_idempotent_for_identical_input() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    let run_create = || {
        isolated_cmd(home.path())
            .current_dir(cwd.path())
            .arg("declared")
            .arg("create")
            .arg("--id")
            .arg("dup")
            .arg("--relation")
            .arg("linked_to_mux")
            .arg("--source")
            .arg("agent_session:harness_key=codex,state_scope=/state,session_key=alpha")
            .arg("--target")
            .arg("mux_session:native_id=tmux:editor")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    };

    let first = run_create();
    let second = run_create();
    let first_text = String::from_utf8(first).expect("utf8");
    let second_text = String::from_utf8(second).expect("utf8");
    assert!(first_text.starts_with("wrote"), "got: {first_text}");
    assert!(second_text.starts_with("unchanged"), "got: {second_text}");
}

#[test]
fn declared_remove_strips_link_from_project_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    fs::write(repo.path().join(".conspectus.toml"), declared_config()).expect("seed");

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("remove")
        .arg("--id")
        .arg("declared-session-mux")
        .assert()
        .success();

    let text = fs::read_to_string(repo.path().join(".conspectus.toml")).expect("read");
    assert!(!text.contains("declared-session-mux"), "config:\n{text}");
}

#[test]
fn declared_remove_reports_missing_id() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("declared")
        .arg("remove")
        .arg("--id")
        .arg("nothing-here")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no declared link"));
}

#[test]
fn declared_create_then_graph_shows_local_declared_candidate() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("created-link")
        .arg("--relation")
        .arg("linked_to_mux")
        .arg("--source")
        .arg("agent_session:harness_key=codex,state_scope=/state,session_key=alpha")
        .arg("--target")
        .arg("mux_session:native_id=tmux:editor")
        .arg("--store")
        .arg("project")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success();

    let assert = isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json");

    let has_local_declared = json["candidate_links"]
        .as_array()
        .expect("candidates")
        .iter()
        .any(|link| link["provenance"] == "local_declared");
    assert!(has_local_declared, "candidates:\n{}", output);
}

#[test]
fn declared_confirm_promotes_discovered_candidate_to_declared_link() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let candidate_id = discovered_belongs_to_repo_candidate_id(home.path(), &repo);

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("confirm")
        .arg("--id")
        .arg(&candidate_id)
        .arg("--scan-root")
        .arg(repo.path())
        .arg("--store")
        .arg("project")
        .assert()
        .success();

    let text = fs::read_to_string(repo.path().join(".conspectus.toml")).expect("config");
    assert!(text.contains("[declared]"), "config:\n{text}");
    assert!(text.contains(&candidate_id), "config:\n{text}");
    assert!(text.contains("state = \"active\""), "config:\n{text}");
}

#[test]
fn declared_ignore_records_state_ignored_with_reason() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let candidate_id = discovered_belongs_to_repo_candidate_id(home.path(), &repo);

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("ignore")
        .arg("--id")
        .arg(&candidate_id)
        .arg("--reason")
        .arg("not useful")
        .arg("--scan-root")
        .arg(repo.path())
        .arg("--store")
        .arg("project")
        .assert()
        .success();

    let text = fs::read_to_string(repo.path().join(".conspectus.toml")).expect("config");
    assert!(text.contains("state = \"ignored\""), "config:\n{text}");
    assert!(text.contains("not useful"), "config:\n{text}");
}

#[test]
fn declared_confirm_errors_when_candidate_id_unknown() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("declared")
        .arg("confirm")
        .arg("--id")
        .arg("does-not-exist")
        .arg("--store")
        .arg("user")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no candidate link with id `does-not-exist`",
        ));
}

#[test]
fn declared_override_marks_existing_link_overridden() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    fs::write(repo.path().join(".conspectus.toml"), declared_config()).expect("seed");

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("override")
        .arg("--id")
        .arg("declared-session-mux")
        .arg("--overridden-by")
        .arg("new-link")
        .arg("--reason")
        .arg("replaced by user")
        .assert()
        .success();

    let text = fs::read_to_string(repo.path().join(".conspectus.toml")).expect("config");
    assert!(text.contains("state = \"overridden\""), "config:\n{text}");
    assert!(
        text.contains("overridden_by = \"new-link\""),
        "config:\n{text}"
    );
    assert!(text.contains("replaced by user"), "config:\n{text}");
}

#[test]
fn declared_override_errors_when_id_missing() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("declared")
        .arg("override")
        .arg("--id")
        .arg("nothing")
        .arg("--overridden-by")
        .arg("replacement")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no declared link `nothing`"));
}

#[test]
fn declared_confirm_in_detailed_graph_preserves_discovered_candidate() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let candidate_id = discovered_belongs_to_repo_candidate_id(home.path(), &repo);

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("confirm")
        .arg("--id")
        .arg(&candidate_id)
        .arg("--scan-root")
        .arg(repo.path())
        .arg("--store")
        .arg("project")
        .assert()
        .success();

    let assert = isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json");

    let provenances: Vec<&str> = json["candidate_links"]
        .as_array()
        .expect("candidates")
        .iter()
        .filter_map(|link| link["provenance"].as_str())
        .collect();
    assert!(
        provenances.contains(&"strong_discovered"),
        "discovered candidate should remain visible: {provenances:?}"
    );
    assert!(
        provenances.contains(&"local_declared"),
        "confirmed candidate should be present: {provenances:?}"
    );
}

/// Helper: run `conspectus graph --format json` from `repo` and return
/// the id of the first `belongs_to_repo` candidate link, which is
/// emitted by every plain repo and so makes a stable confirm/ignore
/// target.
fn discovered_belongs_to_repo_candidate_id(home: &Path, repo: &tempfile::TempDir) -> String {
    let assert = isolated_cmd(home)
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(&output).expect("valid json");
    json["candidate_links"]
        .as_array()
        .expect("candidates")
        .iter()
        .find_map(|link| {
            if link["relation"] == "belongs_to_repo" {
                link["id"].as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .expect("belongs_to_repo candidate")
}

#[test]
fn graph_does_not_mutate_existing_project_declared_config_from_scan_root() {
    let home = tempfile::TempDir::new().expect("home temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let repo = temp_git_repo();
    let config_path = repo.path().join(".conspectus.toml");
    let original = declared_config();
    fs::write(&config_path, original).expect("write project config");

    isolated_cmd(home.path())
        .current_dir(cwd.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .arg("--scan-root")
        .arg(repo.path())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&config_path).expect("read config"),
        original
    );
}

#[test]
fn session_does_not_mutate_existing_project_declared_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let config_path = repo.path().join(".conspectus.toml");
    let original = declared_config();
    fs::write(&config_path, original).expect("write project config");

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("session")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&config_path).expect("read config"),
        original
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

fn declared_config() -> &'static str {
    r#"[session]
projection = "agent"

[declared]
schema_version = 1

[[declared.links]]
id = "declared-session-mux"
relation = "linked_to_mux"
state = "active"
source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
target = { type = "mux_session", native_id = "tmux:missing" }
"#
}

fn ignored_and_overridden_declared_config() -> &'static str {
    r#"[declared]
schema_version = 1

[[declared.links]]
id = "ignored-link"
relation = "linked_to_mux"
state = "ignored"
source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "ignored" }
target = { type = "mux_session", native_id = "tmux:old" }
reason = "stale"

[[declared.links]]
id = "old-link"
relation = "linked_to_mux"
state = "overridden"
source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "old" }
target = { type = "mux_session", native_id = "tmux:old" }
overridden_by = "replacement-link"
"#
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
