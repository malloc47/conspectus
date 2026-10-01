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
