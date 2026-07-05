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

// H-HYG-001: `hook::current_epoch` re-exports the canonical
// helper from `crate::discovery::current_epoch`. Pre-H-HYG-001
// there were 5 verbatim copies scattered across the codebase;
// this preserves the public call path
// (`conspectus::hook::current_epoch()`) while collapsing the
// body to a single definition.
pub use crate::discovery::current_epoch;

/// Build a hook sidecar record from a harness's SessionStart
/// hook payload (H-EXT-005). Looks up the harness key against
/// the adapter registry and delegates to
/// `HarnessAdapter::hook_record_from_payload`.
///
/// Unknown keys yield an error listing the registered set so
/// misconfigured harness hooks fail loudly instead of silently
/// producing a record with a non-registered `harness_key`
/// (which the discovery pipeline would then ignore).
///
/// This replaces the pre-H-EXT-005 per-harness
/// `claude_code_record_from_payload` / `codex_record_from_payload`
/// / `opencode_record_from_payload` writers; every caller now
/// funnels through the registry.
pub fn hook_record_from_payload(
    harness_key: &str,
    payload: &serde_json::Value,
    pid: Option<i64>,
    ppid: Option<i64>,
    tmux: Option<HookTmuxRecord>,
    harness_version: Option<String>,
    observed_epoch: i64,
) -> Result<HookRecord> {
    for adapter in crate::discovery::harness::registered_adapters() {
        if adapter.harness_key() == harness_key {
            return adapter.hook_record_from_payload(
                payload,
                pid,
                ppid,
                tmux,
                harness_version,
                observed_epoch,
            );
        }
    }
    let registered: Vec<&'static str> = crate::discovery::harness::harness_keys().to_vec();
    bail!(
        "unknown harness key `{harness_key}`; registered: {}",
        registered.join(", ")
    );
}

impl HookTmuxRecord {
    pub fn is_empty(&self) -> bool {
        self.session_name.is_none()
            && self.native_id.is_none()
            && self.pane_id.is_none()
            && self.socket_path.is_none()
    }
}

/// Read a payload field as an owned non-empty string.
/// H-EXT-005 promotes this from a module-private helper to
/// `pub` so `HarnessAdapter::hook_record_from_payload` can call
/// it from `crate::discovery::harness` — the same payload
/// convention (skip empties, materialize the value) applies to
/// every harness's hook payload.
pub fn optional_payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
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
#[path = "hook_tests.rs"]
mod tests;
