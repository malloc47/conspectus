//! Session-pin TOML file model + store-selection + write helpers.
//!
//! Per ADR 0057, Conspectus stores user-authored *session pins* —
//! declarations of a logical agent session — as a sibling
//! `[[pins.entries]]` table in the existing TOML config files.
//! Pins are the third user-intent write surface alongside
//! [`crate::declared`] (relationships) and [`crate::aliases`]
//! (node attributes).
//!
//! This module owns:
//! - schema parsing, validation, and TOML round-trip (H-PIN-002),
//! - store selection — picking the right `.conspectus.toml` for a
//!   given pin cwd (H-PIN-005),
//! - atomic read-modify-write helpers for upsert / remove of pin
//!   entries (H-PIN-006).
//!
//! Discovery, resolver binding, and the CLI / TUI surfaces live in
//! their own modules per the H-PIN-* backlog. The atomic-write
//! primitive is shared with [`crate::declared`] via
//! `declared::write_atomic` so all three user-intent surfaces (pins,
//! declared links, aliases) use the same temp-file-and-rename
//! durability rules.
//!
//! The parse layer is strict: malformed entries surface as
//! [`PinParseError`] and the discovery layer is responsible for
//! deciding whether to skip an offending file (matching how
//! [`crate::declared`] and [`crate::aliases`] degrade today).

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use crate::declared::write_atomic;

pub const PINS_SCHEMA_VERSION: u32 = 1;

/// The v1 mux backend. ADR 0057 reserves the field so future backends
/// (`zellij`, …) can land additively without breaking existing files.
pub const TMUX_MUX_BACKEND: &str = "tmux";

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

impl PinLaunch {
    pub fn is_empty(&self) -> bool {
        self.argv.is_empty()
    }
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

pub(crate) fn expand_home_prefix_with(cwd: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return cwd.to_string();
    };
    if cwd == "~" {
        return home.to_string_lossy().into_owned();
    }
    if let Some(rest) = cwd.strip_prefix("~/") {
        return home.join(rest).to_string_lossy().into_owned();
    }
    cwd.to_string()
}

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
    // H-EXT-009: validate against the compile-time registered
    // backend list instead of the historical "only tmux" match.
    // A future backend (H-EXT-010 zellij) becomes a
    // `KNOWN_MUX_BACKENDS` entry and lands here automatically.
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
                // H-EXT-009: report the registered backend set
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinSerializeError(String);

impl fmt::Display for PinSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to serialize pins TOML: {}", self.0)
    }
}

impl std::error::Error for PinSerializeError {}

// ---------------------------------------------------------------------
// Store selection (H-PIN-005)
// ---------------------------------------------------------------------

/// Where an upsert / remove operation should land when the operator
/// does not pass an explicit `--store` override. Always
/// [`PinStoreKind::Project`] for `select_store_for_pin` since pins
/// are inherently cwd-rooted; the user-store path is reached via
/// [`user_pin_store`] only when the CLI's `--store user` override
/// is in play.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinStoreSelection {
    pub kind: PinStoreKind,
    pub path: PathBuf,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PinStoreKind {
    Project,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinStoreSelectionError {
    /// `cwd` was not an absolute path. Mirrors the schema rule from
    /// `PinParseError::RelativeCwd` so the write layer refuses what
    /// the load layer would refuse.
    RelativeCwd { cwd: String },
    /// `cwd` did not exist at write time. ADR 0057 §Store Selection
    /// and Provenance: pins differ from declared links and aliases
    /// by requiring an existing cwd — a pin without one has nothing
    /// to launch.
    CwdMissing { cwd: String },
    /// `--store user` was requested but the loader could not derive
    /// a user config path (no `$HOME`, no `$XDG_CONFIG_HOME`).
    NoUserConfigPath,
}

impl fmt::Display for PinStoreSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeCwd { cwd } => write!(f, "pin cwd `{cwd}` must be an absolute path"),
            Self::CwdMissing { cwd } => write!(
                f,
                "pin cwd `{cwd}` does not exist on the filesystem at write time"
            ),
            Self::NoUserConfigPath => write!(
                f,
                "no user config path could be derived (set $HOME or $XDG_CONFIG_HOME)"
            ),
        }
    }
}

impl std::error::Error for PinStoreSelectionError {}

/// Pick the project-local store a pin with `cwd` should write to.
/// Walks upward from `cwd` looking for an existing `.conspectus.toml`
/// per [`ConfigLoader::locate_project_config`]; falls back to
/// `<cwd>/.conspectus.toml` when none is found upstream so the cwd
/// itself becomes the project owner on first write.
///
/// Pin rejection rules per ADR 0057:
/// - relative cwd → [`PinStoreSelectionError::RelativeCwd`]
/// - nonexistent cwd → [`PinStoreSelectionError::CwdMissing`]
///
/// The user-store path is reached via [`user_pin_store`] when the
/// CLI passes `--store user`; this function never returns
/// [`PinStoreKind::User`].
pub fn select_store_for_pin(
    cwd: &Path,
    loader: &ConfigLoader,
) -> Result<PinStoreSelection, PinStoreSelectionError> {
    // Expand `~` at the boundary so the CLI's `--cwd ~/foo` and the
    // TUI form's `~/foo` are accepted without leaking the shortcut
    // into the absolute-path check below.
    let expanded = expand_home_prefix(&cwd.to_string_lossy());
    let cwd_owned = PathBuf::from(expanded);
    let cwd = cwd_owned.as_path();
    if !cwd.is_absolute() {
        return Err(PinStoreSelectionError::RelativeCwd {
            cwd: cwd.display().to_string(),
        });
    }
    if !cwd.exists() {
        return Err(PinStoreSelectionError::CwdMissing {
            cwd: cwd.display().to_string(),
        });
    }
    let path = loader
        .locate_project_config(cwd)
        .unwrap_or_else(|| cwd.join(PROJECT_CONFIG_FILENAME));
    Ok(PinStoreSelection {
        kind: PinStoreKind::Project,
        path,
    })
}

/// Return the user-level pin store path (`--store user` override).
/// Mirrors [`ConfigLoader::user_config_path`]; the wrap exists so
/// callers in `cli` can stay in pin-module vocabulary.
pub fn user_pin_store(loader: &ConfigLoader) -> Result<PinStoreSelection, PinStoreSelectionError> {
    loader
        .user_config_path()
        .map(|path| PinStoreSelection {
            kind: PinStoreKind::User,
            path,
        })
        .ok_or(PinStoreSelectionError::NoUserConfigPath)
}

// ---------------------------------------------------------------------
// Atomic write helpers (H-PIN-006)
// ---------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinWriteOutcome {
    pub path: PathBuf,
    /// `true` when the operation actually mutated the file on disk;
    /// `false` for no-op upserts (re-applying an identical entry) and
    /// no-op removes (id not present).
    pub changed: bool,
    pub entry_count: usize,
}

#[derive(Debug)]
pub enum PinWriteError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    Serialize {
        message: String,
    },
    Write {
        path: PathBuf,
        source: io::Error,
    },
    /// Caller passed an entry whose validation fails; we surface the
    /// underlying [`PinParseError`] rather than re-encoding it so the
    /// rule list stays in one place.
    Validation(PinParseError),
}

impl fmt::Display for PinWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Serialize { message } => write!(f, "failed to serialize pins TOML: {message}"),
            Self::Write { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
            Self::Validation(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PinWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { .. } | Self::Serialize { .. } => None,
            Self::Validation(err) => Some(err),
        }
    }
}

/// Read every pin entry with `id` from `paths` and return the first
/// match together with the file it came from. Used by `pin rename`,
/// `pin rm`, and `pin rebind` so they can read the existing entry
/// before mutating it.
pub fn load_pin_entry_by_id(
    paths: &[PathBuf],
    id: &str,
) -> Result<Option<(PathBuf, PinEntry)>, PinParseError> {
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        let document = parse_pins_document(&text)?;
        if let Some(entry) = document.entries().iter().find(|entry| entry.id == id) {
            return Ok(Some((path.clone(), entry.clone())));
        }
    }
    Ok(None)
}

/// Upsert `entry` into the `[pins]` section of `path`.
///
/// - Reads the existing file, parses it, mutates only the `[pins]`
///   slice, and writes via [`crate::declared::write_atomic`] so the
///   on-disk file is replaced through a temp-file-and-rename. Any
///   sibling sections (`[session]`, `[declared]`, `[aliases]`, …) are
///   preserved byte-for-byte.
/// - Entries are sorted by `id` after upsert for deterministic output.
/// - When `entry.id` matches an existing entry, the existing entry is
///   replaced; the call is a no-op (`changed: false`) when the new
///   entry equals the existing one.
/// - Validates `entry` via the same rules as
///   [`parse_pins_document`] so the file on disk never holds an
///   invalid entry the parser would reject on the next read.
/// - Malformed existing files are surfaced as
///   [`PinWriteError::Parse`] and the file is left untouched.
pub fn upsert_pin_entry(
    path: impl AsRef<Path>,
    mut entry: PinEntry,
) -> Result<PinWriteOutcome, PinWriteError> {
    let path = path.as_ref();
    // Expand `~` / `~/` shortcuts at the write boundary so callers
    // (CLI `pin create`, TUI form) can pass user-typed home-relative
    // paths without tripping the absolute-path rule.
    entry.cwd = expand_home_prefix(&entry.cwd);
    validate_entry_for_write(&entry)?;

    let (mut document, parsed) = load_document_for_write(path)?;
    let mut entries = parsed.entries().to_vec();
    let mut changed = true;

    if let Some(existing) = entries.iter_mut().find(|existing| existing.id == entry.id) {
        changed = existing != &entry;
        *existing = entry;
    } else {
        entries.push(entry);
    }

    // Detect mux-triple collisions (same backend + name + socket on a
    // different id) explicitly so callers get a typed error rather
    // than a load-time failure on the next read.
    validate_no_duplicate_mux(&entries)?;

    write_entries_if_changed(path, &mut document, entries, changed)
}

/// Remove the pin with `id` from the `[pins]` section of `path`. The
/// outcome's `changed` flag is `false` when no entry with that id
/// existed (idempotent remove).
pub fn remove_pin_entry(
    path: impl AsRef<Path>,
    id: &str,
) -> Result<PinWriteOutcome, PinWriteError> {
    let path = path.as_ref();
    let (mut document, parsed) = load_document_for_write(path)?;
    let mut entries = parsed.entries().to_vec();
    let original_len = entries.len();
    entries.retain(|entry| entry.id != id);
    let changed = entries.len() != original_len;

    write_entries_if_changed(path, &mut document, entries, changed)
}

fn validate_entry_for_write(entry: &PinEntry) -> Result<(), PinWriteError> {
    // Reuse the parse-time validator so the rule set stays
    // authoritative in one place. Wrap a single-entry section so the
    // duplicate-id and duplicate-mux checks see only this entry's
    // self-consistency; cross-entry duplicate detection runs after
    // the upsert in `validate_no_duplicate_mux`.
    let document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries: vec![entry.clone()],
        }),
    };
    let text = to_toml(&document).map_err(|err| PinWriteError::Serialize { message: err.0 })?;
    parse_pins_document(&text).map_err(PinWriteError::Validation)?;
    Ok(())
}

fn validate_no_duplicate_mux(entries: &[PinEntry]) -> Result<(), PinWriteError> {
    let mut seen = BTreeSet::new();
    for entry in entries {
        let key = mux_triple_key(&entry.mux);
        if !seen.insert(key.clone()) {
            return Err(PinWriteError::Validation(PinParseError::DuplicateMux {
                id: entry.id.clone(),
                mux: key,
            }));
        }
    }
    Ok(())
}

fn load_document_for_write(
    path: &Path,
) -> Result<(toml_edit::DocumentMut, PinsDocument), PinWriteError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(PinWriteError::Read {
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    let edit_document = if text.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| PinWriteError::Parse {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            })?
    };
    let parsed = parse_pins_document(&text).map_err(|err| PinWriteError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    Ok((edit_document, parsed))
}

fn write_entries_if_changed(
    path: &Path,
    document: &mut toml_edit::DocumentMut,
    mut entries: Vec<PinEntry>,
    changed: bool,
) -> Result<PinWriteOutcome, PinWriteError> {
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    let entry_count = entries.len();

    if changed {
        if entries.is_empty() {
            document.as_table_mut().remove("pins");
        } else {
            replace_pins_section(document, entries)?;
        }
        write_document(path, document)?;
    }

    Ok(PinWriteOutcome {
        path: path.to_path_buf(),
        changed,
        entry_count,
    })
}

/// Persist `document` to `path`. If the document is empty (no
/// top-level keys remain after `[pins]` was stripped), remove the
/// file instead so the store leaves no dangling header behind.
/// Mirror of `declared::write_document`.
fn write_document(path: &Path, document: &toml_edit::DocumentMut) -> Result<(), PinWriteError> {
    if document.as_table().is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(PinWriteError::Write {
                path: path.to_path_buf(),
                source: err,
            }),
        }
    } else {
        write_atomic(path, &document.to_string()).map_err(|err| PinWriteError::Write {
            path: path.to_path_buf(),
            source: err,
        })
    }
}

fn replace_pins_section(
    document: &mut toml_edit::DocumentMut,
    entries: Vec<PinEntry>,
) -> Result<(), PinWriteError> {
    let pins_document = PinsDocument {
        pins: Some(PinsSection {
            schema_version: PINS_SCHEMA_VERSION,
            entries,
        }),
    };
    let text =
        to_toml(&pins_document).map_err(|err| PinWriteError::Serialize { message: err.0 })?;
    let mut replacement =
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| PinWriteError::Serialize {
                message: format!("serialized pins section did not parse: {err}"),
            })?;
    document["pins"] =
        replacement
            .as_table_mut()
            .remove("pins")
            .ok_or_else(|| PinWriteError::Serialize {
                message: "serialized pins section was missing".to_string(),
            })?;
    Ok(())
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
