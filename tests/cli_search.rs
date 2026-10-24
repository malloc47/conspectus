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
