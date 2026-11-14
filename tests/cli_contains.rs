//! `--contains` narrows the session table to sessions whose transcripts
//! mention the text, reading the transcripts on demand.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn write_session(state: &Path, key: &str, text: &str) {
    let dir = state.join("projects").join("-home-user-project");
    fs::create_dir_all(&dir).unwrap();
    let lines = [
        format!(
            r#"{{"cwd":"/home/user/project","sessionId":"{key}","type":"user","uuid":"u-{key}","parentUuid":null,"message":{{"role":"user","content":[{{"type":"text","text":"{text}"}}]}}}}"#
        ),
        format!(
            r#"{{"cwd":"/home/user/project","sessionId":"{key}","type":"assistant","uuid":"a-{key}","parentUuid":"u-{key}","message":{{"role":"assistant","content":[{{"type":"text","text":"Noted."}}]}}}}"#
        ),
    ];
    fs::write(dir.join(format!("{key}.jsonl")), lines.join("\n") + "\n").unwrap();
}

fn conspectus(home: &Path, claude: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("binary");
    cmd.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("XDG_RUNTIME_DIR", home.join("no-daemon"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("CONSPECTUS_DISABLE_TMUX", "1")
        .env("CONSPECTUS_DISABLE_FORGE", "1")
        .env("CONSPECTUS_CLAUDE_CODE_STATE", claude)
        .current_dir(home);
    cmd
}

#[test]
fn contains_keeps_only_the_sessions_whose_transcripts_mention_the_text() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(
        &claude,
        "aaaaaaaa-0000-0000-0000-000000000001",
        "We chose the quokka design",
    );
    write_session(
        &claude,
        "bbbbbbbb-0000-0000-0000-000000000002",
        "Unrelated chatter",
    );

    conspectus(&home, &claude)
        .args(["table", "sessions", "--wide", "--no-pager"])
        .assert()
        .success()
        .stdout(predicate::str::contains("00000001"))
        .stdout(predicate::str::contains("00000002"));

    conspectus(&home, &claude)
        .args([
            "table",
            "sessions",
            "--wide",
            "--no-pager",
            "--contains",
            "QUOKKA",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("00000001"))
        .stdout(predicate::str::contains("00000002").not());
}

#[test]
fn contains_with_no_match_lists_no_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(
        &claude,
        "aaaaaaaa-0000-0000-0000-000000000001",
        "We chose the quokka design",
    );

    conspectus(&home, &claude)
        .args([
            "table",
            "sessions",
            "--wide",
            "--no-pager",
            "--contains",
            "zebra",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("00000001").not());
}

#[test]
fn contains_composes_with_the_structured_filters() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(
        &claude,
        "aaaaaaaa-0000-0000-0000-000000000001",
        "We chose the quokka design",
    );

    conspectus(&home, &claude)
        .args([
            "table",
            "sessions",
            "--wide",
            "--no-pager",
            "--contains",
            "quokka",
            "--harness",
            "codex",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("00000001").not());
}

#[test]
fn contains_does_not_write_the_text_anywhere() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(
        &claude,
        "aaaaaaaa-0000-0000-0000-000000000001",
        "We chose the quokka design",
    );

    conspectus(&home, &claude)
        .args([
            "table",
            "sessions",
            "--wide",
            "--no-pager",
            "--contains",
            "quokka",
        ])
        .assert()
        .success();

    let mut leaked = Vec::new();
    let mut stack = vec![home];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = fs::read(&path)
                && bytes.windows(6).any(|w| w == b"quokka")
            {
                leaked.push(path);
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "the query or transcript text was written: {leaked:?}"
    );
}

#[test]
fn contains_is_accepted_by_the_other_tables_and_leaves_them_alone() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(&claude, "aaaaaaaa-0000-0000-0000-000000000001", "quokka");
    for table in ["mux", "prs"] {
        conspectus(&home, &claude)
            .args([
                "table",
                table,
                "--wide",
                "--no-pager",
                "--contains",
                "quokka",
            ])
            .assert()
            .success();
    }
}

/// A fake `tmux` in `<home>/fake-tmux/` that lists one live session,
/// `name`, working in the transcripts' project, and prints nothing for
/// every other call. Returns the `PATH` that puts it first.
fn fake_tmux(home: &Path, name: &str) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join("fake-tmux");
    fs::create_dir_all(&dir).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let project = "/home/user/project";
    let pid = std::process::id();
    let rows = dir.join("sessions");
    fs::write(
        &rows,
        format!(
            "{name}\t{project}\t{now}\t{now}\tclaude\t{pid}\t{project}\tclaude\t0\t\t0\t$1\t%1\t\n"
        ),
    )
    .unwrap();
    let script = dir.join("tmux");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\ncase \"$1\" in\nlist-sessions) cat '{rows}' ;;\nesac\n",
            rows = rows.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    std::env::join_paths(std::iter::once(dir).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap()
}

/// The mux table matches each attached session's transcript by the
/// session's own key, as the sessions table does.
#[test]
fn contains_keeps_a_mux_whose_session_s_transcript_mentions_the_text() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let claude = temp.path().join("claude");
    fs::create_dir_all(&home).unwrap();
    write_session(
        &claude,
        "aaaaaaaa-0000-0000-0000-000000000001",
        "We chose the quokka design",
    );
    let path = fake_tmux(&home, "editor");
    let mux_table = |extra: &[&str]| {
        let mut cmd = conspectus(&home, &claude);
        cmd.env_remove("CONSPECTUS_DISABLE_TMUX")
            .env("PATH", &path)
            .args(["table", "mux", "--wide", "--no-pager"])
            .args(extra);
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let all = mux_table(&[]);
    assert!(all.contains("tmux:editor"), "{all}");
    assert!(all.contains("00000001"), "the session is attached: {all}");

    let matched = mux_table(&["--contains", "QUOKKA"]);
    assert!(matched.contains("tmux:editor"), "{matched}");
    let unmatched = mux_table(&["--contains", "zebra"]);
    assert!(!unmatched.contains("tmux:editor"), "{unmatched}");
}

/// Scenario worlds have no transcripts, so the scenario viewer says so
/// instead of quietly ignoring the flag.
#[cfg(debug_assertions)]
#[test]
fn the_scenario_viewer_refuses_contains() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    conspectus(&home, temp.path())
        .args(["dev", "scenario", "tui", "exact-match", "--contains", "x"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("`--contains` isn't available"));
}
