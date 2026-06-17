//! Synthetic harness state fixtures.
//!
//! Test helpers that build the on-disk layouts each harness adapter reads.
//! Fixtures always live under a caller-supplied root – no helper here touches
//! the user's real `~/.codex`, `~/.claude`, `~/.local/share/opencode`, or
//! per-repo aider state. The shapes mirror each harness closely enough that the
//! same builders are used by both crate-internal adapter tests and the
//! end-to-end snapshot tests.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::json;

pub const CODEX_STATE_DIR: &str = "codex";
pub const CLAUDE_CODE_STATE_DIR: &str = "claude";
pub const OPENCODE_STATE_DIR: &str = "opencode";

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

    pub fn write_codex_session(&self, record: &CodexSessionRecord) -> Result<PathBuf> {
        write_codex_session(&self.codex_state_root(), record)
    }

    pub fn write_claude_code_session(&self, record: &ClaudeCodeSessionRecord) -> Result<PathBuf> {
        write_claude_code_session(&self.claude_code_state_root(), record)
    }

    pub fn write_opencode_session(&self, record: &OpenCodeSessionRecord) -> Result<PathBuf> {
        write_opencode_session(&self.opencode_state_root(), record)
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

/// Drops the marker files aider leaves inside a repo worktree. Aider does not
/// have central session IDs, so callers synthesize one later from the repo path
/// and chat-history mtime.
pub fn write_aider_state(repo: &Path) -> Result<PathBuf> {
    fs::create_dir_all(repo)
        .with_context(|| format!("creating aider repo dir at {}", repo.display()))?;
    let history = repo.join(".aider.chat.history.md");
    fs::write(&history, "# aider chat history\n")
        .with_context(|| format!("writing aider history at {}", history.display()))?;
    let input = repo.join(".aider.input.history");
    fs::write(&input, "")
        .with_context(|| format!("writing aider input history at {}", input.display()))?;
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fixture_paths_live_under_supplied_root() {
        let temp = TempDir::new().expect("temp dir");
        let fixture = HarnessFixture::at(temp.path());

        assert!(fixture.codex_state_root().starts_with(temp.path()));
        assert!(fixture.claude_code_state_root().starts_with(temp.path()));
        assert!(fixture.opencode_state_root().starts_with(temp.path()));
    }

    #[test]
    fn codex_session_writes_session_meta_with_cwd_and_timestamp() {
        let temp = TempDir::new().expect("temp dir");
        let fixture = HarnessFixture::at(temp.path());

        let path = fixture
            .write_codex_session(
                &CodexSessionRecord::new("11111111")
                    .with_cwd("/work/repo")
                    .with_timestamp("2026-01-02T03:04:05Z"),
            )
            .expect("write codex session");

        let body = fs::read_to_string(&path).expect("read codex session");
        let parsed: serde_json::Value =
            serde_json::from_str(body.lines().next().expect("first line")).expect("parse jsonl");

        assert_eq!(parsed["type"], "session_meta");
        assert_eq!(parsed["payload"]["id"], "11111111");
        assert_eq!(parsed["payload"]["cwd"], "/work/repo");
        assert_eq!(parsed["payload"]["timestamp"], "2026-01-02T03:04:05Z");
    }

    #[test]
    fn codex_session_omits_optional_fields_when_absent() {
        let temp = TempDir::new().expect("temp dir");
        let fixture = HarnessFixture::at(temp.path());

        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("only-id"))
            .expect("write codex session");

        let body = fs::read_to_string(&path).expect("read codex session");
        let parsed: serde_json::Value =
            serde_json::from_str(body.lines().next().expect("first line")).expect("parse jsonl");

        assert_eq!(parsed["payload"]["id"], "only-id");
        assert!(parsed["payload"].get("cwd").is_none());
        assert!(parsed["payload"].get("timestamp").is_none());
    }

    #[test]
    fn claude_code_session_encodes_cwd_in_project_dir() {
        let temp = TempDir::new().expect("temp dir");
        let fixture = HarnessFixture::at(temp.path());

        let path = fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("uuid-a", "/work/repo"))
            .expect("write claude session");

        assert_eq!(path.file_name().unwrap(), "uuid-a.jsonl");
        let project_dir = path.parent().expect("parent");
        assert_eq!(project_dir.file_name().unwrap(), "-work-repo");
    }

    #[test]
    fn opencode_session_records_time_fields() {
        let temp = TempDir::new().expect("temp dir");
        let fixture = HarnessFixture::at(temp.path());

        let path = fixture
            .write_opencode_session(
                &OpenCodeSessionRecord::new("session-1")
                    .with_directory("/work/repo")
                    .with_created(1_700_000_000_000)
                    .with_updated(1_700_000_500_000),
            )
            .expect("write opencode session");

        let info: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read info")).expect("parse");

        assert_eq!(info["directory"], "/work/repo");
        assert_eq!(info["time"]["created"], 1_700_000_000_000_i64);
        assert_eq!(info["time"]["updated"], 1_700_000_500_000_i64);
    }

    #[test]
    fn aider_state_writes_history_marker_files() {
        let temp = TempDir::new().expect("temp dir");
        let repo = temp.path().join("repo");

        let history = write_aider_state(&repo).expect("write aider state");

        assert!(history.exists());
        assert!(repo.join(".aider.input.history").exists());
    }

    #[test]
    fn malformed_record_is_unparseable() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("nested").join("bad.jsonl");

        write_malformed(&path).expect("write malformed");

        let body = fs::read_to_string(&path).expect("read bad");
        assert!(serde_json::from_str::<serde_json::Value>(&body).is_err());
    }
}
