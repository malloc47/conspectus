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
//! [`crate::declared::write_atomic`] (which is shared via that
//! module). A per-process suffix on the temp name lets concurrent
//! conspectus runs coexist without colliding.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::declared::write_atomic;
use crate::model::{GraphSnapshot, PinBinding};

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

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

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
/// [`crate::declared::write_atomic`]). Creates parent directories on
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

/// Per-pin outcome of [`record_bindings`]. The string is the pin id,
/// the inner result is the write outcome (or the error that prevented
/// it). Callers route these to logging — sidecar write failures
/// should never propagate up through discovery.
pub type RecordOutcome = (String, Result<WriteOutcome, PinBindingError>);

/// Post-resolve sidecar write pass (ADR 0058 §Write path). Iterates
/// `snapshot.pins`; for each pin whose binding settled to
/// [`PinBinding::Bound`] (including bindings sourced from `pin bind`
/// declared overrides per Q6), build a [`PinBindingRecord`] and write
/// it via [`write`]. Pins with `Unbound`, `StaleMux`, or no binding
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
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn sample(observed_epoch: i64) -> PinBindingRecord {
        PinBindingRecord::new("ingest", "ingest", None, "abc123", "codex", observed_epoch)
    }

    fn cache_in(dir: &Path) -> PinBindingsCache {
        PinBindingsCache::new().with_xdg_cache_home(dir)
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let record = PinBindingRecord {
            schema_version: PIN_BINDING_SCHEMA_VERSION,
            pin_id: "ingest".into(),
            mux_name: "ingest-refactor".into(),
            mux_socket: Some("scratch".into()),
            session_id: "abc123".into(),
            harness: "codex".into(),
            observed_epoch: 1_738_742_400,
        };
        let json = to_json(&record).unwrap();
        let parsed = parse_record(&json).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn unknown_json_keys_are_tolerated_on_read() {
        let payload = r#"{
            "schema_version": 1,
            "pin_id": "ingest",
            "mux_name": "ingest",
            "session_id": "abc",
            "harness": "codex",
            "observed_epoch": 1738742400,
            "future_field": "ignored"
        }"#;
        let parsed = parse_record(payload).expect("unknown key tolerance");
        assert_eq!(parsed.pin_id, "ingest");
    }

    #[test]
    fn malformed_json_surfaces_diagnostic() {
        let err = parse_record("not json").unwrap_err();
        assert!(matches!(err, PinBindingError::MalformedJson(_)));
    }

    #[test]
    fn unsupported_schema_version_rejected() {
        let payload = r#"{
            "schema_version": 99,
            "pin_id": "ingest",
            "mux_name": "ingest",
            "session_id": "abc",
            "harness": "codex",
            "observed_epoch": 1
        }"#;
        let err = parse_record(payload).unwrap_err();
        assert!(matches!(err, PinBindingError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn empty_required_fields_rejected() {
        let mut record = sample(1);
        record.session_id = String::new();
        let err = validate(&record).unwrap_err();
        assert!(matches!(
            err,
            PinBindingError::EmptyField {
                field: "session_id",
                ..
            }
        ));
    }

    #[test]
    fn whitespace_socket_rejected_as_empty() {
        let mut record = sample(1);
        record.mux_socket = Some("   ".into());
        let err = validate(&record).unwrap_err();
        assert!(matches!(
            err,
            PinBindingError::EmptyField {
                field: "mux_socket",
                ..
            }
        ));
    }

    #[test]
    fn directory_uses_xdg_cache_when_set() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let dir = cache.directory().unwrap();
        assert_eq!(dir, temp.path().join("conspectus").join("pin-bindings"));
    }

    #[test]
    fn directory_falls_back_to_home_dot_cache() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_home(temp.path());
        let dir = cache.directory().unwrap();
        assert_eq!(
            dir,
            temp.path()
                .join(".cache")
                .join("conspectus")
                .join("pin-bindings"),
        );
    }

    #[test]
    fn directory_none_without_env_overrides() {
        let cache = PinBindingsCache::new();
        assert!(cache.directory().is_none());
    }

    #[test]
    fn path_for_resolves_per_pin_filename() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let path = cache.path_for("ingest").unwrap();
        assert!(path.ends_with("conspectus/pin-bindings/ingest.json"));
    }

    #[test]
    fn read_returns_none_when_directory_absent() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let result = read(&cache, "ingest").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_then_read_round_trips_through_disk() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let record = sample(1_738_742_400);
        let outcome = write(&cache, &record).unwrap();
        assert_eq!(outcome, WriteOutcome::Wrote);

        let read_back = read(&cache, "ingest").unwrap().expect("sidecar exists");
        assert_eq!(read_back, record);
    }

    #[test]
    fn write_creates_parent_directories_on_first_use() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        // Directory does not yet exist.
        let dir = cache.directory().unwrap();
        assert!(!dir.exists());

        write(&cache, &sample(1)).unwrap();

        assert!(dir.is_dir());
        assert!(dir.join("ingest.json").is_file());
    }

    #[test]
    fn write_skips_when_payload_unchanged() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let record = sample(1);

        let first = write(&cache, &record).unwrap();
        assert_eq!(first, WriteOutcome::Wrote);

        // Second write with the same record should skip — no mtime
        // churn on quiet cycles per ADR 0058 §Write path.
        let second = write(&cache, &record).unwrap();
        assert_eq!(second, WriteOutcome::Skipped);
    }

    #[test]
    fn write_replaces_when_payload_changes() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());

        write(&cache, &sample(1)).unwrap();
        let second = write(&cache, &sample(2)).unwrap();
        assert_eq!(second, WriteOutcome::Wrote);

        let read_back = read(&cache, "ingest").unwrap().unwrap();
        assert_eq!(read_back.observed_epoch, 2);
    }

    #[test]
    fn write_refuses_invalid_records() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut record = sample(1);
        record.harness = String::new();
        let err = write(&cache, &record).unwrap_err();
        assert!(matches!(
            err,
            PinBindingError::EmptyField {
                field: "harness",
                ..
            }
        ));
        // Nothing was written to disk.
        assert!(read(&cache, "ingest").unwrap().is_none());
    }

    #[test]
    fn write_without_cache_root_errors() {
        let cache = PinBindingsCache::new();
        let err = write(&cache, &sample(1)).unwrap_err();
        assert!(matches!(err, PinBindingError::Io(_)));
    }

    #[test]
    fn read_surfaces_malformed_file_without_writing() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let dir = cache.directory().unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("ingest.json"), b"this is not json").unwrap();

        let err = read(&cache, "ingest").unwrap_err();
        assert!(matches!(err, PinBindingError::MalformedJson(_)));
    }

    #[test]
    fn delete_removes_existing_sidecar() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        write(&cache, &sample(1)).unwrap();

        let removed = delete(&cache, "ingest").unwrap();
        assert!(removed);
        assert!(read(&cache, "ingest").unwrap().is_none());
    }

    #[test]
    fn delete_returns_false_when_sidecar_missing() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let removed = delete(&cache, "ingest").unwrap();
        assert!(!removed);
    }

    #[test]
    fn delete_is_safe_without_cache_root() {
        let cache = PinBindingsCache::new();
        let removed = delete(&cache, "ingest").unwrap();
        assert!(!removed);
    }

    // ----- record_bindings: post-resolve write pass -----

    mod record {
        use super::*;
        use crate::model::{
            AgentSessionId, GraphSnapshot, MuxSessionId, PinBinding, PinCandidate, PinMuxRef,
            Provenance,
        };

        fn make_pin(id: &str, binding: Option<PinBinding>) -> PinCandidate {
            PinCandidate {
                id: id.to_string(),
                display_name: id.to_string(),
                harness: "codex".to_string(),
                cwd: "/p".to_string(),
                mux: PinMuxRef {
                    backend: "tmux".to_string(),
                    name: id.to_string(),
                    socket_name: None,
                },
                launch_argv: None,
                reason: None,
                provenance: Provenance::LocalPin,
                store_path: "/p/.conspectus.toml".to_string(),
                binding,
            }
        }

        fn bound(session_key: &str, mux_name: &str) -> PinBinding {
            PinBinding::Bound {
                mux: MuxSessionId::new(format!("tmux:{mux_name}")),
                session: AgentSessionId::new("codex", "/state", session_key),
            }
        }

        #[test]
        fn writes_sidecar_for_each_bound_pin() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let mut snap = GraphSnapshot::empty();
            snap.pins
                .push(make_pin("ingest", Some(bound("session-a", "ingest"))));
            snap.pins
                .push(make_pin("review", Some(bound("session-b", "review"))));

            let outcomes = record_bindings(&snap, &cache, 1_738_742_400);

            assert_eq!(outcomes.len(), 2);
            assert!(outcomes.iter().all(|(_, r)| r.is_ok()));

            let ingest = read(&cache, "ingest").unwrap().expect("ingest sidecar");
            assert_eq!(ingest.session_id, "session-a");
            assert_eq!(ingest.observed_epoch, 1_738_742_400);

            let review = read(&cache, "review").unwrap().expect("review sidecar");
            assert_eq!(review.session_id, "session-b");
        }

        #[test]
        fn skips_unbound_and_stale_pins() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let mut snap = GraphSnapshot::empty();
            snap.pins.push(make_pin("u", Some(PinBinding::Unbound)));
            snap.pins.push(make_pin(
                "s",
                Some(PinBinding::StaleMux {
                    mux: MuxSessionId::new("tmux:s"),
                }),
            ));
            snap.pins.push(make_pin("n", None));

            let outcomes = record_bindings(&snap, &cache, 1);

            assert!(
                outcomes.is_empty(),
                "no Bound pins should produce no writes"
            );
            assert!(read(&cache, "u").unwrap().is_none());
            assert!(read(&cache, "s").unwrap().is_none());
            assert!(read(&cache, "n").unwrap().is_none());
        }

        #[test]
        fn empty_pins_is_noop() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let snap = GraphSnapshot::empty();
            let outcomes = record_bindings(&snap, &cache, 1);
            assert!(outcomes.is_empty());
        }

        #[test]
        fn idempotent_second_call_skips_unchanged() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let mut snap = GraphSnapshot::empty();
            snap.pins
                .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

            let first = record_bindings(&snap, &cache, 1);
            let second = record_bindings(&snap, &cache, 1);

            assert_eq!(first[0].1.as_ref().unwrap(), &WriteOutcome::Wrote);
            assert_eq!(second[0].1.as_ref().unwrap(), &WriteOutcome::Skipped);
        }

        #[test]
        fn observed_epoch_change_triggers_replacement() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let mut snap = GraphSnapshot::empty();
            snap.pins
                .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

            let _ = record_bindings(&snap, &cache, 1);
            let second = record_bindings(&snap, &cache, 2);

            assert_eq!(second[0].1.as_ref().unwrap(), &WriteOutcome::Wrote);
            let stored = read(&cache, "ingest").unwrap().unwrap();
            assert_eq!(stored.observed_epoch, 2);
        }

        #[test]
        fn carries_mux_socket_into_sidecar() {
            let temp = tempdir().unwrap();
            let cache = cache_in(temp.path());
            let mut pin = make_pin("ingest", Some(bound("session-a", "ingest")));
            pin.mux.socket_name = Some("scratch".to_string());
            let mut snap = GraphSnapshot::empty();
            snap.pins.push(pin);

            record_bindings(&snap, &cache, 1);

            let stored = read(&cache, "ingest").unwrap().unwrap();
            assert_eq!(stored.mux_socket.as_deref(), Some("scratch"));
        }

        #[test]
        fn cache_without_root_surfaces_error_per_pin() {
            let cache = PinBindingsCache::new();
            let mut snap = GraphSnapshot::empty();
            snap.pins
                .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

            let outcomes = record_bindings(&snap, &cache, 1);

            assert_eq!(outcomes.len(), 1);
            assert!(outcomes[0].1.is_err());
        }
    }
}
