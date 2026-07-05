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
    fn read(&self, locator: &SessionLocator) -> ParseResult {
        // H-EXT-006: the opencode adapter's `transcript_source`
        // resolves `state_root` to the SQLite database file path
        // (or, when the caller passed a directory, materializes
        // the `opencode.db` child). Either way, this parser
        // receives an absolute file path pointing at the SQLite
        // database.
        let db_path = locator.state_root.as_path();
        let session_id = locator.session_key.as_str();
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
    // Pre-pass: scan messages in chronological order to build the
    // per-message aborted flag. An assistant message carrying
    // `error.name = "MessageAbortedError"` was interrupted by the
    // operator; its preceding user message (the prompt that
    // triggered the aborted turn) is tagged too so the whole
    // exchange disappears together when `show_aborted` is off.
    let aborted_message_ids = read_aborted_message_ids(conn, session_id)?;

    let mut stmt = conn.prepare(
        "SELECT m.id, m.data AS message_data, p.data AS part_data, p.time_created \
         FROM message m \
         LEFT JOIN part p ON p.message_id = m.id \
         WHERE m.session_id = ?1 \
         ORDER BY m.time_created ASC, p.time_created ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<i64>>(3)?,
        ))
    })?;

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    for row in rows {
        let (message_id, message_json, part_json, part_time_ms) = row?;
        let Some(part_json) = part_json else {
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
        let aborted = aborted_message_ids.contains(message_id.as_str());
        emit_turns_for_part(role, timestamp, &part, aborted, &mut turns);
    }
    Ok(turns)
}

/// Collect the set of message ids that should render as `aborted`:
/// every assistant message with `error.name = "MessageAbortedError"`
/// (the operator pressed Esc) plus the user message that immediately
/// preceded it in chronological order (the prompt that was
/// interrupted). Pairing the user prompt with the aborted assistant
/// row mirrors what OpenCode's own UI hid from the operator.
fn read_aborted_message_ids(
    conn: &Connection,
    session_id: &str,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.data \
         FROM message m \
         WHERE m.session_id = ?1 \
         ORDER BY m.time_created ASC",
    )?;
    let mut aborted = std::collections::HashSet::<String>::new();
    let mut last_user_id: Option<String> = None;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, data) = row?;
        let Ok(message) = serde_json::from_str::<MessageData>(&data) else {
            continue;
        };
        match message.role.as_deref() {
            Some("user") => {
                last_user_id = Some(id);
            }
            Some("assistant") => {
                let is_aborted_error = message
                    .error
                    .as_ref()
                    .and_then(|e| e.name.as_deref())
                    .is_some_and(|name| name == "MessageAbortedError");
                if is_aborted_error {
                    aborted.insert(id);
                    if let Some(user_id) = last_user_id.take() {
                        aborted.insert(user_id);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(aborted)
}

fn emit_turns_for_part(
    role: TurnRole,
    timestamp: Option<DateTime<Utc>>,
    part: &PartData,
    aborted: bool,
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
                aborted,
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
                aborted,
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
                aborted,
            });
            if let Some(output) = state.and_then(|s| s.output.as_deref()).and_then(nonempty) {
                out.push(TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolResult,
                    body: output,
                    timestamp,
                    aborted,
                });
            }
        }
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
    #[serde(default)]
    error: Option<MessageError>,
}

#[derive(Deserialize)]
struct MessageError {
    #[serde(default)]
    name: Option<String>,
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
#[path = "opencode_tests.rs"]
mod tests;
