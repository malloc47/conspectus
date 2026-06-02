//! OpenCode SQLite transcript reader (H-VIEWER-NATIVE-005).
//!
//! Reads from `opencode.db` per ADR 0013. OpenCode's session
//! storage moved to SQLite; the `session_diff/` directory is
//! diff-only metadata and is NOT the record of truth — this parser
//! ignores it entirely, which is the gap that recall couldn't
//! close (`H-TRANSCRIPT-014`).
//!
//! Schema (observed against live `opencode.db`):
//!
//! - `session(id text PK, directory text, title text, time_created
//!   int, ...)` — one row per session. `directory` is the cwd.
//! - `message(id text PK, session_id text, time_created int,
//!   data text)` — one row per top-level message. `data` is JSON:
//!   `{role: "user"|"assistant", time: {created: <ms>}, ...}`.
//! - `part(id text PK, message_id text, session_id text,
//!   time_created int, data text)` — one row per content part
//!   inside a message. `data.type` discriminates: `text`,
//!   `reasoning`, `tool`, `step-start`, `step-finish`, `patch`,
//!   etc.
//!
//! Translation into the normalized model:
//! - Order: messages by `m.time_created`, then parts by
//!   `p.time_created` within each message. One LEFT JOIN per
//!   session is the v1 query.
//! - `part.type = "text"` → `Message` turn with the parent
//!   message's role.
//! - `part.type = "reasoning"` → `Thinking` turn (assistant
//!   implicit).
//! - `part.type = "tool"` → emit TWO turns from the same row:
//!   - `ToolUse` with body `"<tool>: <state.input>"`.
//!   - `ToolResult` with body `state.output`. OpenCode bundles
//!     call + result in one part; splitting them keeps the
//!     viewer's fold-tool-blocks UX consistent across harnesses.
//! - `part.type = "step-start"` / `"step-finish"` → skipped (model
//!   step lifecycle markers, not transcript content).
//! - `part.type = "patch"` → skipped in v1 (revisit when the
//!   widget gains patch-render UX).
//! - Empty bodies dropped.
//! - Timestamps are epoch-ms; converted to `DateTime<Utc>` via
//!   `chrono::DateTime::from_timestamp_millis`.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};

pub struct OpenCodeParser;

impl HarnessParser for OpenCodeParser {
    fn supports(&self, locator: &SessionLocator) -> bool {
        matches!(locator, SessionLocator::OpenCode { .. })
    }

    fn read(&self, locator: &SessionLocator) -> ParseResult {
        let (db_path, session_id) = match locator {
            SessionLocator::OpenCode {
                db_path,
                session_id,
            } => (db_path.as_path(), session_id.as_str()),
            _ => return Err(ParseError::NotFound),
        };
        if !db_path.exists() {
            return Err(ParseError::NotFound);
        }
        parse_db(db_path, session_id, locator)
    }
}

fn parse_db(db_path: &Path, session_id: &str, locator: &SessionLocator) -> ParseResult {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| ParseError::Io(err.to_string()))?;

    let cwd = read_session_cwd(&conn, session_id).map_err(|err| ParseError::Io(err.to_string()))?;

    let turns =
        read_turns(&conn, session_id).map_err(|err| ParseError::Malformed(err.to_string()))?;

    Ok(TranscriptDocument {
        meta: TranscriptMeta {
            harness: locator.harness_key().to_string(),
            session_key: locator.session_key().to_string(),
            cwd,
        },
        turns,
    })
}

fn read_session_cwd(conn: &Connection, session_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT directory FROM session WHERE id = ?1")?;
    let mut rows = stmt.query(params![session_id])?;
    if let Some(row) = rows.next()? {
        Ok(row.get::<_, Option<String>>(0)?)
    } else {
        Ok(None)
    }
}

fn read_turns(conn: &Connection, session_id: &str) -> rusqlite::Result<Vec<TranscriptTurn>> {
    let mut stmt = conn.prepare(
        "SELECT m.data AS message_data, p.data AS part_data, p.time_created \
         FROM message m \
         LEFT JOIN part p ON p.message_id = m.id \
         WHERE m.session_id = ?1 \
         ORDER BY m.time_created ASC, p.time_created ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    for row in rows {
        let (message_json, part_json, part_time_ms) = row?;
        let Some(part_json) = part_json else {
            // Message with no parts — skip rather than emitting
            // a phantom empty turn.
            continue;
        };
        let Ok(message) = serde_json::from_str::<MessageData>(&message_json) else {
            continue;
        };
        let Ok(part) = serde_json::from_str::<PartData>(&part_json) else {
            continue;
        };
        let timestamp = part_time_ms.and_then(DateTime::from_timestamp_millis);
        let role = match message.role.as_deref() {
            Some("user") => TurnRole::User,
            Some("assistant") => TurnRole::Assistant,
            _ => TurnRole::Assistant,
        };
        emit_turns_for_part(role, timestamp, &part, &mut turns);
    }
    Ok(turns)
}

fn emit_turns_for_part(
    role: TurnRole,
    timestamp: Option<DateTime<Utc>>,
    part: &PartData,
    out: &mut Vec<TranscriptTurn>,
) {
    let Some(part_type) = part.part_type.as_deref() else {
        return;
    };
    match part_type {
        "text" => {
            let Some(body) = part.text.as_deref().and_then(nonempty) else {
                return;
            };
            out.push(TranscriptTurn {
                role,
                kind: TurnKind::Message,
                body,
                timestamp,
            });
        }
        "reasoning" => {
            let Some(body) = part.text.as_deref().and_then(nonempty) else {
                return;
            };
            out.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Thinking,
                body,
                timestamp,
            });
        }
        "tool" => {
            let name = part.tool.as_deref().unwrap_or("(unnamed tool)");
            let state = part.state.as_ref();
            let input_body = state
                .and_then(|s| s.input.as_ref())
                .map(|v| serde_json::to_string(v).unwrap_or_default())
                .unwrap_or_default();
            let call_body = if input_body.is_empty() {
                name.to_string()
            } else {
                format!("{name}: {input_body}")
            };
            out.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolUse,
                body: call_body,
                timestamp,
            });
            // OpenCode bundles call + result in one row. Emit a
            // companion ToolResult turn when the state carries an
            // output, so the renderer can fold call/result as a
            // pair the same way it does for Claude / Codex.
            if let Some(output) = state.and_then(|s| s.output.as_deref()).and_then(nonempty) {
                out.push(TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolResult,
                    body: output,
                    timestamp,
                });
            }
        }
        // step-start / step-finish: model step markers.
        // patch: applied diff — skip for v1; revisit when widget
        // gains patch rendering.
        _ => {}
    }
}

fn nonempty(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

// -- JSON shapes ------------------------------------------------------------

#[derive(Deserialize)]
struct MessageData {
    #[serde(default)]
    role: Option<String>,
}

#[derive(Deserialize)]
struct PartData {
    #[serde(rename = "type", default)]
    part_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    state: Option<ToolState>,
}

#[derive(Deserialize)]
struct ToolState {
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params};
    use std::path::PathBuf;

    /// Build a temp `opencode.db` with the OpenCode schema and seed
    /// it with one session + messages + parts. Returns the tempdir
    /// + locator pointing at the db.
    fn fixture(
        session_id: &str,
        seed: impl Fn(&Connection),
    ) -> (tempfile::TempDir, SessionLocator) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("opencode.db");
        let conn = Connection::open(&db_path).expect("open db");
        conn.execute_batch(
            r#"
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
            "#,
        )
        .expect("create schema");
        seed(&conn);
        drop(conn);
        let locator = SessionLocator::OpenCode {
            db_path,
            session_id: session_id.to_string(),
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

    #[test]
    fn supports_only_opencode_locator() {
        let p = OpenCodeParser;
        assert!(p.supports(&SessionLocator::OpenCode {
            db_path: PathBuf::from("/x"),
            session_id: "s".to_string(),
        }));
        assert!(!p.supports(&SessionLocator::ClaudeCode {
            state_root: PathBuf::from("/x"),
            session_key: "k".to_string(),
        }));
    }

    #[test]
    fn missing_db_reports_not_found() {
        let locator = SessionLocator::OpenCode {
            db_path: PathBuf::from("/nonexistent/opencode.db"),
            session_id: "ses_x".to_string(),
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
}
