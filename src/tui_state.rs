//! TUI state file (F8-013) — persists the last-active view, sort
//! order, and per-view filter/grouping state across `conspectus tui`
//! restarts.
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
//!   "last_view": "sessions",
//!   "sort": "recency",
//!   "view_states": {
//!     "sessions": {
//!       "filter": { "harness": { "any": ["claude-code"] } },
//!       "grouping": { "Sessions": "workspace" }
//!     },
//!     "mux": {
//!       "filter": {},
//!       "grouping": { "Mux": "host" }
//!     }
//!   }
//! }
//! ```
//!
//! Forward-compat rule: read parses into a struct with a flattened
//! `extra` map so unknown fields round-trip through a write
//! unchanged. Missing `sort` / `view_states` keys on old files
//! default to `None` / empty so v1 schema files remain readable.
//!
//! Write semantics: skip-on-unchanged via parse-and-compare so quiet
//! `conspectus tui` runs that never change state produce no mtime
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
use crate::filter::RowFilter;
use crate::tui::{Grouping, Sort, View};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sort: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    view_states: BTreeMap<String, ViewStateRaw>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

/// Per-view persisted state: filter and grouping. Expanded-set,
/// selection, and scroll are intentionally not persisted — they
/// reference per-session graph structure and are meaningless after
/// a restart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ViewStateRaw {
    #[serde(default, skip_serializing_if = "RowFilter::is_empty")]
    filter: RowFilter,
    grouping: Option<Grouping>,
}

/// Snapshot of the full TUI state suitable for serialisation.
/// Carried from `App` → write, and deserialised → `App` on restart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersistedState {
    pub last_view: Option<View>,
    pub sort: Option<Sort>,
    pub view_states: BTreeMap<View, PersistedViewSlot>,
}

/// Per-view saved filter and grouping recovered from the state file.
/// Expanded-set, selection, and scroll are intentionally not persisted
/// — they reference per-session graph structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedViewSlot {
    pub filter: RowFilter,
    pub grouping: Option<Grouping>,
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
/// - The `last_view` value is missing or unknown to this build.
///
/// Never panics; never returns `Err`. Failures collapse to `None` so
/// the caller can fall through to the next precedence rule.
pub fn read_last_view(cache: &TuiStateCache) -> Option<View> {
    let path = cache.path()?;
    let text = fs::read_to_string(&path).ok()?;
    let raw: RawState = serde_json::from_str(&text).ok()?;
    raw.last_view.as_deref().and_then(view_from_snake_case)
}

/// Best-effort read of the full TUI state from disk. Returns `None`
/// when the file is absent or malformed. Individual fields that are
/// unknown to this build (future sort/view names) silently default.
///
/// Persisted view_states whose view name is unknown to this build
/// are skipped — a future Conspectus may write a `"settings"` view;
/// we'd ignore it and let the default view prevail.
pub fn read_tui_state(cache: &TuiStateCache) -> Option<PersistedState> {
    let path = cache.path()?;
    let text = fs::read_to_string(&path).ok()?;
    let raw: RawState = serde_json::from_str(&text).ok()?;
    let sort = raw.sort.as_deref().and_then(sort_from_str);
    let mut view_states = BTreeMap::new();
    for (view_name, vs_raw) in raw.view_states {
        if let Some(view) = view_from_snake_case(&view_name) {
            let grouping = vs_raw
                .grouping
                .or_else(|| Some(Grouping::default_for(view)));
            view_states.insert(
                view,
                PersistedViewSlot {
                    filter: vs_raw.filter,
                    grouping,
                },
            );
        }
    }
    Some(PersistedState {
        last_view: raw.last_view.as_deref().and_then(view_from_snake_case),
        sort,
        view_states,
    })
}

/// Best-effort write of the last-active view. Kept for backward
/// compatibility with the view-switch code path; delegates to
/// [`write_tui_state`] when the caller only has a [`View`] and no
/// other state to write.
///
/// Returns `Ok(())` on a successful write OR a successful skip.
/// Returns `Err(io::Error)` only when the I/O genuinely failed.
pub fn write_last_view(cache: &TuiStateCache, view: View) -> io::Result<()> {
    // Merge with any existing payload so unknown fields round-trip
    // and in-memory-only sort/view_states are preserved.
    let mut raw = load_raw_state(cache)?;
    raw.schema_version = SCHEMA_VERSION;
    raw.last_view = Some(view_to_snake_case(view).to_string());

    write_raw_state(cache, &raw)
}

/// Best-effort write of the full TUI state: last view, sort, and
/// per-view filter/grouping. Skip-on-unchanged so quiet runs do not
/// churn mtime; atomic via tempfile + rename.
///
/// Callers that only know the view can call [`write_last_view`]
/// instead; that function merges with the existing file so the
/// other state fields are preserved.
pub fn write_tui_state(cache: &TuiStateCache, state: &PersistedState) -> io::Result<()> {
    let mut raw = load_raw_state(cache)?;
    raw.schema_version = SCHEMA_VERSION;
    raw.last_view = state.last_view.map(|v| view_to_snake_case(v).to_string());
    raw.sort = state.sort.map(|s| sort_to_str(s).to_string());
    raw.view_states.clear();
    for (view, slot) in &state.view_states {
        raw.view_states.insert(
            view_to_snake_case(*view).to_string(),
            ViewStateRaw {
                filter: slot.filter.clone(),
                grouping: slot.grouping,
            },
        );
    }

    write_raw_state(cache, &raw)
}

fn load_raw_state(cache: &TuiStateCache) -> io::Result<RawState> {
    let path = cache.path().ok_or_else(|| {
        io::Error::other("no state directory resolved (neither $XDG_STATE_HOME nor $HOME set)")
    })?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            Ok(serde_json::from_str::<RawState>(&text).unwrap_or_else(|_| RawState::default()))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(RawState::default()),
        Err(err) => Err(err),
    }
}

fn write_raw_state(cache: &TuiStateCache, raw: &RawState) -> io::Result<()> {
    let path = cache.path().ok_or_else(|| {
        io::Error::other("no state directory resolved (neither $XDG_STATE_HOME nor $HOME set)")
    })?;

    let new_json =
        serde_json::to_string_pretty(raw).map_err(|err| io::Error::other(err.to_string()))?;

    // Skip-on-unchanged: parse-and-compare lets us treat semantically
    // equal records (different field orderings, whitespace) as
    // matches.
    if let Ok(existing) = fs::read_to_string(&path)
        && let Ok(existing_raw) = serde_json::from_str::<RawState>(&existing)
        && existing_raw == *raw
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
            sort: None,
            view_states: BTreeMap::new(),
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

fn sort_to_str(sort: Sort) -> &'static str {
    match sort {
        Sort::Hierarchy => "hierarchy",
        Sort::Recency => "recency",
    }
}

fn sort_from_str(value: &str) -> Option<Sort> {
    match value {
        "hierarchy" => Some(Sort::Hierarchy),
        "recency" => Some(Sort::Recency),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tui_state_tests.rs"]
mod tests;
