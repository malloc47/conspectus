use super::*;
use rusqlite::{Connection, params};
use std::path::PathBuf;

/// Build a temp `opencode.db` with the OpenCode schema and seed
/// it with one session + messages + parts. Returns the tempdir
/// + locator pointing at the db.
fn fixture(session_id: &str, seed: impl Fn(&Connection)) -> (tempfile::TempDir, SessionLocator) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("opencode.db");
    let conn = Connection::open(&db_path).expect("open db");
    conn.execute_batch(
        r"
            CREATE TABLE session (
                id text PRIMARY KEY,
                directory text NOT NULL,
                title text NOT NULL DEFAULT '',
                time_created integer NOT NULL DEFAULT 0
            );
            CREATE TABLE message (
                id text PRIMARY KEY,
                session_id text NOT NULL,
                time_created integer NOT NULL,
                data text NOT NULL
            );
            CREATE TABLE part (
                id text PRIMARY KEY,
                message_id text NOT NULL,
                session_id text NOT NULL,
                time_created integer NOT NULL,
                data text NOT NULL
            );
            ",
    )
    .expect("create schema");
    seed(&conn);
    drop(conn);
    let locator = SessionLocator {
        harness_key: "opencode".to_string(),
        session_key: session_id.to_string(),
        state_root: db_path,
    };
    (dir, locator)
}

fn insert_session(conn: &Connection, id: &str, directory: &str, time_ms: i64) {
    conn.execute(
        "INSERT INTO session (id, directory, time_created) VALUES (?1, ?2, ?3)",
        params![id, directory, time_ms],
    )
    .expect("insert session");
}

fn insert_message(conn: &Connection, id: &str, session: &str, time_ms: i64, data: &str) {
    conn.execute(
        "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
        params![id, session, time_ms, data],
    )
    .expect("insert message");
}

fn insert_part(
    conn: &Connection,
    id: &str,
    message_id: &str,
    session: &str,
    time_ms: i64,
    data: &str,
) {
    conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, message_id, session, time_ms, data],
        )
        .expect("insert part");
}

// Pre-existing `supports_only_opencode_locator`
// test removed. `HarnessParser` no longer carries a
// `supports(&locator) -> bool` method — parser dispatch
// goes through the adapter registry via `transcript_parser`,
// so a per-parser filter is dead weight.

#[test]
fn missing_db_reports_not_found() {
    let locator = SessionLocator {
        harness_key: "opencode".to_string(),
        session_key: "ses_x".to_string(),
        state_root: PathBuf::from("/nonexistent/opencode.db"),
    };
    match OpenCodeParser.read(&locator) {
        Err(ParseError::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn session_with_no_messages_returns_empty_document_with_cwd() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p/proj", 1000);
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.meta.harness, "opencode");
    assert_eq!(doc.meta.session_key, "ses_a");
    assert_eq!(doc.meta.cwd.as_deref(), Some("/p/proj"));
    assert!(doc.turns.is_empty());
}

#[test]
fn text_parts_become_message_turns_with_role_from_parent_message() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(
            conn,
            "msg_1",
            "ses_a",
            1100,
            r#"{"role":"user","time":{"created":1100}}"#,
        );
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"text","text":"hello"}"#,
        );
        insert_message(conn, "msg_2", "ses_a", 1200, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_2",
            "msg_2",
            "ses_a",
            1201,
            r#"{"type":"text","text":"hi back"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].role, TurnRole::User);
    assert_eq!(doc.turns[0].kind, TurnKind::Message);
    assert_eq!(doc.turns[0].body, "hello");
    assert!(doc.turns[0].timestamp.is_some());
    assert_eq!(doc.turns[1].role, TurnRole::Assistant);
    assert_eq!(doc.turns[1].body, "hi back");
}

#[test]
fn reasoning_parts_become_thinking_turns() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"reasoning","text":"thinking out loud"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].role, TurnRole::Assistant);
    assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
    assert_eq!(doc.turns[0].body, "thinking out loud");
}

#[test]
fn tool_part_emits_tool_use_then_tool_result() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"tool","tool":"bash","callID":"c1","state":{"status":"completed","input":{"command":"ls"},"output":"a\nb"}}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 2);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
    assert_eq!(doc.turns[0].body, "bash: {\"command\":\"ls\"}");
    assert_eq!(doc.turns[1].kind, TurnKind::ToolResult);
    assert_eq!(doc.turns[1].body, "a\nb");
}

#[test]
fn tool_part_without_output_emits_tool_use_only() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"tool","tool":"bash","state":{"input":{"command":"ls"}}}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
}

#[test]
fn step_markers_and_patches_are_skipped() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"step-start","snapshot":"abc"}"#,
        );
        insert_part(
            conn,
            "prt_2",
            "msg_1",
            "ses_a",
            1102,
            r#"{"type":"text","text":"actual content"}"#,
        );
        insert_part(
            conn,
            "prt_3",
            "msg_1",
            "ses_a",
            1103,
            r#"{"type":"step-finish"}"#,
        );
        insert_part(
            conn,
            "prt_4",
            "msg_1",
            "ses_a",
            1104,
            r#"{"type":"patch","files":[]}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "actual content");
}

#[test]
fn parts_within_message_sort_by_time_created() {
    // Insert parts out of chronological order — the query
    // must restore chronological order.
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"assistant"}"#);
        insert_part(
            conn,
            "prt_a",
            "msg_1",
            "ses_a",
            1103,
            r#"{"type":"text","text":"third"}"#,
        );
        insert_part(
            conn,
            "prt_b",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"text","text":"first"}"#,
        );
        insert_part(
            conn,
            "prt_c",
            "msg_1",
            "ses_a",
            1102,
            r#"{"type":"text","text":"second"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 3);
    assert_eq!(doc.turns[0].body, "first");
    assert_eq!(doc.turns[1].body, "second");
    assert_eq!(doc.turns[2].body, "third");
}

#[test]
fn other_session_messages_are_excluded() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_session(conn, "ses_b", "/q", 1000);
        insert_message(conn, "msg_a", "ses_a", 1100, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_a",
            "msg_a",
            "ses_a",
            1101,
            r#"{"type":"text","text":"mine"}"#,
        );
        insert_message(conn, "msg_b", "ses_b", 1100, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_b",
            "msg_b",
            "ses_b",
            1101,
            r#"{"type":"text","text":"someone else"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "mine");
}

#[test]
fn empty_text_bodies_are_dropped() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1100, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1101,
            r#"{"type":"text","text":""}"#,
        );
        insert_part(
            conn,
            "prt_2",
            "msg_1",
            "ses_a",
            1102,
            r#"{"type":"text","text":"  \n  "}"#,
        );
        insert_part(
            conn,
            "prt_3",
            "msg_1",
            "ses_a",
            1103,
            r#"{"type":"text","text":"keeper"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    assert_eq!(doc.turns[0].body, "keeper");
}

#[test]
fn message_aborted_error_tags_assistant_and_preceding_user() {
    // Real OpenCode shape: assistant message carries
    // `error.name = "MessageAbortedError"` when the operator
    // hit Esc mid-generation. The preceding user prompt pairs
    // with the aborted assistant so the whole exchange hides
    // together, matching OpenCode's own UI.
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_u1", "ses_a", 1100, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_u1",
            "msg_u1",
            "ses_a",
            1101,
            r#"{"type":"text","text":"first prompt"}"#,
        );
        insert_message(
            conn,
            "msg_a1",
            "ses_a",
            1200,
            r#"{"role":"assistant","finish":"stop"}"#,
        );
        insert_part(
            conn,
            "prt_a1",
            "msg_a1",
            "ses_a",
            1201,
            r#"{"type":"text","text":"first reply"}"#,
        );
        insert_message(conn, "msg_u2", "ses_a", 1300, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_u2",
            "msg_u2",
            "ses_a",
            1301,
            r#"{"type":"text","text":"interrupted thought"}"#,
        );
        insert_message(
            conn,
            "msg_a2",
            "ses_a",
            1400,
            r#"{"role":"assistant","error":{"name":"MessageAbortedError","data":{"message":"Aborted"}}}"#,
        );
        insert_part(
            conn,
            "prt_a2",
            "msg_a2",
            "ses_a",
            1401,
            r#"{"type":"text","text":"partial reply"}"#,
        );
        insert_message(conn, "msg_u3", "ses_a", 1500, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_u3",
            "msg_u3",
            "ses_a",
            1501,
            r#"{"type":"text","text":"replacement prompt"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
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
            ("partial reply", true),
            ("replacement prompt", false),
        ],
        "MessageAbortedError tags the assistant + preceding user prompt",
    );
}

#[test]
fn epoch_ms_timestamps_round_trip_to_datetime_utc() {
    let (_tmp, locator) = fixture("ses_a", |conn| {
        insert_session(conn, "ses_a", "/p", 1000);
        insert_message(conn, "msg_1", "ses_a", 1780277409324, r#"{"role":"user"}"#);
        insert_part(
            conn,
            "prt_1",
            "msg_1",
            "ses_a",
            1780277409324,
            r#"{"type":"text","text":"hi"}"#,
        );
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");
    assert_eq!(doc.turns.len(), 1);
    let ts = doc.turns[0].timestamp.expect("timestamp present");
    // Sanity: same epoch ms reconstruct.
    assert_eq!(ts.timestamp_millis(), 1780277409324);
}

const V2_PLAIN: &str = "ses_eb28a8f36ffeMN3kZxz5Jlmcyh";
const V2_READ: &str = "ses_eb2894c9cffe89j2s5Jid8vrD9";
const V2_FAILED_READ: &str = "ses_eb28a64d7ffequM4bYumVFGVft";
const V2_SUBAGENT_PARENT: &str = "ses_eb2894b15ffejUNOeroIROljGy";
const MIGRATED_TOOL: &str = "ses_eb27ccda4ffeKjSodHyeONASJp";

/// Materialize one of the sanitized opencode database dumps (ADR 0125),
/// let `seed` add rows, and point a locator at `session_id`.
fn db_fixture(
    name: &str,
    session_id: &str,
    seed: impl Fn(&Connection),
) -> (tempfile::TempDir, SessionLocator) {
    let sql = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/opencode")
            .join(name),
    )
    .expect("read fixture");
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("opencode.db");
    let conn = Connection::open(&db_path).expect("open db");
    conn.execute_batch(&sql).expect("load fixture");
    seed(&conn);
    drop(conn);
    let locator = SessionLocator {
        harness_key: "opencode".to_string(),
        session_key: session_id.to_string(),
        state_root: db_path,
    };
    (dir, locator)
}

fn insert_v2_message(conn: &Connection, session: &str, seq: i64, kind: &str, data: &str) {
    conn.execute(
        "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
        params![
            format!("msg_test_{seq}"),
            session,
            kind,
            seq,
            1_792_300_000_000_i64 + seq,
            data
        ],
    )
    .expect("insert session_message");
}

/// `(role, kind, hidden, aborted, body)` for every turn.
fn v2_turns(doc: &TranscriptDocument) -> Vec<(TurnRole, TurnKind, bool, bool, &str)> {
    doc.turns
        .iter()
        .map(|t| (t.role, t.kind, t.hidden, t.aborted, t.body.as_str()))
        .collect()
}

#[test]
fn v2_session_renders_prompts_reasoning_and_replies_in_seq_order() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_PLAIN, |_| {});
    let doc = OpenCodeParser.read(&locator).expect("parse");

    assert_eq!(doc.meta.cwd.as_deref(), Some("/home/user/proj"));
    assert_eq!(
        v2_turns(&doc),
        vec![
            (TurnRole::User, TurnKind::Message, false, false, "Say hello"),
            (
                TurnRole::Assistant,
                TurnKind::Thinking,
                false,
                false,
                "The user wants a short greeting."
            ),
            (
                TurnRole::Assistant,
                TurnKind::Message,
                false,
                false,
                "Hello from the mock model."
            ),
            (
                TurnRole::User,
                TurnKind::Message,
                false,
                false,
                "Say hello again"
            ),
            (
                TurnRole::Assistant,
                TurnKind::Thinking,
                false,
                false,
                "The user wants a short greeting."
            ),
            (
                TurnRole::Assistant,
                TurnKind::Message,
                false,
                false,
                "Hello from the mock model."
            ),
        ]
    );
    assert!(doc.turns[0].timestamp.is_some());
}

#[test]
fn v2_completed_tool_shows_input_and_output() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_READ, |_| {});
    let doc = OpenCodeParser.read(&locator).expect("parse");

    let tool: Vec<_> = doc
        .turns
        .iter()
        .filter(|t| matches!(t.kind, TurnKind::ToolUse | TurnKind::ToolResult))
        .map(|t| (t.kind, t.body.as_str()))
        .collect();
    assert_eq!(
        tool,
        vec![
            (
                TurnKind::ToolUse,
                r#"read: {"path":"/home/user/proj/README.md"}"#
            ),
            (
                TurnKind::ToolResult,
                "Read file /home/user/proj/README.md, lines 1-1\n1: hello from the fixture"
            ),
        ]
    );
    assert_eq!(
        doc.turns.last().map(|t| t.body.as_str()),
        Some("I read the file; it says hello from the fixture.")
    );
}

#[test]
fn v2_errored_tool_shows_the_error_as_its_result() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_FAILED_READ, |_| {});
    let doc = OpenCodeParser.read(&locator).expect("parse");

    let result = doc
        .turns
        .iter()
        .find(|t| t.kind == TurnKind::ToolResult)
        .expect("tool result");
    assert!(
        result
            .body
            .starts_with("Error: Invalid arguments for tool \"read\""),
        "{}",
        result.body
    );
}

#[test]
fn v2_subagent_tool_result_names_the_child_session() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_SUBAGENT_PARENT, |_| {});
    let doc = OpenCodeParser.read(&locator).expect("parse");

    let call = doc
        .turns
        .iter()
        .find(|t| t.kind == TurnKind::ToolUse)
        .expect("tool call");
    assert!(call.body.starts_with("subagent: "), "{}", call.body);
    let result = doc
        .turns
        .iter()
        .find(|t| t.kind == TurnKind::ToolResult)
        .expect("tool result");
    assert!(
        result
            .body
            .contains("<subagent sessionID=\"ses_eb2894ad9ffebJ4LlOK9UvscBN\""),
        "{}",
        result.body
    );
}

#[test]
fn v2_bookkeeping_rows_are_hidden_and_unknown_rows_skipped() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_PLAIN, |conn| {
        let rows = [
            (
                "model-switched",
                r#"{"model":{"id":"other","providerID":"mock"},"time":{"created":1}}"#,
            ),
            ("agent-switched", r#"{"agent":"plan","time":{"created":1}}"#),
            (
                "location-switched",
                r#"{"location":{"directory":"/home/user/other"},"time":{"created":1}}"#,
            ),
            (
                "system",
                r#"{"text":"Tools changed.","time":{"created":1}}"#,
            ),
            (
                "synthetic",
                r#"{"text":"Injected context","time":{"created":1}}"#,
            ),
            (
                "skill",
                r#"{"skill":"s","name":"lint","text":"Lint rules","time":{"created":1}}"#,
            ),
            (
                "compaction",
                r#"{"status":"completed","reason":"manual","summary":"Summary so far","recent":"","time":{"created":1}}"#,
            ),
            (
                "compaction",
                r#"{"status":"failed","reason":"auto","error":{"type":"x","message":"too big"},"time":{"created":1}}"#,
            ),
            (
                "compaction",
                r#"{"status":"running","reason":"auto","summary":"","recent":"","time":{"created":1}}"#,
            ),
            (
                "shell",
                r#"{"shellID":"sh_1","command":"ls","status":"exited","exit":0,"output":"README.md\n","time":{"created":1}}"#,
            ),
            (
                "future-kind",
                r#"{"text":"nobody knows","time":{"created":1}}"#,
            ),
            ("assistant", "not json"),
        ];
        for (offset, (kind, data)) in rows.iter().enumerate() {
            insert_v2_message(conn, V2_PLAIN, 100 + offset as i64, kind, data);
        }
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");

    assert_eq!(
        v2_turns(&doc)[6..],
        [
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Model switched to mock/other"
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Agent switched to plan"
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Location switched to /home/user/other"
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Tools changed."
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Injected context"
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Lint rules"
            ),
            (
                TurnRole::System,
                TurnKind::CompactionSummary,
                false,
                false,
                "Summary so far"
            ),
            (
                TurnRole::System,
                TurnKind::Message,
                true,
                false,
                "Compaction failed: too big"
            ),
            (TurnRole::User, TurnKind::ToolUse, false, false, "shell: ls"),
            (
                TurnRole::User,
                TurnKind::ToolResult,
                false,
                false,
                "README.md\n"
            ),
        ]
    );
}

#[test]
fn v2_interrupted_idle_marks_its_exchange_aborted() {
    let (_tmp, locator) = db_fixture("v2-fresh.sql", V2_PLAIN, |conn| {
        insert_v2_message(
            conn,
            V2_PLAIN,
            100,
            "user",
            r#"{"text":"Start a long task"}"#,
        );
        insert_v2_message(
            conn,
            V2_PLAIN,
            101,
            "assistant",
            r#"{"agent":"build","content":[{"type":"text","text":"Working on"}]}"#,
        );
        insert_v2_message(conn, V2_PLAIN, 102, "idle", r#"{"outcome":"interrupted"}"#);
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");

    let aborted: Vec<_> = doc
        .turns
        .iter()
        .map(|t| (t.body.as_str(), t.aborted))
        .collect();
    assert_eq!(
        aborted[4..],
        [
            ("The user wants a short greeting.", false),
            ("Hello from the mock model.", false),
            ("Start a long task", true),
            ("Working on", true),
        ]
    );
}

#[test]
fn migrated_session_reads_session_message_and_hides_the_migration_note() {
    let (_tmp, locator) = db_fixture("v2-migrated.sql", MIGRATED_TOOL, |_| {});
    let doc = OpenCodeParser.read(&locator).expect("parse");

    let last = doc.turns.last().expect("turns");
    assert_eq!(last.role, TurnRole::System);
    assert!(last.hidden);
    assert!(last.body.starts_with("The available tools have changed."));
    assert!(
        doc.turns
            .iter()
            .any(|t| t.kind == TurnKind::ToolUse && t.body.starts_with("read: "))
    );
}

#[test]
fn migrated_session_without_a_session_v2_row_reads_the_1x_tables() {
    let (_tmp, locator) = db_fixture("v2-migrated.sql", MIGRATED_TOOL, |conn| {
        conn.execute(
            "DELETE FROM session_message WHERE session_id = ?1",
            [MIGRATED_TOOL],
        )
        .expect("drop v2 messages");
        conn.execute("DELETE FROM session_v2 WHERE id = ?1", [MIGRATED_TOOL])
            .expect("drop v2 row");
    });
    let doc = OpenCodeParser.read(&locator).expect("parse");

    assert_eq!(doc.meta.cwd.as_deref(), Some("/home/user/proj"));
    assert!(doc.turns.iter().all(|t| !t.hidden));
    assert!(
        doc.turns
            .iter()
            .any(|t| t.kind == TurnKind::ToolUse && t.body.contains("filePath"))
    );
}
