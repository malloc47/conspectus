//! Session-pin TOML file model.
//!
//! Per ADR 0057, Conspectus stores user-authored *session pins* —
//! declarations of a logical agent session — as a sibling
//! `[[pins.entries]]` table in the existing TOML config files.
//! Pins are the third user-intent write surface alongside
//! [`crate::declared`] (relationships) and [`crate::aliases`]
//! (node attributes).
//!
//! This module owns only schema parsing, validation, and TOML
//! round-trip. Loading pins into discovery, resolver binding, store
//! selection, atomic write helpers, and the CLI / TUI surfaces are
//! implemented in their own modules per the H-PIN-* backlog.
//!
//! The parse layer is strict: malformed entries surface as
//! [`PinParseError`] and the discovery layer is responsible for
//! deciding whether to skip an offending file (matching how
//! [`crate::declared`] and [`crate::aliases`] degrade today).

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    let document: PinsDocument =
        toml::from_str(text).map_err(|err| PinParseError::MalformedToml(err.to_string()))?;
    validate_document(&document)?;
    Ok(document)
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
    if entry.mux.backend != TMUX_MUX_BACKEND {
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
            Self::UnsupportedMuxBackend { entry_id, backend } => write!(
                f,
                "pin `{entry_id}` has unsupported mux.backend `{backend}` (only `tmux` is supported in v1)"
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(id: &str, mux_name: &str) -> PinEntry {
        PinEntry {
            id: id.to_string(),
            display_name: id.to_string(),
            harness: "codex".to_string(),
            cwd: "/home/me/work/repo".to_string(),
            mux: PinMux {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: mux_name.to_string(),
                socket_name: None,
            },
            launch: None,
            reason: None,
        }
    }

    #[test]
    fn round_trip_default_socket_entry() {
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![sample_entry("ingest-refactor", "ingest-refactor")],
            }),
        };

        let encoded = to_toml(&document).expect("serialize");
        let decoded = parse_pins_document(&encoded).expect("parse");

        assert_eq!(decoded, document);
        assert!(encoded.contains("[[pins.entries]]"));
        assert!(encoded.contains("backend = \"tmux\""));
        assert!(!encoded.contains("socket_name"));
    }

    #[test]
    fn round_trip_non_default_socket_entry() {
        let mut entry = sample_entry("scratch-codex", "scratch-codex");
        entry.mux.socket_name = Some("scratch".to_string());
        entry.launch = Some(PinLaunch {
            argv: vec!["codex".to_string()],
        });

        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![entry.clone()],
            }),
        };

        let encoded = to_toml(&document).expect("serialize");
        let decoded = parse_pins_document(&encoded).expect("parse");

        assert_eq!(decoded.entries(), &[entry.clone()]);
        assert!(encoded.contains("socket_name = \"scratch\""));
        assert!(encoded.contains("argv = [\"codex\"]"));
    }

    #[test]
    fn missing_pins_section_yields_empty_document() {
        let document = parse_pins_document("[session]\nprojection = \"agent\"\n").expect("parse");
        assert!(document.entries().is_empty());
    }

    #[test]
    fn unknown_keys_are_ignored_for_forward_compatibility() {
        let document = parse_pins_document(
            r#"
            [pins]
            schema_version = 1
            future_table_key = "ignored"

            [[pins.entries]]
            id = "ingest-refactor"
            display_name = "ingest-refactor"
            harness = "codex"
            cwd = "/home/me/work/repo"
            mux = { backend = "tmux", name = "ingest-refactor", future_mux_key = "ignored" }
            future_entry_key = true
            "#,
        )
        .expect("parse with unknown keys");
        assert_eq!(document.entries().len(), 1);
    }

    #[test]
    fn unsupported_schema_version_is_an_error() {
        let err = parse_pins_document(
            r#"
            [pins]
            schema_version = 99
            "#,
        )
        .expect_err("unsupported schema");
        assert!(matches!(err, PinParseError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn malformed_toml_is_an_error() {
        let err = parse_pins_document("this is not toml = ").expect_err("malformed");
        assert!(matches!(err, PinParseError::MalformedToml(_)));
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![sample_entry("dup", "mux-a"), sample_entry("dup", "mux-b")],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        let err = parse_pins_document(&encoded).expect_err("duplicate id");
        assert!(matches!(err, PinParseError::DuplicateId(id) if id == "dup"));
    }

    #[test]
    fn duplicate_mux_triple_is_rejected() {
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![
                    sample_entry("first", "shared"),
                    sample_entry("second", "shared"),
                ],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        let err = parse_pins_document(&encoded).expect_err("duplicate mux");
        assert!(matches!(err, PinParseError::DuplicateMux { .. }));
    }

    #[test]
    fn same_mux_name_on_different_sockets_is_allowed() {
        let a = sample_entry("a", "shared");
        let mut b = sample_entry("b", "shared");
        b.mux.socket_name = Some("scratch".to_string());

        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![a.clone(), b.clone()],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        let decoded = parse_pins_document(&encoded).expect("parse");
        assert_eq!(decoded.entries().len(), 2);

        let mut c = sample_entry("c", "shared");
        c.mux.socket_name = Some("default".to_string());
        let collides_via_default_sentinel = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![sample_entry("a", "shared"), c],
            }),
        };
        let encoded2 = to_toml(&collides_via_default_sentinel).expect("serialize");
        let err = parse_pins_document(&encoded2).expect_err("default sentinel collides");
        assert!(matches!(err, PinParseError::DuplicateMux { .. }));
    }

    #[test]
    fn empty_required_fields_are_rejected() {
        for (label, mutate) in [
            (
                "id",
                Box::new(|e: &mut PinEntry| e.id = "  ".to_string()) as Box<dyn Fn(&mut PinEntry)>,
            ),
            (
                "display_name",
                Box::new(|e: &mut PinEntry| e.display_name = String::new()),
            ),
            (
                "harness",
                Box::new(|e: &mut PinEntry| e.harness = String::new()),
            ),
            ("cwd", Box::new(|e: &mut PinEntry| e.cwd = String::new())),
            (
                "mux.name",
                Box::new(|e: &mut PinEntry| e.mux.name = String::new()),
            ),
            (
                "mux.socket_name",
                Box::new(|e: &mut PinEntry| e.mux.socket_name = Some(String::new())),
            ),
        ] {
            let mut entry = sample_entry("ingest", "ingest");
            mutate(&mut entry);
            let document = PinsDocument {
                pins: Some(PinsSection {
                    schema_version: PINS_SCHEMA_VERSION,
                    entries: vec![entry],
                }),
            };
            let encoded = to_toml(&document).expect("serialize");
            // Serialization may skip empty fields entirely, in which
            // case the parse error is a missing required field bubbled
            // up as MalformedToml. Either is acceptable as long as the
            // parser refuses to silently load the entry.
            match parse_pins_document(&encoded) {
                Err(PinParseError::EmptyField { field, .. }) => {
                    assert_eq!(field, label, "wrong field flagged for {label}");
                }
                Err(PinParseError::MalformedToml(_)) => {
                    // Serde reported the missing required key; still a
                    // refusal, which is the invariant under test.
                }
                Err(other) => panic!("unexpected parse error for empty `{label}`: {other}"),
                Ok(_) => panic!("parser silently accepted empty `{label}`"),
            }
        }
    }

    #[test]
    fn relative_cwd_is_rejected() {
        let mut entry = sample_entry("ingest", "ingest");
        entry.cwd = "relative/path".to_string();
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![entry],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        let err = parse_pins_document(&encoded).expect_err("relative cwd");
        assert!(matches!(err, PinParseError::RelativeCwd { .. }));
    }

    #[test]
    fn unsupported_mux_backend_is_rejected() {
        let mut entry = sample_entry("ingest", "ingest");
        entry.mux.backend = "zellij".to_string();
        let document = PinsDocument {
            pins: Some(PinsSection {
                schema_version: PINS_SCHEMA_VERSION,
                entries: vec![entry],
            }),
        };
        let encoded = to_toml(&document).expect("serialize");
        let err = parse_pins_document(&encoded).expect_err("zellij not supported");
        assert!(matches!(err, PinParseError::UnsupportedMuxBackend { .. }));
    }

    #[test]
    fn round_trip_preserves_sibling_sections() {
        let text = r#"
[session]
projection = "agent"

[aliases]
schema_version = 1

[[aliases.entries]]
node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
display_name = "alpha-name"

[pins]
schema_version = 1

[[pins.entries]]
id = "ingest"
display_name = "ingest"
harness = "codex"
cwd = "/home/me/work/repo"
mux = { backend = "tmux", name = "ingest" }
"#;
        // We only verify that PinsDocument decodes the pins slice; the
        // sibling sections are owned by other modules. The test guards
        // against accidentally tightening PinsDocument to reject unknown
        // top-level keys.
        let document = parse_pins_document(text).expect("parse alongside siblings");
        assert_eq!(document.entries().len(), 1);
        assert_eq!(document.entries()[0].id, "ingest");
    }

    #[test]
    fn effective_socket_and_native_id() {
        let mut entry = sample_entry("ingest", "ingest");
        assert_eq!(entry.mux.effective_socket(), None);
        assert_eq!(entry.mux.native_id(), "tmux:ingest");

        entry.mux.socket_name = Some("default".to_string());
        assert_eq!(
            entry.mux.effective_socket(),
            None,
            "the literal `default` sentinel collapses to the canonical no-socket case"
        );
        assert_eq!(entry.mux.native_id(), "tmux:ingest");

        entry.mux.socket_name = Some("scratch".to_string());
        assert_eq!(entry.mux.effective_socket(), Some("scratch"));
        assert_eq!(entry.mux.native_id(), "tmux:scratch:ingest");
    }
}
