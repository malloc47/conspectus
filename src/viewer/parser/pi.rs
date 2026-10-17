//! Pi JSONL transcript reader.
//!
//! A Pi session is one JSONL file under the session root, either grouped
//! (`<root>/--<encoded-cwd>--/<ts>_<id>.jsonl`) or flat
//! (`<root>/<ts>_<id>.jsonl`). The first line is a `session` header with
//! the cwd. Every later entry carries `id` and `parentId`, so the file is a
//! tree: `/tree` navigation leaves abandoned branches in place. What the
//! operator saw in Pi is the active branch, found by walking `parentId`
//! from the leaf (the last entry) back to the root. Version 1 files predate
//! the tree; their entries have no ids and render in file order.
//!
//! Translation into the normalized model:
//! - `message` / `user` and `assistant`: one turn per content block.
//!   `text` → [`TurnKind::Message`], `thinking` → [`TurnKind::Thinking`],
//!   `toolCall` → [`TurnKind::ToolUse`] (`name: <arguments>`). Images are
//!   skipped.
//! - `message` / `toolResult` → [`TurnKind::ToolResult`].
//! - `compaction` and `branch_summary` → [`TurnKind::CompactionSummary`].
//! - `custom_message` → a system message, hidden unless `display` is true.
//! - An assistant message with `stopReason: "aborted"` (the operator
//!   pressed Esc) and the user prompt before it are tagged `aborted`
//!   (ADR 0053).
//! - Records Pi's UI never shows become `hidden` turns (ADR 0122): the
//!   `system` message, `custom` extension state, `model_change`,
//!   `thinking_level_change`, `label`, `usage`, `context_edit`, and
//!   `session_info`.
//!
//! Malformed lines are skipped. A missing file is
//! [`ParseError::NotFound`].

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};

pub struct PiParser;

impl HarnessParser for PiParser {
    fn read(&self, locator: &SessionLocator) -> ParseResult {
        let path = find_session_file(&locator.state_root, &locator.session_key)
            .ok_or(ParseError::NotFound)?;
        parse_file(&path, locator)
    }
}

/// The session file for `session_key` under `state_root`. A file named
/// `<ts>_<session_key>.jsonl` wins; otherwise the header ids decide, which
/// covers a file whose name and header disagree. `state_root` may also be
/// the session file itself.
fn find_session_file(state_root: &Path, session_key: &str) -> Option<PathBuf> {
    if state_root.is_file() {
        return Some(state_root.to_path_buf());
    }
    let files = session_files(state_root);
    files
        .iter()
        .find(|path| file_name_session_key(path) == Some(session_key))
        .cloned()
        .or_else(|| {
            files
                .into_iter()
                .find(|path| header_id(path).as_deref() == Some(session_key))
        })
}

/// The id in a `<timestamp>_<session-id>.jsonl` file name. Timestamps
/// contain no `_` and custom ids may, so the split is at the first one.
fn file_name_session_key(path: &Path) -> Option<&str> {
    let (timestamp, id) = path.file_stem()?.to_str()?.split_once('_')?;
    (!timestamp.is_empty() && !id.is_empty()).then_some(id)
}

fn session_files(state_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(state_root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let grouped = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.len() > 4 && name.starts_with("--") && name.ends_with("--")
                });
            if grouped && let Ok(children) = std::fs::read_dir(&path) {
                files.extend(children.flatten().map(|child| child.path()));
            }
        } else {
            files.push(path);
        }
    }
    files.retain(|path| path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file());
    files.sort();
    files
}

fn header_id(path: &Path) -> Option<String> {
    let line = BufReader::new(File::open(path).ok()?)
        .lines()
        .next()?
        .ok()?;
    let header: Entry = serde_json::from_str(&line).ok()?;
    (header.entry_type.as_deref() == Some("session"))
        .then_some(header.id)
        .flatten()
}

fn parse_file(path: &Path, locator: &SessionLocator) -> ParseResult {
    let file = File::open(path).map_err(|err| ParseError::Io(err.to_string()))?;
    let mut cwd = None;
    let mut entries = Vec::new();
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Entry>(&line) else {
            continue;
        };
        if entry.entry_type.as_deref() == Some("session") {
            if cwd.is_none() {
                cwd = entry.cwd.clone();
            }
            continue;
        }
        entries.push(entry);
    }

    let branch = active_branch(&entries);
    let aborted = aborted_entries(&branch);
    let turns = branch
        .iter()
        .enumerate()
        .flat_map(|(index, entry)| {
            let mut turns = turns_for_entry(entry);
            if aborted.contains(&index) {
                for turn in &mut turns {
                    turn.aborted = true;
                }
            }
            turns
        })
        .collect();

    Ok(TranscriptDocument {
        meta: TranscriptMeta {
            harness: locator.harness_key().to_string(),
            session_key: locator.session_key().to_string(),
            cwd,
        },
        turns,
    })
}

/// The entries on the path from the root to the leaf, in order. The leaf is
/// the last entry with an id. Files without ids (version 1) are linear and
/// come back whole. A dangling or cyclic `parentId` ends the walk.
fn active_branch(entries: &[Entry]) -> Vec<&Entry> {
    let Some(leaf) = entries.iter().rev().find(|entry| entry.id.is_some()) else {
        return entries.iter().collect();
    };
    let by_id: HashMap<&str, &Entry> = entries
        .iter()
        .filter_map(|entry| entry.id.as_deref().map(|id| (id, entry)))
        .collect();

    let mut path = vec![leaf];
    let mut seen: HashSet<&str> = leaf.id.as_deref().into_iter().collect();
    let mut current = leaf;
    while let Some(parent_id) = current.parent_id.as_deref() {
        if !seen.insert(parent_id) {
            break;
        }
        let Some(parent) = by_id.get(parent_id) else {
            break;
        };
        path.push(parent);
        current = parent;
    }
    path.reverse();
    path
}

/// Positions in `branch` of every assistant message that stopped with
/// `stopReason: "aborted"`, plus the user prompt it was answering. Pi's
/// UI drops the interrupted exchange, so the viewer hides it by default.
fn aborted_entries(branch: &[&Entry]) -> HashSet<usize> {
    let mut aborted = HashSet::new();
    let mut last_user = None;
    for (index, entry) in branch.iter().enumerate() {
        let Some(message) = entry.message.as_ref() else {
            continue;
        };
        match message.role.as_deref() {
            Some("user") => last_user = Some(index),
            Some("assistant") if message.stop_reason.as_deref() == Some("aborted") => {
                aborted.insert(index);
                if let Some(user) = last_user.take() {
                    aborted.insert(user);
                }
            }
            _ => {}
        }
    }
    aborted
}

fn turns_for_entry(entry: &Entry) -> Vec<TranscriptTurn> {
    let timestamp = parse_timestamp(entry.timestamp.as_deref());
    let turn = |role, kind, body: String, hidden| TranscriptTurn {
        role,
        kind,
        body,
        timestamp,
        aborted: false,
        hidden,
    };
    let metadata = |body: String| vec![turn(TurnRole::System, TurnKind::Message, body, true)];

    match entry.entry_type.as_deref() {
        Some("message") => entry
            .message
            .as_ref()
            .map(|message| message_turns(message, timestamp))
            .unwrap_or_default(),
        Some("compaction") => entry
            .summary
            .as_deref()
            .and_then(nonempty)
            .map(|summary| {
                vec![turn(
                    TurnRole::System,
                    TurnKind::CompactionSummary,
                    summary,
                    false,
                )]
            })
            .unwrap_or_default(),
        Some("branch_summary") => entry
            .summary
            .as_deref()
            .and_then(nonempty)
            .map(|summary| {
                vec![turn(
                    TurnRole::System,
                    TurnKind::CompactionSummary,
                    format!("**Branch summary**\n\n{summary}"),
                    false,
                )]
            })
            .unwrap_or_default(),
        Some("custom_message") => entry
            .content
            .as_ref()
            .and_then(content_text)
            .map(|body| {
                vec![turn(
                    TurnRole::System,
                    TurnKind::Message,
                    body,
                    !entry.display.unwrap_or(false),
                )]
            })
            .unwrap_or_default(),
        Some("custom") => metadata(format!(
            "extension state `{}`: {}",
            entry.custom_type.as_deref().unwrap_or("unknown"),
            entry
                .data
                .as_ref()
                .map(Value::to_string)
                .unwrap_or_default(),
        )),
        Some("model_change") => metadata(format!(
            "model: {}/{}",
            entry.provider.as_deref().unwrap_or("?"),
            entry.model_id.as_deref().unwrap_or("?"),
        )),
        Some("thinking_level_change") => metadata(format!(
            "thinking level: {}",
            entry.thinking_level.as_deref().unwrap_or("?"),
        )),
        Some("label") => metadata(match entry.label.as_deref() {
            Some(label) => format!("label `{label}` on {}", target(entry)),
            None => format!("label cleared on {}", target(entry)),
        }),
        Some("usage") => metadata(format!(
            "usage ({}): {}",
            entry.kind.as_deref().unwrap_or("?"),
            entry
                .usage
                .as_ref()
                .map(Value::to_string)
                .unwrap_or_default(),
        )),
        Some("context_edit") => metadata(format!("context edit on {}", target(entry))),
        Some("session_info") => match entry.name.as_deref() {
            Some(name) => metadata(format!("session name: {name}")),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn target(entry: &Entry) -> &str {
    entry.target_id.as_deref().unwrap_or("?")
}

fn message_turns(message: &Message, timestamp: Option<DateTime<Utc>>) -> Vec<TranscriptTurn> {
    let turn = |role, kind, body, hidden| TranscriptTurn {
        role,
        kind,
        body,
        timestamp,
        aborted: false,
        hidden,
    };
    match message.role.as_deref() {
        Some("user") => blocks_or_text(message.content.as_ref())
            .into_iter()
            .filter_map(|block| block_turn(TurnRole::User, &block))
            .map(|(kind, body)| turn(TurnRole::User, kind, body, false))
            .collect(),
        Some("assistant") => blocks_or_text(message.content.as_ref())
            .into_iter()
            .filter_map(|block| block_turn(TurnRole::Assistant, &block))
            .map(|(kind, body)| turn(TurnRole::Assistant, kind, body, false))
            .collect(),
        Some("toolResult") => message
            .content
            .as_ref()
            .and_then(content_text)
            .map(|body| vec![turn(TurnRole::Assistant, TurnKind::ToolResult, body, false)])
            .unwrap_or_default(),
        Some("system") => system_prompt(message)
            .map(|body| vec![turn(TurnRole::System, TurnKind::Message, body, true)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Content as blocks: a plain string becomes one `text` block.
fn blocks_or_text(content: Option<&Value>) -> Vec<Block> {
    match content {
        Some(Value::String(text)) => vec![Block {
            block_type: Some("text".to_string()),
            text: Some(text.clone()),
            ..Block::default()
        }],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn block_turn(role: TurnRole, block: &Block) -> Option<(TurnKind, String)> {
    match block.block_type.as_deref()? {
        "text" => Some((TurnKind::Message, nonempty(block.text.as_deref()?)?)),
        "thinking" if role == TurnRole::Assistant => Some((
            TurnKind::Thinking,
            block
                .thinking
                .as_deref()
                .and_then(nonempty)
                .unwrap_or_else(|| "(reasoning hidden by the model)".to_string()),
        )),
        "toolCall" => {
            let name = block.name.as_deref().unwrap_or("(unnamed tool)");
            Some((
                TurnKind::ToolUse,
                match &block.arguments {
                    Some(arguments) => format!("{name}: {arguments}"),
                    None => name.to_string(),
                },
            ))
        }
        _ => None,
    }
}

/// Text from a string or a list of content blocks, joined; `None` when
/// there's no text.
fn content_text(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    nonempty(text.trim())
}

/// The system prompt: its sections, or the plain content when there are
/// none.
fn system_prompt(message: &Message) -> Option<String> {
    let from_sections = message.sections.as_ref().and_then(|sections| {
        let parts: Vec<String> = sections
            .iter()
            .filter_map(|(name, value)| {
                let text = value.as_str()?.trim();
                (!text.is_empty()).then(|| format!("**{name}**\n\n{text}"))
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    });
    from_sections.or_else(|| message.content.as_ref().and_then(content_text))
}

fn nonempty(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

fn parse_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw?)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

// -- JSONL entry shapes -----------------------------------------------------
//
// One permissive struct covers every entry type; unknown fields are
// ignored and every field defaults.

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "parentId", default)]
    parent_id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    message: Option<Message>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(rename = "customType", default)]
    custom_type: Option<String>,
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    display: Option<bool>,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(rename = "modelId", default)]
    model_id: Option<String>,
    #[serde(rename = "thinkingLevel", default)]
    thinking_level: Option<String>,
    #[serde(rename = "targetId", default)]
    target_id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    usage: Option<Value>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    sections: Option<serde_json::Map<String, Value>>,
    #[serde(rename = "stopReason", default)]
    stop_reason: Option<String>,
}

#[derive(Default, Deserialize)]
struct Block {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<Value>,
}

#[cfg(test)]
#[path = "pi_tests.rs"]
mod tests;
