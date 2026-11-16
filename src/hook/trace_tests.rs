use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::{Value, json};

use super::*;

fn trace() -> Trace {
    Trace::named("run-1").expect("a valid name")
}

fn entry(input: &[u8]) -> Entry<'_> {
    Entry {
        t_ms: 1_794_400_904_859,
        harness: "claude-code",
        input,
        env: EnvFlags {
            tmux_pane: true,
            tmux: true,
            claude_pid: false,
        },
        events: vec!["tool_finished".to_string()],
        applied: "written",
        after: Some(After {
            state: "working",
            detail: Some("generating"),
            subagents: 1,
            via_subagent: false,
            since_ms: 1_794_400_900_000,
            rev: 41,
        }),
    }
}

fn parsed(line: &str) -> Value {
    assert!(line.ends_with('\n'), "{line:?}");
    assert_eq!(line.matches('\n').count(), 1, "{line:?}");
    serde_json::from_str(line.trim_end()).expect("one JSON value")
}

#[test]
fn a_name_is_a_file_name_of_the_documented_shape() {
    for name in [
        "a",
        "A9",
        "run-1",
        "claude.2026-11-16_a",
        "0",
        &"n".repeat(64),
    ] {
        assert!(valid_name(name), "{name}");
    }
    for name in [
        "",
        ".hidden",
        "-flag",
        "_x",
        "..",
        "a/b",
        "a b",
        "a\nb",
        "caf\u{e9}",
        "a:b",
        &"n".repeat(65),
    ] {
        assert!(!valid_name(name), "{name:?}");
    }
    assert!(Trace::named("run-1").is_some());
    assert!(Trace::named("../x").is_none());
}

#[test]
fn the_trace_file_is_under_hook_trace_in_the_state_directory() {
    assert_eq!(
        trace().path(Path::new("/state/conspectus")),
        Path::new("/state/conspectus/hook-trace/run-1.jsonl")
    );
}

#[test]
fn a_line_says_what_the_writer_saw_and_did() {
    let payload = json!({
        "session_id": "3f2a9c1e",
        "cwd": "/home/user/project",
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "ls" },
        "tool_use_id": "toolu_01",
    });
    let input = payload.to_string();
    let line = parsed(&trace().render(&entry(input.as_bytes())).unwrap());
    assert_eq!(line["v"], 1);
    assert_eq!(line["t_ms"], 1_794_400_904_859i64);
    assert_eq!(line["harness"], "claude-code");
    assert_eq!(
        line["payload"],
        json!({
            "session_id": pseudonym_of("3f2a9c1e"),
            "cwd": "<path>",
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": "<object:1 keys>",
            "tool_use_id": pseudonym_of("toolu_01"),
        })
    );
    assert_eq!(
        line["env"],
        json!({ "TMUX_PANE": true, "TMUX": true, "CLAUDE_PID": false })
    );
    assert_eq!(line["events"], json!(["tool_finished"]));
    assert_eq!(line["applied"], "written");
    assert_eq!(
        line["after"],
        json!({
            "state": "working",
            "detail": "generating",
            "subagents": 1,
            "via_subagent": false,
            "since_ms": 1_794_400_900_000i64,
            "rev": 41,
        })
    );
}

fn pseudonym_of(value: &str) -> String {
    super::super::redact::pseudonym("run-1", value)
}

#[test]
fn the_fields_come_in_the_documented_order() {
    let text = trace().render(&entry(b"{}")).unwrap();
    let positions: Vec<usize> = [
        "\"v\":",
        "\"t_ms\":",
        "\"harness\":",
        "\"payload\":",
        "\"env\":",
        "\"events\":",
        "\"applied\":",
        "\"after\":",
    ]
    .iter()
    .map(|name| {
        text.find(name)
            .unwrap_or_else(|| panic!("{name} in {text}"))
    })
    .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
}

#[test]
fn a_state_with_no_detail_and_no_stored_entry_leave_those_fields_out() {
    let mut bare = entry(b"{}");
    bare.after = Some(After {
        state: "starting",
        detail: None,
        subagents: 0,
        via_subagent: false,
        since_ms: 5,
        rev: 1,
    });
    let line = parsed(&trace().render(&bare).unwrap());
    assert!(line["after"].get("detail").is_none(), "{line}");
    assert_eq!(line["after"]["state"], "starting");

    bare.after = None;
    bare.events = Vec::new();
    bare.applied = "none";
    let line = parsed(&trace().render(&bare).unwrap());
    assert!(line.get("after").is_none(), "{line}");
    assert_eq!(line["events"], json!([]));
    assert_eq!(line["applied"], "none");
}

#[test]
fn input_that_is_not_one_json_value_is_only_a_size() {
    for input in [
        &b"not json at all, a secret"[..],
        b"{\"a\":1} trailing",
        b"",
        b"{\"a\":",
    ] {
        let line = parsed(&trace().render(&entry(input)).unwrap());
        assert_eq!(
            line["payload"],
            format!("<unparsed:{} bytes>", input.len()),
            "{}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn a_harness_that_is_not_a_name_is_not_echoed() {
    let mut odd = entry(b"{}");
    odd.harness = "claude code; rm -rf /";
    let line = parsed(&trace().render(&odd).unwrap());
    assert_eq!(line["harness"], "<invalid>");
    odd.harness = "pi";
    assert_eq!(parsed(&trace().render(&odd).unwrap())["harness"], "pi");
}

#[test]
fn a_line_over_the_cap_keeps_its_fields_and_gives_up_its_payload() {
    // 64 keys, each a list of 8 objects of 64 keys: far past the cap even
    // after every string is a placeholder.
    let object: serde_json::Map<String, Value> = (0..64)
        .map(|k| (format!("key_{k}"), json!("text of some length")))
        .collect();
    let list = Value::Array(vec![Value::Object(object); 8]);
    let top: serde_json::Map<String, Value> = (0..64)
        .map(|n| (format!("list_{n}"), list.clone()))
        .collect();
    let input = Value::Object(top).to_string();
    let line = trace().render(&entry(input.as_bytes())).unwrap();
    assert!(line.len() < MAX_LINE_BYTES, "{}", line.len());
    let line = parsed(&line);
    assert_eq!(line["payload"], json!("<object:64 keys>"));
    assert_eq!(line["applied"], "written");
    assert_eq!(line["after"]["state"], "working");
}

#[test]
fn a_line_under_the_cap_keeps_its_structure() {
    let input = json!({ "items": [{ "a": "x" }] }).to_string();
    let line = parsed(&trace().render(&entry(input.as_bytes())).unwrap());
    assert_eq!(line["payload"], json!({ "items": [{ "a": "<str:1>" }] }));
}

#[test]
fn recording_creates_a_private_directory_and_file_and_appends() {
    let state = tempfile::tempdir().unwrap();
    let state = state.path().join("state/conspectus");
    let trace = trace();
    trace
        .record(&state, &entry(br#"{"session_id":"s1"}"#))
        .unwrap();
    trace
        .record(&state, &entry(br#"{"session_id":"s2"}"#))
        .unwrap();

    let dir = state.join("hook-trace");
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&state), 0o700);
    let file = dir.join("run-1.jsonl");
    assert_eq!(mode(&file), 0o600);
    let text = fs::read_to_string(&file).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[0]["payload"]["session_id"], pseudonym_of("s1"));
    assert_eq!(lines[1]["payload"]["session_id"], pseudonym_of("s2"));
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        1,
        "the trace is one file"
    );
}

#[test]
fn recording_leaves_the_mode_of_an_existing_file_and_directory_alone() {
    let state = tempfile::tempdir().unwrap();
    let dir = state.path().join("hook-trace");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("run-1.jsonl");
    fs::write(&file, "").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    trace().record(state.path(), &entry(b"{}")).unwrap();
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn recording_where_the_directory_cannot_be_made_is_an_error_the_caller_ignores() {
    let state = tempfile::tempdir().unwrap();
    // A file where the directory belongs.
    fs::write(state.path().join("hook-trace"), "in the way").unwrap();
    assert!(trace().record(state.path(), &entry(b"{}")).is_err());
    // A directory where the file belongs.
    let other = tempfile::tempdir().unwrap();
    fs::create_dir_all(other.path().join("hook-trace/run-1.jsonl")).unwrap();
    assert!(trace().record(other.path(), &entry(b"{}")).is_err());
    // A state directory that is itself a file.
    let file = state.path().join("plain");
    fs::write(&file, "").unwrap();
    assert!(trace().record(&file, &entry(b"{}")).is_err());
}
