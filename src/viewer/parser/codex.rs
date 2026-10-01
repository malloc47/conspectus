//! Codex JSONL transcript reader.
//!
//! Reads `<state_root>/sessions/YYYY/MM/DD/rollout-*-<session_key>.jsonl`
//! and produces a [`TranscriptDocument`] in the viewer's
//! normalized model (ADR 0052 §"Own its data model — as a
//! normalized superset"). Codex's native schema differs from
//! Claude's:
//!
//! - Two record-grouping families. Outer `type` is one of
//!   `session_meta`, `turn_context`, `response_item`, `event_msg`,
//!   `compacted`. The actual conversation lives in `response_item`
//!   records; `event_msg` records are engine telemetry and are
//!   skipped; `session_meta` carries `cwd`.
//! - `response_item.payload.type` discriminates: `message` /
//!   `reasoning` / `function_call` / `function_call_output` /
//!   `custom_tool_call` / `custom_tool_call_output` /
//!   `web_search_call`.
//! - `message.role` is `user` / `assistant` / `developer` /
//!   `system`. The `developer` and `system` roles are
//!   conspectus / codex injected instructions and produce no
//!   operator-facing turns.
//! - Message content blocks are `input_text` (user/dev/sys) and
//!   `output_text` (assistant), each with `text`.
//! - `reasoning` records carry `summary` (array of text blocks) and
//!   `encrypted_content` (opaque). If neither summary nor visible
//!   content is present, no turn is emitted.
//! - `compacted` records carry a summary message; emit one
//!   `CompactionSummary` turn for them.
//!
//! Translation into the normalized model:
//! - One [`TranscriptTurn`] per content block (matches Claude
//!   parser pattern so the renderer doesn't branch on harness).
//! - `message` with role `user`/`assistant` → `Message` turns.
//! - `reasoning` → `Thinking` turn (skipped if empty).
//! - `function_call` / `custom_tool_call` / `web_search_call` →
//!   `ToolUse` (body = `"<name>: <args>"`).
//! - `function_call_output` / `custom_tool_call_output` →
//!   `ToolResult` (body = output, assistant role).
//! - `compacted` → `CompactionSummary`.
//! - `developer` and `system` role messages → skipped.
//! - The channel markers `<turn_aborted>` / `<proposed_plan>`
//!   appear as synthetic message bodies; drop those exact bodies
//!   as not-real-user-text.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};

pub struct CodexParser;

impl HarnessParser for CodexParser {
    fn read(&self, locator: &SessionLocator) -> ParseResult {
        // locator.state_root is the codex state root
        // the adapter's `transcript_source` produced.
        let state_root = locator.state_root.as_path();
        let session_key = locator.session_key.as_str();
        let file_path = find_rollout_file(state_root, session_key).ok_or(ParseError::NotFound)?;
        parse_file(&file_path, locator)
    }
}

/// Walk `<state_root>/sessions/<year>/<month>/<day>/` looking for
/// a rollout file whose basename ends with `-<session_key>.jsonl`.
/// Std-only — no `walkdir` dep.
fn find_rollout_file(state_root: &Path, session_key: &str) -> Option<PathBuf> {
    let sessions = state_root.join("sessions");
    let suffix = format!("-{session_key}.jsonl");
    for year_entry in std::fs::read_dir(&sessions).ok()?.flatten() {
        let year = year_entry.path();
        if !year.is_dir() {
            continue;
        }
        let Ok(month_iter) = std::fs::read_dir(&year) else {
            continue;
        };
        for month_entry in month_iter.flatten() {
            let month = month_entry.path();
            if !month.is_dir() {
                continue;
            }
            let Ok(day_iter) = std::fs::read_dir(&month) else {
                continue;
            };
            for day_entry in day_iter.flatten() {
                let day = day_entry.path();
                if !day.is_dir() {
                    continue;
                }
                let Ok(file_iter) = std::fs::read_dir(&day) else {
                    continue;
                };
                for file_entry in file_iter.flatten() {
                    let path = file_entry.path();
                    if path.is_file()
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.ends_with(&suffix))
                    {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn parse_file(path: &Path, locator: &SessionLocator) -> ParseResult {
    let file = File::open(path).map_err(|err| ParseError::Io(err.to_string()))?;
    let reader = BufReader::new(file);

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    let mut cwd: Option<String> = None;
    // Codex abort detection: track the index of the most-recent
    // user `Message` turn that hasn't yet been resolved by a clean
    // `task_complete` event. When `event_msg.turn_aborted` fires,
    // mark that user turn *and* every later turn in `turns` (any
    // partial assistant/reasoning/tool-call output before the
    // operator hit Esc) as `aborted: true`. The whole exchange
    // then disappears together when `show_aborted` is off — matches
    // what Codex's own UI hid from the operator.
    let mut pending_user_turn_idx: Option<usize> = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Record>(&line) else {
            continue;
        };
        let timestamp = parse_timestamp(record.timestamp.as_deref());

        match record.record_type.as_deref() {
            Some("session_meta") => {
                if cwd.is_none() {
                    cwd = record.payload.as_ref().and_then(|p| p.cwd.clone());
                }
            }
            Some("turn_context") => {
                // turn_context also carries cwd, but session_meta
                // is earlier and authoritative; only fall back here
                // if session_meta didn't carry it.
                if cwd.is_none() {
                    cwd = record.payload.as_ref().and_then(|p| p.cwd.clone());
                }
            }
            Some("response_item") => {
                let before = turns.len();
                if let Some(payload) = &record.payload {
                    emit_response_item_turns(payload, timestamp, &mut turns);
                }
                // If this response_item produced a fresh user
                // `Message`, remember it as the candidate for the
                // next `turn_aborted` event. We use the first new
                // index — Codex doesn't split user prose across
                // multiple content blocks in practice.
                for (offset, turn) in turns.iter().enumerate().skip(before) {
                    if turn.role == TurnRole::User && turn.kind == TurnKind::Message {
                        pending_user_turn_idx = Some(offset);
                        break;
                    }
                }
            }
            Some("event_msg") => {
                match record
                    .payload
                    .as_ref()
                    .and_then(|p| p.payload_type.as_deref())
                {
                    Some("turn_aborted") => {
                        if let Some(start) = pending_user_turn_idx.take() {
                            for turn in &mut turns[start..] {
                                turn.aborted = true;
                            }
                        }
                    }
                    Some("task_complete") => {
                        // Turn finished cleanly — the candidate is no
                        // longer at risk of being aborted by a later
                        // event.
                        pending_user_turn_idx = None;
                    }
                    _ => {}
                }
            }
            Some("compacted") => {
                let body = record
                    .payload
                    .as_ref()
                    .and_then(|p| p.message.clone())
                    .unwrap_or_default();
                let body = body.trim();
                if !body.is_empty() {
                    turns.push(TranscriptTurn {
                        role: TurnRole::User,
                        kind: TurnKind::CompactionSummary,
                        body: body.to_string(),
                        timestamp,
                        aborted: false,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(TranscriptDocument {
        meta: TranscriptMeta {
            harness: locator.harness_key().to_string(),
            session_key: locator.session_key().to_string(),
            cwd,
        },
        turns,
    })
}

fn emit_response_item_turns(
    payload: &Payload,
    timestamp: Option<DateTime<Utc>>,
    out: &mut Vec<TranscriptTurn>,
) {
    let Some(payload_type) = payload.payload_type.as_deref() else {
        return;
    };
    match payload_type {
        "message" => emit_message_turns(payload, timestamp, out),
        "reasoning" => emit_reasoning_turn(payload, timestamp, out),
        "function_call" | "custom_tool_call" | "web_search_call" => {
            emit_tool_call_turn(payload, timestamp, out);
        }
        "function_call_output" | "custom_tool_call_output" => {
            emit_tool_output_turn(payload, timestamp, out);
        }
        // Unknown payload types (future schema additions) are
        // dropped silently to keep older builds forward-compatible.
        _ => {}
    }
}

fn emit_message_turns(
    payload: &Payload,
    timestamp: Option<DateTime<Utc>>,
    out: &mut Vec<TranscriptTurn>,
) {
    let role = match payload.role.as_deref() {
        Some("user") => TurnRole::User,
        Some("assistant") => TurnRole::Assistant,
        // developer and system messages are codex / conspectus
        // injected instructions — no operator turn.
        _ => return,
    };
    let Some(blocks) = &payload.content else {
        return;
    };
    for block in blocks {
        let Some(block_type) = block.block_type.as_deref() else {
            continue;
        };
        if !matches!(block_type, "input_text" | "output_text") {
            continue;
        }
        let Some(text) = block.text.as_deref() else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() || is_channel_marker(trimmed) {
            continue;
        }
        out.push(TranscriptTurn {
            role,
            kind: TurnKind::Message,
            body: text.to_string(),
            timestamp,
            aborted: false,
        });
    }
}

fn emit_reasoning_turn(
    payload: &Payload,
    timestamp: Option<DateTime<Utc>>,
    out: &mut Vec<TranscriptTurn>,
) {
    // Visible reasoning text lives in `summary` (array of text
    // blocks) or `content` (array of text blocks). `encrypted_content`
    // is opaque and we don't render it.
    let mut body_parts: Vec<String> = Vec::new();
    if let Some(summary) = &payload.summary {
        for block in summary {
            if let Some(text) = block.text.as_deref() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    body_parts.push(trimmed.to_string());
                }
            }
        }
    }
    if let Some(content) = &payload.content {
        for block in content {
            if let Some(text) = block.text.as_deref() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    body_parts.push(trimmed.to_string());
                }
            }
        }
    }
    // Empty-but-present reasoning record: emit a placeholder so the
    // operator's thinking toggle has visible effect. Matches the
    // Claude Code parser's behaviour for opaque-content blocks.
    // The *presence* of a
    // reasoning record is the signal even when the body is
    // `encrypted_content` only.
    let body = if body_parts.is_empty() {
        "(reasoning hidden by the model)".to_string()
    } else {
        body_parts.join("\n\n")
    };
    out.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Thinking,
        body,
        timestamp,
        aborted: false,
    });
}

fn emit_tool_call_turn(
    payload: &Payload,
    timestamp: Option<DateTime<Utc>>,
    out: &mut Vec<TranscriptTurn>,
) {
    let name = payload.name.as_deref().unwrap_or("(unnamed tool)");
    let arguments = payload.arguments.as_deref().unwrap_or("").trim();
    let body = if arguments.is_empty() {
        name.to_string()
    } else {
        format!("{name}: {arguments}")
    };
    out.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolUse,
        body,
        timestamp,
        aborted: false,
    });
}

fn emit_tool_output_turn(
    payload: &Payload,
    timestamp: Option<DateTime<Utc>>,
    out: &mut Vec<TranscriptTurn>,
) {
    let Some(output) = payload.output.as_deref() else {
        return;
    };
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return;
    }
    out.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolResult,
        body: trimmed.to_string(),
        timestamp,
        aborted: false,
    });
}

/// Codex channel markers. When a message body
/// is *exactly* one of these markers it represents an engine
/// signal, not user prose, so we drop it. Partial matches are
/// rendered normally since real text often quotes the marker
/// names.
fn is_channel_marker(body: &str) -> bool {
    matches!(body, "<turn_aborted>" | "<proposed_plan>")
}

fn parse_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw?)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

// -- JSONL record shapes ----------------------------------------------------

#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type", default)]
    record_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    payload: Option<Payload>,
}

/// Permissive payload shape — every field is optional and unknown
/// fields are ignored. Field set is the union across every
/// `response_item.payload.type`, `session_meta.payload`, and
/// `compacted.payload` shape we observe.
#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "type", default)]
    payload_type: Option<String>,
    // session_meta / turn_context
    #[serde(default)]
    cwd: Option<String>,
    // compacted
    #[serde(default)]
    message: Option<String>,
    // response_item.message
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Vec<ContentBlock>>,
    // response_item.reasoning
    #[serde(default)]
    summary: Option<Vec<ContentBlock>>,
    // response_item.function_call / custom_tool_call / web_search_call
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
    // response_item.function_call_output / custom_tool_call_output
    #[serde(default)]
    output: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
