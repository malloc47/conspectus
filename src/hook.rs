//! Hook observation storage and payload conversion.
//!
//! Harness hooks call `conspectus hook write ...`; this module owns the
//! provider-neutral observation schema and daemonless spool.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u16 = 1;
pub const ACTIVE_TTL_SECONDS: i64 = 15 * 60;
pub const LATEST_FILENAME: &str = "hooks-latest.json";

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
    pub path: PathBuf,
    pub replaced_existing: bool,
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

    pub fn latest_path(&self) -> PathBuf {
        self.root.join(LATEST_FILENAME)
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

        let path = self.latest_path();
        let mut store = LatestHookStore::read(&path);
        let key = mux_record_key(record);
        let replaced_existing = store.records.contains_key(&key);
        let should_write = store
            .records
            .get(&key)
            .is_none_or(|existing| record.observed_epoch >= existing.observed_epoch);
        if should_write {
            store.records.insert(key, record.clone());
            store.write_atomic(&path)?;
            set_user_only_file(&path)?;
        }
        let outcome = HookWriteOutcome {
            path,
            replaced_existing,
        };
        Ok(outcome)
    }

    pub fn read_records(&self) -> Vec<HookRecord> {
        let mut records = LatestHookStore::read(&self.latest_path()).into_records();
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
    pid: Option<i64>,
    ppid: Option<i64>,
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
        pid,
        ppid,
        tmux: tmux.filter(|tmux| !tmux.is_empty()),
        transcript_path: optional_string(payload, "transcript_path"),
        hook_event_name: optional_string(payload, "hook_event_name"),
        observed_epoch,
        harness_version,
    })
}

pub fn codex_record_from_payload(
    payload: &serde_json::Value,
    pid: Option<i64>,
    ppid: Option<i64>,
    tmux: Option<HookTmuxRecord>,
    harness_version: Option<String>,
    observed_epoch: i64,
) -> Result<HookRecord> {
    let Some(session_id) = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
    else {
        bail!("Codex hook payload missing string `session_id`");
    };
    if session_id.is_empty() {
        bail!("Codex hook payload has empty `session_id`");
    }

    Ok(HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "codex".to_string(),
        session_key: session_id.to_string(),
        cwd: optional_string(payload, "cwd"),
        pid,
        ppid,
        tmux: tmux.filter(|tmux| !tmux.is_empty()),
        transcript_path: optional_string(payload, "transcript_path"),
        hook_event_name: optional_string(payload, "hook_event_name"),
        observed_epoch,
        harness_version,
    })
}

/// Build a hook record from an opencode plugin payload.
///
/// The opencode plugin (see `plugins/opencode-hook`) normalizes the SDK
/// `Event` union into a flat shape matching the Claude/Codex writers
/// before piping to stdin. See `H-MUXPROC-014` audit notes in
/// `docs/backlog.md` for the field mapping per `Event` variant.
pub fn opencode_record_from_payload(
    payload: &serde_json::Value,
    pid: Option<i64>,
    ppid: Option<i64>,
    tmux: Option<HookTmuxRecord>,
    harness_version: Option<String>,
    observed_epoch: i64,
) -> Result<HookRecord> {
    let Some(session_id) = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
    else {
        bail!("OpenCode hook payload missing string `session_id`");
    };
    if session_id.is_empty() {
        bail!("OpenCode hook payload has empty `session_id`");
    }

    Ok(HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "opencode".to_string(),
        session_key: session_id.to_string(),
        cwd: optional_string(payload, "cwd"),
        pid,
        ppid,
        tmux: tmux.filter(|tmux| !tmux.is_empty()),
        // opencode sessions are tracked in the project sqlite store rather
        // than a per-session JSONL transcript file. The plugin may still
        // forward an explicit `transcript_path` if a project surface like
        // `info.share.url` or a future field maps onto it; leave the slot
        // open and accept it when present.
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

#[derive(Debug, Default, Deserialize, Serialize)]
struct LatestHookStore {
    schema_version: u16,
    records: BTreeMap<String, HookRecord>,
}

impl LatestHookStore {
    fn read(path: &Path) -> Self {
        if !path.is_file() {
            return Self::default_for_write();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|body| serde_json::from_str::<Self>(&body).ok())
            .filter(|store| store.schema_version == SCHEMA_VERSION)
            .unwrap_or_else(Self::default_for_write)
    }

    fn default_for_write() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            records: BTreeMap::new(),
        }
    }

    fn into_records(self) -> Vec<HookRecord> {
        self.records.into_values().collect()
    }

    fn write_atomic(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create hook state dir {}", parent.display()))?;
        let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
        let body = serde_json::to_vec_pretty(self)?;
        fs::write(&tmp, body).with_context(|| format!("failed to write {}", tmp.display()))?;
        set_user_only_file(&tmp)?;
        fs::rename(&tmp, path)
            .with_context(|| format!("failed to rename {} to {}", tmp.display(), path.display()))
    }
}

fn mux_record_key(record: &HookRecord) -> String {
    if let Some(tmux) = &record.tmux {
        let mux = tmux
            .native_id
            .as_deref()
            .or(tmux.session_name.as_deref())
            .filter(|value| !value.is_empty());
        if let Some(mux) = mux {
            let socket = tmux.socket_path.as_deref().unwrap_or("default");
            return format!("tmux:{socket}:{mux}");
        }
    }
    if let Some(cwd) = record.cwd.as_deref() {
        return format!("cwd:{cwd}");
    }
    format!("session:{}:{}", record.harness_key, record.session_key)
}

fn read_json_records(root: &Path) -> Vec<HookRecord> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter(|path| path.file_name().is_none_or(|name| name != LATEST_FILENAME))
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
    fn latest_store_round_trips_hook_record() {
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

        assert!(!outcome.replaced_existing);
        assert!(outcome.path.is_file());
        assert_eq!(store.read_records(), vec![record]);
    }

    #[test]
    fn latest_store_keeps_only_newest_record_for_mux() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = HookStore::new(temp.path());
        let mut older = HookRecord {
            schema_version: SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: "old".to_string(),
            cwd: Some("/work".to_string()),
            pid: None,
            ppid: None,
            tmux: Some(HookTmuxRecord {
                session_name: Some("editor".to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            transcript_path: None,
            hook_event_name: Some("SessionStart".to_string()),
            observed_epoch: 100,
            harness_version: None,
        };
        let mut newer = older.clone();
        newer.session_key = "new".to_string();
        newer.observed_epoch = 200;

        store.write_record(&newer).expect("write newer");
        older.session_key = "older-late-arrival".to_string();
        store.write_record(&older).expect("write older");

        assert_eq!(store.read_records(), vec![newer]);
    }

    #[test]
    fn claude_payload_requires_session_id() {
        let err = claude_code_record_from_payload(
            &serde_json::json!({"cwd": "/work"}),
            Some(1),
            Some(2),
            None,
            None,
            100,
        )
        .expect_err("missing id");

        assert!(err.to_string().contains("session_id"));
    }

    #[test]
    fn codex_payload_builds_hook_record() {
        let record = codex_record_from_payload(
            &serde_json::json!({
                "session_id": "019e531f-19ee-7823-816f-4526ef89d70b",
                "transcript_path": "/home/me/.codex/sessions/2026/05/23/rollout.jsonl",
                "cwd": "/work",
                "hook_event_name": "SessionStart"
            }),
            Some(10),
            Some(9),
            Some(HookTmuxRecord {
                session_name: Some("editor".to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            Some("0.128.0".to_string()),
            100,
        )
        .expect("record");

        assert_eq!(record.harness_key, "codex");
        assert_eq!(record.session_key, "019e531f-19ee-7823-816f-4526ef89d70b");
        assert_eq!(
            record.transcript_path.as_deref(),
            Some("/home/me/.codex/sessions/2026/05/23/rollout.jsonl")
        );
        assert_eq!(record.cwd.as_deref(), Some("/work"));
        assert_eq!(record.hook_event_name.as_deref(), Some("SessionStart"));
        assert_eq!(record.pid, Some(10));
        assert_eq!(record.ppid, Some(9));
    }

    #[test]
    fn opencode_payload_builds_hook_record() {
        let record = opencode_record_from_payload(
            &serde_json::json!({
                "session_id": "ses_01HZX2J5Y",
                "cwd": "/home/me/src/proj",
                "hook_event_name": "session.updated"
            }),
            Some(42),
            Some(41),
            Some(HookTmuxRecord {
                session_name: Some("work".to_string()),
                native_id: Some("$3".to_string()),
                pane_id: Some("%5".to_string()),
                socket_path: Some("/run/user/1000/tmux-1000/default".to_string()),
            }),
            Some("1.14.19".to_string()),
            1_700_000_500,
        )
        .expect("record");

        assert_eq!(record.harness_key, "opencode");
        assert_eq!(record.session_key, "ses_01HZX2J5Y");
        assert_eq!(record.cwd.as_deref(), Some("/home/me/src/proj"));
        assert_eq!(record.hook_event_name.as_deref(), Some("session.updated"));
        assert_eq!(record.pid, Some(42));
        assert_eq!(record.ppid, Some(41));
        assert_eq!(record.harness_version.as_deref(), Some("1.14.19"));
        assert!(record.transcript_path.is_none());
        let tmux = record.tmux.as_ref().expect("tmux carried through");
        assert_eq!(tmux.pane_id.as_deref(), Some("%5"));
    }

    #[test]
    fn opencode_payload_requires_session_id() {
        let err = opencode_record_from_payload(
            &serde_json::json!({"cwd": "/work"}),
            Some(1),
            Some(2),
            None,
            None,
            100,
        )
        .expect_err("missing id");
        assert!(err.to_string().contains("session_id"));
    }

    #[test]
    fn opencode_payload_rejects_empty_session_id() {
        let err = opencode_record_from_payload(
            &serde_json::json!({"session_id": ""}),
            Some(1),
            Some(2),
            None,
            None,
            100,
        )
        .expect_err("empty id");
        assert!(err.to_string().contains("empty"));
    }
}
