//! Synthetic harness state fixtures.
//!
//! Test helpers that build the on-disk layouts each harness adapter reads.
//! Fixtures always live under a caller-supplied root – no helper here touches
//! the user's real `~/.codex`, `~/.claude`, `~/.local/share/opencode`,
//! `~/.pi`, or per-repo aider state. The shapes mirror each harness closely enough that the
//! same builders are used by both crate-internal adapter tests and the
//! end-to-end snapshot tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::json;

/// Fixed epoch stamped onto every fixture file's mtime with
/// `File::set_modified`, so adapters read deterministic times
/// through their normal production path.
pub const FIXTURE_MTIME_EPOCH: i64 = 1_700_000_000;

/// Stamp `path` with the fixed fixture mtime so
/// harness discovery can observe a deterministic epoch without
/// argv sniffing. No-op-on-error because integration tests on
/// filesystems that reject `set_modified` (rare — WSL 9p in
/// some configurations) still work with the wall-clock mtime.
pub fn stamp_fixture_mtime(path: &Path) -> Result<()> {
    let file = fs::OpenOptions::new().write(true).open(path)?;
    file.set_modified(UNIX_EPOCH + Duration::from_secs(FIXTURE_MTIME_EPOCH as u64))?;
    Ok(())
}

pub const CODEX_STATE_DIR: &str = "codex";
pub const CLAUDE_CODE_STATE_DIR: &str = "claude";
pub const OPENCODE_STATE_DIR: &str = "opencode";
pub const PI_STATE_DIR: &str = "pi";

/// Common builder rooted at a single temp directory.
pub struct HarnessFixture {
    root: PathBuf,
}

impl HarnessFixture {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn codex_state_root(&self) -> PathBuf {
        self.root.join(CODEX_STATE_DIR)
    }

    pub fn claude_code_state_root(&self) -> PathBuf {
        self.root.join(CLAUDE_CODE_STATE_DIR)
    }

    pub fn opencode_state_root(&self) -> PathBuf {
        self.root.join(OPENCODE_STATE_DIR)
    }

    pub fn pi_state_root(&self) -> PathBuf {
        self.root.join(PI_STATE_DIR)
    }

    pub fn write_codex_session(&self, record: &CodexSessionRecord) -> Result<PathBuf> {
        write_codex_session(&self.codex_state_root(), record)
    }

    pub fn write_claude_code_session(&self, record: &ClaudeCodeSessionRecord) -> Result<PathBuf> {
        write_claude_code_session(&self.claude_code_state_root(), record)
    }

    pub fn write_opencode_session(&self, record: &OpenCodeSessionRecord) -> Result<PathBuf> {
        write_opencode_session(&self.opencode_state_root(), record)
    }

    pub fn write_pi_session(&self, record: &PiSessionRecord) -> Result<PathBuf> {
        write_pi_session(&self.pi_state_root(), record)
    }

    pub fn write_aider_state(&self, repo: impl AsRef<Path>) -> Result<PathBuf> {
        write_aider_state(repo.as_ref())
    }
}

/// Minimal Codex rollout record. Mirrors the first `session_meta` JSONL line
/// found under `~/.codex/sessions/rollout-<id>.jsonl`.
#[derive(Clone, Debug, Default)]
pub struct CodexSessionRecord {
    pub session_id: String,
    pub cwd: Option<String>,
    pub instructions: Option<String>,
    pub timestamp_rfc3339: Option<String>,
    pub forked_from_id: Option<String>,
    /// Trailing assistant `output_text` appended after `session_meta` so the
    /// rollout's `last_message_preview` reader (ADR 0023) has something to
    /// return. Fixtures that omit it produce sessions with no preview text.
    pub assistant_message: Option<String>,
}

impl CodexSessionRecord {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Self::default()
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp_rfc3339 = Some(timestamp.into());
        self
    }

    pub fn with_forked_from(mut self, parent_session_id: impl Into<String>) -> Self {
        self.forked_from_id = Some(parent_session_id.into());
        self
    }

    pub fn with_assistant_message(mut self, text: impl Into<String>) -> Self {
        self.assistant_message = Some(text.into());
        self
    }
}

pub fn write_codex_session(state_root: &Path, record: &CodexSessionRecord) -> Result<PathBuf> {
    let sessions_dir = state_root.join("sessions");
    fs::create_dir_all(&sessions_dir)
        .with_context(|| format!("creating codex sessions dir at {}", sessions_dir.display()))?;
    let path = sessions_dir.join(format!("rollout-{}.jsonl", record.session_id));
    let mut payload = serde_json::Map::new();
    payload.insert("id".into(), json!(record.session_id));

    if let Some(cwd) = &record.cwd {
        payload.insert("cwd".into(), json!(cwd));
    }

    if let Some(instructions) = &record.instructions {
        payload.insert("instructions".into(), json!(instructions));
    }

    if let Some(timestamp) = &record.timestamp_rfc3339 {
        payload.insert("timestamp".into(), json!(timestamp));
    }

    if let Some(parent_id) = &record.forked_from_id {
        payload.insert("forked_from_id".into(), json!(parent_id));
    }

    let entry = json!({ "type": "session_meta", "payload": payload });
    let mut body = format!("{entry}\n");

    if let Some(text) = &record.assistant_message {
        let message = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": text }],
            },
        });
        body.push_str(&format!("{message}\n"));
    }

    fs::write(&path, body)
        .with_context(|| format!("writing codex session at {}", path.display()))?;
    stamp_fixture_mtime(&path).ok();
    Ok(path)
}

/// Minimal Claude Code session record. The state directory mirrors
/// `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`; the first JSONL line
/// carries `sessionId` and `cwd`.
#[derive(Clone, Debug, Default)]
pub struct ClaudeCodeSessionRecord {
    pub session_id: String,
    pub cwd: String,
    pub summary: Option<String>,
    pub timestamp_rfc3339: Option<String>,
    /// Trailing assistant text message appended after the session header so
    /// the JSONL reader's `last_message_preview` extractor (ADR 0023) has
    /// content to return. Fixtures that omit it produce sessions with no
    /// preview text.
    pub assistant_message: Option<String>,
    /// Question-and-answer exchanges written before
    /// [`Self::assistant_message`], each user turn chained to its reply
    /// so the transcript viewer treats none as aborted.
    pub conversation: Vec<(String, String)>,
}

impl ClaudeCodeSessionRecord {
    pub fn new(session_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            cwd: cwd.into(),
            ..Self::default()
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp_rfc3339 = Some(timestamp.into());
        self
    }

    pub fn with_assistant_message(mut self, text: impl Into<String>) -> Self {
        self.assistant_message = Some(text.into());
        self
    }

    /// Append one user question and the assistant's answer.
    pub fn with_exchange(mut self, question: impl Into<String>, answer: impl Into<String>) -> Self {
        self.conversation.push((question.into(), answer.into()));
        self
    }
}

pub fn write_claude_code_session(
    state_root: &Path,
    record: &ClaudeCodeSessionRecord,
) -> Result<PathBuf> {
    let project_dir = state_root.join("projects").join(encode_cwd(&record.cwd));
    fs::create_dir_all(&project_dir).with_context(|| {
        format!(
            "creating claude-code project dir at {}",
            project_dir.display()
        )
    })?;
    let path = project_dir.join(format!("{}.jsonl", record.session_id));

    let mut entry = serde_json::Map::new();
    entry.insert("sessionId".into(), json!(record.session_id));
    entry.insert("cwd".into(), json!(record.cwd));

    if let Some(summary) = &record.summary {
        entry.insert("summary".into(), json!(summary));
    }

    if let Some(timestamp) = &record.timestamp_rfc3339 {
        entry.insert("timestamp".into(), json!(timestamp));
    }

    let mut body = format!("{}\n", serde_json::Value::Object(entry));

    let text_content = |text: &str| json!([{ "type": "text", "text": text }]);
    let mut parent: Option<String> = None;
    for (n, (question, answer)) in record.conversation.iter().enumerate() {
        let user_uuid = format!("{}-user-{n}", record.session_id);
        let assistant_uuid = format!("{}-assistant-{n}", record.session_id);
        let user = json!({
            "type": "user",
            "sessionId": record.session_id,
            "uuid": user_uuid,
            "parentUuid": parent,
            "timestamp": record.timestamp_rfc3339,
            "message": { "role": "user", "content": text_content(question) },
        });
        let assistant = json!({
            "type": "assistant",
            "sessionId": record.session_id,
            "uuid": assistant_uuid,
            "parentUuid": user_uuid,
            "timestamp": record.timestamp_rfc3339,
            "message": { "role": "assistant", "content": text_content(answer) },
        });
        body.push_str(&format!("{user}\n{assistant}\n"));
        parent = Some(assistant_uuid);
    }

    if let Some(text) = &record.assistant_message {
        let message = json!({
            "type": "assistant",
            "sessionId": record.session_id,
            "uuid": format!("{}-assistant-msg", record.session_id),
            "message": {
                "role": "assistant",
                "content": [{ "type": "text", "text": text }],
            },
        });
        body.push_str(&format!("{message}\n"));
    }

    fs::write(&path, body)
        .with_context(|| format!("writing claude-code session at {}", path.display()))?;
    stamp_fixture_mtime(&path).ok();
    Ok(path)
}

/// Encodes a cwd the way Claude Code does (`/` -> `-`).
pub fn encode_cwd(cwd: &str) -> String {
    cwd.replace('/', "-")
}

/// Minimal opencode session record. The state root mirrors
/// `~/.local/share/opencode/storage/session/<id>/info.json`.
#[derive(Clone, Debug, Default)]
pub struct OpenCodeSessionRecord {
    pub session_id: String,
    pub directory: Option<String>,
    pub title: Option<String>,
    pub created_epoch_ms: Option<i64>,
    pub updated_epoch_ms: Option<i64>,
    /// When set, the writer also lays down a modern `opencode.db` with a
    /// matching `session` row and a single `text` part so the sqlite-backed
    /// last-message-preview reader has content to surface.
    pub assistant_message: Option<String>,
}

impl OpenCodeSessionRecord {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Self::default()
        }
    }

    pub fn with_directory(mut self, directory: impl Into<String>) -> Self {
        self.directory = Some(directory.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_created(mut self, epoch_ms: i64) -> Self {
        self.created_epoch_ms = Some(epoch_ms);
        self
    }

    pub fn with_updated(mut self, epoch_ms: i64) -> Self {
        self.updated_epoch_ms = Some(epoch_ms);
        self
    }

    pub fn with_assistant_message(mut self, text: impl Into<String>) -> Self {
        self.assistant_message = Some(text.into());
        self
    }
}

pub fn write_opencode_session(
    state_root: &Path,
    record: &OpenCodeSessionRecord,
) -> Result<PathBuf> {
    let session_dir = state_root
        .join("storage")
        .join("session")
        .join(&record.session_id);
    fs::create_dir_all(&session_dir)
        .with_context(|| format!("creating opencode session dir at {}", session_dir.display()))?;
    let path = session_dir.join("info.json");

    let mut info = serde_json::Map::new();
    info.insert("id".into(), json!(record.session_id));

    if let Some(directory) = &record.directory {
        info.insert("directory".into(), json!(directory));
    }

    if let Some(title) = &record.title {
        info.insert("title".into(), json!(title));
    }

    let mut time = serde_json::Map::new();

    if let Some(created) = record.created_epoch_ms {
        time.insert("created".into(), json!(created));
    }

    if let Some(updated) = record.updated_epoch_ms {
        time.insert("updated".into(), json!(updated));
    }

    if !time.is_empty() {
        info.insert("time".into(), serde_json::Value::Object(time));
    }

    fs::write(&path, serde_json::to_string_pretty(&info)?)
        .with_context(|| format!("writing opencode session at {}", path.display()))?;
    stamp_fixture_mtime(&path).ok();

    if record.assistant_message.is_some() {
        append_opencode_db_entry(state_root, record)?;
    }

    Ok(path)
}

/// Append (or initialize) `state_root/opencode.db` with a `session` row and a
/// single text `part` for the supplied record. Mirrors the schema the
/// production adapter probes for and the `part` shape its preview reader
/// expects (a `text` part whose `data.text` is non-empty). Best-effort
/// idempotent: tables are created on first call and reused thereafter.
fn append_opencode_db_entry(state_root: &Path, record: &OpenCodeSessionRecord) -> Result<()> {
    fs::create_dir_all(state_root)
        .with_context(|| format!("creating opencode state root at {}", state_root.display()))?;
    let db_path = state_root.join("opencode.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("opening opencode sqlite at {}", db_path.display()))?;

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS session (\
                 id TEXT PRIMARY KEY, \
                 directory TEXT, \
                 title TEXT, \
                 parent_id TEXT, \
                 time_created INTEGER, \
                 time_updated INTEGER \
             )",
            [],
        )
        .context("creating opencode session table")?;
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS part (\
                 id TEXT, \
                 message_id TEXT, \
                 session_id TEXT, \
                 time_created INTEGER, \
                 time_updated INTEGER, \
                 data TEXT \
             )",
            [],
        )
        .context("creating opencode part table")?;

    connection
        .execute(
            "INSERT OR REPLACE INTO session \
             (id, directory, title, parent_id, time_created, time_updated) \
             VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            rusqlite::params![
                record.session_id,
                record.directory,
                record.title,
                record.created_epoch_ms,
                record.updated_epoch_ms,
            ],
        )
        .context("inserting opencode session row")?;

    if let Some(text) = &record.assistant_message {
        let data = json!({ "type": "text", "text": text }).to_string();
        let part_id = format!("{}-text-part", record.session_id);
        let message_id = format!("{}-text-message", record.session_id);
        let time_created = record.updated_epoch_ms.or(record.created_epoch_ms);
        connection
            .execute(
                "INSERT OR REPLACE INTO part \
                 (id, message_id, session_id, time_created, time_updated, data) \
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                rusqlite::params![part_id, message_id, record.session_id, time_created, data],
            )
            .context("inserting opencode part row")?;
    }

    Ok(())
}

/// Minimal Pi session record. The state root mirrors
/// `~/.pi/agent/sessions/--<encoded-cwd>--/<timestamp>_<session-id>.jsonl`
/// (or the flat `<timestamp>_<session-id>.jsonl` layout); the first JSONL
/// line is the `session` header carrying `id`, `cwd`, and `parentSession`.
#[derive(Clone, Debug, Default)]
pub struct PiSessionRecord {
    pub session_id: String,
    pub cwd: String,
    pub timestamp_rfc3339: Option<String>,
    /// `/name` display name, written as a trailing `session_info` entry.
    pub name: Option<String>,
    /// Parent session file path, for `/fork` and `/clone` children.
    pub parent_session: Option<String>,
    /// Question-and-answer exchanges, each entry chained to the previous
    /// one through `parentId`.
    pub conversation: Vec<(String, String)>,
    /// Trailing assistant text message for the `last_message_preview`
    /// reader (ADR 0023).
    pub assistant_message: Option<String>,
    /// Write directly under the state root instead of a `--<cwd>--`
    /// directory, as Pi does for a custom session directory.
    pub flat: bool,
}

impl PiSessionRecord {
    pub fn new(session_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            cwd: cwd.into(),
            ..Self::default()
        }
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp_rfc3339 = Some(timestamp.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_parent_session(mut self, path: impl AsRef<Path>) -> Self {
        self.parent_session = Some(path.as_ref().to_string_lossy().into_owned());
        self
    }

    /// Append one user question and the assistant's answer.
    pub fn with_exchange(mut self, question: impl Into<String>, answer: impl Into<String>) -> Self {
        self.conversation.push((question.into(), answer.into()));
        self
    }

    pub fn with_assistant_message(mut self, text: impl Into<String>) -> Self {
        self.assistant_message = Some(text.into());
        self
    }

    pub fn flat(mut self) -> Self {
        self.flat = true;
        self
    }
}

pub fn write_pi_session(state_root: &Path, record: &PiSessionRecord) -> Result<PathBuf> {
    let dir = if record.flat {
        state_root.to_path_buf()
    } else {
        state_root.join(format!("--{}--", encode_pi_cwd(&record.cwd)))
    };
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating pi session dir at {}", dir.display()))?;
    let timestamp = record
        .timestamp_rfc3339
        .as_deref()
        .unwrap_or("2026-01-02T03:04:05.000Z");
    let file_stamp = timestamp.replace([':', '.'], "-");
    let path = dir.join(format!("{file_stamp}_{}.jsonl", record.session_id));

    let mut header = serde_json::Map::new();
    header.insert("type".into(), json!("session"));
    header.insert("version".into(), json!(3));
    header.insert("id".into(), json!(record.session_id));
    header.insert("timestamp".into(), json!(timestamp));
    header.insert("cwd".into(), json!(record.cwd));
    if let Some(parent) = &record.parent_session {
        header.insert("parentSession".into(), json!(parent));
    }
    let mut body = format!("{}\n", serde_json::Value::Object(header));

    let mut parent: Option<String> = None;
    let mut push = |body: &mut String, entry: serde_json::Value| {
        let id = format!("{}-{}", record.session_id, body.lines().count());
        let mut entry = entry;
        entry["id"] = json!(id);
        entry["parentId"] = json!(parent);
        entry["timestamp"] = json!(timestamp);
        body.push_str(&format!("{entry}\n"));
        parent = Some(id);
    };
    let text = |text: &str| json!([{ "type": "text", "text": text }]);
    for (question, answer) in &record.conversation {
        push(
            &mut body,
            json!({ "type": "message", "message": { "role": "user", "content": text(question) } }),
        );
        push(
            &mut body,
            json!({
                "type": "message",
                "message": { "role": "assistant", "content": text(answer), "stopReason": "stop" },
            }),
        );
    }
    if let Some(answer) = &record.assistant_message {
        push(
            &mut body,
            json!({
                "type": "message",
                "message": { "role": "assistant", "content": text(answer), "stopReason": "stop" },
            }),
        );
    }
    if let Some(name) = &record.name {
        push(&mut body, json!({ "type": "session_info", "name": name }));
    }

    fs::write(&path, body).with_context(|| format!("writing pi session at {}", path.display()))?;
    stamp_fixture_mtime(&path).ok();
    Ok(path)
}

/// Encodes a cwd the way Pi does for its session directories: the leading
/// separator is dropped and `/`, `\\`, and `:` become `-`.
pub fn encode_pi_cwd(cwd: &str) -> String {
    cwd.trim_start_matches(['/', '\\'])
        .replace(['/', '\\', ':'], "-")
}

/// Drops the marker files aider leaves inside a repo worktree. Aider does not
/// have central session IDs, so callers synthesize one later from the repo path
/// and chat-history mtime.
pub fn write_aider_state(repo: &Path) -> Result<PathBuf> {
    fs::create_dir_all(repo)
        .with_context(|| format!("creating aider repo dir at {}", repo.display()))?;
    let history = repo.join(".aider.chat.history.md");
    fs::write(&history, "# aider chat history\n")
        .with_context(|| format!("writing aider history at {}", history.display()))?;
    stamp_fixture_mtime(&history).ok();
    let input = repo.join(".aider.input.history");
    fs::write(&input, "")
        .with_context(|| format!("writing aider input history at {}", input.display()))?;
    stamp_fixture_mtime(&input).ok();
    Ok(history)
}

/// Writes a malformed JSONL file at the given path. Used to verify that
/// adapters degrade gracefully on bad records.
pub fn write_malformed(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "creating parent for malformed fixture: {}",
                parent.display()
            )
        })?;
    }

    fs::write(path, "{not valid json\n")
        .with_context(|| format!("writing malformed fixture at {}", path.display()))?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
#[path = "fixtures_tests.rs"]
mod tests;
