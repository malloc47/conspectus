//! The legacy pin-store registry (ADR 0090), read-only.
//!
//! Old builds recorded the project `.conspectus.toml` files that held
//! `[[pins.entries]]` in `$XDG_STATE_HOME/conspectus/pin-stores.json`,
//! so a pin in a repo outside the scan roots stayed visible. Nothing
//! writes it now, but until `conspectus pin migrate` has converted the
//! entries, discovery still reads it to find them. It goes one release
//! after the slots ship (backlog `CSP-690`).

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// The registry file: the project stores it recorded.
#[derive(Debug, Default, Deserialize)]
struct RawRegistry {
    #[serde(default)]
    stores: Vec<String>,
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
}

fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}
