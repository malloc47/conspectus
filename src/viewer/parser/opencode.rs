//! OpenCode SQLite transcript reader.
//!
//! A session stored in opencode 2.x's `session_v2` table is read from
//! `session_message` by [`v2`]; everything below describes the 1.x
//! layout, used for sessions that only have a 1.x `session` row.
//!
//! Reads from `opencode.db` per ADR 0013. OpenCode's session
//! storage moved to SQLite; the `session_diff/` directory is
//! diff-only metadata and is NOT the record of truth, so this
//! parser ignores it entirely.
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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;

use super::{HarnessParser, ParseError, ParseResult, TranscriptIndex, TranscriptSource};
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};

mod v2;

pub struct OpenCodeParser;

impl HarnessParser for OpenCodeParser {
    fn read(&self, locator: &SessionLocator) -> ParseResult {
        // The opencode adapter's `transcript_source`
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

    fn index(&self, state_root: &Path) -> Box<dyn TranscriptIndex> {
        Box::new(DbIndex(state_root.to_path_buf()))
    }

    fn read_source(&self, source: &TranscriptSource, locator: &SessionLocator) -> ParseResult {
        match source {
            TranscriptSource::Sqlite { db, session } => {
                with_worker_connection(db, |conn| parse_conn(conn, session, locator))
            }
            TranscriptSource::File(_) => Err(ParseError::NotFound),
        }
    }

    fn candidates(
        &self,
        state_root: &Path,
        runs: &[String],
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Option<HashSet<String>> {
        with_worker_connection(state_root, |conn| {
            Ok(candidate_sessions(conn, runs, cancelled))
        })
        .ok()
        .flatten()
    }
}

/// Every session is in the one database the locator names.
struct DbIndex(PathBuf);

impl TranscriptIndex for DbIndex {
    fn locate(&self, session_key: &str) -> Option<TranscriptSource> {
        self.0.exists().then(|| TranscriptSource::Sqlite {
            db: self.0.clone(),
            session: session_key.to_string(),
        })
    }
}

thread_local! {
    /// One read-only connection per database per worker thread, so a scan
    /// opens the database once per worker instead of once per session.
    static CONNECTIONS: RefCell<HashMap<PathBuf, Connection>> = RefCell::new(HashMap::new());
}

/// Run `f` on this thread's read-only connection to `db`, opening it (with
/// `query_only` on) the first time.
fn with_worker_connection<T>(
    db: &Path,
    f: impl FnOnce(&Connection) -> Result<T, ParseError>,
) -> Result<T, ParseError> {
    CONNECTIONS.with(|cell| {
        let mut connections = cell.borrow_mut();
        if !connections.contains_key(db) {
            let conn = open_read_only(db)?;
            conn.execute_batch("PRAGMA query_only = ON;")
                .map_err(|err| ParseError::Io(err.to_string()))?;
            connections.insert(db.to_path_buf(), conn);
        }
        f(&connections[db])
    })
}

/// Whether a write through the worker connection to `db` is refused, as
/// ADR 0143 requires of readers.
#[cfg(test)]
pub(crate) fn worker_connection_refuses_writes(db: &Path) -> bool {
    with_worker_connection(db, |conn| {
        Ok(conn.execute_batch("CREATE TABLE intruder (a)").is_err())
    })
    .unwrap_or(false)
}

fn open_read_only(db_path: &Path) -> Result<Connection, ParseError> {
    Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| ParseError::Io(err.to_string()))
}

/// Session ids with a stored row whose JSON contains every run (compared
/// case-insensitively in ASCII, as SQLite's `lower` folds), one query per
/// table. A table the database lacks (1.x has `part`, 2.x has
/// `session_message`) contributes nothing.
///
/// `None`, meaning "can't narrow, scan everything", for every failure: a
/// query or row error (such as a busy database), none of the tables
/// existing, or cancellation. An error never becomes an empty candidate
/// set, because that would rule out sessions that match.
fn candidate_sessions(
    conn: &Connection,
    runs: &[String],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Option<HashSet<String>> {
    if runs.is_empty() {
        return None;
    }
    let conditions = (1..=runs.len())
        .map(|n| format!("instr(lower(data), ?{n}) > 0"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let mut found = HashSet::new();
    let mut any_table = false;
    for table in ["part", "message", "session_message"] {
        if cancelled() {
            return None;
        }
        let sql = format!("SELECT DISTINCT session_id FROM {table} WHERE {conditions}");
        let mut stmt = match conn.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(err) if err.to_string().contains("no such table") => continue,
            Err(_) => return None,
        };
        any_table = true;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(runs.iter()), |row| {
                row.get::<_, String>(0)
            })
            .ok()?;
        for id in rows {
            found.insert(id.ok()?);
        }
    }
    any_table.then_some(found)
}

fn parse_db(db_path: &Path, session_id: &str, locator: &SessionLocator) -> ParseResult {
    let conn = open_read_only(db_path)?;
    parse_conn(&conn, session_id, locator)
}

fn parse_conn(conn: &Connection, session_id: &str, locator: &SessionLocator) -> ParseResult {
    let (cwd, turns) = if let Some(cwd) = v2::session_cwd(conn, session_id) {
        let turns = v2::read_turns(conn, session_id)
            .map_err(|err| ParseError::Malformed(err.to_string()))?;
        (cwd, turns)
    } else {
        let cwd =
            read_session_cwd(conn, session_id).map_err(|err| ParseError::Io(err.to_string()))?;
        let turns =
            read_turns(conn, session_id).map_err(|err| ParseError::Malformed(err.to_string()))?;
        (cwd, turns)
    };

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
                hidden: false,
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
                hidden: false,
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
                hidden: false,
            });
            if let Some(output) = state.and_then(|s| s.output.as_deref()).and_then(nonempty) {
                out.push(TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolResult,
                    body: output,
                    timestamp,
                    aborted,
                    hidden: false,
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
