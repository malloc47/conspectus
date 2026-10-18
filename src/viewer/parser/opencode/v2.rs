//! OpenCode 2.x transcript reader (ADR 0125).
//!
//! 2.x keeps one row per transcript entry in `session_message`, ordered
//! by `seq`, with a typed JSON `data` blob whose `type` matches the row's
//! `type` column:
//!
//! - `user` → a `Message` turn from `data.text`.
//! - `assistant` → one turn per `data.content` item: `text` → `Message`,
//!   `reasoning` → `Thinking`, `tool` → `ToolUse` (`name: input`) plus a
//!   `ToolResult` from `state.content` text items, or from
//!   `state.error.message` for an errored tool.
//! - `shell` (a command the operator ran from the prompt) → `ToolUse`
//!   and `ToolResult`, like a tool.
//! - `compaction` with status `completed` → `CompactionSummary`; a failed
//!   compaction → a hidden system note.
//! - `idle` with outcome `interrupted` → no turn of its own; it tags the
//!   exchange it ended, from the last user prompt on, as aborted.
//! - `system`, `synthetic`, `skill`, and `model-switched` /
//!   `agent-switched` / `location-switched` → hidden system notes. They
//!   are model-facing context and bookkeeping that opencode's own UI
//!   doesn't show as conversation (ADR 0122).
//! - Anything else, and rows whose `data` isn't JSON, are skipped.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::viewer::model::{TranscriptTurn, TurnKind, TurnRole};

/// The session's cwd when `session_v2` holds a row for it. `None` when
/// the table or the row is missing, so the caller falls back to the 1.x
/// layout.
pub(super) fn session_cwd(conn: &Connection, session_id: &str) -> Option<Option<String>> {
    conn.query_row(
        "SELECT directory FROM session_v2 WHERE id = ?1",
        params![session_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .ok()
    .flatten()
}

pub(super) fn read_turns(
    conn: &Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<TranscriptTurn>> {
    let mut stmt = conn.prepare(
        "SELECT type, data, time_created FROM session_message \
         WHERE session_id = ?1 ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;

    let mut turns = Vec::new();
    let mut exchange_start = 0;
    for row in rows {
        let (message_type, data, time_ms) = row?;
        let Ok(data) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let timestamp = time_ms.and_then(DateTime::from_timestamp_millis);
        let mut out = Turns {
            turns: &mut turns,
            timestamp,
        };
        match message_type.as_str() {
            "user" => {
                exchange_start = out.turns.len();
                out.push(
                    TurnRole::User,
                    TurnKind::Message,
                    text(&data, "text"),
                    false,
                );
            }
            "assistant" => assistant_turns(&data, &mut out),
            "shell" => shell_turns(&data, &mut out),
            "compaction" => compaction_turns(&data, &mut out),
            "idle" => {
                if data.get("outcome").and_then(Value::as_str) == Some("interrupted") {
                    for turn in &mut out.turns[exchange_start..] {
                        turn.aborted = true;
                    }
                }
            }
            "system" | "synthetic" | "skill" => {
                out.push(
                    TurnRole::System,
                    TurnKind::Message,
                    text(&data, "text"),
                    true,
                );
            }
            "model-switched" => {
                let model = data.get("model");
                let id = model.and_then(|m| text(m, "id"));
                let provider = model.and_then(|m| text(m, "providerID"));
                let note = match (provider, id) {
                    (Some(provider), Some(id)) => {
                        Some(format!("Model switched to {provider}/{id}"))
                    }
                    (None, Some(id)) => Some(format!("Model switched to {id}")),
                    _ => None,
                };
                out.push(TurnRole::System, TurnKind::Message, note, true);
            }
            "agent-switched" => {
                let note = text(&data, "agent").map(|agent| format!("Agent switched to {agent}"));
                out.push(TurnRole::System, TurnKind::Message, note, true);
            }
            "location-switched" => {
                let note = data
                    .get("location")
                    .and_then(|location| text(location, "directory"))
                    .map(|directory| format!("Location switched to {directory}"));
                out.push(TurnRole::System, TurnKind::Message, note, true);
            }
            _ => {}
        }
    }
    Ok(turns)
}

/// Appends turns stamped with one row's timestamp.
struct Turns<'a> {
    turns: &'a mut Vec<TranscriptTurn>,
    timestamp: Option<DateTime<Utc>>,
}

impl Turns<'_> {
    fn push(&mut self, role: TurnRole, kind: TurnKind, body: Option<String>, hidden: bool) {
        let Some(body) = body.filter(|body| !body.trim().is_empty()) else {
            return;
        };
        self.turns.push(TranscriptTurn {
            role,
            kind,
            body,
            timestamp: self.timestamp,
            aborted: false,
            hidden,
        });
    }
}

fn assistant_turns(data: &Value, out: &mut Turns<'_>) {
    let Some(content) = data.get("content").and_then(Value::as_array) else {
        return;
    };
    for item in content {
        match item.get("type").and_then(Value::as_str) {
            Some("text") => {
                out.push(
                    TurnRole::Assistant,
                    TurnKind::Message,
                    text(item, "text"),
                    false,
                );
            }
            Some("reasoning") => {
                out.push(
                    TurnRole::Assistant,
                    TurnKind::Thinking,
                    text(item, "text"),
                    false,
                );
            }
            Some("tool") => {
                let name = text(item, "name").unwrap_or_else(|| "(unnamed tool)".to_string());
                let state = item.get("state");
                let input = state.and_then(|state| state.get("input"));
                out.push(
                    TurnRole::Assistant,
                    TurnKind::ToolUse,
                    Some(call_body(&name, input)),
                    false,
                );
                out.push(
                    TurnRole::Assistant,
                    TurnKind::ToolResult,
                    state.and_then(tool_output),
                    false,
                );
            }
            _ => {}
        }
    }
}

fn shell_turns(data: &Value, out: &mut Turns<'_>) {
    let Some(command) = text(data, "command") else {
        return;
    };
    out.push(
        TurnRole::User,
        TurnKind::ToolUse,
        Some(format!("shell: {command}")),
        false,
    );
    let output = data.get("output").and_then(|output| match output {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    });
    out.push(TurnRole::User, TurnKind::ToolResult, output, false);
}

fn compaction_turns(data: &Value, out: &mut Turns<'_>) {
    match data.get("status").and_then(Value::as_str) {
        Some("completed") => out.push(
            TurnRole::System,
            TurnKind::CompactionSummary,
            text(data, "summary"),
            false,
        ),
        Some("failed") => {
            let reason = data
                .get("error")
                .and_then(|error| text(error, "message"))
                .unwrap_or_else(|| "unknown error".to_string());
            out.push(
                TurnRole::System,
                TurnKind::Message,
                Some(format!("Compaction failed: {reason}")),
                true,
            );
        }
        _ => {}
    }
}

/// `name: <input JSON>`, or just `name` when there is no input. A tool
/// still streaming its input holds it as a raw string.
fn call_body(name: &str, input: Option<&Value>) -> String {
    let input = match input {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(raw)) => raw.clone(),
        Some(Value::Object(map)) if map.is_empty() => String::new(),
        Some(other) => other.to_string(),
    };
    if input.is_empty() {
        name.to_string()
    } else {
        format!("{name}: {input}")
    }
}

/// The tool's output text: its `content` text items, or the error
/// message when it failed without any.
fn tool_output(state: &Value) -> Option<String> {
    let content: Vec<&str> = state
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect();
    if !content.is_empty() {
        return Some(content.join("\n"));
    }
    state
        .get("error")
        .and_then(|error| text(error, "message"))
        .map(|message| format!("Error: {message}"))
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}
