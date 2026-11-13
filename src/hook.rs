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
pub const LATEST_FILENAME: &str = "hooks-latest.json";
/// The most records the spool keeps. A write past it drops the records
/// observed longest ago. One record per pane has no natural bound, since
/// every pane a hook ever ran in gets a slot.
pub const MAX_SPOOL_RECORDS: usize = 256;

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
    /// The harness process that ran the hook is a background process
    /// (a shared service or helper daemon) rather than an interactive
    /// pane's agent. Its environment, and so the writer's tmux context,
    /// belongs to whichever client started it, so such a record never
    /// produces mux links. The opencode 2.x shared service is one: its
    /// server plugins run inside it with the first client's `TMUX_PANE`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background_harness: bool,
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
        let mut store = match LatestHookStore::read(&path) {
            SpoolRead::Records(store) => store,
            SpoolRead::Newer(version) => bail!(
                "{} has hook schema_version `{version}`, newer than this binary's \
                 `{SCHEMA_VERSION}`; left untouched",
                path.display()
            ),
        };
        let key = spool_slot(record);
        let replaced_existing = store.records.contains_key(&key);
        let should_write = store
            .records
            .get(&key)
            .is_none_or(|existing| record.observed_epoch >= existing.observed_epoch);
        if should_write {
            store.records.insert(key, record.clone());
            store.trim_to(MAX_SPOOL_RECORDS);
            store.write_atomic(&path)?;
            set_user_only_file(&path)?;
        }
        let outcome = HookWriteOutcome {
            path,
            replaced_existing,
        };
        Ok(outcome)
    }

    /// The spool's records, the newest per slot, then the legacy
    /// per-event records beside it. A spool from a newer schema reads as
    /// empty.
    pub fn read_records(&self) -> Vec<HookRecord> {
        let mut records = match LatestHookStore::read(&self.latest_path()) {
            SpoolRead::Records(store) => store.into_records(),
            SpoolRead::Newer(_) => Vec::new(),
        };
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

/// Build a hook sidecar record from a harness's SessionStart
/// hook payload. Looks up the harness key against
/// the adapter registry and delegates to
/// `HarnessAdapter::hook_record_from_payload`.
///
/// Unknown keys yield an error listing the registered set so
/// misconfigured harness hooks fail loudly instead of silently
/// producing a record with a non-registered `harness_key`
/// (which the discovery pipeline would then ignore).
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

/// Read a payload field as an owned non-empty string. Public so
/// `HarnessAdapter::hook_record_from_payload` implementations share
/// the payload convention (skip empties, materialize the value).
pub fn optional_payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// The spool file: `{schema_version, records: {slot -> record}}`.
///
/// The slot a record sits in is derived from the record itself
/// ([`spool_slot`]), never trusted from the file, so a spool written
/// under an older key scheme reads correctly: each record moves to its
/// own slot, the newest one per slot wins, and the next accepted write
/// stores them under the derived keys. The schema version stays 1, so a
/// binary from before the pane slots reads the file too and adds its
/// record under its own key without dropping any other.
#[derive(Debug, Default, Deserialize, Serialize)]
struct LatestHookStore {
    schema_version: u16,
    records: BTreeMap<String, HookRecord>,
}

/// What a spool file holds, as this binary can use it.
enum SpoolRead {
    /// The records, each in its slot. A missing, unreadable, or corrupt
    /// file reads as an empty store, which the next write replaces.
    Records(LatestHookStore),
    /// A spool a newer binary wrote with a higher schema version: its
    /// records are not read, and it is never overwritten.
    Newer(u16),
}

/// Just the version of a spool file, read before its records.
#[derive(Deserialize)]
struct SpoolHeader {
    schema_version: u16,
}

impl LatestHookStore {
    fn read(path: &Path) -> SpoolRead {
        let Ok(body) = fs::read_to_string(path) else {
            return SpoolRead::Records(Self::default_for_write());
        };
        if let Ok(header) = serde_json::from_str::<SpoolHeader>(&body)
            && header.schema_version > SCHEMA_VERSION
        {
            return SpoolRead::Newer(header.schema_version);
        }
        let store = serde_json::from_str::<Self>(&body)
            .ok()
            .filter(|store| store.schema_version == SCHEMA_VERSION)
            .map_or_else(Self::default_for_write, Self::into_slots);
        SpoolRead::Records(store)
    }

    fn default_for_write() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            records: BTreeMap::new(),
        }
    }

    /// Move every record to the slot it belongs in, keeping the newest
    /// per slot; on a tie, the record already stored under its slot.
    fn into_slots(self) -> Self {
        let mut records: BTreeMap<String, HookRecord> = BTreeMap::new();
        for (stored_key, record) in self.records {
            let slot = spool_slot(&record);
            let keep = records.get(&slot).is_none_or(|kept| {
                record.observed_epoch > kept.observed_epoch
                    || (record.observed_epoch == kept.observed_epoch && stored_key == slot)
            });
            if keep {
                records.insert(slot, record);
            }
        }
        Self {
            schema_version: self.schema_version,
            records,
        }
    }

    /// Drop the records observed longest ago until at most `max` remain.
    fn trim_to(&mut self, max: usize) {
        let excess = self.records.len().saturating_sub(max);
        if excess == 0 {
            return;
        }
        let mut by_age: Vec<(i64, String)> = self
            .records
            .iter()
            .map(|(slot, record)| (record.observed_epoch, slot.clone()))
            .collect();
        by_age.sort();
        for (_, slot) in by_age.into_iter().take(excess) {
            self.records.remove(&slot);
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

/// The spool slot `record` occupies; a newer record in the same slot
/// replaces it.
///
/// - A pane of a mux session: `tmux:{socket}:{session}:pane:{pane}`, so
///   agents in split panes of one session keep separate records.
/// - A mux session with no pane: `tmux:{socket}:{session}`, one slot per
///   session, as attribution treats such records.
/// - Outside a mux: the harness process (`pid:{pid}`), else the
///   directory (`cwd:{cwd}`), else the session itself.
fn spool_slot(record: &HookRecord) -> String {
    if let Some(tmux) = &record.tmux {
        let mux = tmux
            .native_id
            .as_deref()
            .or(tmux.session_name.as_deref())
            .filter(|value| !value.is_empty());
        if let Some(mux) = mux {
            let socket = tmux.socket_path.as_deref().unwrap_or("default");
            return match tmux.pane_id.as_deref().filter(|pane| !pane.is_empty()) {
                Some(pane) => format!("tmux:{socket}:{mux}:pane:{pane}"),
                None => format!("tmux:{socket}:{mux}"),
            };
        }
    }
    if let Some(pid) = record.pid {
        return format!("pid:{pid}");
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
