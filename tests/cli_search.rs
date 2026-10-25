//! `conspectus search`: finds sessions by what was said in them, reading
//! transcripts on demand in an isolated home.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

const ALPHA: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const BETA: &str = "bbbbbbbb-0000-0000-0000-000000000002";

fn record(key: &str, kind: &str, uuid: &str, parent: &str, time: &str, content: &str) -> String {
    format!(
        r#"{{"cwd":"/home/user/project","sessionId":"{key}","type":"{kind}","uuid":"{uuid}","parentUuid":{parent},"timestamp":"{time}","message":{{"role":"{kind}","content":{content}}}}}"#
    )
}

fn text(body: &str) -> String {
    format!(r#"[{{"type":"text","text":"{body}"}}]"#)
}

/// Two sessions: ALPHA mentions "quokka" twice in messages and once in a
/// tool call; BETA only chats.
fn write_sessions(state: &Path) {
    let dir = state.join("projects").join("-home-user-project");
    fs::create_dir_all(&dir).unwrap();
    let alpha = [
        record(
            ALPHA,
            "user",
            "a1",
            "null",
            "2026-10-01T10:00:00.000Z",
            &text("We chose the quokka design"),
        ),
        record(
            ALPHA,
            "assistant",
            "a2",
            "\"a1\"",
            "2026-10-01T10:00:05.000Z",
            &text("The Quokka layout is noted."),
        ),
        record(
            ALPHA,
            "assistant",
            "a3",
            "\"a2\"",
            "2026-10-01T10:00:09.000Z",
            r#"[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"echo marsupial-in-a-tool"}}]"#,
        ),
    ];
    let beta = [
        record(
            BETA,
            "user",
            "b1",
            "null",
            "2026-10-02T09:00:00.000Z",
            &text("Unrelated chatter"),
        ),
        record(
            BETA,
            "assistant",
            "b2",
            "\"b1\"",
            "2026-10-02T09:00:03.000Z",
            &text("Nothing to see"),
        ),
    ];
    fs::write(dir.join(format!("{ALPHA}.jsonl")), alpha.join("\n") + "\n").unwrap();
    fs::write(dir.join(format!("{BETA}.jsonl")), beta.join("\n") + "\n").unwrap();
}

struct World {
    _temp: tempfile::TempDir,
    home: std::path::PathBuf,
    claude: std::path::PathBuf,
}

fn world() -> World {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_sessions(&claude);
    World {
        _temp: temp,
        home,
        claude,
    }
}

impl World {
    fn search(&self) -> Command {
        let mut cmd = Command::cargo_bin("conspectus").expect("binary");
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", self.home.join("no-daemon"))
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_STATE_HOME", self.home.join(".local/state"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .env("CONSPECTUS_DISABLE_TMUX", "1")
            .env("CONSPECTUS_DISABLE_FORGE", "1")
            .env("CONSPECTUS_CLAUDE_CODE_STATE", &self.claude)
            .current_dir(&self.home)
            .arg("search");
        cmd
    }
}

fn stdout_of(mut cmd: Command) -> String {
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn json_per_session_is_ranked_and_stable() {
    let world = world();
    let mut cmd = world.search();
    cmd.args(["quokka", "--format", "json"]);
    insta::assert_snapshot!("search_json_per_session", stdout_of(cmd));
}

#[test]
fn json_per_message_lists_each_matching_turn() {
    let world = world();
    let mut cmd = world.search();
    cmd.args(["quokka", "--format", "json", "--group", "message"]);
    insta::assert_snapshot!("search_json_per_message", stdout_of(cmd));
}

#[test]
fn json_flat_lists_matching_messages_without_session_grouping() {
    let world = world();
    let mut cmd = world.search();
    cmd.args(["quokka", "--format", "json", "--group", "flat"]);
    insta::assert_snapshot!("search_json_flat", stdout_of(cmd));
}

#[test]
fn a_limit_on_flat_results_is_noted_on_stderr_and_in_json() {
    let world = world();
    let output = {
        let mut cmd = world.search();
        cmd.args([
            "quokka", "--group", "flat", "--limit", "1", "--color", "never",
        ]);
        cmd.output().unwrap()
    };
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 2, "header plus one row: {stdout}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("showing the newest 1 of"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args([
            "quokka", "--group", "flat", "--limit", "1", "--format", "json",
        ]);
        cmd
    });
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["limited"]["shown"], 1);
    assert!(value["limited"]["total"].as_u64().unwrap() > 1, "{out}");
}

#[test]
fn the_table_has_a_header_one_row_per_session_and_exit_zero() {
    let world = world();
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args(["QUOKKA", "--width", "100"]);
        cmd
    });
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].starts_with("SESSION"), "{out}");
    assert!(
        lines[0].contains("HITS") && lines[0].contains("SNIPPET"),
        "{out}"
    );
    assert!(lines[1].contains("claude-code"), "{out}");
    assert!(lines[1].contains("quokka"), "{out}");
    assert!(lines.iter().all(|l| l.chars().count() <= 100), "{out}");
}

#[test]
fn no_match_exits_one_with_nothing_on_stdout() {
    let world = world();
    world
        .search()
        .arg("zebra")
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty());
}

#[test]
fn no_match_in_table_mode_says_so_on_stderr_only() {
    let world = world();
    world
        .search()
        .arg("zebra")
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("No sessions match \"zebra\""));
}

#[test]
fn no_match_in_json_is_an_empty_document_with_exit_one() {
    let world = world();
    let mut cmd = world.search();
    cmd.args(["zebra", "--format", "json"]);
    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["results"], serde_json::json!([]));
    assert_eq!(value["truncated"], serde_json::json!([]));
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("No sessions match"),
        "the note is for table mode"
    );
}

#[test]
fn per_message_rows_run_in_turn_order_with_one_based_turns() {
    let world = world();
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args(["quokka", "--group", "message", "--color", "never"]);
        cmd
    });
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "{out}");
    assert!(
        lines[1].contains("#1") && lines[1].contains(" you "),
        "{out}"
    );
    assert!(
        lines[2].contains("#2") && lines[2].contains(" ai "),
        "{out}"
    );
}

#[test]
fn tool_rows_use_the_tui_vocabulary() {
    let world = world();
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args(["marsupial", "--tools", "--group", "message"]);
        cmd
    });
    assert!(out.contains("tool call"), "{out}");
}

#[test]
fn thinking_is_searched_only_when_asked() {
    let world = world();
    let dir = world.claude.join("projects/-home-user-project");
    let key = "cccccccc-0000-0000-0000-000000000003";
    let line = record(
        key,
        "assistant",
        "c1",
        "null",
        "2026-10-03T09:00:00.000Z",
        r#"[{"type":"thinking","thinking":"pondering the axolotl question"}]"#,
    );
    fs::write(dir.join(format!("{key}.jsonl")), line + "\n").unwrap();
    world.search().arg("axolotl").assert().code(1);
    world
        .search()
        .args(["axolotl", "--thinking"])
        .assert()
        .success()
        .stdout(predicate::str::contains("axolotl"));
}

#[test]
fn mux_state_narrows_by_the_derived_state() {
    let world = world();
    world
        .search()
        .args(["quokka", "--mux-state", "unmuxed"])
        .assert()
        .success();
    world
        .search()
        .args(["quokka", "--mux-state", "attached"])
        .assert()
        .code(1);
}

#[test]
fn a_matching_harness_and_age_keep_the_results() {
    let world = world();
    world
        .search()
        .args(["quokka", "--harness", "claude-code", "--max-age", "36500d"])
        .assert()
        .success()
        .stdout(predicate::str::contains("quokka"));
}

#[test]
fn a_zero_limit_is_an_error() {
    let world = world();
    world
        .search()
        .args(["quokka", "--limit", "0"])
        .assert()
        .code(2);
}

#[test]
fn verbose_reports_what_was_searched_and_what_was_not() {
    let world = world();
    world
        .search()
        .args(["quokka", "--verbose"])
        .assert()
        .success()
        .stderr(predicate::str::contains("searched 2 of 2 sessions"))
        .stderr(predicate::str::contains("no transcript parser"));
}

#[test]
fn config_sets_the_grouping_and_a_bad_value_is_reported() {
    let world = world();
    let config = world.home.join(".config/conspectus");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.toml"),
        "[search]\ngroup = \"message\"\n",
    )
    .unwrap();
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args(["quokka", "--format", "json"]);
        cmd
    });
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["group"], "message");

    fs::write(config.join("config.toml"), "[search]\ngroup = \"turn\"\n").unwrap();
    let mut cmd = world.search();
    cmd.args(["quokka", "--format", "json"]);
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["group"], "session", "a bad value keeps the default");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid `[search] group` value `turn`"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn errors_exit_two() {
    let world = world();
    world
        .search()
        .arg("  ")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("the query is empty"));
    world
        .search()
        .args(["quokka", "--max-age", "forever"])
        .assert()
        .code(2);
}

#[test]
fn tool_content_is_searched_only_when_asked() {
    let world = world();
    world.search().arg("marsupial").assert().code(1);
    world
        .search()
        .args(["marsupial", "--tools"])
        .assert()
        .success()
        .stdout(predicate::str::contains("marsupial"));
}

#[test]
fn harness_and_limit_narrow_the_results() {
    let world = world();
    world
        .search()
        .args(["quokka", "--harness", "codex"])
        .assert()
        .code(1);
    let out = stdout_of({
        let mut cmd = world.search();
        cmd.args([
            "quokka", "--group", "message", "--limit", "1", "--width", "100",
        ]);
        cmd
    });
    assert_eq!(out.lines().count(), 2, "header plus one row: {out}");
}

#[test]
fn help_documents_the_exit_status() {
    let world = world();
    world
        .search()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Exit status"))
        .stdout(predicate::str::contains("2 on an error"));
}

#[test]
fn searching_never_writes_transcript_text_to_disk() {
    let world = world();
    stdout_of({
        let mut cmd = world.search();
        // Discovery's own cache holds a message preview, so skip it; this
        // test is about what the search itself keeps.
        cmd.args(["quokka", "--format", "json", "--no-cache"]);
        cmd
    });
    let mut leaked = Vec::new();
    let mut stack = vec![world.home];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = fs::read(&path)
                && bytes.windows(6).any(|w| w.eq_ignore_ascii_case(b"quokka"))
            {
                leaked.push(path);
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "transcript text reached disk: {leaked:?}"
    );
}
