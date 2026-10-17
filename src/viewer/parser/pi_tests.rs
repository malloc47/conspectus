use super::*;
use std::fs;
use std::io::Write;

/// Lay down `<state_root>/--work--/2026-05-12T13-18-53-000Z_<session_key>.jsonl`
/// with a `session` header followed by `lines`.
fn fixture(session_key: &str, lines: &[&str]) -> (tempfile::TempDir, SessionLocator) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cwd_dir = dir.path().join("--work--");
    fs::create_dir_all(&cwd_dir).expect("cwd dir");
    let file_path = cwd_dir.join(format!("2026-05-12T13-18-53-000Z_{session_key}.jsonl"));
    let mut file = fs::File::create(&file_path).expect("create jsonl");
    writeln!(
        file,
        r#"{{"type":"session","version":3,"id":"{session_key}","cwd":"/work"}}"#
    )
    .expect("write header");
    for line in lines {
        writeln!(file, "{line}").expect("write line");
    }
    let locator = SessionLocator {
        harness_key: "pi".to_string(),
        session_key: session_key.to_string(),
        state_root: dir.path().to_path_buf(),
    };
    (dir, locator)
}

/// `(kind, hidden, body)` for every turn, for compact assertions.
fn shape(document: &TranscriptDocument) -> Vec<(TurnKind, bool, &str)> {
    document
        .turns
        .iter()
        .map(|turn| (turn.kind, turn.hidden, turn.body.as_str()))
        .collect()
}

#[test]
fn missing_file_reports_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let locator = SessionLocator {
        harness_key: "pi".to_string(),
        session_key: "absent".to_string(),
        state_root: dir.path().to_path_buf(),
    };
    assert!(matches!(PiParser.read(&locator), Err(ParseError::NotFound)));
}

#[test]
fn header_cwd_becomes_document_meta() {
    let (_tmp, locator) = fixture("sess", &[]);
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(doc.meta.cwd.as_deref(), Some("/work"));
    assert_eq!(doc.meta.harness, "pi");
}

#[test]
fn user_and_assistant_text_become_message_turns() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"timestamp":"2026-05-12T13:20:00Z","message":{"role":"user","content":"hello there"}}"#,
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":[{"type":"text","text":"sure, on it"}],"stopReason":"stop"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].role, TurnRole::User);
    assert_eq!(doc.turns[0].body, "hello there");
    assert_eq!(
        doc.turns[0].timestamp.map(|time| time.to_rfc3339()),
        Some("2026-05-12T13:20:00+00:00".to_string())
    );
    assert_eq!(doc.turns[1].role, TurnRole::Assistant);
    assert_eq!(doc.turns[1].body, "sure, on it");
}

#[test]
fn thinking_tool_calls_and_tool_results_become_their_kinds() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"assistant","content":[{"type":"thinking","thinking":"read it first"},{"type":"toolCall","id":"t1","name":"read","arguments":{"path":"src/lib.rs"}}],"stopReason":"toolUse"}}"#,
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"toolResult","toolCallId":"t1","content":[{"type":"text","text":"pub fn lib() {}"}]}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(
        shape(&doc),
        [
            (TurnKind::Thinking, false, "read it first"),
            (TurnKind::ToolUse, false, r#"read: {"path":"src/lib.rs"}"#),
            (TurnKind::ToolResult, false, "pub fn lib() {}"),
        ]
    );
}

#[test]
fn compaction_and_branch_summaries_become_compaction_turns() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"branch_summary","id":"a","parentId":null,"fromId":"x","summary":"Approach A was too slow."}"#,
            r#"{"type":"compaction","id":"b","parentId":"a","summary":"Planned the refactor.","firstKeptEntryId":"a","tokensBefore":50000}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(
        shape(&doc),
        [
            (
                TurnKind::CompactionSummary,
                false,
                "**Branch summary**\n\nApproach A was too slow."
            ),
            (TurnKind::CompactionSummary, false, "Planned the refactor."),
        ]
    );
}

#[test]
fn active_branch_excludes_abandoned_tree_branches() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"plan it"}}"#,
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"user","content":"try approach A"}}"#,
            r#"{"type":"message","id":"c","parentId":"b","message":{"role":"assistant","content":"approach A done","stopReason":"stop"}}"#,
            r#"{"type":"message","id":"d","parentId":"a","message":{"role":"user","content":"try approach B"}}"#,
            r#"{"type":"message","id":"e","parentId":"d","message":{"role":"assistant","content":"approach B done","stopReason":"stop"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    let bodies: Vec<&str> = doc.turns.iter().map(|turn| turn.body.as_str()).collect();
    assert_eq!(bodies, ["plan it", "try approach B", "approach B done"]);
}

#[test]
fn legacy_entries_without_ids_render_in_file_order() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","message":{"role":"user","content":"first"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"second"}]}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    let bodies: Vec<&str> = doc.turns.iter().map(|turn| turn.body.as_str()).collect();
    assert_eq!(bodies, ["first", "second"]);
}

#[test]
fn records_pi_does_not_show_are_hidden_system_turns() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"system","content":"","sections":{"preamble":"You are Pi."}}}"#,
            r#"{"type":"model_change","id":"b","parentId":"a","provider":"anthropic","modelId":"claude-sonnet-5"}"#,
            r#"{"type":"thinking_level_change","id":"c","parentId":"b","thinkingLevel":"high"}"#,
            r#"{"type":"custom","id":"d","parentId":"c","customType":"todo","data":{"open":2}}"#,
            r#"{"type":"custom_message","id":"e","parentId":"d","customType":"todo","content":"injected context","display":false}"#,
            r#"{"type":"custom_message","id":"f","parentId":"e","customType":"todo","content":[{"type":"text","text":"two todos remain"}],"display":true}"#,
            r#"{"type":"label","id":"g","parentId":"f","targetId":"f","label":"done"}"#,
            r#"{"type":"usage","id":"h","parentId":"g","kind":"cache_warm","usage":{"input":1}}"#,
            r#"{"type":"context_edit","id":"i","parentId":"h","targetId":"f","replacement":null}"#,
            r#"{"type":"session_info","id":"j","parentId":"i","name":"Refactor"}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(
        shape(&doc),
        [
            (TurnKind::Message, true, "**preamble**\n\nYou are Pi."),
            (TurnKind::Message, true, "model: anthropic/claude-sonnet-5"),
            (TurnKind::Message, true, "thinking level: high"),
            (
                TurnKind::Message,
                true,
                r#"extension state `todo`: {"open":2}"#
            ),
            (TurnKind::Message, true, "injected context"),
            (TurnKind::Message, false, "two todos remain"),
            (TurnKind::Message, true, "label `done` on f"),
            (
                TurnKind::Message,
                true,
                r#"usage (cache_warm): {"input":1}"#
            ),
            (TurnKind::Message, true, "context edit on f"),
            (TurnKind::Message, true, "session name: Refactor"),
        ]
    );
    assert!(doc.turns.iter().all(|turn| turn.role == TurnRole::System));
}

#[test]
fn message_aborted_tags_assistant_and_preceding_user() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"first prompt"}}"#,
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"first answer","stopReason":"stop"}}"#,
            r#"{"type":"message","id":"c","parentId":"b","message":{"role":"user","content":"interrupted prompt"}}"#,
            r#"{"type":"message","id":"d","parentId":"c","message":{"role":"assistant","content":[{"type":"text","text":"partial"}],"stopReason":"aborted"}}"#,
            r#"{"type":"message","id":"e","parentId":"d","message":{"role":"user","content":"replacement prompt"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    let aborted: Vec<(&str, bool)> = doc
        .turns
        .iter()
        .map(|turn| (turn.body.as_str(), turn.aborted))
        .collect();
    assert_eq!(
        aborted,
        [
            ("first prompt", false),
            ("first answer", false),
            ("interrupted prompt", true),
            ("partial", true),
            ("replacement prompt", false),
        ],
        "the aborted assistant message and the user prompt it answered are tagged",
    );
}

#[test]
fn malformed_lines_are_skipped() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"before"}}"#,
            "not json at all",
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"after"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    let bodies: Vec<&str> = doc.turns.iter().map(|turn| turn.body.as_str()).collect();
    assert_eq!(bodies, ["before", "after"]);
}

#[test]
fn dangling_parent_ends_the_walk() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"unreachable"}}"#,
            r#"{"type":"message","id":"b","parentId":"missing","message":{"role":"user","content":"leaf"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(shape(&doc), [(TurnKind::Message, false, "leaf")]);
}

#[test]
fn parent_cycle_ends_the_walk() {
    let (_tmp, locator) = fixture(
        "sess",
        &[
            r#"{"type":"message","id":"a","parentId":"b","message":{"role":"user","content":"one"}}"#,
            r#"{"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"two"}}"#,
        ],
    );
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(
        shape(&doc),
        [
            (TurnKind::Message, false, "one"),
            (TurnKind::Message, false, "two"),
        ]
    );
}

#[test]
fn session_file_is_found_by_exact_id() {
    // `b` must not match `<ts>_a_b.jsonl`, whose id is `a_b`.
    let dir = tempfile::tempdir().expect("tempdir");
    let write = |name: &str, id: &str, text: &str| {
        fs::write(
            dir.path().join(name),
            format!(
                "{{\"type\":\"session\",\"version\":3,\"id\":\"{id}\",\"cwd\":\"/w\"}}\n\
                 {{\"type\":\"message\",\"id\":\"m\",\"parentId\":null,\"message\":{{\"role\":\"user\",\"content\":\"{text}\"}}}}\n"
            ),
        )
        .expect("write");
    };
    write("2026-05-12T13-18-53-000Z_a_b.jsonl", "a_b", "from a_b");
    write("2026-05-12T13-18-54-000Z_b.jsonl", "b", "from b");
    let locator = SessionLocator {
        harness_key: "pi".to_string(),
        session_key: "b".to_string(),
        state_root: dir.path().to_path_buf(),
    };
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(shape(&doc), [(TurnKind::Message, false, "from b")]);
}

#[test]
fn session_file_is_found_by_header_id_when_its_name_disagrees() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("2026-05-12T13-18-53-000Z_renamed.jsonl"),
        concat!(
            r#"{"type":"session","version":3,"id":"real-id","cwd":"/work/x"}"#,
            "\n",
            r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"hi"}}"#,
            "\n",
        ),
    )
    .expect("write");
    let locator = SessionLocator {
        harness_key: "pi".to_string(),
        session_key: "real-id".to_string(),
        state_root: dir.path().to_path_buf(),
    };
    let doc = PiParser.read(&locator).expect("parse");
    assert_eq!(shape(&doc), [(TurnKind::Message, false, "hi")]);
}
