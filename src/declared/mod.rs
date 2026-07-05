//! Declared-link TOML file models (H-REF-005 split).
//!
//! These types model the durable `[declared]` config section
//! from ADR 0014. Loading them does not imply graph discovery
//! or persistence; later phases convert validated entries into
//! `GraphLink` evidence and write them back.
//!
//! Post-H-REF-005 concerns are split into three submodules:
//! - **this file** — TOML types + `DeclaredEndpoint` compact
//!   codec + `parse_declared_document` / `to_toml` +
//!   `validate_document` + every `DeclaredParseError` /
//!   `DeclaredSerializeError` / `DeclaredWriteError` type.
//! - [`store`] — read-modify-write helpers
//!   (`upsert_declared_link`, `remove_declared_link`,
//!   `load_declared_link_by_id`) + store selection value types
//!   (`DeclaredStoreSelection`, `DeclaredStoreKind`,
//!   `DeclaredWriteOutcome`).
//! - [`snapshot`] — graph-driven decision helpers
//!   (`select_store_for_declaration`,
//!   `declared_endpoint_from_node_id`) that need a
//!   `GraphSnapshot` to make their choice.
//!
//! Every pre-H-REF-005 public identifier is re-exported from
//! this module so callers reach for `crate::declared::…` as
//! before.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::RelationKind;

pub mod snapshot;
pub mod store;

// Re-exports keeping the pre-H-REF-005 `crate::declared::*`
// surface intact.
pub use snapshot::{declared_endpoint_from_node_id, select_store_for_declaration};
pub use store::{
    DeclaredStoreKind, DeclaredStoreSelection, DeclaredWriteOutcome, load_declared_link_by_id,
    remove_declared_link, upsert_declared_link,
};
// H-REF-005: `write_atomic` remains reachable at
// `crate::declared::write_atomic` for the pin / alias / tui
// state consumers that share the atomic-write primitive.
pub(crate) use store::write_atomic;

pub const DECLARED_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<DeclaredSection>,
}

impl DeclaredDocument {
    pub fn links(&self) -> &[DeclaredLink] {
        self.declared
            .as_ref()
            .map(|declared| declared.links.as_slice())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredSection {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<DeclaredLink>,
}

impl Default for DeclaredSection {
    fn default() -> Self {
        Self {
            schema_version: DECLARED_SCHEMA_VERSION,
            links: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredLink {
    pub id: String,
    pub relation: RelationKind,
    pub state: DeclaredLinkState,
    pub source: DeclaredEndpoint,
    pub target: DeclaredEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredLinkState {
    Active,
    Ignored,
    Overridden,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeclaredEndpoint {
    Repo {
        common_dir: String,
    },
    Checkout {
        repo_common_dir: String,
        root: String,
    },
    Workspace {
        root: String,
    },
    AgentSession {
        harness_key: String,
        state_scope: String,
        session_key: String,
    },
    MuxSession {
        native_id: String,
    },
    Pin {
        id: String,
    },
    RuntimeProcess {
        observation_key: String,
    },
    Branch {
        repo_common_dir: String,
        refname: String,
    },
    Fork {
        provider_source_key: String,
    },
    ForgePr {
        provider: String,
        host: String,
        owner: String,
        repo: String,
        number: u64,
    },
}

impl DeclaredEndpoint {
    /// H-REF-001: shared codec entrypoint. Parses the compact
    /// `type:key=value,…` CLI form callers use for
    /// `conspectus declared` operations. Pre-H-REF-001 the
    /// parse + label sides lived only in `cli.rs`; centralizing
    /// them here means adding a new endpoint variant is one
    /// change instead of three.
    ///
    /// Field names match the declared TOML field names so the
    /// CLI syntax and the TOML store cannot drift.
    pub fn parse_compact(raw: &str) -> Result<Self, String> {
        let (kind, fields) = raw.split_once(':').ok_or_else(endpoint_syntax_error)?;
        let fields = parse_endpoint_fields(fields)?;
        match kind {
            "repo" => Ok(DeclaredEndpoint::Repo {
                common_dir: required_field(&fields, "common_dir")?,
            }),
            "checkout" => Ok(DeclaredEndpoint::Checkout {
                repo_common_dir: required_field(&fields, "repo_common_dir")?,
                root: required_field(&fields, "root")?,
            }),
            "workspace" => Ok(DeclaredEndpoint::Workspace {
                root: required_field(&fields, "root")?,
            }),
            "agent_session" => Ok(DeclaredEndpoint::AgentSession {
                harness_key: required_field(&fields, "harness_key")?,
                state_scope: required_field(&fields, "state_scope")?,
                session_key: required_field(&fields, "session_key")?,
            }),
            "mux_session" => Ok(DeclaredEndpoint::MuxSession {
                native_id: required_field(&fields, "native_id")?,
            }),
            "pin" => Ok(DeclaredEndpoint::Pin {
                id: required_field(&fields, "id")?,
            }),
            "runtime_process" => Ok(DeclaredEndpoint::RuntimeProcess {
                observation_key: required_field(&fields, "observation_key")?,
            }),
            "branch" => Ok(DeclaredEndpoint::Branch {
                repo_common_dir: required_field(&fields, "repo_common_dir")?,
                refname: required_field(&fields, "refname")?,
            }),
            "fork" => Ok(DeclaredEndpoint::Fork {
                provider_source_key: required_field(&fields, "provider_source_key")?,
            }),
            "forge_pr" => Ok(DeclaredEndpoint::ForgePr {
                provider: required_field(&fields, "provider")?,
                host: required_field(&fields, "host")?,
                owner: required_field(&fields, "owner")?,
                repo: required_field(&fields, "repo")?,
                number: required_field(&fields, "number")?
                    .parse()
                    .map_err(|_| "endpoint field `number` must be an integer".to_string())?,
            }),
            _ => Err(endpoint_syntax_error()),
        }
    }

    /// H-REF-001: canonical compact-form label for this
    /// endpoint. Mirror of [`Self::parse_compact`] so
    /// `endpoint.compact_label().parse_compact()` round-trips
    /// for every variant.
    pub fn compact_label(&self) -> String {
        match self {
            DeclaredEndpoint::Repo { common_dir } => {
                format!("repo:common_dir={common_dir}")
            }
            DeclaredEndpoint::Checkout {
                repo_common_dir,
                root,
            } => {
                format!("checkout:repo_common_dir={repo_common_dir},root={root}")
            }
            DeclaredEndpoint::Workspace { root } => {
                format!("workspace:root={root}")
            }
            DeclaredEndpoint::AgentSession {
                harness_key,
                state_scope,
                session_key,
            } => {
                format!(
                    "agent_session:harness_key={harness_key},state_scope={state_scope},session_key={session_key}"
                )
            }
            DeclaredEndpoint::MuxSession { native_id } => {
                format!("mux_session:native_id={native_id}")
            }
            DeclaredEndpoint::Pin { id } => {
                format!("pin:id={id}")
            }
            DeclaredEndpoint::RuntimeProcess { observation_key } => {
                format!("runtime_process:observation_key={observation_key}")
            }
            DeclaredEndpoint::Branch {
                repo_common_dir,
                refname,
            } => {
                format!("branch:repo_common_dir={repo_common_dir},refname={refname}")
            }
            DeclaredEndpoint::Fork {
                provider_source_key,
            } => {
                format!("fork:provider_source_key={provider_source_key}")
            }
            DeclaredEndpoint::ForgePr {
                provider,
                host,
                owner,
                repo,
                number,
            } => {
                format!(
                    "forge_pr:provider={provider},host={host},owner={owner},repo={repo},number={number}"
                )
            }
        }
    }
}

fn parse_endpoint_fields(raw: &str) -> Result<std::collections::BTreeMap<&str, &str>, String> {
    if raw.is_empty() {
        return Err(endpoint_syntax_error());
    }
    let mut fields = std::collections::BTreeMap::new();
    for part in raw.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(endpoint_syntax_error)?;
        if key.is_empty() || value.is_empty() {
            return Err(endpoint_syntax_error());
        }
        fields.insert(key, value);
    }
    Ok(fields)
}

fn required_field(
    fields: &std::collections::BTreeMap<&str, &str>,
    key: &str,
) -> Result<String, String> {
    fields
        .get(key)
        .map(|value| (*value).to_string())
        .ok_or_else(|| format!("missing endpoint field `{key}`"))
}

fn endpoint_syntax_error() -> String {
    "invalid endpoint syntax; expected type:key=value,... using declared TOML field names"
        .to_string()
}

pub fn parse_declared_document(text: &str) -> Result<DeclaredDocument, DeclaredParseError> {
    let document: DeclaredDocument =
        toml::from_str(text).map_err(|err| DeclaredParseError::MalformedToml(err.to_string()))?;
    validate_document(&document)?;
    Ok(document)
}

pub fn to_toml(document: &DeclaredDocument) -> Result<String, DeclaredSerializeError> {
    toml::to_string_pretty(document).map_err(|err| DeclaredSerializeError(err.to_string()))
}

fn validate_document(document: &DeclaredDocument) -> Result<(), DeclaredParseError> {
    let Some(declared) = &document.declared else {
        return Ok(());
    };

    if declared.schema_version != DECLARED_SCHEMA_VERSION {
        return Err(DeclaredParseError::UnsupportedSchemaVersion(
            declared.schema_version,
        ));
    }

    let mut ids = BTreeSet::new();
    for link in &declared.links {
        if link.id.trim().is_empty() {
            return Err(DeclaredParseError::EmptyId);
        }
        if !ids.insert(link.id.clone()) {
            return Err(DeclaredParseError::DuplicateId(link.id.clone()));
        }
        if link.state == DeclaredLinkState::Overridden && link.overridden_by.is_none() {
            return Err(DeclaredParseError::MissingOverriddenBy(link.id.clone()));
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredParseError {
    MalformedToml(String),
    UnsupportedSchemaVersion(u32),
    EmptyId,
    DuplicateId(String),
    MissingOverriddenBy(String),
}

impl fmt::Display for DeclaredParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToml(err) => write!(f, "malformed declared-link TOML: {err}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported declared.schema_version `{version}`")
            }
            Self::EmptyId => write!(f, "declared link id must not be empty"),
            Self::DuplicateId(id) => write!(f, "duplicate declared link id `{id}`"),
            Self::MissingOverriddenBy(id) => {
                write!(f, "overridden declared link `{id}` must set overridden_by")
            }
        }
    }
}

impl std::error::Error for DeclaredParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredSerializeError(String);

impl fmt::Display for DeclaredSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to serialize declared-link TOML: {}", self.0)
    }
}

impl std::error::Error for DeclaredSerializeError {}

#[derive(Debug)]
pub enum DeclaredWriteError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    Serialize { message: String },
    Write { path: PathBuf, source: io::Error },
}

impl fmt::Display for DeclaredWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Serialize { message } => {
                write!(f, "failed to serialize declared links: {message}")
            }
            Self::Write { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DeclaredWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { .. } | Self::Serialize { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
