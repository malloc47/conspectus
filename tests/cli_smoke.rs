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
fn read_only_commands_do_not_mutate_alias_bearing_config() {
    // ADR 0029 invariant: every read-only surface (graph, table,
    // alias list) must leave alias-bearing TOML files byte-for-byte
    // untouched. Mirrors the declared-link read-only audit.
    let home = tempfile::TempDir::new().expect("home");
    let temp = tempfile::TempDir::new().expect("temp");
    let config = temp.path().join(".conspectus.toml");
    let original = r#"[aliases]
schema_version = 1

[[aliases.entries]]
node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
display_name = "ingest-refactor"
"#;
    fs::write(&config, original).expect("seed");

    for args in [
        vec!["graph", "--format", "json", "--scan-root"],
        vec!["table", "sessions", "--scan-root"],
        vec!["alias", "list", "--scan-root"],
    ] {
        let mut cmd = isolated_cmd(home.path());
        let mut full = args.clone();
        full.push(temp.path().to_str().expect("utf-8 path"));
        cmd.args(&full);
        cmd.assert().success();

        let after = fs::read_to_string(&config).expect("read after");
        assert_eq!(after, original, "command {args:?} mutated alias config",);
    }
}

#[test]
fn rename_help_lists_session_and_mux() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("rename")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("session"))
        .stdout(predicate::str::contains("mux"));
}

#[test]
fn rename_session_rejects_name_and_clear_together() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("rename")
        .arg("session")
        .arg("codex:alpha")
        .arg("ingest-refactor")
        .arg("--clear")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "'[NAME]' cannot be used with '--clear'",
        ));
}

#[test]
fn rename_session_requires_name_or_clear() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("rename")
        .arg("session")
        .arg("codex:alpha")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "specify either a new <NAME> or --clear",
        ));
}

#[test]
fn rename_mux_rejects_clear_flag() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("rename")
        .arg("mux")
        .arg("tmux:editor")
        .arg("--clear")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "mux sessions have no Conspectus-owned alias to clear",
        ));
}

#[test]
fn alias_help_lists_list_subcommand() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("alias")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"));
}

#[test]
fn alias_list_empty_stores_prints_nothing() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("alias")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn alias_list_renders_project_alias_entries() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join(".conspectus.toml"),
        r#"[aliases]
schema_version = 1

[[aliases.entries]]
node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
display_name = "ingest-refactor"
"#,
    )
    .expect("write project config");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("alias")
        .arg("list")
        .arg("--scan-root")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("project"))
        .stdout(predicate::str::contains("agent_session:codex:/state:alpha"))
        .stdout(predicate::str::contains("ingest-refactor"));
}

#[test]
fn rename_session_writes_alias_for_codex_session() {
    let home = tempfile::TempDir::new().expect("home temp");
    let codex_state = home.path().join(".codex");
    fs::create_dir_all(
        codex_state
            .join("sessions")
            .join("2026")
            .join("05")
            .join("23"),
    )
    .expect("codex state dir");
    // Minimal codex rollout file the harness discovery accepts.
    let rollout = codex_state.join("sessions/2026/05/23/rollout-2026-05-23T00-00-00-alpha.jsonl");
    fs::write(
        &rollout,
        r#"{"timestamp":"2026-05-23T00:00:00Z","type":"session_meta","payload":{"id":"alpha","cwd":"/tmp"}}
"#,
    )
    .expect("rollout");

    let project = tempfile::TempDir::new().expect("project");

    let mut cmd = isolated_cmd(home.path());
    cmd.env("CONSPECTUS_CODEX_STATE", &codex_state);
    cmd.current_dir(project.path())
        .arg("rename")
        .arg("session")
        .arg("codex:alpha")
        .arg("ingest-refactor")
        .arg("--no-mux")
        .arg("--store")
        .arg("user")
        .arg("--scan-root")
        .arg(project.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("wrote alias `ingest-refactor`"));

    let user_config = home.path().join(".config/conspectus/config.toml");
    let written = fs::read_to_string(&user_config).expect("user config exists");
    assert!(
        written.contains("display_name = \"ingest-refactor\""),
        "alias missing from user config:\n{written}"
    );
    assert!(written.contains("session_key = \"alpha\""));
}

#[test]
fn rename_session_clear_removes_alias_entry() {
    let home = tempfile::TempDir::new().expect("home temp");
    let codex_state = home.path().join(".codex");
    fs::create_dir_all(codex_state.join("sessions/2026/05/23")).expect("dir");
    let rollout = codex_state.join("sessions/2026/05/23/rollout-alpha.jsonl");
    fs::write(
        &rollout,
        r#"{"timestamp":"2026-05-23T00:00:00Z","type":"session_meta","payload":{"id":"alpha","cwd":"/tmp"}}
"#,
    )
    .expect("rollout");

    let project = tempfile::TempDir::new().expect("project");

    // Seed the alias by running the rename command first; this uses
    // the same state_scope as discovery, so the clear step finds it.
    let mut seed = isolated_cmd(home.path());
    seed.env("CONSPECTUS_CODEX_STATE", &codex_state);
    seed.current_dir(project.path())
        .arg("rename")
        .arg("session")
        .arg("codex:alpha")
        .arg("ingest-refactor")
        .arg("--no-mux")
        .arg("--store")
        .arg("user")
        .arg("--scan-root")
        .arg(project.path());
    seed.assert().success();

    let user_config = home.path().join(".config/conspectus/config.toml");
    assert!(
        user_config.is_file(),
        "seed should have created user config"
    );

    let mut clear = isolated_cmd(home.path());
    clear.env("CONSPECTUS_CODEX_STATE", &codex_state);
    clear
        .current_dir(project.path())
        .arg("rename")
        .arg("session")
        .arg("codex:alpha")
        .arg("--clear")
        .arg("--scan-root")
        .arg(project.path());
    clear
        .assert()
        .success()
        .stdout(predicate::str::contains("removed alias from"));

    let surviving = fs::read_to_string(&user_config).unwrap_or_default();
    assert!(
        !surviving.contains("[aliases]"),
        "aliases not pruned: {surviving}"
    );
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
fn hook_write_claude_code_writes_latest_observation() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("claude-code")
        .arg("--state-root")
        .arg(state.path())
        .write_stdin(
            r#"{
              "session_id": "session-1",
              "cwd": "/work",
              "transcript_path": "/tmp/transcript.jsonl",
              "hook_event_name": "SessionStart"
            }"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let record = read_only_hook_record(state.path());
    assert_eq!(record["harness_key"], "claude-code");
    assert_eq!(record["session_key"], "session-1");
    assert_eq!(record["transcript_path"], "/tmp/transcript.jsonl");
}

#[test]
fn hook_write_codex_writes_latest_observation() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("codex")
        .arg("--state-root")
        .arg(state.path())
        .write_stdin(
            r#"{
              "session_id": "019e531f-19ee-7823-816f-4526ef89d70b",
              "cwd": "/work",
              "transcript_path": "/tmp/rollout.jsonl",
              "hook_event_name": "SessionStart"
            }"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let record = read_only_hook_record(state.path());
    assert_eq!(record["harness_key"], "codex");
    assert_eq!(
        record["session_key"],
        "019e531f-19ee-7823-816f-4526ef89d70b"
    );
    assert_eq!(record["transcript_path"], "/tmp/rollout.jsonl");
}

#[test]
fn hook_write_opencode_writes_latest_observation() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("opencode")
        .arg("--state-root")
        .arg(state.path())
        .write_stdin(
            r#"{
              "session_id": "ses_01HZX2J5Y",
              "cwd": "/home/me/src/proj",
              "hook_event_name": "session.updated"
            }"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let record = read_only_hook_record(state.path());
    assert_eq!(record["harness_key"], "opencode");
    assert_eq!(record["session_key"], "ses_01HZX2J5Y");
    assert_eq!(record["cwd"], "/home/me/src/proj");
    assert_eq!(record["hook_event_name"], "session.updated");
}

fn read_only_hook_record(root: &std::path::Path) -> serde_json::Value {
    let path = root.join("hooks-latest.json");
    assert!(path.is_file(), "{} missing", path.display());
    let body = std::fs::read_to_string(path).expect("read latest hook spool");
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    value["records"]
        .as_object()
        .expect("records map")
        .values()
        .next()
        .expect("one record")
        .clone()
}

#[test]
fn hook_write_opencode_rejects_empty_payload() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("opencode")
        .arg("--state-root")
        .arg(state.path())
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("opencode hook payload was empty"));
}

#[test]
fn hook_init_status_remove_claude_code_preserves_existing_settings() {
    let home = tempfile::TempDir::new().expect("home temp");
    let settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("settings parent");
    fs::write(
        &settings,
        r#"{
          "theme": "dark",
          "hooks": {
            "SessionStart": [
              {
                "matcher": "startup",
                "hooks": [
                  { "type": "command", "command": "echo existing" }
                ]
              }
            ]
          }
        }"#,
    )
    .expect("write settings");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("init")
        .arg("claude-code")
        .arg("--command")
        .arg("conspectus hook write claude-code")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed Claude Code hook"));

    isolated_cmd(home.path())
        .arg("hook")
        .arg("status")
        .arg("claude-code")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed\tclaude-code"));

    let installed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).expect("read")).expect("json");
    assert_eq!(installed["theme"], "dark");
    assert!(
        fs::read_to_string(&settings)
            .expect("read")
            .contains("echo existing")
    );

    isolated_cmd(home.path())
        .arg("hook")
        .arg("remove")
        .arg("claude-code")
        .assert()
        .success()
        .stdout(predicate::str::contains("removed Claude Code hook"));

    let removed = fs::read_to_string(&settings).expect("read");
    assert!(removed.contains("echo existing"));
    assert!(!removed.contains("hook write claude-code"));
}

#[test]
fn hook_init_status_remove_codex_preserves_existing_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let config = home.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().expect("parent")).expect("config parent");
    fs::write(
        &config,
        r#"
model = "gpt-5.4-mini"

[hooks]
SessionStart = [
  { hooks = [
      { type = "command", command = "echo existing", async = false },
    ] },
]
"#,
    )
    .expect("write config");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("init")
        .arg("codex")
        .arg("--command")
        .arg("conspectus hook write codex")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed Codex hook"));

    isolated_cmd(home.path())
        .arg("hook")
        .arg("status")
        .arg("codex")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed\tcodex"));

    let installed = fs::read_to_string(&config).expect("read");
    assert!(installed.contains("model = \"gpt-5.4-mini\""));
    assert!(installed.contains("echo existing"));
    assert!(installed.contains("hook write codex"));
    assert!(installed.contains("async = false"));

    isolated_cmd(home.path())
        .arg("hook")
        .arg("remove")
        .arg("codex")
        .assert()
        .success()
        .stdout(predicate::str::contains("removed Codex hook"));

    let removed = fs::read_to_string(&config).expect("read");
    assert!(removed.contains("echo existing"));
    assert!(!removed.contains("hook write codex"));
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
            .any(|node| node["type"] == "checkout")
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
fn table_sessions_renders_agent_projection() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("AGENT"),
        "`table sessions` should render the agent projection header; got:\n{output}",
    );
}

#[test]
fn table_mux_renders_mux_projection() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("mux")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("MUX"),
        "`table mux` should render the mux projection header; got:\n{output}",
    );
}

#[test]
fn table_help_lists_row_types() {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");

    cmd.arg("table")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("sessions"))
        .stdout(predicate::str::contains("mux"))
        .stdout(predicate::str::contains("union"));
}

#[test]
fn table_requires_a_row_type() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    // `conspectus table` with no positional should error out and point
    // at the available row-type subcommands.
    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .assert()
        .failure();
}

#[test]
fn table_sessions_width_flag_truncates_long_cells_within_target() {
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
        .arg("table")
        .arg("sessions")
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
fn table_sessions_wide_flag_emits_untruncated_output() {
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
        .arg("table")
        .arg("sessions")
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
fn table_sessions_piped_output_defaults_to_wide() {
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
        .arg("table")
        .arg("sessions")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.contains(long_cwd),
        "piped output should not truncate by default; got:\n{output}",
    );
}

#[test]
fn node_show_resolves_full_display_form() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    fs::write(
        codex_state.join("rollout-node-show.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"node-show-test\",\"cwd\":\"/work/show\"}}\n",
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    // First, locate the discovered session's full id via `graph --format json`.
    let graph_assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let graph = String::from_utf8(graph_assert.get_output().stdout.clone()).expect("utf8 stdout");
    let json: serde_json::Value = serde_json::from_str(&graph).expect("valid json");
    let agent_session = json["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|n| n["type"] == "agent_session")
        .expect("agent session node");
    let state_scope = agent_session["id"]["state_scope"].as_str().unwrap();
    let session_key = agent_session["id"]["session_key"].as_str().unwrap();
    let display = format!("agent_session:codex:{state_scope}:{session_key}");

    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("node")
        .arg("show")
        .arg(&display)
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(output.contains("kind: agent_session"), "got:\n{output}");
    assert!(
        output.contains("session_key: node-show-test"),
        "got:\n{output}"
    );
    assert!(output.contains("cwd:"), "got:\n{output}");
}

#[test]
fn node_show_resolves_harness_label() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    fs::write(
        codex_state.join("rollout-label.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"label-test\",\"cwd\":\"/work/label\"}}\n",
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("node")
        .arg("show")
        .arg("codex:label-test")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(output.contains("kind: agent_session"), "got:\n{output}");
    assert!(output.contains("session_key: label-test"), "got:\n{output}");
}

#[test]
fn node_show_resolves_external_session_id_from_table_sessions() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    fs::write(
        codex_state.join("rollout-short.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"short-id-test\",\"cwd\":\"/work/short\"}}\n",
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    // Pull the harness-native session id off the session table.
    let session_assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("table")
        .arg("sessions")
        .arg("--wide")
        .assert()
        .success();
    let session_out =
        String::from_utf8(session_assert.get_output().stdout.clone()).expect("utf8 stdout");
    let body_row = session_out
        .lines()
        .skip(2)
        .find(|line| line.contains("codex:short-id-test"))
        .expect("body row with session");
    let session_id: String = body_row
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    assert_eq!(session_id, "short-id-test");

    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("node")
        .arg("show")
        .arg(&session_id)
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(output.contains("kind: agent_session"), "got:\n{output}");
    assert!(
        output.contains("session_key: short-id-test"),
        "got:\n{output}",
    );
}

#[test]
fn node_show_errors_on_unknown_id() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");

    isolated_cmd(home.path())
        .current_dir(scan_root.path())
        .arg("node")
        .arg("show")
        .arg("does-not-exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no node matches"));
}

#[test]
fn table_sessions_layout_card_emits_keyed_lines() {
    let home = tempfile::TempDir::new().expect("home temp");
    let scan_root = tempfile::TempDir::new().expect("scan temp");
    let codex_state = home.path().join(".codex").join("sessions");
    fs::create_dir_all(&codex_state).expect("codex sessions dir");
    fs::write(
        codex_state.join("rollout-card-test.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"card-test\",\"cwd\":\"/work/card\"}}\n",
    )
    .expect("write codex session");

    let codex_state_root: PathBuf = home.path().join(".codex");
    let assert = isolated_cmd(home.path())
        .env("CONSPECTUS_CODEX_STATE", &codex_state_root)
        .current_dir(scan_root.path())
        .arg("table")
        .arg("sessions")
        .arg("--layout")
        .arg("card")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    // Card format puts each column on its own `KEY: value` line.
    assert!(
        output.contains("AGENT:"),
        "card layout should label each cell with its column key:\n{output}",
    );
    assert!(
        output.contains("CWD:"),
        "card layout should include CWD key:\n{output}",
    );
    assert!(
        output.contains("/work/card"),
        "card layout should include the cwd value:\n{output}",
    );
}

#[test]
fn table_sessions_wide_and_width_flags_conflict() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--wide")
        .arg("--width")
        .arg("80")
        .assert()
        .failure();
}

#[test]
fn table_union_renders_union_projection() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("union")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    assert!(
        output.starts_with("ID") && output.contains("KIND"),
        "`table union` should render the union projection header; got:\n{output}",
    );
}

#[test]
fn table_rejects_unknown_row_type() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("ledger")
        .assert()
        .failure();
}

#[test]
fn table_sessions_columns_supports_branch_repo_optional_columns() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--columns")
        .arg("id,agent,checkout,branch,repo,fork,declared")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(
        header_tokens,
        vec![
            "ID", "AGENT", "CHECKOUT", "BRANCH", "REPO", "FORK", "DECLARED",
        ],
    );
}

#[test]
fn table_sessions_color_always_emits_ansi_even_when_piped() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--color")
        .arg("always")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(
        output.contains('\u{1b}'),
        "expected ANSI escape (ESC) in --color=always output:\n{output:?}",
    );
    assert!(output.contains("ID"), "still contains the ID header");
}

#[test]
fn table_sessions_color_never_emits_no_ansi() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .env("CLICOLOR_FORCE", "1")
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--color")
        .arg("never")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(
        !output.contains('\u{1b}'),
        "--color=never must skip ANSI even when CLICOLOR_FORCE is set",
    );
}

#[test]
fn table_sessions_color_auto_defaults_to_no_color_on_pipe() {
    // assert_cmd's captured stdout is non-TTY, so auto resolves to
    // "no color". This locks the convention that piped output stays
    // grep/awk-friendly.
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(
        !output.contains('\u{1b}'),
        "auto color must stay off when piped:\n{output:?}",
    );
}

#[test]
fn table_sessions_no_color_env_overrides_color_auto() {
    // NO_COLOR is meant to override auto. We set --color=always to
    // confirm it does NOT override that explicit user flag (per the
    // ADR-0022 precedence), then drop --color so auto applies and
    // NO_COLOR wins.
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let still_colored = isolated_cmd(home.path())
        .env("NO_COLOR", "1")
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--color")
        .arg("always")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8(still_colored).unwrap().contains('\u{1b}'),
        "explicit --color=always wins over NO_COLOR",
    );

    let auto = isolated_cmd(home.path())
        .env("NO_COLOR", "1")
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--color")
        .arg("auto")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let auto = String::from_utf8(auto).unwrap();
    assert!(
        !auto.contains('\u{1b}'),
        "NO_COLOR must override auto:\n{auto:?}",
    );
}

#[test]
fn columns_color_always_emits_ansi() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("columns")
        .arg("sessions")
        .arg("--color")
        .arg("always")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(
        output.contains('\u{1b}'),
        "columns --color=always should emit ANSI:\n{output:?}",
    );
}

#[test]
fn table_sessions_pager_flag_routes_through_pager_command() {
    // `PAGER=cat` + `--pager` (which forces paging even on non-TTY)
    // round-trips the rendered output through cat, so stdout matches
    // the direct-print version. Verifies the pager spawn path works
    // end-to-end.
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let direct = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let paged = isolated_cmd(home.path())
        .env("PAGER", "cat")
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--pager")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(
        String::from_utf8(direct).unwrap(),
        String::from_utf8(paged).unwrap(),
        "cat as the pager should pass content through unchanged",
    );
}

#[test]
fn table_sessions_no_pager_flag_disables_pager_even_when_forced_pager_env_present() {
    // Setting PAGER=false would normally fail (false exits non-zero),
    // but with --no-pager we should skip the pager entirely and emit
    // output directly. The success exit and non-empty stdout confirm
    // the pager was bypassed.
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .env("PAGER", "false")
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--no-pager")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(output.starts_with("ID"), "got:\n{output}");
}

#[test]
fn table_sessions_pager_and_no_pager_flags_conflict() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--pager")
        .arg("--no-pager")
        .assert()
        .failure();
}

#[test]
fn columns_pager_flag_routes_through_pager_command() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let direct = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("columns")
        .arg("sessions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let paged = isolated_cmd(home.path())
        .env("PAGER", "cat")
        .current_dir(temp.path())
        .arg("columns")
        .arg("sessions")
        .arg("--pager")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(
        String::from_utf8(direct).unwrap(),
        String::from_utf8(paged).unwrap(),
    );
}

#[test]
fn columns_lists_every_row_type_with_default_marker() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    for (row_type, expected_default, expected_optional) in [
        ("sessions", "id ", "checkout "),
        ("mux", "id ", "activity "),
        ("union", "id ", "label "),
        ("prs", "id ", "draft "),
        ("forks", "id ", "scope "),
    ] {
        let assert = isolated_cmd(home.path())
            .current_dir(temp.path())
            .arg("columns")
            .arg(row_type)
            .assert()
            .success();
        let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

        let default_line = output
            .lines()
            .find(|line| line.starts_with(expected_default))
            .unwrap_or_else(|| panic!("missing {expected_default} line for {row_type}:\n{output}"));
        assert!(
            default_line.contains("(default)"),
            "expected (default) marker for {row_type} {expected_default:?}: {default_line:?}",
        );

        // Optional/expected non-default column key should appear in
        // the listing.
        assert!(
            output
                .lines()
                .any(|line| line.starts_with(expected_optional)),
            "missing {expected_optional} column for {row_type}:\n{output}",
        );
    }
}

#[test]
fn columns_rejects_unknown_row_type() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("columns")
        .arg("ledger")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid table row-type"));
}

#[test]
fn table_forks_renders_fork_projection_header() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("forks")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(
        header_tokens,
        vec!["ID", "FORK", "PROVIDER", "PARENT", "CHILDREN"],
    );
}

#[test]
fn table_prs_renders_pr_projection_header() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("prs")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(
        header_tokens,
        vec!["ID", "PR", "STATE", "BRANCH", "ATTACHED"],
    );
}

#[test]
fn table_prs_supports_columns_flag_with_optional_columns() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("prs")
        .arg("--columns")
        .arg("id,pr,state,draft,branch,repo")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(
        header_tokens,
        vec!["ID", "PR", "STATE", "DRAFT", "BRANCH", "REPO"],
    );
}

#[test]
fn table_sessions_columns_flag_overrides_default_set() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--columns")
        .arg("id,agent,cwd")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(header_tokens, vec!["ID", "AGENT", "CWD"]);
    assert!(!output.contains("MUX"), "got:\n{output}");
    assert!(!output.contains("LINEAGE"), "got:\n{output}");
}

#[test]
fn table_sessions_columns_flag_supports_delta_tokens() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--columns=-cwd,-mux-conf,-pr-conf")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(header_tokens, vec!["ID", "AGENT", "MUX", "PR", "LINEAGE"]);
}

#[test]
fn table_sessions_columns_flag_rejects_unknown_column() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .arg("--columns")
        .arg("+nope")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown column"))
        .stderr(predicate::str::contains("sessions"));
}

#[test]
fn table_sessions_columns_config_drives_default_when_flag_absent() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = home.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    fs::write(
        project.join(".conspectus.toml"),
        "[table.sessions]\ncolumns = [\"id\", \"agent\", \"cwd\"]\n",
    )
    .expect("write project config");

    let assert = isolated_cmd(home.path())
        .current_dir(&project)
        .arg("table")
        .arg("sessions")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(header_tokens, vec!["ID", "AGENT", "CWD"]);
}

#[test]
fn table_sessions_columns_cli_overrides_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = home.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    fs::write(
        project.join(".conspectus.toml"),
        "[table.sessions]\ncolumns = [\"id\", \"agent\"]\n",
    )
    .expect("write project config");

    let assert = isolated_cmd(home.path())
        .current_dir(&project)
        .arg("table")
        .arg("sessions")
        .arg("--columns")
        .arg("id,cwd")
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    let header_tokens: Vec<&str> = output.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(header_tokens, vec!["ID", "CWD"]);
}

#[test]
fn legacy_session_config_section_emits_stderr_warning() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = home.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    fs::write(
        project.join(".conspectus.toml"),
        "[session]\nprojection = \"union\"\n",
    )
    .expect("write project config");

    isolated_cmd(home.path())
        .current_dir(&project)
        .arg("table")
        .arg("sessions")
        .assert()
        .success()
        .stderr(predicate::str::contains("session"))
        .stderr(predicate::str::contains("table"));
}

#[test]
fn table_sessions_output_is_deterministic_across_runs() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    let first = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second = isolated_cmd(home.path())
        .current_dir(temp.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(first, second);
}

#[test]
fn graph_and_table_do_not_create_config_files_in_clean_repo() {
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
        .arg("table")
        .arg("sessions")
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
        "checkout:repo_common_dir={},root={}",
        common_dir.display(),
        repo_root.display()
    );

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("declared")
        .arg("create")
        .arg("--id")
        .arg("checkout-belongs")
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
    assert!(text.contains("checkout-belongs"), "config:\n{text}");
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
fn table_sessions_does_not_mutate_existing_project_declared_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();
    let config_path = repo.path().join(".conspectus.toml");
    let original = declared_config();
    fs::write(&config_path, original).expect("write project config");

    isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("table")
        .arg("sessions")
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

#[test]
fn tui_help_describes_subcommand_and_flags() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["tui", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("interactive terminal UI"))
        .stdout(predicate::str::contains("--scan-root"))
        .stdout(predicate::str::contains("--view"))
        .stdout(predicate::str::contains("--refresh-interval"))
        .stdout(predicate::str::contains("--mux-preview-interval"))
        .stdout(predicate::str::contains("--no-live-preview"))
        .stdout(predicate::str::contains("--sessions-grouping"));
}

#[test]
fn tui_rejects_unknown_view_value() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["tui", "--view", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn tui_rejects_malformed_refresh_interval() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["tui", "--refresh-interval", "abc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --refresh-interval"));
}

#[test]
fn tui_accepts_checkout_sessions_grouping_value() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args([
            "tui",
            "--sessions-grouping",
            "checkout",
            "--refresh-interval",
            "abc",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --refresh-interval"));
}

#[test]
fn tui_rejects_legacy_worktree_sessions_grouping_value() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["tui", "--sessions-grouping", "worktree"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'worktree'"));
}

#[test]
fn tui_rejects_unknown_sessions_grouping_value() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["tui", "--sessions-grouping", "potato"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_list_includes_named_replay_worlds() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["dev", "scenario", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ambiguous-mux"))
        .stdout(predicate::str::contains("codex-fd-current"));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_graph_renders_generated_world() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["dev", "scenario", "graph", "exact-match"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"nodes\""))
        .stdout(predicate::str::contains("session-x"))
        .stdout(predicate::str::contains("tmux:editor"));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_table_renders_generated_world() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args([
            "dev",
            "scenario",
            "table",
            "ambiguous-mux",
            "sessions",
            "--wide",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ambiguous"))
        .stdout(predicate::str::contains("MUX"));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_tui_help_lists_filter_group_and_sort_flags() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args(["dev", "scenario", "tui", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--grouping"))
        .stdout(predicate::str::contains("--harness"))
        .stdout(predicate::str::contains("--mux-state"))
        .stdout(predicate::str::contains("--max-age"))
        .stdout(predicate::str::contains("--sort"));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_tui_validates_grouping_for_selected_view() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args([
            "dev",
            "scenario",
            "tui",
            "ambiguous-mux",
            "--view",
            "sessions",
            "--grouping",
            "host",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid --grouping `host` for --view sessions",
        ));
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_tui_validates_filter_flags_before_launch() {
    Command::cargo_bin("conspectus")
        .expect("conspectus binary exists")
        .args([
            "dev",
            "scenario",
            "tui",
            "ambiguous-mux",
            "--max-age",
            "abc",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --max-age `abc`"));
}
