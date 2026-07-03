//! Codex JSONL transcript reader (H-VIEWER-NATIVE-004).
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
//! - Per H-PREVIEW-006 the channel markers `<turn_aborted>` /
//!   `<proposed_plan>` appear as synthetic message bodies; drop
//!   those exact bodies as not-real-user-text.

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
        // H-EXT-006: locator.state_root is the codex state root
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
    // Claude Code parser's behaviour for opaque-content blocks
    // (`H-VIEWER-NATIVE-011` operator feedback). The *presence* of a
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

/// Codex channel markers (per H-PREVIEW-006). When a message body
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
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Lay down `<state_root>/sessions/2026/05/12/rollout-<ts>-<session_key>.jsonl`
    /// pre-loaded with `lines`.
    fn fixture(session_key: &str, lines: &[&str]) -> (tempfile::TempDir, SessionLocator) {
        let dir = tempfile::tempdir().expect("tempdir");
        let day = dir
            .path()
            .join("sessions")
            .join("2026")
            .join("05")
            .join("12");
        fs::create_dir_all(&day).expect("day dir");
        let file_path = day.join(format!("rollout-2026-05-12T13-18-53-{session_key}.jsonl"));
        let mut file = fs::File::create(&file_path).expect("create jsonl");
        for line in lines {
            writeln!(file, "{line}").expect("write line");
        }
        let locator = SessionLocator {
            harness_key: "codex".to_string(),
            session_key: session_key.to_string(),
            state_root: dir.path().to_path_buf(),
        };
        (dir, locator)
    }

    // H-EXT-006: pre-existing `supports_only_codex_locator`
    // test removed; parser dispatch happens through the adapter
    // registry so per-parser filters are dead weight.

    #[test]
    fn missing_file_reports_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let locator = SessionLocator {
            harness_key: "codex".to_string(),
            session_key: "absent".to_string(),
            state_root: dir.path().to_path_buf(),
        };
        match CodexParser.read(&locator) {
            Err(ParseError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn session_meta_carries_cwd() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"session_meta","timestamp":"2026-05-12T13:18:53Z","payload":{"id":"sess","cwd":"/p/proj"}}"#,
                r#"{"type":"response_item","timestamp":"2026-05-12T13:19:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.meta.harness, "codex");
        assert_eq!(doc.meta.session_key, "sess");
        assert_eq!(doc.meta.cwd.as_deref(), Some("/p/proj"));
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "hello");
    }

    #[test]
    fn user_and_assistant_messages_become_message_turns() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","timestamp":"2026-05-12T13:19:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"do the thing"}]}}"#,
                r#"{"type":"response_item","timestamp":"2026-05-12T13:19:30Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"sure, on it"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].role, TurnRole::User);
        assert_eq!(doc.turns[0].kind, TurnKind::Message);
        assert_eq!(doc.turns[0].body, "do the thing");
        assert!(doc.turns[0].timestamp.is_some());
        assert_eq!(doc.turns[1].role, TurnRole::Assistant);
        assert_eq!(doc.turns[1].body, "sure, on it");
    }

    #[test]
    fn developer_and_system_messages_are_skipped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<permissions instructions> ..."}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"system","content":[{"type":"input_text","text":"system prompt"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"actual user text"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "actual user text");
    }

    #[test]
    fn function_call_becomes_tool_use_with_name_and_arguments() {
        let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:20:00Z","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}","call_id":"call_1"}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].role, TurnRole::Assistant);
        assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
        assert_eq!(doc.turns[0].body, "exec_command: {\"cmd\":\"ls\"}");
    }

    #[test]
    fn function_call_output_becomes_tool_result() {
        let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:20:01Z","payload":{"type":"function_call_output","call_id":"call_1","output":"hello\nworld"}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::ToolResult);
        assert_eq!(doc.turns[0].body, "hello\nworld");
    }

    #[test]
    fn reasoning_with_summary_text_emits_thinking_turn() {
        let line = r#"{"type":"response_item","timestamp":"2026-05-12T13:21:00Z","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"considering options A and B"}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].role, TurnRole::Assistant);
        assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
        assert_eq!(doc.turns[0].body, "considering options A and B");
    }

    #[test]
    fn reasoning_with_only_encrypted_content_emits_placeholder_turn() {
        // Real-data shape mirroring Claude: codex reasoning records
        // carry the visible thinking in `summary`/`content`, but
        // some carry only `encrypted_content`. Operator's thinking
        // toggle must have visible effect so we emit a placeholder
        // rather than dropping the record. Matches the Claude
        // parser's behaviour.
        let line = r#"{"type":"response_item","payload":{"type":"reasoning","summary":[],"content":null,"encrypted_content":"gAAA..."}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
        assert!(
            doc.turns[0].body.contains("hidden"),
            "placeholder text, got {:?}",
            doc.turns[0].body
        );
    }

    #[test]
    fn event_msg_records_are_skipped_entirely() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"event_msg","payload":{"type":"token_count","tokens":1234}}"#,
                r#"{"type":"event_msg","payload":{"type":"user_message","text":"echo"}}"#,
                r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"real"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "real");
    }

    #[test]
    fn turn_aborted_marker_body_is_dropped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<turn_aborted>"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "hello");
    }

    #[test]
    fn proposed_plan_marker_body_is_dropped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<proposed_plan>"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert!(doc.turns.is_empty());
    }

    #[test]
    fn compacted_record_emits_compaction_summary_turn() {
        let line = r#"{"type":"compacted","timestamp":"2026-05-13T21:54:07Z","payload":{"message":"Summary of prior work.","replacement_history":[]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::CompactionSummary);
        assert_eq!(doc.turns[0].body, "Summary of prior work.");
    }

    #[test]
    fn web_search_call_emits_tool_use_turn() {
        let line = r#"{"type":"response_item","payload":{"type":"web_search_call","name":"web_search","arguments":"{\"query\":\"rust transcript viewer\"}"}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
        assert!(doc.turns[0].body.starts_with("web_search: "));
    }

    #[test]
    fn malformed_and_blank_lines_skip() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"a"}]}}"#,
                r#""#,
                r#"not json"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"b"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].body, "a");
        assert_eq!(doc.turns[1].body, "b");
    }

    #[test]
    fn turn_aborted_event_marks_preceding_user_message_and_partial_response() {
        // Real Codex shape: user message, then a partial reasoning
        // turn, then `event_msg.turn_aborted` because the operator
        // hit Esc. Detection should mark the user message AND the
        // partial reasoning turn so the whole exchange disappears
        // together when `show_aborted` is off.
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first prompt"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first reply"}]}}"#,
                r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"interrupted thought"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"partial reasoning before abort"}]}}"#,
                r#"{"type":"event_msg","payload":{"type":"turn_aborted"}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"replacement prompt"}]}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
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
                ("partial reasoning before abort", true),
                ("replacement prompt", false),
            ],
            "abort flag tags the aborted user prompt and the partial reasoning that landed before Esc",
        );
    }

    #[test]
    fn task_complete_event_clears_abort_candidate() {
        // If `task_complete` fires between a user message and a
        // later `turn_aborted`, the user message must not be
        // retroactively tagged. The abort only applies to the most
        // recent unresolved turn.
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"settled prompt"}]}}"#,
                r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
                r#"{"type":"event_msg","payload":{"type":"turn_aborted"}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert!(
            !doc.turns[0].aborted,
            "task_complete cleared the candidate before turn_aborted fired",
        );
    }

    #[test]
    fn rollout_file_located_under_nested_year_month_day_path() {
        // session_key suffix matching is the real lookup mechanism.
        let (_tmp, locator) = fixture(
            "019df146-41e8-7fb0-8df0-dc326b4fdee8",
            &[
                r#"{"type":"session_meta","payload":{"id":"019df146-41e8-7fb0-8df0-dc326b4fdee8","cwd":"/p"}}"#,
            ],
        );
        let doc = CodexParser.read(&locator).expect("parse");
        assert_eq!(doc.meta.cwd.as_deref(), Some("/p"));
    }
}
