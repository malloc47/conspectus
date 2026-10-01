//! Per-pin session-binding sidecar (ADR 0058).
//!
//! Each fresh `(pin_id, mux_name, mux_socket, session_id, harness,
//! observed_epoch)` binding produced by the resolver is recorded to a
//! per-pin JSON file under
//! `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. The
//! sidecar is a *rebuildable cache*, not authoritative state: the
//! resolver never reads it; only `pin launch` consumes it to decide
//! whether to splice `HarnessAdapter::resume_argv` into the launched
//! tmux session.
//!
//! This module owns the format, parse/serialize, validation, atomic
//! read-modify-write, and the cache-root resolution. Discovery
//! integration (write pass) and the launch consumer live in their
//! own modules per the H-PIN-RESUME-* backlog.
//!
//! ## Format
//!
//! One JSON file per pin. Schema version is a `u32` so additive
//! evolutions land without breaking existing files; unknown fields
//! are tolerated on read but never round-tripped. Malformed files
//! surface as [`PinBindingError::MalformedJson`] and are *left
//! untouched* by the write path — operators are expected to inspect
//! and clear them rather than have conspectus silently overwrite.
//!
//! ## Atomicity
//!
//! Writes use the same temp-file-and-rename pattern as
//! `crate::declared::write_atomic` (which is shared via that
//! module). A per-process suffix on the temp name lets concurrent
//! conspectus runs coexist without colliding.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::declared::write_atomic;
use crate::model::{
    AgentSessionId, Diagnostic, GraphNode, GraphSnapshot, LinkState, NodeId, PinBinding,
    PinLastSession, RelationKind,
};

/// Current sidecar schema version. Bumped only on incompatible
/// format changes; additive field growth lands without a bump.
pub const PIN_BINDING_SCHEMA_VERSION: u32 = 1;

/// One pin's last observed binding. Matches ADR 0058 §Sidecar shape.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PinBindingRecord {
    pub schema_version: u32,
    pub pin_id: String,
    pub mux_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mux_socket: Option<String>,
    pub session_id: String,
    pub harness: String,
    pub observed_epoch: i64,
}

impl PinBindingRecord {
    /// Build a record stamped with the current schema version.
    /// Callers that round-trip a parsed record should preserve the
    /// version they read rather than calling this constructor.
    pub fn new(
        pin_id: impl Into<String>,
        mux_name: impl Into<String>,
        mux_socket: Option<String>,
        session_id: impl Into<String>,
        harness: impl Into<String>,
        observed_epoch: i64,
    ) -> Self {
        Self {
            schema_version: PIN_BINDING_SCHEMA_VERSION,
            pin_id: pin_id.into(),
            mux_name: mux_name.into(),
            mux_socket,
            session_id: session_id.into(),
            harness: harness.into(),
            observed_epoch,
        }
    }
}

/// Parse a record from a JSON string. Validates the schema version
/// and required fields; unknown JSON keys are tolerated and dropped
/// on re-serialize.
pub fn parse_record(text: &str) -> Result<PinBindingRecord, PinBindingError> {
    let record: PinBindingRecord = serde_json::from_str(text)
        .map_err(|err| PinBindingError::MalformedJson(err.to_string()))?;
    validate(&record)?;
    Ok(record)
}

/// Serialize a record to pretty-printed JSON. The pretty form keeps
/// the cache file readable for operators who inspect it by hand.
pub fn to_json(record: &PinBindingRecord) -> Result<String, PinBindingError> {
    serde_json::to_string_pretty(record).map_err(|err| PinBindingError::Serialize(err.to_string()))
}

fn validate(record: &PinBindingRecord) -> Result<(), PinBindingError> {
    if record.schema_version != PIN_BINDING_SCHEMA_VERSION {
        return Err(PinBindingError::UnsupportedSchemaVersion(
            record.schema_version,
        ));
    }
    for (field, value) in [
        ("pin_id", record.pin_id.as_str()),
        ("mux_name", record.mux_name.as_str()),
        ("session_id", record.session_id.as_str()),
        ("harness", record.harness.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(PinBindingError::EmptyField {
                pin_id: record.pin_id.clone(),
                field,
            });
        }
    }
    if let Some(socket) = record.mux_socket.as_deref()
        && socket.trim().is_empty()
    {
        return Err(PinBindingError::EmptyField {
            pin_id: record.pin_id.clone(),
            field: "mux_socket",
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinBindingError {
    MalformedJson(String),
    UnsupportedSchemaVersion(u32),
    EmptyField { pin_id: String, field: &'static str },
    Serialize(String),
    Io(String),
}

impl std::fmt::Display for PinBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedJson(msg) => write!(f, "malformed pin-binding sidecar JSON: {msg}"),
            Self::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported pin-binding sidecar schema version: {v}")
            }
            Self::EmptyField { pin_id, field } => write!(
                f,
                "pin-binding sidecar for `{pin_id}` has empty required field `{field}`"
            ),
            Self::Serialize(msg) => write!(f, "could not serialize pin-binding sidecar: {msg}"),
            Self::Io(msg) => write!(f, "pin-binding sidecar I/O error: {msg}"),
        }
    }
}

impl std::error::Error for PinBindingError {}

impl From<io::Error> for PinBindingError {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Cache-root resolver. Mirrors [`crate::config::ConfigLoader`]'s
/// shape so tests can override `$XDG_CACHE_HOME` and `$HOME`
/// without mutating process state.
#[derive(Clone, Debug, Default)]
pub struct PinBindingsCache {
    home: Option<PathBuf>,
    xdg_cache_home: Option<PathBuf>,
}

impl PinBindingsCache {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a cache resolver populated from the process environment.
    pub fn from_env() -> Self {
        Self {
            home: env_path("HOME"),
            xdg_cache_home: env_path("XDG_CACHE_HOME"),
        }
    }

    #[cfg(test)]
    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    #[cfg(test)]
    pub fn with_xdg_cache_home(mut self, xdg: impl Into<PathBuf>) -> Self {
        self.xdg_cache_home = Some(xdg.into());
        self
    }

    /// Return the directory where per-pin sidecars live. Returns
    /// `None` only when no base directory is known (no `$HOME`, no
    /// `$XDG_CACHE_HOME`).
    pub fn directory(&self) -> Option<PathBuf> {
        if let Some(xdg) = &self.xdg_cache_home {
            return Some(xdg.join("conspectus").join("pin-bindings"));
        }
        self.home
            .as_ref()
            .map(|home| home.join(".cache").join("conspectus").join("pin-bindings"))
    }

    /// Return the full sidecar path for `pin_id`.
    pub fn path_for(&self, pin_id: &str) -> Option<PathBuf> {
        self.directory()
            .map(|dir| dir.join(format!("{pin_id}.json")))
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// Read the sidecar for `pin_id`. Returns:
/// - `Ok(Some(record))` on a valid file,
/// - `Ok(None)` when the cache directory or sidecar is absent (the
///   normal "pin has no recorded history yet" case),
/// - `Err(PinBindingError::MalformedJson | ::UnsupportedSchemaVersion
///   | ::EmptyField)` when the file exists but cannot be trusted —
///   callers decide whether to surface a diagnostic, leave the file
///   alone, or delete it.
pub fn read(
    cache: &PinBindingsCache,
    pin_id: &str,
) -> Result<Option<PinBindingRecord>, PinBindingError> {
    let Some(path) = cache.path_for(pin_id) else {
        return Ok(None);
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(PinBindingError::Io(err.to_string())),
    };
    parse_record(&text).map(Some)
}

/// Write the sidecar for `record.pin_id`, but only when the on-disk
/// payload differs. Returns `Ok(WriteOutcome::Wrote)` when the file
/// was created or replaced, `Ok(WriteOutcome::Skipped)` when the
/// existing file already matched. Quiet cycles produce no I/O beyond
/// the read and a parse.
///
/// Atomic via tempfile-and-rename (shared with
/// `crate::declared::write_atomic`). Creates parent directories on
/// demand.
pub fn write(
    cache: &PinBindingsCache,
    record: &PinBindingRecord,
) -> Result<WriteOutcome, PinBindingError> {
    validate(record)?;
    let Some(path) = cache.path_for(&record.pin_id) else {
        return Err(PinBindingError::Io(
            "no cache directory resolved (neither $XDG_CACHE_HOME nor $HOME set)".to_string(),
        ));
    };
    let new_json = to_json(record)?;

    // Skip-on-unchanged: parse-and-compare lets us treat semantically
    // equal records (different field orderings, whitespace) as
    // matches even when the byte payload differs. A purely byte-level
    // compare would churn on every JSON pretty-printer revision.
    if let Ok(existing) = fs::read_to_string(&path)
        && let Ok(existing_record) = parse_record(&existing)
        && existing_record == *record
    {
        return Ok(WriteOutcome::Skipped);
    }

    write_atomic(&path, &new_json).map_err(|err| PinBindingError::Io(err.to_string()))?;
    Ok(WriteOutcome::Wrote)
}

/// Delete the sidecar for `pin_id`. Returns `Ok(true)` if a file
/// was removed, `Ok(false)` if no file existed. Per ADR 0058 §Q7
/// this is the cleanup path the launch consumer invokes when a
/// recorded session can no longer be found on disk.
pub fn delete(cache: &PinBindingsCache, pin_id: &str) -> Result<bool, PinBindingError> {
    let Some(path) = cache.path_for(pin_id) else {
        return Ok(false);
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(PinBindingError::Io(err.to_string())),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteOutcome {
    /// Sidecar was created or replaced.
    Wrote,
    /// Existing sidecar already matched; no I/O performed beyond the
    /// read+parse.
    Skipped,
}

/// Outcome of walking a recorded session id forward through ADR 0018
/// `parent_session` lineage to find the current head. See
/// [`lineage_head`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineageOutcome {
    /// Walk completed: `head` is the current leaf of the chain. May
    /// be the starting session itself when the recorded session has
    /// no successors.
    Head(AgentSessionId),
    /// The starting session was not found in the snapshot (deleted,
    /// not yet discovered, or never existed). The launch consumer
    /// per ADR 0058 Q7 deletes the sidecar in this case.
    SessionMissing,
    /// The walk encountered a fork — an ancestor with multiple
    /// recorded successors. Per ADR 0058 Q8 the launch consumer
    /// refuses to disambiguate and falls back to default argv.
    Fork {
        at: AgentSessionId,
        successors: Vec<AgentSessionId>,
    },
}

/// Walk forward through ADR 0018 `parent_session` lineage starting
/// from `(harness, session_key)` in `snapshot`. Each step finds
/// `ParentSession` candidate links whose target is the current
/// session (i.e., successors with `parent_session = current`) and
/// follows them. Stops at the first session with zero or more than
/// one successor.
///
/// Cycle defense: a visited-set prevents infinite walks if the
/// graph is malformed; on cycle detection the walk returns the
/// current session as the head (best-effort honesty rather than
/// looping or panicking).
pub fn lineage_head(snapshot: &GraphSnapshot, harness: &str, session_key: &str) -> LineageOutcome {
    let Some(start) = find_session(snapshot, harness, session_key) else {
        return LineageOutcome::SessionMissing;
    };

    let mut current = start;
    let mut visited: BTreeSet<AgentSessionId> = BTreeSet::new();
    visited.insert(current.clone());

    loop {
        let target = NodeId::AgentSession(current.clone());
        let mut successors: Vec<AgentSessionId> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::ParentSession
                    && matches!(link.state, LinkState::Active)
                    && link.target_node_id() == Some(&target)
            })
            .filter_map(|link| match &link.source {
                NodeId::AgentSession(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        successors.sort();
        successors.dedup();

        match successors.len() {
            0 => return LineageOutcome::Head(current),
            1 => {
                let next = successors.into_iter().next().unwrap();
                if !visited.insert(next.clone()) {
                    // Defensive: cycle in `parent_session` would
                    // otherwise loop forever. Return what we have.
                    return LineageOutcome::Head(current);
                }
                current = next;
            }
            _ => {
                return LineageOutcome::Fork {
                    at: current,
                    successors,
                };
            }
        }
    }
}

fn find_session(
    snapshot: &GraphSnapshot,
    harness: &str,
    session_key: &str,
) -> Option<AgentSessionId> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(s)
            if s.id.harness_key == harness && s.id.session_key == session_key =>
        {
            Some(s.id.clone())
        }
        _ => None,
    })
}

/// Decorate `PinUnbound` diagnostics in `snapshot` with the recorded
/// last-bound session from the sidecar, when one exists (ADR 0058
/// §TUI surface / Q5). Resolver attribution stays purely
/// observation-driven; this post-resolve pass enriches the
/// already-emitted diagnostic so downstream consumers (TUI status
/// hint, `pin show`, right detail pane) can advertise resume
/// affordances when continuity is available.
///
/// Pins whose `PinUnbound` already carries a `last_session` are left
/// alone. Sidecar read failures (malformed JSON, etc.) leave the
/// diagnostic unenriched rather than overwriting it with garbage.
pub fn decorate_unbound_diagnostics(snapshot: &mut GraphSnapshot, cache: &PinBindingsCache) {
    for diagnostic in &mut snapshot.diagnostics {
        let Diagnostic::PinUnbound {
            pin_id,
            last_session,
            ..
        } = diagnostic
        else {
            continue;
        };
        if last_session.is_some() {
            continue;
        }
        if let Ok(Some(record)) = read(cache, pin_id) {
            *last_session = Some(PinLastSession {
                session_id: record.session_id,
                observed_epoch: record.observed_epoch,
            });
        }
    }
}

/// Per-pin outcome of [`record_bindings`]. The string is the pin id,
/// the inner result is the write outcome (or the error that prevented
/// it). Callers route these to logging — sidecar write failures
/// should never propagate up through discovery.
pub type RecordOutcome = (String, Result<WriteOutcome, PinBindingError>);

/// Post-resolve sidecar write pass (ADR 0058 §Write path). Iterates
/// `snapshot.pins`; for each pin whose binding settled to
/// [`PinBinding::Bound`] (including bindings sourced from `pin bind`
/// declared overrides per Q6), build a [`PinBindingRecord`] and write
/// it via `write`. Pins with `Unbound`, `StaleMux`, or no binding
/// at all are skipped — the sidecar only records observed successful
/// bindings, never failure modes.
///
/// Returns one entry per pin that was a candidate for writing
/// (whether the write succeeded, was skipped as unchanged, or
/// errored). Pins with non-Bound bindings are omitted from the
/// result so callers don't have to filter noise.
///
/// `now_epoch` is the timestamp stamped into each new record. Passed
/// in rather than read from the clock so tests can pin it
/// deterministically.
pub fn record_bindings(
    snapshot: &GraphSnapshot,
    cache: &PinBindingsCache,
    now_epoch: i64,
) -> Vec<RecordOutcome> {
    let mut outcomes = Vec::new();
    for pin in &snapshot.pins {
        let Some(PinBinding::Bound { session, .. }) = pin.binding.as_ref() else {
            continue;
        };
        let record = PinBindingRecord::new(
            &pin.id,
            &pin.mux.name,
            pin.mux.socket_name.clone(),
            &session.session_key,
            &pin.harness,
            now_epoch,
        );
        outcomes.push((pin.id.clone(), write(cache, &record)));
    }
    outcomes
}

#[cfg(test)]
#[path = "pin_bindings_tests.rs"]
mod tests;
