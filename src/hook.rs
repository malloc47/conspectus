//! Hook observation storage and payload conversion.
//!
//! Harness hooks call `conspectus hook write ...`; this module owns the
//! provider-neutral observation schema and storage backend.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u16 = 1;
pub const ACTIVE_TTL_SECONDS: i64 = 15 * 60;
pub const SQLITE_FILENAME: &str = "hooks.sqlite3";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookRecord {
    pub schema_version: u16,
    pub harness_key: String,
    pub session_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux: Option<HookTmuxRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_event_name: Option<String>,
    pub observed_epoch: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_version: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookTmuxRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookWriteOutcome {
    pub database_path: PathBuf,
    pub record_id: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookStore {
    root: PathBuf,
}

impl HookStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_env() -> Option<Self> {
        default_root().map(Self::new)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database_path(&self) -> PathBuf {
        self.root.join(SQLITE_FILENAME)
    }

    pub fn write_record(&self, record: &HookRecord) -> Result<HookWriteOutcome> {
        if record.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported hook schema_version `{}`; expected `{SCHEMA_VERSION}`",
                record.schema_version
            );
        }
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create hook state dir {}", self.root.display()))?;
        set_user_only_dir(&self.root)?;

        let database_path = self.database_path();
        let connection = Connection::open(&database_path)
            .with_context(|| format!("failed to open hook store {}", database_path.display()))?;
        connection.busy_timeout(std::time::Duration::from_millis(250))?;
        init_database(&connection)?;

        let body = serde_json::to_string(record)?;
        connection.execute(
            "INSERT INTO hook_records (
                schema_version, harness_key, session_key, observed_epoch, record_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.schema_version,
                record.harness_key,
                record.session_key,
                record.observed_epoch,
                body
            ],
        )?;
        let outcome = HookWriteOutcome {
            database_path,
            record_id: connection.last_insert_rowid(),
        };
        set_user_only_file(&outcome.database_path)?;
        Ok(outcome)
    }

    pub fn read_records(&self) -> Vec<HookRecord> {
        let mut records = read_sqlite_records(&self.database_path());
        records.extend(read_json_records(&self.root));
        records
    }
}

pub fn default_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CONSPECTUS_HOOK_SIDECAR_STATE") {
        return Some(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(path).join("conspectus/hooks"));
    }

    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("conspectus")
            .join("hooks")
    })
}

pub fn current_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

pub fn claude_code_record_from_payload(
    payload: &serde_json::Value,
    pid: i64,
    ppid: i64,
    tmux: Option<HookTmuxRecord>,
    harness_version: Option<String>,
    observed_epoch: i64,
) -> Result<HookRecord> {
    let Some(session_id) = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
    else {
        bail!("Claude Code hook payload missing string `session_id`");
    };
    if session_id.is_empty() {
        bail!("Claude Code hook payload has empty `session_id`");
    }

    Ok(HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: session_id.to_string(),
        cwd: optional_string(payload, "cwd"),
        pid: Some(pid),
        ppid: Some(ppid),
        tmux: tmux.filter(|tmux| !tmux.is_empty()),
        transcript_path: optional_string(payload, "transcript_path"),
        hook_event_name: optional_string(payload, "hook_event_name"),
        observed_epoch,
        harness_version,
    })
}

impl HookTmuxRecord {
    pub fn is_empty(&self) -> bool {
        self.session_name.is_none()
            && self.native_id.is_none()
            && self.pane_id.is_none()
            && self.socket_path.is_none()
    }
}

fn optional_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn init_database(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS hook_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            schema_version INTEGER NOT NULL,
            harness_key TEXT NOT NULL,
            session_key TEXT NOT NULL,
            observed_epoch INTEGER NOT NULL,
            record_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_hook_records_fresh
            ON hook_records(harness_key, session_key, observed_epoch DESC);
        ",
    )
}

fn read_sqlite_records(path: &Path) -> Vec<HookRecord> {
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };
    let Ok(mut statement) = connection
        .prepare("SELECT record_json FROM hook_records ORDER BY observed_epoch ASC, id ASC")
    else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
        return Vec::new();
    };

    rows.filter_map(std::result::Result::ok)
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect()
}

fn read_json_records(root: &Path) -> Vec<HookRecord> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect()
}

#[cfg(unix)]
fn set_user_only_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to chmod {}", path.display()))
}

#[cfg(not(unix))]
fn set_user_only_dir(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_user_only_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to chmod {}", path.display()))
}

#[cfg(not(unix))]
fn set_user_only_file(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_store_round_trips_hook_record() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = HookStore::new(temp.path());
        let record = HookRecord {
            schema_version: SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: "session-1".to_string(),
            cwd: Some("/work".to_string()),
            pid: Some(1),
            ppid: Some(2),
            tmux: Some(HookTmuxRecord {
                session_name: Some("editor".to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            transcript_path: Some("/tmp/transcript.jsonl".to_string()),
            hook_event_name: Some("SessionStart".to_string()),
            observed_epoch: 1_700_000_000,
            harness_version: Some("1.0.0".to_string()),
        };

        let outcome = store.write_record(&record).expect("write record");

        assert_eq!(outcome.record_id, 1);
        assert!(outcome.database_path.is_file());
        assert_eq!(store.read_records(), vec![record]);
    }

    #[test]
    fn claude_payload_requires_session_id() {
        let err = claude_code_record_from_payload(
            &serde_json::json!({"cwd": "/work"}),
            1,
            2,
            None,
            None,
            100,
        )
        .expect_err("missing id");

        assert!(err.to_string().contains("session_id"));
    }
}
