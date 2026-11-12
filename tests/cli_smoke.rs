use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

/// Builds a `conspectus` binary command isolated from the host environment:
/// HOME is pinned to an empty directory (so the harness adapters see no real
/// `~/.codex` etc.), tmux discovery is disabled, and `XDG_RUNTIME_DIR` is
/// pointed at a nonexistent subdirectory of `home` so the CLI's
/// `warm_start_discover_and_resolve` path (`src/cli/mod.rs:172`) cannot
/// find and defer to a real `conspectus serve` socket. Without the
/// runtime-dir override the tests silently pull the operator's live daemon
/// snapshot instead of exercising the local discovery + render path they
/// claim to pin, which surfaces as spurious failures on any box that has
/// the daemon running. Tests that want harness or tmux discovery override
/// these env vars explicitly.
fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    // Pin every XDG base directory under the temp home: CI runners set
    // XDG_CONFIG_HOME, and any XDG variable inherited from the host would
    // send the CLI's writes to the developer's real config, state, data,
    // or cache.
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_STATE_HOME", home.join(".local").join("state"));
    cmd.env("XDG_DATA_HOME", home.join(".local").join("share"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    // `hook init codex` writes under `CODEX_HOME`; one inherited from the
    // host would point it at the developer's real Codex config.
    cmd.env("CODEX_HOME", home.join(".codex"));
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd.env_remove("CONSPECTUS_PI_STATE");
    cmd.env_remove("PI_CODING_AGENT_DIR");
    cmd.env_remove("PI_CODING_AGENT_SESSION_DIR");
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
fn graph_json_explain_adds_resolver_score_breakdowns() {
    let home = tempfile::TempDir::new().expect("home temp");
    let repo = temp_git_repo();

    let default_assert = isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let default_output =
        String::from_utf8(default_assert.get_output().stdout.clone()).expect("utf8 stdout");
    let default_json: serde_json::Value =
        serde_json::from_str(&default_output).expect("valid json output");
    let default_relationship = default_json["resolved_relationships"]
        .as_array()
        .expect("relationships")
        .first()
        .expect("at least one resolved relationship");
    assert!(
        default_relationship.get("explanation").is_none(),
        "default graph JSON should stay compact: {default_relationship:?}"
    );

    let explain_assert = isolated_cmd(home.path())
        .current_dir(repo.path())
        .arg("graph")
        .arg("--format")
        .arg("json")
        .arg("--explain")
        .assert()
        .success();
    let explain_output =
        String::from_utf8(explain_assert.get_output().stdout.clone()).expect("utf8 stdout");
    let explain_json: serde_json::Value =
        serde_json::from_str(&explain_output).expect("valid json output");
    let explained_relationship = explain_json["resolved_relationships"]
        .as_array()
        .expect("relationships")
        .first()
        .expect("at least one resolved relationship");
    assert!(
        explained_relationship["explanation"]["selected"]["axes"]
            .as_array()
            .expect("score axes")
            .iter()
            .any(|axis| axis["name"] == "provenance"),
        "explain JSON should include selected score axes: {explained_relationship:?}"
    );
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
fn hook_write_pi_writes_latest_observation() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("pi")
        .arg("--state-root")
        .arg(state.path())
        .env("CONSPECTUS_PI_HOOK_VERSION", "1")
        .write_stdin(
            r#"{
              "session_id": "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d",
              "cwd": "/home/me/src/proj",
              "transcript_path": "/home/me/.pi/agent/sessions/--home-me-src-proj--/2026-10-16T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl",
              "hook_event_name": "session_start:resume"
            }"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let record = read_only_hook_record(state.path());
    assert_eq!(record["harness_key"], "pi");
    assert_eq!(
        record["session_key"],
        "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d"
    );
    assert_eq!(record["cwd"], "/home/me/src/proj");
    assert_eq!(record["hook_event_name"], "session_start:resume");
    assert_eq!(record["harness_version"], "1");
}

#[test]
fn hook_write_opencode_drops_an_empty_payload_silently() {
    let home = tempfile::TempDir::new().expect("home temp");
    let state = tempfile::TempDir::new().expect("state temp");

    // The harness waits on its hook, so a bad payload is dropped
    // without a word or a failing exit status.
    isolated_cmd(home.path())
        .arg("hook")
        .arg("write")
        .arg("opencode")
        .arg("--state-root")
        .arg(state.path())
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());
    assert!(!state.path().join("hooks-latest.json").exists());
}

#[test]
fn hook_init_status_remove_claude_code_preserves_existing_settings() {
    let home = tempfile::TempDir::new().expect("home temp");
    let settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("settings parent");
    let original = serde_json::json!({
        "theme": "dark",
        "permissions": { "allow": ["Bash(git status)"] },
        "hooks": {
            "SessionStart": [
                { "matcher": "startup", "hooks": [{ "type": "command", "command": "echo existing" }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command", "command": "/home/me/bin/notify-done", "timeout": 30 }] }
            ]
        }
    });
    fs::write(&settings, original.to_string()).expect("write settings");
    let hook = |args: &[&str]| {
        isolated_cmd(home.path())
            .arg("hook")
            .args(args)
            .assert()
            .success()
    };

    hook(&["status", "claude-code"])
        .stdout(predicate::str::starts_with("not-installed\tclaude-code\t"))
        .stdout(predicate::str::contains("missing\tclaude-code\t").count(14));

    hook(&["init", "claude-code", "--dry-run"])
        .stdout(predicate::str::contains("would install Claude Code hooks"));
    assert_eq!(
        read_settings(&settings),
        original,
        "a dry run writes nothing"
    );

    hook(&[
        "init",
        "claude-code",
        "--command",
        "conspectus hook write claude-code",
    ])
    .stdout(predicate::str::contains("installed Claude Code hooks"))
    .stdout(predicate::str::contains("(14 added, 0 updated)"));

    hook(&["status", "claude-code"])
        .stdout(predicate::str::starts_with("installed\tclaude-code\t"))
        .stdout(predicate::str::contains("installed\tclaude-code\t").count(15))
        .stdout(predicate::str::contains("\tSessionStart\n"))
        .stdout(predicate::str::contains("\tSessionEnd\n"))
        .stderr(predicate::str::is_empty());

    let installed = read_settings(&settings);
    assert_eq!(installed["theme"], "dark");
    assert_eq!(installed["permissions"], original["permissions"]);
    assert_eq!(
        installed["hooks"]["SessionStart"][0],
        original["hooks"]["SessionStart"][0]
    );
    assert_eq!(installed["hooks"]["Stop"][0], original["hooks"]["Stop"][0]);
    let events = installed["hooks"].as_object().expect("hooks");
    assert_eq!(events.len(), 14, "{installed}");
    for (event, groups) in events {
        let own = groups
            .as_array()
            .expect("groups")
            .last()
            .expect("Conspectus's group");
        assert_eq!(
            own["hooks"][0]["command"],
            "conspectus hook write claude-code"
        );
        assert_eq!(own["hooks"][0]["timeout"], 5, "{event}");
    }

    hook(&["init", "claude-code"]).stdout(predicate::str::contains(
        "Claude Code hooks already installed",
    ));
    assert_eq!(read_settings(&settings), installed);

    hook(&["remove", "claude-code"]).stdout(predicate::str::contains("removed Claude Code hooks"));
    assert_eq!(read_settings(&settings), original);
    hook(&["remove", "claude-code"])
        .stdout(predicate::str::contains("Claude Code hooks not installed"));
}

#[test]
fn hook_init_claude_code_upgrades_a_session_start_only_install() {
    let home = tempfile::TempDir::new().expect("home temp");
    let settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("settings parent");
    // What `hook init claude-code` wrote when it installed one entry.
    let old = serde_json::json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "resume|startup|clear|compact",
                    "hooks": [{ "type": "command", "command": "/old/bin/conspectus hook write claude-code" }]
                }
            ]
        }
    });
    fs::write(&settings, old.to_string()).expect("write settings");
    let hook = |args: &[&str]| {
        isolated_cmd(home.path())
            .arg("hook")
            .args(args)
            .assert()
            .success()
    };

    hook(&["status", "claude-code"])
        .stdout(predicate::str::starts_with("partial\tclaude-code\t"))
        .stdout(predicate::str::contains("outdated\tclaude-code\t").count(1))
        .stdout(predicate::str::contains("\tSessionStart\n"))
        .stdout(predicate::str::contains("missing\tclaude-code\t").count(13))
        .stderr(predicate::str::contains("conspectus hook init claude-code"));

    hook(&["init", "claude-code"]).stdout(predicate::str::contains("(13 added, 1 updated)"));
    let upgraded = read_settings(&settings);
    let session_start = upgraded["hooks"]["SessionStart"]
        .as_array()
        .expect("groups");
    assert_eq!(session_start.len(), 1, "never duplicated: {upgraded}");
    assert_eq!(
        session_start[0],
        serde_json::json!({
            "matcher": "startup|resume|clear|compact|fork",
            "hooks": [{
                "type": "command",
                "command": "/old/bin/conspectus hook write claude-code",
                "timeout": 5
            }]
        })
    );
    // New events run the command the old install ran.
    assert_eq!(
        upgraded["hooks"]["Notification"][0]["hooks"][0]["command"],
        "/old/bin/conspectus hook write claude-code"
    );
    hook(&["status", "claude-code"])
        .stdout(predicate::str::starts_with("installed\tclaude-code\t"))
        .stdout(predicate::str::contains("installed\tclaude-code\t").count(15));

    hook(&["remove", "claude-code"]);
    assert_eq!(read_settings(&settings), serde_json::json!({}));
}

#[test]
fn hook_init_claude_code_project_scope_leaves_the_user_settings_alone() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = tempfile::TempDir::new().expect("project temp");
    let hook = |args: &[&str]| {
        isolated_cmd(home.path())
            .current_dir(project.path())
            .arg("hook")
            .args(args)
            .assert()
            .success()
    };

    hook(&["init", "claude-code", "--scope", "project"])
        .stdout(predicate::str::contains("(14 added, 0 updated)"));
    let settings = project.path().join(".claude/settings.json");
    assert_eq!(
        read_settings(&settings)["hooks"]
            .as_object()
            .expect("hooks")
            .len(),
        14
    );
    assert!(!home.path().join(".claude").exists());

    hook(&["status", "claude-code", "--scope", "project"])
        .stdout(predicate::str::starts_with("installed\tclaude-code\t"));
    hook(&["status", "claude-code"])
        .stdout(predicate::str::starts_with("not-installed\tclaude-code\t"));
    hook(&["remove", "claude-code", "--scope", "project"]);
    assert_eq!(read_settings(&settings), serde_json::json!({}));
}

#[test]
fn hook_init_claude_code_refuses_a_hooks_table_it_would_replace() {
    let home = tempfile::TempDir::new().expect("home temp");
    let settings = home.path().join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("settings parent");
    let text = r#"{"hooks": {"Stop": {"not": "a list"}}}"#;
    fs::write(&settings, text).expect("write settings");

    isolated_cmd(home.path())
        .args(["hook", "init", "claude-code"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("hooks.Stop"));
    assert_eq!(fs::read_to_string(&settings).expect("read"), text);
}

/// The last line `hook status <harness>` prints: the live-state health.
fn health_line(home: &Path, harness: &str) -> String {
    let assert = isolated_cmd(home)
        .env("CODEX_HOME", home.join(".codex"))
        .args(["hook", "status", harness])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    stdout.lines().last().expect("a health line").to_string()
}

fn install_claude_hooks(home: &Path) {
    isolated_cmd(home)
        .args([
            "hook",
            "init",
            "claude-code",
            "--command",
            "conspectus hook write claude-code",
        ])
        .assert()
        .success();
}

#[test]
fn hook_status_says_how_to_turn_live_state_on_when_the_hooks_are_not_installed() {
    let home = tempfile::TempDir::new().expect("home temp");
    let store = home.path().join(".local/state/conspectus/live-state.json");
    assert_eq!(
        health_line(home.path(), "claude-code"),
        format!(
            "off\tclaude-code\t{}\tlive state: off; `conspectus hook init claude-code` installs \
             the hooks that report it",
            store.display()
        )
    );
}

#[test]
fn hook_status_counts_the_sessions_whose_hooks_report_live_state() {
    use conspectus::live_store::{LiveEvent, LiveEventKind, LiveStore, now_ms};

    let home = tempfile::TempDir::new().expect("home temp");
    install_claude_hooks(home.path());
    assert!(
        health_line(home.path(), "claude-code").starts_with("no-events\tclaude-code\t"),
        "installed, with nothing heard and no agent running"
    );

    let store = LiveStore::at(home.path().join(".local/state/conspectus"));
    for session in ["alpha", "beta"] {
        store
            .apply_event(&LiveEvent::new(
                "claude-code",
                session,
                now_ms(),
                LiveEventKind::TurnStarted,
            ))
            .expect("write live state");
    }
    let line = health_line(home.path(), "claude-code");
    assert!(
        line.starts_with(&format!(
            "receiving\tclaude-code\t{}\tlive state: 2 sessions, newest event ",
            store.path().display()
        )),
        "{line}"
    );
    assert!(line.ends_with(" ago"), "{line}");
}

#[test]
fn hook_status_calls_an_installed_hook_silent_when_its_agent_runs_without_events() {
    use conspectus::model::{
        GraphNode, GraphSnapshot, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole,
    };

    let home = tempfile::TempDir::new().expect("home temp");
    install_claude_hooks(home.path());
    // The graph the last discovery saved holds a Claude Code agent: this
    // test's own process, which runs while `hook status` looks.
    let agent = GraphSnapshot {
        nodes: vec![GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: RuntimeProcessId::new("mux_session:tmux:work:root:1:pid:2"),
            observation_key: "mux_session:tmux:work:root:1:pid:2".to_string(),
            pid: Some(i64::from(std::process::id())),
            parent_pid: None,
            root_pane_pid: None,
            command: Some("claude".to_string()),
            cwd: None,
            harness_key: Some("claude-code".to_string()),
            role: Some(RuntimeProcessRole::HumanAgent),
            depth: Some(0),
            observed_epoch: None,
        })],
        ..GraphSnapshot::empty()
    };
    conspectus::snapshot::write_atomic(
        &home.path().join(".local/share/conspectus/graph.bin"),
        &agent,
    )
    .expect("write graph.bin");

    let line = health_line(home.path(), "claude-code");
    assert!(
        line.starts_with("silent\tclaude-code\t"),
        "an installed hook with a running agent and no events: {line}"
    );
    assert!(
        line.ends_with(
            "\tlive state: active but no events: hooks may be untrusted, disabled, or the \
             agent was started before they were installed"
        ),
        "{line}"
    );

    // Not installed, the same agent is told how to install the hooks.
    isolated_cmd(home.path())
        .args(["hook", "remove", "claude-code"])
        .assert()
        .success();
    assert!(health_line(home.path(), "claude-code").starts_with("off\tclaude-code\t"));
}

#[test]
fn hook_status_reports_codex_live_state_health() {
    use conspectus::live_store::{LiveEvent, LiveEventKind, LiveStore, now_ms};

    let home = tempfile::TempDir::new().expect("home temp");
    assert!(
        health_line(home.path(), "codex").starts_with("off\tcodex\t"),
        "not installed"
    );
    isolated_cmd(home.path())
        .args([
            "hook",
            "init",
            "codex",
            "--command",
            "conspectus hook write codex",
        ])
        .assert()
        .success();
    assert!(
        health_line(home.path(), "codex").starts_with("no-events\tcodex\t"),
        "installed, with nothing heard and no agent running"
    );

    let store = LiveStore::at(home.path().join(".local/state/conspectus"));
    store
        .apply_event(&LiveEvent::new(
            "codex",
            "019a7c3e-5b2d-7f41-9c8e-2d4b6a1f0e37",
            now_ms(),
            LiveEventKind::TurnStarted,
        ))
        .expect("write live state");
    let line = health_line(home.path(), "codex");
    assert!(
        line.starts_with(&format!(
            "receiving\tcodex\t{}\tlive state: 1 session, newest event ",
            store.path().display()
        )),
        "{line}"
    );
}

#[test]
fn hook_status_reports_pi_live_state_health() {
    let home = tempfile::TempDir::new().expect("home temp");
    let store = home.path().join(".local/state/conspectus/live-state.json");
    assert_eq!(
        health_line(home.path(), "pi"),
        format!(
            "off\tpi\t{}\tlive state: off; `conspectus hook init pi` installs the hooks that \
             report it",
            store.display()
        )
    );
    isolated_cmd(home.path())
        .args(["hook", "init", "pi"])
        .assert()
        .success();
    assert!(
        health_line(home.path(), "pi").starts_with("no-events\tpi\t"),
        "installed, with nothing heard and no agent running"
    );

    // A snapshot the extension sends, through the writer.
    let snapshot = serde_json::json!({
        "session_id": "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d",
        "cwd": "/home/user/projects/my-project",
        "hook_event_name": "live:agent_start",
        "live": {
            "instance": "mhv8k2b5-2c4e6a8b0d2f4a63", "seq": 2,
            "observed_ms": conspectus::live_store::now_ms(), "heartbeat_s": 60,
            "state": "working", "detail": "generating", "subagents": 0,
            "since_ms": conspectus::live_store::now_ms(),
        },
    });
    isolated_cmd(home.path())
        .args(["hook", "write", "pi"])
        .write_stdin(snapshot.to_string())
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());
    let line = health_line(home.path(), "pi");
    assert!(
        line.starts_with(&format!(
            "receiving\tpi\t{}\tlive state: 1 session, newest event ",
            store.display()
        )),
        "{line}"
    );
    assert!(
        !home
            .path()
            .join(".local/state/conspectus/hooks/hooks-latest.json")
            .exists(),
        "a live-state snapshot is no attribution record"
    );
}

fn read_settings(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read settings")).expect("settings json")
}

/// A Codex install case from `tests/fixtures/hooks/codex/install/`.
fn codex_case(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks/codex/install")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// The events `hook init codex` installs.
const CODEX_EVENTS: usize = 11;

/// A scratch home whose `~/.codex/config.toml` holds `text`.
fn codex_home(text: &str) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::TempDir::new().expect("home temp");
    let config = home.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().expect("parent")).expect("config parent");
    fs::write(&config, text).expect("write config");
    (home, config)
}

fn codex_hook(home: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    isolated_cmd(home).arg("hook").args(args).assert()
}

#[test]
fn hook_init_status_remove_codex_keeps_an_operators_tables_byte_for_byte() {
    let original = codex_case("operator-array-of-tables.toml");
    let (home, config) = codex_home(&original);
    let home = home.path();

    codex_hook(home, &["status", "codex"])
        .success()
        .stdout(predicate::str::starts_with("not-installed\tcodex\t"))
        .stdout(predicate::str::contains("missing\tcodex\t").count(CODEX_EVENTS));
    codex_hook(home, &["init", "codex", "--dry-run"])
        .success()
        .stdout(predicate::str::contains("would install Codex hooks"));
    assert_eq!(
        fs::read_to_string(&config).expect("read"),
        original,
        "a dry run writes nothing"
    );

    codex_hook(
        home,
        &["init", "codex", "--command", "conspectus hook write codex"],
    )
    .success()
    .stdout(predicate::str::contains(format!(
        "installed Codex hooks in {} ({CODEX_EVENTS} added, 0 updated)",
        config.display()
    )))
    .stdout(predicate::str::contains("open /hooks in Codex"))
    .stderr(predicate::str::is_empty());
    let installed = fs::read_to_string(&config).expect("read");
    for line in original.lines() {
        assert!(installed.contains(line), "lost {line:?}:\n{installed}");
    }
    assert!(
        installed.contains(
            "[[hooks.SessionStart]]\nhooks = [{ type = \"command\", command = \"conspectus hook \
             write codex\", timeout = 5 }]\n"
        ),
        "{installed}"
    );

    codex_hook(home, &["status", "codex"])
        .success()
        .stdout(predicate::str::starts_with("installed\tcodex\t"))
        .stdout(predicate::str::contains("installed\tcodex\t").count(CODEX_EVENTS + 1))
        .stdout(predicate::str::contains("\tSessionStart\n"))
        .stderr(predicate::str::is_empty());
    codex_hook(home, &["init", "codex"])
        .success()
        .stdout(predicate::str::contains("Codex hooks already installed"))
        .stdout(predicate::str::contains("/hooks"));
    assert_eq!(fs::read_to_string(&config).expect("read"), installed);

    codex_hook(home, &["remove", "codex"])
        .success()
        .stdout(predicate::str::contains("removed Codex hooks"))
        .stderr(predicate::str::is_empty());
    assert_eq!(fs::read_to_string(&config).expect("read"), original);
    codex_hook(home, &["remove", "codex"])
        .success()
        .stdout(predicate::str::contains("Codex hooks not installed"));
}

#[test]
fn hook_init_codex_upgrades_an_older_install_in_place() {
    let original = codex_case("inline-lists.toml");
    let (home, config) = codex_home(&original);
    let home = home.path();

    codex_hook(home, &["status", "codex"])
        .success()
        .stdout(predicate::str::starts_with("partial\tcodex\t"))
        .stdout(predicate::str::contains("outdated\tcodex\t").count(1))
        .stderr(predicate::str::contains("conspectus hook init codex"));
    codex_hook(home, &["init", "codex"])
        .success()
        .stdout(predicate::str::contains(format!(
            "({} added, 1 updated)",
            CODEX_EVENTS - 1
        )));
    let upgraded = fs::read_to_string(&config).expect("read");
    assert_eq!(
        upgraded.matches("hook write codex").count(),
        CODEX_EVENTS,
        "never duplicated: {upgraded}"
    );
    assert!(upgraded.contains("echo existing"));
    codex_hook(home, &["status", "codex"])
        .success()
        .stdout(predicate::str::starts_with("installed\tcodex\t"));
}

#[test]
fn hook_init_codex_keeps_inline_and_dotted_hooks_and_the_recorded_trust() {
    for name in ["inline-hooks.toml", "dotted-keys.toml", "hooks-state.toml"] {
        let original = codex_case(name);
        let (home, config) = codex_home(&original);
        let home = home.path();
        codex_hook(
            home,
            &["init", "codex", "--command", "conspectus hook write codex"],
        )
        .success();
        let installed = fs::read_to_string(&config).expect("read");
        assert!(installed.contains("hook write codex"), "{name}");
        assert_eq!(
            installed.matches("trusted_hash").count(),
            original.matches("trusted_hash").count(),
            "{name}: no trust written"
        );
        codex_hook(home, &["remove", "codex"]).success();
        assert_eq!(
            fs::read_to_string(&config).expect("read"),
            original,
            "{name}"
        );
    }
}

#[test]
fn hook_remove_codex_takes_its_group_from_between_the_operators() {
    let original = codex_case("conspectus-between-operator-groups.toml");
    let (home, config) = codex_home(&original);
    codex_hook(home.path(), &["remove", "codex"])
        .success()
        .stdout(predicate::str::contains("removed Codex hooks"))
        .stderr(predicate::str::contains("review the hooks that came after"))
        .stderr(predicate::str::contains("/hooks"));
    let removed = fs::read_to_string(&config).expect("read");
    assert!(!removed.contains("hook write codex"), "{removed}");
    assert!(removed.contains("echo first") && removed.contains("echo third"));
}

#[test]
fn hook_init_codex_installs_beside_a_hooks_json_and_leaves_it_alone() {
    let (home, config) = codex_home("model = \"gpt-5.4\"\n");
    let hooks_json = config.with_file_name("hooks.json");
    let json = codex_case("hooks.json");
    fs::write(&hooks_json, &json).expect("hooks.json");

    codex_hook(home.path(), &["init", "codex"])
        .success()
        .stdout(predicate::str::contains("installed Codex hooks"))
        .stderr(predicate::str::contains(format!(
            "{} also holds Codex hooks",
            hooks_json.display()
        )));
    assert!(
        fs::read_to_string(&config)
            .expect("read")
            .contains("hook write codex")
    );
    codex_hook(home.path(), &["status", "codex"])
        .success()
        .stdout(predicate::str::starts_with("installed\tcodex\t"))
        .stderr(predicate::str::contains("hooks.json"));
    codex_hook(home.path(), &["remove", "codex"]).success();
    assert_eq!(fs::read_to_string(&hooks_json).expect("read"), json);
    assert_eq!(
        fs::read_to_string(&config).expect("read"),
        "model = \"gpt-5.4\"\n"
    );
}

#[test]
fn hook_init_codex_refuses_a_hooks_value_it_would_replace() {
    let text = "[hooks.SessionStart]\nmatcher = \"startup\"\n";
    let (home, config) = codex_home(text);
    codex_hook(home.path(), &["init", "codex"])
        .failure()
        .stderr(predicate::str::contains("hooks.SessionStart"));
    assert_eq!(fs::read_to_string(&config).expect("read"), text);
}

#[test]
fn hook_init_codex_writes_through_a_symlink_and_keeps_the_files_mode() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::TempDir::new().expect("home temp");
    let dotfiles = home.path().join("dotfiles");
    fs::create_dir_all(&dotfiles).expect("dotfiles");
    let target = dotfiles.join("codex-config.toml");
    fs::write(&target, "model = \"gpt-5.4\"\n").expect("target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("chmod");
    let link = home.path().join(".codex/config.toml");
    fs::create_dir_all(link.parent().expect("parent")).expect("codex dir");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");

    codex_hook(home.path(), &["init", "codex"]).success();
    assert!(
        fs::symlink_metadata(&link)
            .expect("link")
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::read_to_string(&target)
            .expect("target")
            .contains("hook write codex")
    );
    assert_eq!(
        fs::metadata(&target).expect("target").permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn hook_init_codex_project_scope_leaves_the_user_config_alone() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = tempfile::TempDir::new().expect("project temp");
    let hook = |args: &[&str]| {
        isolated_cmd(home.path())
            .current_dir(project.path())
            .arg("hook")
            .args(args)
            .assert()
            .success()
    };
    hook(&["init", "codex", "--scope", "project"]).stdout(predicate::str::contains(format!(
        "({CODEX_EVENTS} added, 0 updated)"
    )));
    let config = project.path().join(".codex/config.toml");
    assert!(
        fs::read_to_string(&config)
            .expect("read")
            .contains("hook write codex")
    );
    assert!(!home.path().join(".codex").exists());
    hook(&["status", "codex", "--scope", "project"])
        .stdout(predicate::str::starts_with("installed\tcodex\t"));
    hook(&["status", "codex"]).stdout(predicate::str::starts_with("not-installed\tcodex\t"));
    hook(&["remove", "codex", "--scope", "project"]);
    assert_eq!(fs::read_to_string(&config).expect("read"), "");
}

#[test]
fn hook_init_status_remove_pi_manages_only_its_own_extension_file() {
    let home = tempfile::TempDir::new().expect("home temp");
    let extensions = home.path().join(".pi/agent/extensions");
    let installed = extensions.join("conspectus-hook.ts");
    let neighbor = extensions.join("my-extension.ts");
    fs::create_dir_all(&extensions).expect("extensions dir");
    fs::write(&neighbor, "export default function () {}\n").expect("neighbor");
    let hook = |args: &[&str]| {
        isolated_cmd(home.path())
            .arg("hook")
            .args(args)
            .assert()
            .success()
    };

    hook(&["status", "pi"]).stdout(predicate::str::contains("not-installed\tpi"));
    hook(&["init", "pi", "--dry-run"]).stdout(predicate::str::contains("would install"));
    assert!(!installed.exists(), "dry run must not write");

    hook(&["init", "pi"]).stdout(predicate::str::contains("installed Pi hook extension"));
    let body = fs::read_to_string(&installed).expect("installed extension");
    assert!(body.starts_with("// conspectus-hook:"));
    assert!(body.contains(r#"["hook", "write", "pi"]"#));
    assert!(body.contains(r#"const EXTENSION_VERSION = "2";"#));
    hook(&["status", "pi"])
        .stdout(predicate::str::contains("installed\tpi"))
        .stdout(predicate::str::contains("no-events\tpi"))
        .stderr(predicate::str::contains("waiting needs Pi 0.84.4 or later"))
        .stderr(predicate::str::contains("1.1.0"))
        .stderr(predicate::str::contains("outdated").not());
    hook(&["init", "pi"]).stdout(predicate::str::contains("already installed"));

    // An older copy is reported and replaced.
    fs::write(&installed, "// conspectus-hook: an older version\n").expect("downgrade");
    hook(&["status", "pi"])
        .stdout(predicate::str::contains("outdated\tpi"))
        .stderr(predicate::str::contains("is an older version"))
        .stderr(predicate::str::contains(
            "`conspectus hook init pi` replaces it",
        ))
        .stderr(predicate::str::contains("no live state"))
        .stderr(predicate::str::contains("0.84.4"));
    // The first version reports sessions only, so live state is off.
    fs::write(
        &installed,
        "// conspectus-hook: installed by `conspectus hook init pi`\nconst EXTENSION_VERSION = \"1\";\n",
    )
    .expect("the first version");
    hook(&["status", "pi"])
        .stdout(predicate::str::contains("outdated\tpi"))
        .stdout(predicate::str::contains("off\tpi"))
        .stderr(predicate::str::contains(
            "is version 1 and this build installs version 2",
        ));
    hook(&["init", "pi"]).stdout(predicate::str::contains("installed Pi hook extension"));
    assert_eq!(fs::read_to_string(&installed).expect("reinstalled"), body);
    // A newer version than this build's still reports live state.
    fs::write(
        &installed,
        "// conspectus-hook: installed\nconst EXTENSION_VERSION = \"9\";\n",
    )
    .expect("a newer version");
    hook(&["status", "pi"])
        .stdout(predicate::str::contains("outdated\tpi"))
        .stdout(predicate::str::contains("no-events\tpi"))
        .stderr(predicate::str::contains("is version 9"))
        .stderr(predicate::str::contains("no live state").not());
    hook(&["init", "pi"]).stdout(predicate::str::contains("installed Pi hook extension"));

    hook(&["remove", "pi"]).stdout(predicate::str::contains("removed Pi hook extension"));
    assert!(!installed.exists());
    hook(&["remove", "pi"]).stdout(predicate::str::contains("not installed"));
    assert_eq!(
        fs::read_to_string(&neighbor).expect("neighbor survives"),
        "export default function () {}\n"
    );
}

#[test]
fn hook_init_and_remove_pi_leave_a_foreign_file_alone() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = tempfile::TempDir::new().expect("project temp");
    let path = project.path().join(".pi/extensions/conspectus-hook.ts");
    fs::create_dir_all(path.parent().expect("parent")).expect("project extensions");
    fs::write(&path, "// someone else's extension\n").expect("foreign");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["hook", "status", "pi", "--scope", "project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("foreign\tpi"));
    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["hook", "init", "pi", "--scope", "project"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("wasn't written by Conspectus"));
    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["hook", "remove", "pi", "--scope", "project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("leaving it in place"));
    assert_eq!(
        fs::read_to_string(&path).expect("foreign survives"),
        "// someone else's extension\n"
    );
}

#[test]
fn hook_init_pi_project_scope_and_agent_dir_override() {
    let home = tempfile::TempDir::new().expect("home temp");
    let project = tempfile::TempDir::new().expect("project temp");
    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["hook", "init", "pi", "--scope", "project"])
        .assert()
        .success();
    assert!(
        project
            .path()
            .join(".pi/extensions/conspectus-hook.ts")
            .is_file()
    );

    let agent = tempfile::TempDir::new().expect("agent temp");
    isolated_cmd(home.path())
        .env("PI_CODING_AGENT_DIR", agent.path())
        .args(["hook", "init", "pi"])
        .assert()
        .success();
    assert!(agent.path().join("extensions/conspectus-hook.ts").is_file());
    assert!(!home.path().join(".pi").exists());
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
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");
    let missing = temp.path().join("missing");

    // Use isolated_cmd so `XDG_RUNTIME_DIR` steers the CLI away from
    // any running `conspectus serve` socket; otherwise the CLI's
    // daemon short-circuit (`src/cli/mod.rs:172`) would return the
    // daemon's snapshot without ever validating the operator's
    // `--scan-root` argument and this rejection assertion would
    // flip on any box with the daemon running.
    isolated_cmd(home.path())
        .arg("graph")
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

    assert!(
        !json["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| node["type"] == "mux_session"),
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

    assert!(
        output.lines().nth(2).is_some(),
        "expected at least one body row"
    );
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
fn the_live_columns_are_listed_opt_in_and_chosen_by_flag_or_config() {
    let home = tempfile::TempDir::new().expect("home temp");
    let temp = tempfile::TempDir::new().expect("temp dir");

    for row_type in ["sessions", "mux"] {
        let assert = isolated_cmd(home.path())
            .current_dir(temp.path())
            .args(["columns", row_type])
            .assert()
            .success();
        let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        let line = output
            .lines()
            .find(|line| line.starts_with("live "))
            .unwrap_or_else(|| panic!("no live column for {row_type}:\n{output}"));
        assert!(!line.contains("(default)"), "{line}");

        let assert = isolated_cmd(home.path())
            .current_dir(temp.path())
            .args(["table", row_type, "--no-cache", "--columns", "id,live"])
            .assert()
            .success();
        let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        assert_eq!(
            output
                .lines()
                .next()
                .map(str::split_whitespace)
                .map(Iterator::collect::<Vec<_>>),
            Some(vec!["ID", "LIVE"]),
            "{output}"
        );
    }

    let config = home.path().join(".config/conspectus/config.toml");
    fs::create_dir_all(config.parent().expect("parent")).expect("config dir");
    fs::write(&config, "[table.mux]\ncolumns = [\"mux\", \"live\"]\n").expect("config");
    let assert = isolated_cmd(home.path())
        .current_dir(temp.path())
        .args(["table", "mux", "--no-cache"])
        .assert()
        .success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert_eq!(
        output
            .lines()
            .next()
            .map(str::split_whitespace)
            .map(Iterator::collect::<Vec<_>>),
        Some(vec!["MUX", "LIVE"]),
        "{output}"
    );
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
        .stderr(predicate::str::contains("invalid value 'all' for '--store"));
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
    assert!(has_local_declared, "candidates:\n{output}");
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
                link["id"].as_str().map(std::string::ToString::to_string)
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
fn tui_rejects_removed_view_values_and_names_the_valid_views() {
    for removed in ["union", "forks"] {
        Command::cargo_bin("conspectus")
            .expect("conspectus binary exists")
            .args(["tui", "--view", removed])
            .assert()
            .failure()
            .stderr(predicate::str::contains(format!(
                "invalid value '{removed}' for '--view <VIEW>'"
            )))
            .stderr(predicate::str::contains(
                "[possible values: sessions, mux, prs, pinned, history, search]",
            ));
    }
}

#[cfg(debug_assertions)]
#[test]
fn dev_scenario_tui_rejects_removed_view_values_and_names_the_valid_views() {
    for removed in ["union", "forks"] {
        Command::cargo_bin("conspectus")
            .expect("conspectus binary exists")
            .args(["dev", "scenario", "tui", "ambiguous-mux", "--view", removed])
            .assert()
            .failure()
            .stderr(predicate::str::contains(format!(
                "invalid value '{removed}' for '--view <VIEW>'"
            )))
            .stderr(predicate::str::contains(
                "[possible values: sessions, mux, prs, pinned, history, search]",
            ));
    }
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

#[test]
fn csp_is_the_same_program_under_its_short_name() {
    // ADR 0118: `csp` is installed alongside `conspectus`; help follows
    // the name it was run as, and the version names the product.
    Command::cargo_bin("csp")
        .expect("csp binary exists")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: csp [COMMAND]"));
    Command::cargo_bin("csp")
        .expect("csp binary exists")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("conspectus "));
}
