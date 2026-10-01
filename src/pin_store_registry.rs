//! Pin-store registry sidecar (ADR 0090).
//!
//! Records the on-disk locations of project `.conspectus.toml` files
//! that hold `[[pins.entries]]`, so a pin created in a repo that is not
//! part of the active scan root — and whose mux session isn't running
//! yet, so no discovered node points into the repo — stays visible on
//! later discovery cycles instead of vanishing.
//!
//! Lives at `$XDG_STATE_HOME/conspectus/pin-stores.json` (sibling to
//! the TUI state file at `.../tui-state.json`). Shape on disk:
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "stores": [
//!     "/home/you/src/repoA/.conspectus.toml",
//!     "/home/you/work/repoB/.conspectus.toml"
//!   ]
//! }
//! ```
//!
//! This is a **rebuildable cache**, not authoritative state (ADR 0087
//! category 2): deleting it only means out-of-root pins stop showing
//! until they're written again; it carries no operator intent
//! Conspectus authored (that lives in the `[[pins.entries]]` sections
//! themselves) and no payload (ADR 0086 Tier 3). Only project stores
//! are recorded — the user-scope store is always consulted directly by
//! the pin loader, so it never needs an entry here.
//!
//! Forward-compat: reads parse into a struct with a flattened `extra`
//! map so unknown fields round-trip through a write unchanged. Writes
//! are best-effort, idempotent, skip-on-unchanged, and atomic via the
//! shared `write_atomic` helper (tempfile + rename). Reads never error:
//! a missing or malformed file collapses to an empty list.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::PROJECT_CONFIG_FILENAME;
use crate::declared::write_atomic;

/// Schema version baked into every write. Forward-compat reads accept
/// any value but only act on the fields they understand.
pub const SCHEMA_VERSION: u32 = 1;

/// One JSON record on disk. `extra` holds any fields the current
/// schema does not recognize so a future schema bump that adds fields
/// does not lose them on downgrade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RawRegistry {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stores: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl Default for RawRegistry {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            stores: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// Resolver for the `pin-stores.json` file path. Mirrors
/// [`crate::tui_state::TuiStateCache`] so the env contract is uniform
/// across state-file consumers.
#[derive(Clone, Debug, Default)]
pub struct PinStoreRegistry {
    home: Option<PathBuf>,
    xdg_state_home: Option<PathBuf>,
}

impl PinStoreRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a resolver populated from the process environment.
    pub fn from_env() -> Self {
        Self {
            home: env_path("HOME"),
            xdg_state_home: env_path("XDG_STATE_HOME"),
        }
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn with_xdg_state_home(mut self, xdg: impl Into<PathBuf>) -> Self {
        self.xdg_state_home = Some(xdg.into());
        self
    }

    /// Directory that hosts the registry file. Returns `None` only when
    /// neither `$XDG_STATE_HOME` nor `$HOME` is set.
    pub fn directory(&self) -> Option<PathBuf> {
        if let Some(xdg) = &self.xdg_state_home {
            return Some(xdg.join("conspectus"));
        }
        self.home
            .as_ref()
            .map(|home| home.join(".local").join("state").join("conspectus"))
    }

    /// Full path to the registry file, or `None` when no base directory
    /// is known.
    pub fn path(&self) -> Option<PathBuf> {
        self.directory().map(|dir| dir.join("pin-stores.json"))
    }

    /// Best-effort read of registered store paths that still exist on
    /// disk. Never errors: a missing base directory, absent file, or
    /// malformed JSON all collapse to an empty list. Paths that no
    /// longer resolve to a file are filtered out so callers never try
    /// to read a dead store.
    pub fn read(&self) -> Vec<PathBuf> {
        let Some(path) = self.path() else {
            return Vec::new();
        };
        let Ok(text) = fs::read_to_string(&path) else {
            return Vec::new();
        };
        let Ok(raw) = serde_json::from_str::<RawRegistry>(&text) else {
            return Vec::new();
        };
        raw.stores
            .into_iter()
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .collect()
    }

    /// Record a project pin-store path so later discovery cycles find
    /// it even when the repo is outside the scan root.
    ///
    /// Best-effort and idempotent. Only project stores (a path whose
    /// file name is [`PROJECT_CONFIG_FILENAME`]) are recorded; the
    /// user-scope config store is a no-op because discovery always
    /// consults it directly. Non-absolute paths are ignored so the
    /// on-disk list stays unambiguous. Registered paths that no longer
    /// resolve to a file are pruned on write so the registry
    /// self-heals as repos come and go.
    ///
    /// Returns `Ok(())` on a successful write OR a successful skip
    /// (nothing to record / unchanged). Returns `Err(io::Error)` only
    /// when the I/O genuinely failed; callers treat that as non-fatal.
    pub fn record(&self, store_path: &Path) -> io::Result<()> {
        let is_project_store =
            store_path.file_name().and_then(|n| n.to_str()) == Some(PROJECT_CONFIG_FILENAME);
        if !is_project_store || !store_path.is_absolute() {
            return Ok(());
        }
        let value = store_path.to_string_lossy().into_owned();

        let mut raw = self.load_raw()?;
        // Prune dead entries first so a stale path doesn't linger, then
        // add the new one (which we know exists — the caller just wrote
        // it) and keep the list sorted + deduped for stable output.
        raw.stores.retain(|s| Path::new(s).is_file());
        if !raw.stores.iter().any(|s| s == &value) {
            raw.stores.push(value);
        }
        raw.stores.sort();
        raw.stores.dedup();
        raw.schema_version = SCHEMA_VERSION;

        self.write_raw(&raw)
    }

    fn load_raw(&self) -> io::Result<RawRegistry> {
        let path = self.path().ok_or_else(no_state_dir_err)?;
        match fs::read_to_string(&path) {
            Ok(text) => Ok(serde_json::from_str::<RawRegistry>(&text).unwrap_or_default()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(RawRegistry::default()),
            Err(err) => Err(err),
        }
    }

    fn write_raw(&self, raw: &RawRegistry) -> io::Result<()> {
        let path = self.path().ok_or_else(no_state_dir_err)?;
        let new_json =
            serde_json::to_string_pretty(raw).map_err(|err| io::Error::other(err.to_string()))?;

        // Skip-on-unchanged: semantically equal records (field order,
        // whitespace) count as a match so quiet runs don't churn mtime.
        if let Ok(existing) = fs::read_to_string(&path)
            && let Ok(existing_raw) = serde_json::from_str::<RawRegistry>(&existing)
            && existing_raw == *raw
        {
            return Ok(());
        }

        write_atomic(&path, &new_json)
    }
}

fn no_state_dir_err() -> io::Error {
    io::Error::other("no state directory resolved (neither $XDG_STATE_HOME nor $HOME set)")
}

fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "pin_store_registry_tests.rs"]
mod tests;
