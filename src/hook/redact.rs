//! Redaction of a hook payload for the trace (ADR 0242).
//!
//! `conspectus hook write` runs [`redact_payload`] over the whole payload
//! when `CONSPECTUS_HOOK_TRACE` asks for a trace, and writes the result.
//! The rule is default-deny: a value survives only because a rule below
//! names it, so a field no one has seen leaves a placeholder, never text.
//!
//! - Numbers, booleans, and nulls are kept.
//! - A string is replaced by `"<str:N>"`, N its length in characters,
//!   unless its key puts it in one of these classes:
//!   - *enum keys* (`hook_event_name`, `source`, `notification_type`,
//!     `state`, ...) keep a value shaped like a word, up to 48 characters
//!     of `[A-Za-z0-9_.:-]`; `error` is one only beside a `StopFailure`'s
//!     event name, since it is a failure's text anywhere else;
//!   - `tool_name` keeps a word-shaped value, except an MCP tool's name,
//!     which becomes `"mcp__<redacted>"`;
//!   - *identifier keys* (`session_id`, `agent_id`, `agentId`, ...) become
//!     `"id:"` and eight hex digits of a salted SHA-256, so one id keeps
//!     one pseudonym in one trace and has another in the next;
//!   - *path keys* (`cwd`, `transcript_path`, `file_path`, ...) become
//!     `"<path>"`.
//! - The value of a *payload-bearing key* (`prompt`, `tool_input`,
//!   `tool_response`, `message`, ...) is replaced whole by its size:
//!   `"<str:N>"`, `"<object:K keys>"`, or `"<array:K>"`. The one exception
//!   is the Agent tool's result, whose `status` and `agentId` stay.
//! - An object key is kept when it has the shape of an identifier (at most
//!   40 characters of `[A-Za-z0-9_]`, not starting with a digit) and sits
//!   at depth 3 or less; otherwise it reads `"<key>"`. An array keeps its
//!   first 8 elements and an object its first 64 entries, then a marker.
//!
//! A key's class is looked up by its name with case and underscores
//! ignored, so `hookEventName` and `hook_event_name` are one key; only
//! `tool_name` itself is a tool name, since another harness's `toolName`
//! names whatever that harness decides. The hidden key of a value is
//! still classified by its real name.

use std::fmt::Write as _;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Elements of an array kept before the `<+N more>` marker.
const MAX_ARRAY: usize = 8;
/// Entries of an object kept before the `<more>` marker.
const MAX_OBJECT: usize = 64;
/// The deepest object whose keys are shown. The payload's own keys are
/// depth 1, and an array adds none.
const MAX_KEY_DEPTH: usize = 3;
/// The longest key shown.
const MAX_KEY_LEN: usize = 40;
/// The longest value an enum key keeps.
const MAX_ENUM_LEN: usize = 48;

/// The Claude Code tools whose result carries `status` and `agentId`: the
/// subagent tool under its current name and the one it had before.
const AGENT_TOOLS: [&str; 2] = ["Agent", "Task"];

/// Keys, as [`normalized`], whose value is payload: the conversation, a
/// tool's input or output, or text a person wrote.
const PAYLOAD_KEYS: &[&str] = &[
    "prompt",
    "toolinput",
    "toolresponse",
    "message",
    "messages",
    "lastassistantmessage",
    "errordetails",
    "errormessage",
    "compactsummary",
    "custominstructions",
    "title",
    "sessiontitle",
    "permissionsuggestions",
    "sessioncrons",
    "description",
    "command",
    "content",
    "text",
    "body",
    "summary",
    "query",
    "question",
    "questions",
    "options",
    "answer",
    "answers",
    "input",
    "args",
    "output",
    "stdout",
    "stderr",
    "result",
    "plan",
    "instructions",
    "systemprompt",
];

/// Keys, as [`normalized`], whose value is vocabulary: a word the
/// harness's own documentation lists. A version key joins them.
const ENUM_KEYS: &[&str] = &[
    "hookeventname",
    "source",
    "trigger",
    "reason",
    "notificationtype",
    "permissionmode",
    "error",
    "status",
    "state",
    "detail",
    "kind",
    "type",
    "stopreason",
    "errorclass",
    "mode",
    "level",
    "behavior",
];

/// What a key says about the value under it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    /// Nothing: a string here is a placeholder.
    Plain,
    /// Payload: the value is replaced whole by its size.
    Payload,
    /// A word from a harness's vocabulary.
    Enum,
    /// A tool's name.
    ToolName,
    /// An identifier.
    Id,
    /// A file or directory path.
    Path,
}

/// `payload` with every free-text value replaced: see the module
/// documentation. `salt` is the trace's name, which keys the id
/// pseudonyms.
pub fn redact_payload(payload: &Value, salt: &str) -> Value {
    Redactor { salt }.value(Class::Plain, payload, 0)
}

/// The pseudonym of `value` in the trace named `salt`: `id:` and the
/// first 8 hex digits of SHA-256 over `salt`, a NUL byte, and `value`.
pub fn pseudonym(salt: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0u8]);
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::from("id:");
    for byte in &digest[..4] {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The size of `value` as a placeholder, or the value itself when it is a
/// number, a flag, or null.
pub fn size_placeholder(value: &Value) -> Value {
    match value {
        Value::String(text) => str_placeholder(text),
        Value::Object(map) => Value::String(format!("<object:{} keys>", map.len())),
        Value::Array(items) => Value::String(format!("<array:{}>", items.len())),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
    }
}

fn str_placeholder(text: &str) -> Value {
    Value::String(format!("<str:{}>", text.chars().count()))
}

struct Redactor<'a> {
    salt: &'a str,
}

impl Redactor<'_> {
    /// `value` under a key of `class`. `depth` is the number of objects
    /// that hold it.
    fn value(&self, class: Class, value: &Value, depth: usize) -> Value {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
            Value::String(text) => self.string(class, text),
            Value::Array(_) | Value::Object(_) if class == Class::Payload => {
                size_placeholder(value)
            }
            Value::Array(items) => {
                let mut kept: Vec<Value> = items
                    .iter()
                    .take(MAX_ARRAY)
                    .map(|item| self.value(class, item, depth))
                    .collect();
                if items.len() > MAX_ARRAY {
                    kept.push(Value::String(format!(
                        "<+{} more>",
                        items.len() - MAX_ARRAY
                    )));
                }
                Value::Array(kept)
            }
            Value::Object(map) => self.object(map, depth + 1),
        }
    }

    fn string(&self, class: Class, text: &str) -> Value {
        match class {
            Class::Enum if word_shaped(text) => Value::String(text.to_string()),
            Class::ToolName if text.starts_with("mcp__") => {
                Value::String("mcp__<redacted>".to_string())
            }
            Class::ToolName if word_shaped(text) => Value::String(text.to_string()),
            Class::Id => Value::String(pseudonym(self.salt, text)),
            Class::Path => Value::String("<path>".to_string()),
            Class::Plain | Class::Payload | Class::Enum | Class::ToolName => str_placeholder(text),
        }
    }

    /// An object whose keys are at `depth`.
    fn object(&self, map: &Map<String, Value>, depth: usize) -> Value {
        let agent_tool = map
            .get("tool_name")
            .and_then(Value::as_str)
            .is_some_and(|name| AGENT_TOOLS.contains(&name));
        // `error` is a failure's type only in a `StopFailure`; in a failed
        // tool or a failed model request it is the failure's text.
        let stop_failure =
            map.get("hook_event_name").and_then(Value::as_str) == Some("StopFailure");
        let mut out = Map::new();
        let mut hidden = 0usize;
        for (key, value) in map.iter().take(MAX_OBJECT) {
            let shown = if depth <= MAX_KEY_DEPTH && key_shaped(key) {
                key.clone()
            } else {
                hidden += 1;
                if hidden == 1 {
                    "<key>".to_string()
                } else {
                    format!("<key:{hidden}>")
                }
            };
            let redacted = if agent_tool && normalized(key) == "toolresponse" {
                self.agent_result(value, depth)
            } else {
                let class = match classify(key) {
                    Class::Enum if !stop_failure && normalized(key) == "error" => Class::Plain,
                    class => class,
                };
                self.value(class, value, depth)
            };
            out.insert(shown, redacted);
        }
        if map.len() > MAX_OBJECT {
            out.insert("<more>".to_string(), Value::from(map.len() - MAX_OBJECT));
        }
        Value::Object(out)
    }

    /// The Agent tool's `tool_response`: the status and the agent's id,
    /// which the Claude Code mapping reads, and the rest by its size.
    fn agent_result(&self, value: &Value, depth: usize) -> Value {
        let Value::Object(map) = value else {
            return size_placeholder(value);
        };
        let mut out = Map::new();
        let mut others = 0usize;
        for (key, item) in map {
            match key.as_str() {
                "status" => {
                    out.insert(key.clone(), self.value(Class::Enum, item, depth));
                }
                "agentId" => {
                    out.insert(key.clone(), self.value(Class::Id, item, depth));
                }
                _ => others += 1,
            }
        }
        if others > 0 {
            out.insert(
                "<other>".to_string(),
                Value::String(format!("<object:{others} keys>")),
            );
        }
        Value::Object(out)
    }
}

/// What `key` says about its value.
fn classify(key: &str) -> Class {
    let name = normalized(key);
    if PAYLOAD_KEYS.contains(&name.as_str()) {
        Class::Payload
    } else if id_key(key) {
        Class::Id
    } else if path_key(key) {
        Class::Path
    } else if key == "tool_name" {
        Class::ToolName
    } else if ENUM_KEYS.contains(&name.as_str()) || version_key(key) {
        Class::Enum
    } else {
        Class::Plain
    }
}

/// `key` in lowercase without underscores, so `toolName`, `tool_name`,
/// and `ToolName` are one key.
fn normalized(key: &str) -> String {
    key.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// An identifier key: `id`, `instance`, `uuid`, and anything ending
/// `_id`, `_ids`, `_uuid`, `Id`, `Ids`, `ID`, `IDs`, or `UUID`.
fn id_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "id" | "ids" | "instance" | "uuid"
    ) || [
        "_id", "_ids", "_uuid", "Id", "Ids", "ID", "IDs", "Uuid", "UUID",
    ]
    .iter()
    .any(|suffix| key.ends_with(suffix))
}

/// A version key: `version`, and anything ending `_version` or `Version`.
fn version_key(key: &str) -> bool {
    key == "version" || key.ends_with("_version") || key.ends_with("Version")
}

/// A path key: `cwd`, `path`, `dir`, `directory`, and anything ending
/// `_path`, `_dir`, `_directory`, `Path`, `Dir`, or `Directory`, each
/// also in the plural.
fn path_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "cwd" | "pwd" | "path" | "paths" | "dir" | "dirs" | "directory" | "workdir"
    ) || [
        "_path",
        "_paths",
        "_dir",
        "_dirs",
        "_directory",
        "Path",
        "Paths",
        "Dir",
        "Dirs",
        "Directory",
    ]
    .iter()
    .any(|suffix| key.ends_with(suffix))
}

/// Whether `key` may be shown: an identifier of at most 40 characters.
fn key_shaped(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && key.len() <= MAX_KEY_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether `text` looks like a word from a vocabulary: 1 to 48 characters
/// of `[A-Za-z0-9_.:-]`.
fn word_shaped(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_ENUM_LEN
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;
