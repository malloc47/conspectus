// Extracted from claude_code.rs H-HYG-011 rolling wave via #[path = "claude_code_tests.rs"] mod tests;
use super::*;
use std::fs;
use std::io::Write;

/// Lay down `<state_root>/projects/<proj>/<session_key>.jsonl`
/// pre-loaded with `lines`. Returns the tempdir + locator.
fn fixture(session_key: &str, lines: &[&str]) -> (tempfile::TempDir, SessionLocator) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("projects").join("-home-user-proj");
    fs::create_dir_all(&project).expect("project dir");
    let mut file =
        fs::File::create(project.join(format!("{session_key}.jsonl"))).expect("create jsonl");
    for line in lines {
        writeln!(file, "{line}").expect("write line");
    }
    let locator = SessionLocator {
        harness_key: "claude-code".to_string(),
        session_key: session_key.to_string(),
        state_root: dir.path().to_path_buf(),
    };
    (dir, locator)
}

// H-EXT-006: pre-existing `supports_only_claude_code_locator`
// test removed; parser dispatch happens through the adapter
// registry so per-parser filters are dead weight.

#[test]
fn missing_file_reports_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let locator = SessionLocator {
        harness_key: "claude-code".to_string(),
        session_key: "absent".to_string(),
        state_root: dir.path().to_path_buf(),
    };
    match ClaudeCodeParser.read(&locator) {
        Err(ParseError::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn plain_user_assistant_exchange_round_trips() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"user","timestamp":"2026-06-01T16:52:36Z","cwd":"/proj","message":{"role":"user","content":"hello"}}"#,
            r#"{"type":"assistant","timestamp":"2026-06-01T16:53:00Z","message":{"role":"assistant","content":[{"type":"text","text":"hi back"}]}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.meta.harness, "claude-code");
    assert_eq!(doc.meta.session_key, "sess");
    assert_eq!(doc.meta.cwd.as_deref(), Some("/proj"));
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].role, TurnRole::User);
    assert_eq!(doc.turns[0].kind, TurnKind::Message);
    assert_eq!(doc.turns[0].body, "hello");
    assert!(doc.turns[0].timestamp.is_some());
    assert_eq!(doc.turns[1].role, TurnRole::Assistant);
    assert_eq!(doc.turns[1].body, "hi back");
}

#[test]
fn assistant_text_and_tool_use_emit_separate_turns() {
    // One assistant record carrying both text and a tool_use
    // block — the normalizer fans them into distinct turns so
    // the renderer can fold tool blocks independently.
    let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:00:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Let me check the file."},{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/x"}}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].kind, TurnKind::Message);
    assert_eq!(doc.turns[0].body, "Let me check the file.");
    assert_eq!(doc.turns[1].kind, TurnKind::ToolUse);
    assert!(
        doc.turns[1].body.starts_with("Read: "),
        "got {:?}",
        doc.turns[1].body
    );
    assert!(doc.turns[1].body.contains("\"file_path\":\"/x\""));
}

#[test]
fn tool_result_with_string_content_is_captured() {
    let line = r#"{"type":"user","timestamp":"2026-06-01T17:01:00Z","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"ok"}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].role, TurnRole::User);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolResult);
    assert_eq!(doc.turns[0].body, "ok");
}

#[test]
fn tool_result_with_text_block_list_content_is_flattened() {
    let line = r#"{"type":"user","timestamp":"2026-06-01T17:02:00Z","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":[{"type":"text","text":"line one"},{"type":"text","text":"line two"}]}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "line one\nline two");
}

#[test]
fn thinking_block_emits_thinking_turn() {
    let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:03:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm, options...","signature":"ignored"}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
    assert_eq!(doc.turns[0].body, "hmm, options...");
}

/// Real-data shape: assistant turns where the visible
/// `thinking` field is empty and the reasoning lives only in
/// the opaque `encrypted_content` blob. Renderer needs *some*
/// turn to render so the operator's `y` toggle has visible
/// effect.
#[test]
fn thinking_block_with_only_encrypted_content_emits_placeholder_turn() {
    let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:04:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"","signature":"sig","encrypted_content":"gAAA..."}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
    assert!(
        doc.turns[0].body.contains("hidden"),
        "placeholder text should hint at encrypted reasoning, got {:?}",
        doc.turns[0].body
    );
}

#[test]
fn compaction_summary_record_becomes_compaction_summary_turn() {
    let line = r#"{"type":"user","isCompactSummary":true,"timestamp":"2026-06-01T18:00:00Z","message":{"role":"user","content":"Summary of prior turns."}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::CompactionSummary);
    assert_eq!(doc.turns[0].body, "Summary of prior turns.");
    assert_eq!(doc.turns[0].role, TurnRole::User);
}

#[test]
fn empty_text_blocks_and_empty_bodies_are_dropped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"user","message":{"role":"user","content":""}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":""}]}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert!(
        doc.turns.is_empty(),
        "empty bodies should not produce turns, got {:?}",
        doc.turns
    );
}

#[test]
fn non_user_assistant_record_types_are_skipped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"custom-title","customTitle":"debug","sessionId":"sess"}"#,
            r#"{"type":"agent-name","agentName":"main"}"#,
            r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-06-01T18:00:00Z"}"#,
            r#"{"type":"user","message":{"role":"user","content":"the only real turn"}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "the only real turn");
}

#[test]
fn malformed_lines_and_blank_lines_are_skipped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"user","message":{"role":"user","content":"good"}}"#,
            r#""#,
            r#"not json"#,
            r#"{"type":"user","message":{"role":"user","content":"also good"}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].body, "good");
    assert_eq!(doc.turns[1].body, "also good");
}

#[test]
fn cwd_captured_from_first_record_that_has_one() {
    // Common pattern: the first few records (custom-title etc.)
    // omit cwd; the first user record carries it.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"custom-title","customTitle":"x"}"#,
            r#"{"type":"user","cwd":"/p/proj","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"user","cwd":"/p/different","message":{"role":"user","content":"later"}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(
        doc.meta.cwd.as_deref(),
        Some("/p/proj"),
        "first-wins, ignoring later changes"
    );
}

#[test]
fn orphan_plain_text_user_record_is_marked_aborted() {
    // Real Claude Code shape: a plain-text user record with no
    // descendants in the parent/child uuid graph corresponds to
    // a message the operator typed and then Esc'd before the
    // agent began responding. The replacement message follows
    // with its own parent chain.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"first prompt"}}"#,
            r#"{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[{"type":"text","text":"first reply"}]}}"#,
            r#"{"type":"user","uuid":"u2","parentUuid":"a1","message":{"role":"user","content":"interrupted thought"}}"#,
            r#"{"type":"user","uuid":"u3","parentUuid":"a1","message":{"role":"user","content":"replacement prompt"}}"#,
            r#"{"type":"assistant","uuid":"a3","parentUuid":"u3","message":{"role":"assistant","content":[{"type":"text","text":"reply to replacement"}]}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
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
            ("replacement prompt", false),
            ("reply to replacement", false),
        ],
        "orphan u2 tagged aborted; the other plain-text user records have children so they stay",
    );
}

#[test]
fn last_record_is_never_tagged_aborted() {
    // The chronological tail of an active session has no
    // children but represents the *in-flight* turn the agent
    // is still answering, not an interrupt. Don't tag it.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"first prompt"}}"#,
            r#"{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[{"type":"text","text":"first reply"}]}}"#,
            r#"{"type":"user","uuid":"u2","parentUuid":"a1","message":{"role":"user","content":"tail prompt — still in flight"}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(
        doc.turns.last().unwrap().body,
        "tail prompt — still in flight"
    );
    assert!(
        !doc.turns.last().unwrap().aborted,
        "chronological tail is in-flight, not aborted",
    );
}

#[test]
fn tool_result_user_records_are_not_tagged_aborted() {
    // user-role records that carry tool_result blocks have no
    // children by design (Claude doesn't re-reply to them as
    // user turns). They must NOT be tagged aborted just because
    // they're orphans in the uuid graph.
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"assistant","uuid":"a1","parentUuid":null,"message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/x"}}]}}"#,
            r#"{"type":"user","uuid":"u_tr","parentUuid":"a1","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"assistant","uuid":"a2","parentUuid":"u_tr","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#,
        ],
    );
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    let tool_result = doc
        .turns
        .iter()
        .find(|t| t.kind == TurnKind::ToolResult)
        .expect("tool result emitted");
    assert!(
        !tool_result.aborted,
        "tool_result-only user records are not subject to the orphan heuristic"
    );
}

#[test]
fn tool_use_without_input_emits_name_only_body() {
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"ListDir"}]}}"#;
    let (_tmp, locator) = fixture("sess", &[line]);
    let doc = ClaudeCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
    assert_eq!(doc.turns[0].body, "ListDir");
}
