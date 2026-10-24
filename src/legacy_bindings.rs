//! The legacy per-pin binding sidecar (ADR 0058), read-only.
//!
//! Old builds recorded each pin's last bound session in
//! `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. Nothing
//! writes those files now: a mux's history record lists the sessions it
//! hosted (ADR 0140). `conspectus pin migrate` reads a pin's sidecar
//! once, to seed its history record with the session it last ran. The
//! directory goes one release after the slots ship (backlog `CSP-690`).

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    #[cfg(test)]
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

#[cfg(test)]
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
    EmptyField {
        pin_id: String,
        field: &'static str,
    },
    #[cfg(test)]
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
            #[cfg(test)]
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
