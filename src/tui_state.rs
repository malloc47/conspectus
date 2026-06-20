//! TUI state file (F8-013) — persists the last-active view across
//! `conspectus tui` restarts.
//!
//! Lives at `$XDG_STATE_HOME/conspectus/tui-state.json` (sibling to
//! the hook state-root at `$XDG_STATE_HOME/conspectus/hooks/`). The
//! file is **rebuildable cache**, not authoritative state: nothing
//! relies on its existence to function; the worst case is the
//! operator starts in their configured default view instead of the
//! one they were last in.
//!
//! Shape on disk:
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "last_view": "sessions"
//! }
//! ```
//!
//! Forward-compat rule: read parses into a struct with a flattened
//! `extra` map so unknown fields round-trip through a write
//! unchanged. Future schema bumps that add fields will not strand
//! old writes on downgrade.
//!
//! Write semantics: skip-on-unchanged via parse-and-compare so quiet
//! `conspectus tui` runs that never switch views produce no mtime
//! churn. Atomic via the shared `write_atomic` helper (tempfile +
//! rename).
//!
//! Read invariant: only `conspectus tui` reads or writes this file.
//! Every other surface (`graph`, `table`, `query`, `node show`,
//! `pin *`) must leave it byte-identical. Enforced by
//! `tests/cli_tui_state_invariants.rs`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::declared::write_atomic;
use crate::tui::View;

/// Schema version baked into every write. Forward-compat reads
/// accept any value but only act on the values they understand.
pub const SCHEMA_VERSION: u32 = 1;

/// One JSON record on disk. `extra` holds any fields the current
/// schema does not recognize so a future schema bump that adds
/// fields does not lose them on downgrade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RawState {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_view: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

/// Resolver for the `tui-state.json` file path. Mirrors the
/// pin-binding cache resolver so the env contract is uniform across
/// state-file consumers.
#[derive(Clone, Debug, Default)]
pub struct TuiStateCache {
    home: Option<PathBuf>,
    xdg_state_home: Option<PathBuf>,
}

impl TuiStateCache {
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

    /// Directory that hosts the state file. Returns `None` only when
    /// neither `$XDG_STATE_HOME` nor `$HOME` is set.
    pub fn directory(&self) -> Option<PathBuf> {
        if let Some(xdg) = &self.xdg_state_home {
            return Some(xdg.join("conspectus"));
        }
        self.home
            .as_ref()
            .map(|home| home.join(".local").join("state").join("conspectus"))
    }

    /// Full path to the state file, or `None` when no base directory
    /// is known.
    pub fn path(&self) -> Option<PathBuf> {
        self.directory().map(|dir| dir.join("tui-state.json"))
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// Best-effort read of the last-active view. Returns `None` when:
/// - The state file is absent.
/// - The file exists but is malformed JSON.
/// - The `last_view` value is missing or unknown to this build (a
///   future Conspectus might write `"settings"`; we'd treat that as
///   absent and let the configured default win).
///
/// Never panics; never returns `Err`. Failures collapse to `None` so
/// the caller can fall through to the next precedence rule
/// (`[tui] default_view` then `View::Sessions`).
pub fn read_last_view(cache: &TuiStateCache) -> Option<View> {
    let path = cache.path()?;
    let text = fs::read_to_string(&path).ok()?;
    let raw: RawState = serde_json::from_str(&text).ok()?;
    raw.last_view.as_deref().and_then(view_from_snake_case)
}

/// Best-effort write of the last-active view. Skip-on-unchanged so
/// quiet runs do not churn mtime; atomic via tempfile + rename.
///
/// Returns `Ok(())` on a successful write OR a successful skip.
/// Returns `Err(io::Error)` only when the I/O genuinely failed; the
/// caller (`App::switch_view`) logs at debug and continues — losing
/// the persisted view is not a fatal condition.
pub fn write_last_view(cache: &TuiStateCache, view: View) -> io::Result<()> {
    let path = cache.path().ok_or_else(|| {
        io::Error::other("no state directory resolved (neither $XDG_STATE_HOME nor $HOME set)")
    })?;

    // Merge with any existing payload so unknown fields round-trip.
    let mut raw = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<RawState>(&text).unwrap_or_else(|_| RawState::default()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => RawState::default(),
        Err(err) => return Err(err),
    };
    raw.schema_version = SCHEMA_VERSION;
    raw.last_view = Some(view_to_snake_case(view).to_string());

    let new_json =
        serde_json::to_string_pretty(&raw).map_err(|err| io::Error::other(err.to_string()))?;

    // Skip-on-unchanged: parse-and-compare lets us treat semantically
    // equal records (different field orderings, whitespace) as
    // matches.
    if let Ok(existing) = fs::read_to_string(&path)
        && let Ok(existing_raw) = serde_json::from_str::<RawState>(&existing)
        && existing_raw == raw
    {
        return Ok(());
    }

    write_atomic(&path, &new_json)
}

impl Default for RawState {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            last_view: None,
            extra: BTreeMap::new(),
        }
    }
}

fn view_to_snake_case(view: View) -> &'static str {
    match view {
        View::Sessions => "sessions",
        View::Mux => "mux",
        View::Union => "union",
        View::Prs => "prs",
        View::Forks => "forks",
    }
}

fn view_from_snake_case(value: &str) -> Option<View> {
    match value {
        "sessions" => Some(View::Sessions),
        "mux" => Some(View::Mux),
        "union" => Some(View::Union),
        "prs" => Some(View::Prs),
        "forks" => Some(View::Forks),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn cache_in(dir: &TempDir) -> TuiStateCache {
        TuiStateCache::default().with_xdg_state_home(dir.path())
    }

    #[test]
    fn read_returns_none_when_file_absent() {
        let dir = TempDir::new().expect("tempdir");
        assert!(read_last_view(&cache_in(&dir)).is_none());
    }

    #[test]
    fn round_trip_persists_the_view() {
        let dir = TempDir::new().expect("tempdir");
        let cache = cache_in(&dir);
        write_last_view(&cache, View::Mux).expect("write");
        assert_eq!(read_last_view(&cache), Some(View::Mux));
    }

    #[test]
    fn write_skips_when_payload_matches() {
        let dir = TempDir::new().expect("tempdir");
        let cache = cache_in(&dir);
        write_last_view(&cache, View::Prs).expect("first write");
        let mtime_before = mtime(&cache);
        // Sleep so a real overwrite would show up as a different
        // mtime on coarse-grained filesystems.
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_last_view(&cache, View::Prs).expect("second write");
        let mtime_after = mtime(&cache);
        assert_eq!(
            mtime_before, mtime_after,
            "skip-on-unchanged must not touch the file"
        );
    }

    #[test]
    fn malformed_json_collapses_to_none_on_read() {
        let dir = TempDir::new().expect("tempdir");
        let cache = cache_in(&dir);
        let path = cache.path().expect("path");
        fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        fs::write(&path, "not json").expect("write garbage");
        assert!(read_last_view(&cache).is_none());
    }

    #[test]
    fn unknown_view_value_collapses_to_none() {
        let dir = TempDir::new().expect("tempdir");
        let cache = cache_in(&dir);
        let path = cache.path().expect("path");
        fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        fs::write(&path, r#"{"schema_version": 1, "last_view": "settings"}"#)
            .expect("write future view");
        assert!(read_last_view(&cache).is_none());
    }

    #[test]
    fn unknown_fields_round_trip_through_write() {
        let dir = TempDir::new().expect("tempdir");
        let cache = cache_in(&dir);
        let path = cache.path().expect("path");
        fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        fs::write(
            &path,
            r#"{"schema_version": 2, "last_view": "mux", "extra_field": "preserved"}"#,
        )
        .expect("seed");
        write_last_view(&cache, View::Sessions).expect("write");
        let after = fs::read_to_string(&path).expect("read");
        assert!(
            after.contains("\"extra_field\""),
            "unknown field must round-trip: {after}",
        );
        assert!(
            after.contains("\"sessions\""),
            "new view must persist: {after}",
        );
    }

    #[test]
    fn directory_prefers_xdg_state_home_then_falls_back_to_home() {
        let dir = TempDir::new().expect("tempdir");
        let with_xdg = TuiStateCache::default()
            .with_xdg_state_home(dir.path())
            .with_home(dir.path().join("home"));
        assert_eq!(
            with_xdg.directory(),
            Some(dir.path().join("conspectus")),
            "xdg wins",
        );
        let xdg_absent = TuiStateCache::default().with_home(dir.path().join("home"));
        assert_eq!(
            xdg_absent.directory(),
            Some(
                dir.path()
                    .join("home")
                    .join(".local")
                    .join("state")
                    .join("conspectus"),
            ),
        );
    }

    fn mtime(cache: &TuiStateCache) -> std::time::SystemTime {
        fs::metadata(cache.path().expect("path"))
            .expect("metadata")
            .modified()
            .expect("mtime")
    }
}
