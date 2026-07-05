//! Claude Code JSONL transcript reader (H-VIEWER-NATIVE-003).
//!
//! Reads `<state_root>/projects/*/<session_key>.jsonl` and produces
//! a [`TranscriptDocument`] in the viewer's normalized
//! superset-of-all-harnesses model (ADR 0052 §"Own its data
//! model — as a normalized superset"). Claude Code's native
//! schema:
//!
//! - One JSON record per line.
//! - Record-level: `type`, `uuid`, `parentUuid`, `timestamp`
//!   (RFC3339), `cwd`, `sessionId`, `message`, `isMeta`,
//!   `isCompactSummary`, etc.
//! - `message.content` is a string OR a list of content blocks.
//! - Block `type`: `text` / `thinking` / `tool_use` / `tool_result`.
//! - Compaction summary: synthetic `type=user` record with
//!   `isCompactSummary=true`, carrying the post-compaction
//!   summary text.
//! - `type=user|assistant` are the only record types this parser
//!   emits turns from; metadata record types
//!   (`custom-title`, `agent-name`, `system`, etc.) are skipped.
//!
//! Translation into the normalized model:
//! - One [`TranscriptTurn`] per content block, so the renderer can
//!   fold tool / thinking blocks independently of the surrounding
//!   prose.
//! - String `content` → one [`TurnKind::Message`] turn.
//! - `text` block → [`TurnKind::Message`].
//! - `thinking` block → [`TurnKind::Thinking`].
//! - `tool_use` block → [`TurnKind::ToolUse`], body `name: <json>`.
//! - `tool_result` block → [`TurnKind::ToolResult`], body is the
//!   flattened content (string or joined text blocks).
//! - Compaction-summary records → one [`TurnKind::CompactionSummary`]
//!   turn rather than a regular user message.
//! - Empty bodies are dropped (no turn emitted).
//!
//! Errors degrade. Malformed lines are skipped silently (matches
//! the H-PREVIEW extractor in `src/discovery/harness/claude_code.rs`).
//! A missing file produces [`ParseError::NotFound`]; the bridge
//! converts that into a `TranscriptDocument::unavailable` and the
//! widget shows the "transcript unavailable" banner.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};

pub struct ClaudeCodeParser;

impl HarnessParser for ClaudeCodeParser {
    fn read(&self, locator: &SessionLocator) -> ParseResult {
        // H-EXT-006: the locator is already dispatched to us via
        // the registry, so `state_root` here is the claude-code
        // state root the adapter's `transcript_source` produced.
        let state_root = locator.state_root.as_path();
        let session_key = locator.session_key.as_str();
        let file_path = find_session_file(state_root, session_key).ok_or(ParseError::NotFound)?;
        parse_file(&file_path, locator)
    }
}

/// Walk `<state_root>/projects/*/` looking for `<session_key>.jsonl`.
/// Claude Code stores one project subdir per cwd; the session UUID
/// is the file stem. Std-only — no `glob` dependency.
fn find_session_file(state_root: &Path, session_key: &str) -> Option<PathBuf> {
    let projects = state_root.join("projects");
    let entries = std::fs::read_dir(&projects).ok()?;
    let needle = format!("{session_key}.jsonl");
    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let candidate = project_dir.join(&needle);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn parse_file(path: &Path, locator: &SessionLocator) -> ParseResult {
    let file = File::open(path).map_err(|err| ParseError::Io(err.to_string()))?;
    let reader = BufReader::new(file);

    // Two-pass: collect records, then build the parent→child uuid
    // index, then emit turns with the abort flag set for orphan
    // plain-text user records. Storing the whole record list is a
    // larger working-set than the single-pass version but is bounded
    // by file size (~MBs in practice) and we already pay this
    // memory cost for the emitted turn list.
    let mut records: Vec<Record> = Vec::new();
    let mut cwd: Option<String> = None;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Record>(&line) else {
            continue;
        };
        if cwd.is_none() {
            cwd = record.cwd.clone();
        }
        records.push(record);
    }

    // Parent set: every uuid that is anyone's `parentUuid`. A
    // plain-text user record whose uuid is *not* in this set was
    // never followed by an agent response — i.e. the user hit Esc
    // before Claude generated any reply.
    let parent_uuids: std::collections::HashSet<&str> = records
        .iter()
        .filter_map(|r| r.parent_uuid.as_deref())
        .collect();

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    let last_index = records.len().saturating_sub(1);
    for (i, record) in records.iter().enumerate() {
        // Orphan rule: plain-text user records (role=user, content
        // is a String) whose uuid has no children. We also require
        // that the record is not the last one in the file — the
        // chronological tail is the in-flight turn the agent is
        // still answering, not an abort.
        let is_orphan_user_text = is_plain_text_user_record(record)
            && record
                .uuid
                .as_deref()
                .is_some_and(|u| !parent_uuids.contains(u))
            && i < last_index;
        emit_turns_for_record(record, is_orphan_user_text, &mut turns);
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

/// `true` when the record is a normal user-typed prose message —
/// `type=user` with a plain-string `message.content`. Used to scope
/// the orphan-leaf abort heuristic so tool_result-only user records
/// (which never have children either, by design) aren't tagged.
fn is_plain_text_user_record(record: &Record) -> bool {
    if record.is_compact_summary {
        return false;
    }
    if record.record_type.as_deref() != Some("user") {
        return false;
    }
    matches!(
        record.message.as_ref().and_then(|m| m.content.as_ref()),
        Some(MessageContent::String(_))
    )
}

/// Translate one Claude record into zero or more normalized turns.
/// `aborted` is the abort flag for every turn emitted from this
/// record — see [`is_plain_text_user_record`] for the orphan-leaf
/// detection at the call site.
fn emit_turns_for_record(record: &Record, aborted: bool, out: &mut Vec<TranscriptTurn>) {
    if record.is_compact_summary {
        let body = compaction_summary_body(record);
        if !body.is_empty() {
            out.push(TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::CompactionSummary,
                body,
                timestamp: parse_timestamp(record.timestamp.as_deref()),
                aborted: false,
            });
        }
        return;
    }

    let role = match record.record_type.as_deref() {
        Some("user") => TurnRole::User,
        Some("assistant") => TurnRole::Assistant,
        _ => return,
    };
    let timestamp = parse_timestamp(record.timestamp.as_deref());
    let Some(message) = &record.message else {
        return;
    };

    match &message.content {
        Some(MessageContent::String(text)) => {
            if let Some(body) = nonempty(text) {
                out.push(TranscriptTurn {
                    role,
                    kind: TurnKind::Message,
                    body,
                    timestamp,
                    aborted,
                });
            }
        }
        Some(MessageContent::Blocks(blocks)) => {
            for block in blocks {
                if let Some(mut turn) = turn_from_block(role, timestamp, block) {
                    turn.aborted = aborted;
                    out.push(turn);
                }
            }
        }
        None => {}
    }
}

fn compaction_summary_body(record: &Record) -> String {
    let Some(message) = &record.message else {
        return String::new();
    };
    match &message.content {
        Some(MessageContent::String(text)) => text.trim().to_string(),
        Some(MessageContent::Blocks(blocks)) => blocks
            .iter()
            .filter_map(|b| {
                if b.block_type.as_deref() == Some("text") {
                    b.text.as_deref()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
            .trim()
            .to_string(),
        None => String::new(),
    }
}

fn turn_from_block(
    role: TurnRole,
    timestamp: Option<DateTime<Utc>>,
    block: &ContentBlock,
) -> Option<TranscriptTurn> {
    let block_type = block.block_type.as_deref()?;
    match block_type {
        "text" => {
            let text = block.text.as_deref()?;
            let body = nonempty(text)?;
            Some(TranscriptTurn {
                role,
                kind: TurnKind::Message,
                body,
                timestamp,
                aborted: false,
            })
        }
        "thinking" => {
            // `thinking` usually carries the visible chain-of-thought
            // in the `thinking` field; some assistant turns emit
            // *only* `encrypted_content` (opaque, omitted from our
            // deserializer) and the visible field is an empty
            // string. Render a placeholder in that case so toggling
            // the thinking view actually surfaces something — the
            // *presence* of a reasoning block is the operator
            // signal even when the body is sealed.
            let body = block
                .thinking
                .as_deref()
                .or(block.text.as_deref())
                .and_then(nonempty)
                .unwrap_or_else(|| "(reasoning hidden by the model)".to_string());
            Some(TranscriptTurn {
                role,
                kind: TurnKind::Thinking,
                body,
                timestamp,
                aborted: false,
            })
        }
        "tool_use" => {
            let name = block.name.as_deref().unwrap_or("(unnamed tool)");
            let input = block
                .input
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default())
                .unwrap_or_default();
            let body = if input.is_empty() {
                name.to_string()
            } else {
                format!("{name}: {input}")
            };
            Some(TranscriptTurn {
                role,
                kind: TurnKind::ToolUse,
                body,
                timestamp,
                aborted: false,
            })
        }
        "tool_result" => {
            let body = flatten_tool_result_content(block)?;
            Some(TranscriptTurn {
                role,
                kind: TurnKind::ToolResult,
                body,
                timestamp,
                aborted: false,
            })
        }
        // Unknown block types (future schema additions, OAuth
        // attachments, etc.) are dropped silently.
        _ => None,
    }
}

/// Tool-result `content` can be a string OR a list of text blocks
/// (each `{type: "text", text: "..."}`). Flatten to one string;
/// `None` if the content is missing or fully empty.
fn flatten_tool_result_content(block: &ContentBlock) -> Option<String> {
    let content = block.content.as_ref()?;
    let flattened = match content {
        ToolResultContent::String(text) => text.trim().to_string(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| {
                if b.block_type.as_deref() == Some("text") {
                    b.text.as_deref()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
    };
    nonempty(&flattened)
}

fn nonempty(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn parse_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw?)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

// -- JSONL record shapes ----------------------------------------------------
//
// Mirrored from real Claude Code transcript samples. Tag is
// permissive (`default` on every field) because the schema has
// version-shaped variation; unknown fields are ignored.

#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type", default)]
    record_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(rename = "isCompactSummary", default)]
    is_compact_summary: bool,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(rename = "parentUuid", default)]
    parent_uuid: Option<String>,
    #[serde(default)]
    message: Option<MessageBody>,
}

#[derive(Deserialize)]
struct MessageBody {
    #[serde(default)]
    content: Option<MessageContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageContent {
    String(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    content: Option<ToolResultContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ToolResultContent {
    String(String),
    Blocks(Vec<ContentBlock>),
}

#[cfg(test)]
#[path = "claude_code_tests.rs"]
mod tests;
