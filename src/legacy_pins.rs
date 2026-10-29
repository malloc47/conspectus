//! Legacy session pins (ADR 0057): `[[pins.entries]]` in a project's
//! `.conspectus.toml` or the user config.
//!
//! Pin slots (ADR 0141) replaced them. Until the operator runs
//! `conspectus pin migrate`, discovery still reads these entries and
//! shows each as an in-memory mux slot, so nothing disappears on
//! upgrade. This module only reads them, converts them to slots, and
//! removes the converted tables; nothing writes new entries. It goes
//! one release after the slots ship (backlog `CSP-690`).

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::declared::write_atomic;
use crate::model::{FieldValue, PinKind, PinSlot, SlotFields};

pub const PINS_SCHEMA_VERSION: u32 = 1;

/// Top-level TOML document carrying the optional `[pins]` table.
///
/// Designed to deserialize cleanly from a config file that may also
/// contain `[session]`, `[declared]`, `[aliases]`, or other unrelated
/// sections — the [`PinsDocument`] only owns the pins slice.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinsDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<PinsSection>,
}

impl PinsDocument {
    pub fn entries(&self) -> &[PinEntry] {
        self.pins
            .as_ref()
            .map(|section| section.entries.as_slice())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinsSection {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<PinEntry>,
}

impl Default for PinsSection {
    fn default() -> Self {
        Self {
            schema_version: PINS_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

/// A single session pin. Mirror of the ADR 0057 schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinEntry {
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux: PinMux,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<PinLaunch>,
    /// When set, the pin is worktree-backed (ADR 0094): `cwd` is the
    /// repo anchor and this branch's worktree is created (if absent)
    /// and entered at launch, alongside the mux + agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PinWorktree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Worktree intent for a worktree-backed pin (ADR 0094). The worktree
/// is realized at launch — resolved from the repo's worktrees, created
/// from the repo default branch if absent, and used as the session's
/// working directory in place of the pin's `cwd` anchor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinWorktree {
    /// Branch the worktree checks out.
    pub branch: String,
}

/// Mux backend coordinates the pin binds against. v1 only accepts
/// `backend = "tmux"`; `socket_name` is tmux-specific and optional
/// (equivalent to `tmux -L <name>`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinMux {
    pub backend: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_name: Option<String>,
}

impl PinMux {
    /// Normalized socket override for binding and launch. Returns
    /// `Some(name)` only when a non-default socket is in effect;
    /// `"default"` collapses to `None` so callers can pass the result
    /// straight into [`crate::discovery::tmux`] without checking the
    /// sentinel everywhere.
    pub fn effective_socket(&self) -> Option<&str> {
        match self.socket_name.as_deref() {
            None | Some("default") => None,
            Some(name) => Some(name),
        }
    }

    /// Mux native id encoding per ADR 0057:
    /// `tmux:<name>` for the default socket (byte-for-byte
    /// compatible with today's `MuxSessionId.native_id`) and
    /// `tmux:<socket>:<name>` for a non-default socket.
    pub fn native_id(&self) -> String {
        match self.effective_socket() {
            None => format!("{}:{}", self.backend, self.name),
            Some(socket) => format!("{}:{}:{}", self.backend, socket, self.name),
        }
    }
}

/// Optional launch overrides. When absent, the harness adapter's
/// [`HarnessAdapter::launch_argv`](crate::discovery::harness)
/// default applies. `env` is reserved in the schema for a follow-up
/// story per ADR 0057 §Open Questions Deferred.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinLaunch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
}

pub fn parse_pins_document(text: &str) -> Result<PinsDocument, PinParseError> {
    let mut document: PinsDocument =
        toml::from_str(text).map_err(|err| PinParseError::MalformedToml(err.to_string()))?;
    canonicalize_document_cwds(&mut document);
    validate_document(&document)?;
    Ok(document)
}

/// Rewrite each entry's `cwd` so a leading `~` or `~/` is expanded to
/// `$HOME`. Applied at every I/O boundary (TOML parse, pin upsert, and
/// store selection) so user-typed shortcuts survive the strict
/// absolute-path check in `validate_entry` and `select_store_for_pin`.
/// Internal callers can rely on `entry.cwd` being absolute once it has
/// passed through any of those boundaries.
fn canonicalize_document_cwds(document: &mut PinsDocument) {
    let Some(section) = document.pins.as_mut() else {
        return;
    };
    for entry in &mut section.entries {
        entry.cwd = expand_home_prefix(&entry.cwd);
    }
}

/// Expand a leading `~` or `~/` in `cwd` to `$HOME`. Returns the input
/// unchanged when `$HOME` is unset or the input does not start with
/// `~`. Only the bare `~` and `~/`-rooted forms are handled; the
/// `~user/...` form is not supported because Conspectus has no way to
/// reach a foreign user's home directory portably.
pub(crate) fn expand_home_prefix(cwd: &str) -> String {
    expand_home_prefix_with(cwd, std::env::var_os("HOME").map(PathBuf::from).as_deref())
}

/// [`expand_home_prefix`] against `home`: the one `~` rule, which the
/// TUI's path fields also check and commit with.
pub(crate) fn expand_home_prefix_with(cwd: &str, home: Option<&Path>) -> String {
    tuikit::path_field::expand_home(cwd, home)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
pub fn to_toml(document: &PinsDocument) -> Result<String, PinSerializeError> {
    toml::to_string_pretty(document).map_err(|err| PinSerializeError(err.to_string()))
}

fn validate_document(document: &PinsDocument) -> Result<(), PinParseError> {
    let Some(section) = &document.pins else {
        return Ok(());
    };

    if section.schema_version != PINS_SCHEMA_VERSION {
        return Err(PinParseError::UnsupportedSchemaVersion(
            section.schema_version,
        ));
    }

    let mut seen_ids = BTreeSet::new();
    let mut seen_mux = BTreeSet::new();
    for entry in &section.entries {
        validate_entry(entry)?;

        if !seen_ids.insert(entry.id.clone()) {
            return Err(PinParseError::DuplicateId(entry.id.clone()));
        }

        let mux_key = mux_triple_key(&entry.mux);
        if !seen_mux.insert(mux_key.clone()) {
            return Err(PinParseError::DuplicateMux {
                id: entry.id.clone(),
                mux: mux_key,
            });
        }
    }

    Ok(())
}

fn validate_entry(entry: &PinEntry) -> Result<(), PinParseError> {
    if entry.id.trim().is_empty() {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "id",
        });
    }
    if entry.display_name.trim().is_empty() {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "display_name",
        });
    }
    if entry.harness.trim().is_empty() {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "harness",
        });
    }
    if entry.cwd.trim().is_empty() {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "cwd",
        });
    }
    if !Path::new(&entry.cwd).is_absolute() {
        return Err(PinParseError::RelativeCwd {
            entry_id: entry.id.clone(),
            cwd: entry.cwd.clone(),
        });
    }
    // Validate against the compile-time registered backend list,
    // so a new backend only needs a `KNOWN_MUX_BACKENDS` entry.
    if !crate::discovery::tmux::KNOWN_MUX_BACKENDS.contains(&entry.mux.backend.as_str()) {
        return Err(PinParseError::UnsupportedMuxBackend {
            entry_id: entry.id.clone(),
            backend: entry.mux.backend.clone(),
        });
    }
    if entry.mux.name.trim().is_empty() {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "mux.name",
        });
    }
    if let Some(socket) = entry.mux.socket_name.as_deref()
        && socket.trim().is_empty()
    {
        return Err(PinParseError::EmptyField {
            entry_id: entry.id.clone(),
            field: "mux.socket_name",
        });
    }
    Ok(())
}

fn mux_triple_key(mux: &PinMux) -> String {
    let socket = mux.socket_name.as_deref().unwrap_or("default");
    format!("{}|{}|{}", mux.backend, socket, mux.name)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinParseError {
    MalformedToml(String),
    UnsupportedSchemaVersion(u32),
    EmptyField {
        entry_id: String,
        field: &'static str,
    },
    RelativeCwd {
        entry_id: String,
        cwd: String,
    },
    UnsupportedMuxBackend {
        entry_id: String,
        backend: String,
    },
    DuplicateId(String),
    DuplicateMux {
        id: String,
        mux: String,
    },
}

impl fmt::Display for PinParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToml(err) => write!(f, "malformed pins TOML: {err}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported pins.schema_version `{version}`")
            }
            Self::EmptyField { entry_id, field } => {
                write!(f, "pin `{entry_id}` has empty `{field}`")
            }
            Self::RelativeCwd { entry_id, cwd } => {
                write!(f, "pin `{entry_id}` cwd `{cwd}` must be an absolute path")
            }
            Self::UnsupportedMuxBackend { entry_id, backend } => {
                // Report the registered backend set
                // instead of a hardcoded "only tmux."
                let registered = crate::discovery::tmux::KNOWN_MUX_BACKENDS.join(", ");
                write!(
                    f,
                    "pin `{entry_id}` has unsupported mux.backend `{backend}` (registered backends: {registered})"
                )
            }
            Self::DuplicateId(id) => write!(f, "duplicate pin id `{id}`"),
            Self::DuplicateMux { id, mux } => {
                write!(f, "pin `{id}` collides with another pin on mux `{mux}`")
            }
        }
    }
}

impl std::error::Error for PinParseError {}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinSerializeError(String);

#[cfg(test)]
impl fmt::Display for PinSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to serialize pins TOML: {}", self.0)
    }
}

#[cfg(test)]
impl std::error::Error for PinSerializeError {}

// ---------------------------------------------------------------------
// Conversion and removal
// ---------------------------------------------------------------------

/// The mux slot a legacy entry stands for: the entry's launch settings
/// become overrides (the mux name, socket, working directory, harness,
/// argv, and worktree branch), its display name the label, and it has
/// no target until `pin migrate` seeds a history record for it.
pub fn entry_to_slot(entry: &PinEntry, order: u32) -> PinSlot {
    let text = |value: &str| FieldValue::Text(value.to_string());
    let mut overrides = SlotFields::new();
    overrides.insert("name".into(), text(&entry.mux.name));
    if let Some(socket) = entry.mux.effective_socket() {
        overrides.insert("socket".into(), text(socket));
    }
    overrides.insert("cwd".into(), text(&entry.cwd));
    overrides.insert("harness".into(), text(&entry.harness));
    if let Some(launch) = &entry.launch
        && !launch.argv.is_empty()
    {
        overrides.insert("argv".into(), FieldValue::List(launch.argv.clone()));
    }
    if let Some(worktree) = &entry.worktree {
        overrides.insert("worktree_branch".into(), text(&worktree.branch));
    }
    let mut snapshot = SlotFields::new();
    snapshot.insert("label".into(), text(&entry.display_name));
    PinSlot {
        id: entry.id.clone(),
        kind: PinKind::Mux,
        target: None,
        pinned_at: None,
        overrides,
        snapshot,
        order,
        legacy: true,
    }
}

/// Remove the entries named by `ids` from the `[pins]` table of the
/// config file at `path`, keeping every other table, comment, and the
/// remaining entries as they were. The `[pins]` table goes when its last
/// entry does, and a file left empty is removed. Returns how many
/// entries were removed.
pub fn remove_entries(path: &Path, ids: &BTreeSet<String>) -> io::Result<usize> {
    let text = fs::read_to_string(path)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|err| io::Error::other(format!("{}: {err}", path.display())))?;
    let Some(entries) = document
        .get_mut("pins")
        .and_then(|pins| pins.as_table_mut())
        .and_then(|pins| pins.get_mut("entries"))
        .and_then(|entries| entries.as_array_of_tables_mut())
    else {
        return Ok(0);
    };
    let mut removed = 0;
    let mut index = 0;
    while index < entries.len() {
        let id = entries
            .get(index)
            .and_then(|entry| entry.get("id"))
            .and_then(|id| id.as_str());
        if id.is_some_and(|id| ids.contains(id)) {
            entries.remove(index);
            removed += 1;
        } else {
            index += 1;
        }
    }
    if removed == 0 {
        return Ok(0);
    }
    if entries.is_empty() {
        document.as_table_mut().remove("pins");
    }
    // A symlinked config belongs to the file it points at (often a
    // dotfiles checkout): empty it rather than unlink the link.
    let is_symlink = fs::symlink_metadata(path)?.file_type().is_symlink();
    if document.as_table().is_empty() && !is_symlink {
        fs::remove_file(path)?;
    } else {
        write_atomic(path, &document.to_string())?;
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "legacy_pins_tests.rs"]
mod tests;
