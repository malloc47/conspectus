// Extracted from codex.rs H-HYG-011 rolling wave via #[path = "codex_tests.rs"] mod tests;
use super::*;
use std::fs;
use std::io::Write;

/// Lay down `<state_root>/sessions/2026/05/12/rollout-<ts>-<session_key>.jsonl`
/// pre-loaded with `lines`.
fn fixture(session_key: &str, lines: &[&str]) -> (tempfile::TempDir, SessionLocator) {
    let dir = tempfile::tempdir().expect("tempdir");
    let day = dir
        .path()
        .join("sessions")
        .join("2026")
        .join("05")
        .join("12");
    fs::create_dir_all(&day).expect("day dir");
    let file_path = day.join(format!("rollout-2026-05-12T13-18-53-{session_key}.jsonl"));
    let mut file = fs::File::create(&file_path).expect("create jsonl");
    for line in lines {
        writeln!(file, "{line}").expect("write line");
    }
    let locator = SessionLocator {
        harness_key: "codex".to_string(),
        session_key: session_key.to_string(),
        state_root: dir.path().to_path_buf(),
    };
    (dir, locator)
}

// H-EXT-006: pre-existing `supports_only_codex_locator`
// test removed; parser dispatch happens through the adapter
// registry so per-parser filters are dead weight.

#[test]
fn missing_file_reports_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let locator = SessionLocator {
        harness_key: "codex".to_string(),
        session_key: "absent".to_string(),
        state_root: dir.path().to_path_buf(),
    };
    match CodexParser.read(&locator) {
        Err(ParseError::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn session_meta_carries_cwd() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"session_meta","timestamp":"2026-05-12T13:18:53Z","payload":{"id":"sess","cwd":"/p/proj"}}"#,
            r#"{"type":"response_item","timestamp":"2026-05-12T13:19:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.meta.harness, "codex");
    assert_eq!(doc.meta.session_key, "sess");
    assert_eq!(doc.meta.cwd.as_deref(), Some("/p/proj"));
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "hello");
}

#[test]
fn user_and_assistant_messages_become_message_turns() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","timestamp":"2026-05-12T13:19:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"do the thing"}]}}"#,
            r#"{"type":"response_item","timestamp":"2026-05-12T13:19:30Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"sure, on it"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].role, TurnRole::User);
    assert_eq!(doc.turns[0].kind, TurnKind::Message);
    assert_eq!(doc.turns[0].body, "do the thing");
    assert!(doc.turns[0].timestamp.is_some());
    assert_eq!(doc.turns[1].role, TurnRole::Assistant);
    assert_eq!(doc.turns[1].body, "sure, on it");
}

#[test]
fn developer_and_system_messages_are_skipped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<permissions instructions> ..."}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"system","content":[{"type":"input_text","text":"system prompt"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"actual user text"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "actual user text");
}

#[test]
fn function_call_becomes_tool_use_with_name_and_arguments() {
    let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:20:00Z","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}","call_id":"call_1"}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].role, TurnRole::Assistant);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
    assert_eq!(doc.turns[0].body, "exec_command: {\"cmd\":\"ls\"}");
}

#[test]
fn function_call_output_becomes_tool_result() {
    let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:20:01Z","payload":{"type":"function_call_output","call_id":"call_1","output":"hello\nworld"}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolResult);
    assert_eq!(doc.turns[0].body, "hello\nworld");
}

#[test]
fn reasoning_with_summary_text_emits_thinking_turn() {
    let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:21:00Z","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"considering options A and B"}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].role, TurnRole::Assistant);
    assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
    assert_eq!(doc.turns[0].body, "considering options A and B");
}

#[test]
fn reasoning_with_only_encrypted_content_emits_placeholder_turn() {
    // Real-data shape mirroring Claude: codex reasoning records
    // carry the visible thinking in `summary`/`content`, but
    // some carry only `encrypted_content`. Operator's thinking
    // toggle must have visible effect so we emit a placeholder
    // rather than dropping the record. Matches the Claude
    // parser's behaviour.
    let line = r#"{"type":"response_item","payload":{"type":"reasoning","summary":[],"content":null,"encrypted_content":"gAAA..."}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
    assert!(
        doc.turns[0].body.contains("hidden"),
        "placeholder text, got {:?}",
        doc.turns[0].body
    );
}

#[test]
fn event_msg_records_are_skipped_entirely() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"event_msg","payload":{"type":"token_count","tokens":1234}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","text":"echo"}}"#,
            r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"real"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "real");
}

#[test]
fn turn_aborted_marker_body_is_dropped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<turn_aborted>"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "hello");
}

#[test]
fn proposed_plan_marker_body_is_dropped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<proposed_plan>"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert!(doc.turns.is_empty());
}

#[test]
fn compacted_record_emits_compaction_summary_turn() {
    let line = r#"{"type":"compacted","timestamp":"2026-05-13T21:54:07Z","payload":{"message":"Summary of prior work.","replacement_history":[]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::CompactionSummary);
    assert_eq!(doc.turns[0].body, "Summary of prior work.");
}

#[test]
fn web_search_call_emits_tool_use_turn() {
    let line = r#"{"type":"response_item","payload":{"type":"web_search_call","name":"web_search","arguments":"{\"query\":\"rust transcript viewer\"}"}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
    assert!(doc.turns[0].body.starts_with("web_search: "));
}

#[test]
fn malformed_and_blank_lines_skip() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"a"}]}}"#,
            r"",
            r"not json",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"b"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].body, "a");
    assert_eq!(doc.turns[1].body, "b");
}

#[test]
fn turn_aborted_event_marks_preceding_user_message_and_partial_response() {
    // Real Codex shape: user message, then a partial reasoning
    // turn, then `event_msg.turn_aborted` because the operator
    // hit Esc. Detection should mark the user message AND the
    // partial reasoning turn so the whole exchange disappears
    // together when `show_aborted` is off.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first prompt"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first reply"}]}}"#,
            r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"interrupted thought"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"partial reasoning before abort"}]}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_aborted"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"replacement prompt"}]}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    let bodies: Vec<(&str, bool)> = doc
        .turns
        .iter()
        .map(|t| (t.body.as_str(), t.aborted))
        .collect();
    assert_eq!(
        bodies,
        vec![
            ("first prompt", false),
            ("first reply", false),
            ("interrupted thought", true),
            ("partial reasoning before abort", true),
            ("replacement prompt", false),
        ],
        "abort flag tags the aborted user prompt and the partial reasoning that landed before Esc",
    );
}

#[test]
fn task_complete_event_clears_abort_candidate() {
    // If `task_complete` fires between a user message and a
    // later `turn_aborted`, the user message must not be
    // retroactively tagged. The abort only applies to the most
    // recent unresolved turn.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"settled prompt"}]}}"#,
            r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_aborted"}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert!(
        !doc.turns[0].aborted,
        "task_complete cleared the candidate before turn_aborted fired",
    );
}

#[test]
fn rollout_file_located_under_nested_year_month_day_path() {
    // session_key suffix matching is the real lookup mechanism.
    let (_tmp, locator) = fixture(
        "019df146-41e8-7fb0-8df0-dc326b4fdee8",
        &[
            r#"{"type":"session_meta","payload":{"id":"019df146-41e8-7fb0-8df0-dc326b4fdee8","cwd":"/p"}}"#,
        ],
    );
    let doc = CodexParser.read(&locator).expect("parse");
    assert_eq!(doc.meta.cwd.as_deref(), Some("/p"));
}
