//! A small multi-harness transcript corpus for the scan tests.
//!
//! Message bodies are chosen to stress the byte prefilter: quotes,
//! backslashes, newlines, slashes, non-ASCII, and spaces are all stored
//! escaped (or as `\uXXXX`) in the JSON on disk.

use std::fs;
use std::io::Write;
use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::viewer::model::SessionLocator;
use crate::viewer::parser::claude_code::ClaudeCodeParser;
use crate::viewer::parser::codex::CodexParser;
use crate::viewer::parser::opencode::OpenCodeParser;
use crate::viewer::parser::pi::PiParser;

use super::{ScanSession, SessionKey};

/// Bodies that appear in the corpus, one per interesting escaping case.
pub const BODIES: &[&str] = &[
    "say \"hello\" to the world",
    "C:\\Users\\dev\\notes.txt",
    "line one\nline two",
    "caf\u{e9} \u{fc}n\u{ef}code \u{65e5}\u{672c}\u{8a9e}",
    "see path/to/file.rs for details",
    "a needle in a haystack",
    "tab\there and emoji \u{1f600} done",
];

pub struct Corpus {
    pub dir: tempfile::TempDir,
    pub sessions: Vec<ScanSession>,
}

/// Serialize `value`, writing every non-ASCII character as `\uXXXX` (as
/// some encoders do) and `/` as `\/`.
pub fn escape_everything(value: &Value) -> String {
    let compact = serde_json::to_string(value).expect("serialize");
    let mut out = String::with_capacity(compact.len());
    for c in compact.chars() {
        match c {
            '/' => out.push_str("\\/"),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out
}

fn write_lines(path: &Path, lines: &[String]) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let mut file = fs::File::create(path).expect("create");
    for line in lines {
        writeln!(file, "{line}").expect("write");
    }
}

fn locator(harness: &str, key: &str, root: &Path) -> SessionLocator {
    SessionLocator {
        harness_key: harness.to_string(),
        session_key: key.to_string(),
        state_root: root.to_path_buf(),
    }
}

fn session(
    harness: &str,
    key: &str,
    root: &Path,
    parser: &'static dyn crate::viewer::parser::HarnessParser,
) -> ScanSession {
    ScanSession {
        key: SessionKey {
            harness_key: harness.to_string(),
            session_key: key.to_string(),
        },
        locator: locator(harness, key, root),
        parser,
    }
}

/// `plain` writes JSON as the standard encoder does; otherwise every
/// non-ASCII character and slash is escaped.
fn encode(value: &Value, plain: bool) -> String {
    if plain {
        serde_json::to_string(value).expect("serialize")
    } else {
        escape_everything(value)
    }
}

fn claude_lines(bodies: &[&str], plain: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        let (kind, role) = if i.is_multiple_of(2) {
            ("user", "user")
        } else {
            ("assistant", "assistant")
        };
        let content = if role == "user" {
            json!(body)
        } else {
            json!([{"type": "text", "text": body}])
        };
        lines.push(encode(
            &json!({
                "type": kind,
                "uuid": format!("u{i}"),
                "parentUuid": if i == 0 { Value::Null } else { json!(format!("u{}", i - 1)) },
                "timestamp": format!("2026-06-01T16:5{i}:00Z"),
                "cwd": "/proj",
                "message": {"role": role, "content": content},
            }),
            plain,
        ));
    }
    lines
}

fn codex_lines(bodies: &[&str], plain: bool) -> Vec<String> {
    let mut lines = vec![encode(
        &json!({"type":"session_meta","timestamp":"2026-05-12T13:18:53Z","payload":{"id":"x","cwd":"/p"}}),
        plain,
    )];
    for (i, body) in bodies.iter().enumerate() {
        let (role, kind) = if i.is_multiple_of(2) {
            ("user", "input_text")
        } else {
            ("assistant", "output_text")
        };
        lines.push(encode(
            &json!({
                "type": "response_item",
                "timestamp": format!("2026-05-12T13:2{i}:00Z"),
                "payload": {"type":"message","role":role,"content":[{"type":kind,"text":body}]},
            }),
            plain,
        ));
    }
    lines
}

fn pi_lines(key: &str, bodies: &[&str], plain: bool) -> Vec<String> {
    let mut lines = vec![encode(
        &json!({"type":"session","version":3,"id":key,"cwd":"/work"}),
        plain,
    )];
    for (i, body) in bodies.iter().enumerate() {
        let role = if i.is_multiple_of(2) {
            "user"
        } else {
            "assistant"
        };
        let content = if role == "user" {
            json!(body)
        } else {
            json!([{"type": "text", "text": body}])
        };
        lines.push(encode(
            &json!({
                "type": "message",
                "id": format!("m{i}"),
                "parentId": if i == 0 { Value::Null } else { json!(format!("m{}", i - 1)) },
                "timestamp": format!("2026-05-12T13:2{i}:00Z"),
                "message": {"role": role, "content": content, "stopReason": "stop"},
            }),
            plain,
        ));
    }
    lines
}

/// Create the OpenCode database at `db` with the 1.x layout and one
/// session per entry of `sessions`, each holding its bodies as text parts.
fn opencode_db(db: &Path, sessions: &[(&str, &[&str])]) {
    let conn = Connection::open(db).expect("open db");
    conn.execute_batch(
        "CREATE TABLE session (id text PRIMARY KEY, directory text NOT NULL, \
           title text NOT NULL DEFAULT '', time_created integer NOT NULL DEFAULT 0);
         CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, \
           time_created integer NOT NULL, data text NOT NULL);
         CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, \
           session_id text NOT NULL, time_created integer NOT NULL, data text NOT NULL);",
    )
    .expect("schema");
    for (key, bodies) in sessions {
        conn.execute(
            "INSERT INTO session (id, directory, time_created) VALUES (?1, '/oc', 1)",
            params![key],
        )
        .expect("session");
        for (i, body) in bodies.iter().enumerate() {
            let role = if i.is_multiple_of(2) {
                "user"
            } else {
                "assistant"
            };
            let message = format!("{key}-m{i}");
            conn.execute(
                "INSERT INTO message VALUES (?1, ?2, ?3, ?4)",
                params![
                    message,
                    key,
                    100 + i as i64,
                    json!({"role": role, "time": {"created": 100 + i as i64}}).to_string()
                ],
            )
            .expect("message");
            conn.execute(
                "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("{message}-p"),
                    message,
                    key,
                    100 + i as i64,
                    json!({"type": "text", "text": body}).to_string()
                ],
            )
            .expect("part");
        }
    }
}

/// Two sessions per file-based harness (one stored the way the standard
/// encoder writes it, one fully escaped), a mixed one for opencode, and a
/// session each with no matching text.
pub fn corpus() -> Corpus {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let mut sessions = Vec::new();

    let claude = root.join("claude");
    for (key, plain) in [("claude-plain", true), ("claude-escaped", false)] {
        write_lines(
            &claude.join("projects/p1").join(format!("{key}.jsonl")),
            &claude_lines(BODIES, plain),
        );
        sessions.push(session("claude-code", key, &claude, &ClaudeCodeParser));
    }
    write_lines(
        &claude.join("projects/p1/claude-quiet.jsonl"),
        &claude_lines(&["nothing to see", "really nothing"], true),
    );
    sessions.push(session(
        "claude-code",
        "claude-quiet",
        &claude,
        &ClaudeCodeParser,
    ));

    let codex = root.join("codex");
    for (key, plain) in [("codex-plain", true), ("codex-escaped", false)] {
        write_lines(
            &codex
                .join("sessions/2026/05/12")
                .join(format!("rollout-2026-05-12T13-18-53-{key}.jsonl")),
            &codex_lines(BODIES, plain),
        );
        sessions.push(session("codex", key, &codex, &CodexParser));
    }

    let pi = root.join("pi");
    for (key, plain) in [("pi-plain", true), ("pi-escaped", false)] {
        write_lines(
            &pi.join("--work--")
                .join(format!("2026-05-12T13-18-53-000Z_{key}.jsonl")),
            &pi_lines(key, BODIES, plain),
        );
        sessions.push(session("pi", key, &pi, &PiParser));
    }

    let db = root.join("opencode.db");
    opencode_db(
        &db,
        &[
            ("ses_all", BODIES),
            ("ses_quiet", &["nothing here", "still nothing"]),
        ],
    );
    for key in ["ses_all", "ses_quiet"] {
        sessions.push(session("opencode", key, &db, &OpenCodeParser));
    }

    Corpus { dir, sessions }
}

/// A deterministic pseudo-random sentence generator for the benchmark.
pub struct Words(u64);

impl Words {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn text(&mut self, words: usize) -> String {
        const VOCAB: &[&str] = &[
            "the",
            "build",
            "fails",
            "when",
            "cache",
            "is",
            "stale",
            "because",
            "of",
            "a",
            "missing",
            "lockfile",
            "and",
            "retry",
            "with",
            "--verbose",
            "to",
            "see",
            "why",
            "src/main.rs",
            "error:",
            "cannot",
            "find",
            "type",
            "`Session`",
            "in",
            "scope",
            "refactor",
            "module",
            "tests",
            "pass",
            "locally",
            "but",
            "fail",
            "on",
            "CI",
            "caf\u{e9}",
            "\"quoted\"",
            "C:\\dir",
            "line\nbreak",
        ];
        let mut out = String::new();
        for i in 0..words {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(VOCAB[(self.next() % VOCAB.len() as u64) as usize]);
        }
        out
    }
}
