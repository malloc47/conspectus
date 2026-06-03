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
    fn supports(&self, locator: &SessionLocator) -> bool {
        matches!(locator, SessionLocator::ClaudeCode { .. })
    }

    fn read(&self, locator: &SessionLocator) -> ParseResult {
        let (state_root, session_key) = match locator {
            SessionLocator::ClaudeCode {
                state_root,
                session_key,
            } => (state_root.as_path(), session_key.as_str()),
            _ => return Err(ParseError::NotFound),
        };
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

    let mut turns: Vec<TranscriptTurn> = Vec::new();
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
        emit_turns_for_record(&record, &mut turns);
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

/// Translate one Claude record into zero or more normalized turns.
fn emit_turns_for_record(record: &Record, out: &mut Vec<TranscriptTurn>) {
    if record.is_compact_summary {
        let body = compaction_summary_body(record);
        if !body.is_empty() {
            out.push(TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::CompactionSummary,
                body,
                timestamp: parse_timestamp(record.timestamp.as_deref()),
            });
        }
        return;
    }

    let role = match record.record_type.as_deref() {
        Some("user") => TurnRole::User,
        Some("assistant") => TurnRole::Assistant,
        // System / custom-title / agent-name / attachment / etc.
        // are metadata records — no turn for the operator.
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
                });
            }
        }
        Some(MessageContent::Blocks(blocks)) => {
            for block in blocks {
                if let Some(turn) = turn_from_block(role, timestamp, block) {
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
            })
        }
        "tool_result" => {
            let body = flatten_tool_result_content(block)?;
            Some(TranscriptTurn {
                role,
                kind: TurnKind::ToolResult,
                body,
                timestamp,
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
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Lay down `<state_root>/projects/<proj>/<session_key>.jsonl`
    /// pre-loaded with `lines`. Returns the tempdir + locator.
    fn fixture(session_key: &str, lines: &[&str]) -> (tempfile::TempDir, SessionLocator) {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().join("projects").join("-home-user-proj");
        fs::create_dir_all(&project).expect("project dir");
        let mut file =
            fs::File::create(project.join(format!("{session_key}.jsonl"))).expect("create jsonl");
        for line in lines {
            writeln!(file, "{line}").expect("write line");
        }
        let locator = SessionLocator::ClaudeCode {
            state_root: dir.path().to_path_buf(),
            session_key: session_key.to_string(),
        };
        (dir, locator)
    }

    #[test]
    fn supports_only_claude_code_locator() {
        let p = ClaudeCodeParser;
        assert!(p.supports(&SessionLocator::ClaudeCode {
            state_root: PathBuf::from("/x"),
            session_key: "k".to_string(),
        }));
        assert!(!p.supports(&SessionLocator::Codex {
            state_root: PathBuf::from("/x"),
            session_key: "k".to_string(),
        }));
        assert!(!p.supports(&SessionLocator::OpenCode {
            db_path: PathBuf::from("/x"),
            session_id: "s".to_string(),
        }));
    }

    #[test]
    fn missing_file_reports_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let locator = SessionLocator::ClaudeCode {
            state_root: dir.path().to_path_buf(),
            session_key: "absent".to_string(),
        };
        match ClaudeCodeParser.read(&locator) {
            Err(ParseError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn plain_user_assistant_exchange_round_trips() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"user","timestamp":"2026-06-01T16:52:36Z","cwd":"/proj","message":{"role":"user","content":"hello"}}"#,
                r#"{"type":"assistant","timestamp":"2026-06-01T16:53:00Z","message":{"role":"assistant","content":[{"type":"text","text":"hi back"}]}}"#,
            ],
        );
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.meta.harness, "claude-code");
        assert_eq!(doc.meta.session_key, "sess");
        assert_eq!(doc.meta.cwd.as_deref(), Some("/proj"));
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].role, TurnRole::User);
        assert_eq!(doc.turns[0].kind, TurnKind::Message);
        assert_eq!(doc.turns[0].body, "hello");
        assert!(doc.turns[0].timestamp.is_some());
        assert_eq!(doc.turns[1].role, TurnRole::Assistant);
        assert_eq!(doc.turns[1].body, "hi back");
    }

    #[test]
    fn assistant_text_and_tool_use_emit_separate_turns() {
        // One assistant record carrying both text and a tool_use
        // block — the normalizer fans them into distinct turns so
        // the renderer can fold tool blocks independently.
        let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:00:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Let me check the file."},{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/x"}}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].kind, TurnKind::Message);
        assert_eq!(doc.turns[0].body, "Let me check the file.");
        assert_eq!(doc.turns[1].kind, TurnKind::ToolUse);
        assert!(
            doc.turns[1].body.starts_with("Read: "),
            "got {:?}",
            doc.turns[1].body
        );
        assert!(doc.turns[1].body.contains("\"file_path\":\"/x\""));
    }

    #[test]
    fn tool_result_with_string_content_is_captured() {
        let line = r#"{"type":"user","timestamp":"2026-06-01T17:01:00Z","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"ok"}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].role, TurnRole::User);
        assert_eq!(doc.turns[0].kind, TurnKind::ToolResult);
        assert_eq!(doc.turns[0].body, "ok");
    }

    #[test]
    fn tool_result_with_text_block_list_content_is_flattened() {
        let line = r#"{"type":"user","timestamp":"2026-06-01T17:02:00Z","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":[{"type":"text","text":"line one"},{"type":"text","text":"line two"}]}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "line one\nline two");
    }

    #[test]
    fn thinking_block_emits_thinking_turn() {
        let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:03:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm, options...","signature":"ignored"}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
        assert_eq!(doc.turns[0].body, "hmm, options...");
    }

    /// Real-data shape: assistant turns where the visible
    /// `thinking` field is empty and the reasoning lives only in
    /// the opaque `encrypted_content` blob. Renderer needs *some*
    /// turn to render so the operator's `y` toggle has visible
    /// effect.
    #[test]
    fn thinking_block_with_only_encrypted_content_emits_placeholder_turn() {
        let line = r#"{"type":"assistant","timestamp":"2026-06-01T17:04:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"","signature":"sig","encrypted_content":"gAAA..."}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::Thinking);
        assert!(
            doc.turns[0].body.contains("hidden"),
            "placeholder text should hint at encrypted reasoning, got {:?}",
            doc.turns[0].body
        );
    }

    #[test]
    fn compaction_summary_record_becomes_compaction_summary_turn() {
        let line = r#"{"type":"user","isCompactSummary":true,"timestamp":"2026-06-01T18:00:00Z","message":{"role":"user","content":"Summary of prior turns."}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::CompactionSummary);
        assert_eq!(doc.turns[0].body, "Summary of prior turns.");
        assert_eq!(doc.turns[0].role, TurnRole::User);
    }

    #[test]
    fn empty_text_blocks_and_empty_bodies_are_dropped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"user","message":{"role":"user","content":""}}"#,
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":""}]}}"#,
            ],
        );
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert!(
            doc.turns.is_empty(),
            "empty bodies should not produce turns, got {:?}",
            doc.turns
        );
    }

    #[test]
    fn non_user_assistant_record_types_are_skipped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"custom-title","customTitle":"debug","sessionId":"sess"}"#,
                r#"{"type":"agent-name","agentName":"main"}"#,
                r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-06-01T18:00:00Z"}"#,
                r#"{"type":"user","message":{"role":"user","content":"the only real turn"}}"#,
            ],
        );
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].body, "the only real turn");
    }

    #[test]
    fn malformed_lines_and_blank_lines_are_skipped() {
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"user","message":{"role":"user","content":"good"}}"#,
                r#""#,
                r#"not json"#,
                r#"{"type":"user","message":{"role":"user","content":"also good"}}"#,
            ],
        );
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].body, "good");
        assert_eq!(doc.turns[1].body, "also good");
    }

    #[test]
    fn cwd_captured_from_first_record_that_has_one() {
        // Common pattern: the first few records (custom-title etc.)
        // omit cwd; the first user record carries it.
        let (_tmp, locator) = fixture(
            "sess",
            &[
                r#"{"type":"custom-title","customTitle":"x"}"#,
                r#"{"type":"user","cwd":"/p/proj","message":{"role":"user","content":"hi"}}"#,
                r#"{"type":"user","cwd":"/p/different","message":{"role":"user","content":"later"}}"#,
            ],
        );
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(
            doc.meta.cwd.as_deref(),
            Some("/p/proj"),
            "first-wins, ignoring later changes"
        );
    }

    #[test]
    fn tool_use_without_input_emits_name_only_body() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"ListDir"}]}}"#;
        let (_tmp, locator) = fixture("sess", &[line]);
        let doc = ClaudeCodeParser.read(&locator).expect("parse");
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].kind, TurnKind::ToolUse);
        assert_eq!(doc.turns[0].body, "ListDir");
    }
}
