//! Session-alias overlay TOML file model.
//!
//! Per ADR 0029, Conspectus stores operator-chosen display names for
//! agent sessions as a sidecar `[aliases]` table in the existing TOML
//! config files. The harness-native title is never mutated; the alias
//! overlay is applied at projection time with precedence
//! `alias > title > id-suffix`.
//!
//! This module is intentionally parallel to `crate::declared`: the
//! storage schema reuses [`DeclaredEndpoint`] as the node-key encoding
//! and the atomic-write machinery in `crate::declared::write_atomic`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::declared::{DeclaredEndpoint, declared_endpoint_from_node_id, write_atomic};
use crate::model::NodeId;

pub const ALIASES_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AliasesDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<AliasesSection>,
}

impl AliasesDocument {
    pub fn entries(&self) -> &[AliasEntry] {
        self.aliases
            .as_ref()
            .map(|section| section.entries.as_slice())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AliasesSection {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AliasEntry>,
}

impl Default for AliasesSection {
    fn default() -> Self {
        Self {
            schema_version: ALIASES_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AliasEntry {
    pub node: DeclaredEndpoint,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub fn parse_aliases_document(text: &str) -> Result<AliasesDocument, AliasParseError> {
    let document: AliasesDocument =
        toml::from_str(text).map_err(|err| AliasParseError::MalformedToml(err.to_string()))?;
    validate_document(&document)?;
    Ok(document)
}

pub fn to_toml(document: &AliasesDocument) -> Result<String, AliasSerializeError> {
    toml::to_string_pretty(document).map_err(|err| AliasSerializeError(err.to_string()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasWriteOutcome {
    pub path: PathBuf,
    pub changed: bool,
    pub entry_count: usize,
}

pub fn upsert_alias_entry(
    path: impl AsRef<Path>,
    entry: AliasEntry,
) -> Result<AliasWriteOutcome, AliasWriteError> {
    let path = path.as_ref();
    if entry.display_name.trim().is_empty() {
        return Err(AliasWriteError::EmptyDisplayName);
    }
    let (mut document, parsed) = load_document_for_write(path)?;
    let mut entries = parsed.entries().to_vec();
    let mut changed = true;

    if let Some(existing) = entries
        .iter_mut()
        .find(|existing| existing.node == entry.node)
    {
        changed = existing != &entry;
        *existing = entry;
    } else {
        entries.push(entry);
    }

    write_entries_if_changed(path, &mut document, entries, changed)
}

pub fn remove_alias_entry(
    path: impl AsRef<Path>,
    node: &DeclaredEndpoint,
) -> Result<AliasWriteOutcome, AliasWriteError> {
    let path = path.as_ref();
    let (mut document, parsed) = load_document_for_write(path)?;
    let mut entries = parsed.entries().to_vec();
    let original_len = entries.len();
    entries.retain(|entry| &entry.node != node);
    let changed = entries.len() != original_len;

    write_entries_if_changed(path, &mut document, entries, changed)
}

/// Load the alias entry for `node` from the first store in `paths`
/// that contains a matching entry. Returns the file the entry came
/// from so callers (`alias remove`, lockstep) can write back to the
/// same store.
pub fn load_alias_entry_for_node(
    paths: &[PathBuf],
    node: &DeclaredEndpoint,
) -> Result<Option<(PathBuf, AliasEntry)>, AliasParseError> {
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        let document = parse_aliases_document(&text)?;
        if let Some(entry) = document.entries().iter().find(|entry| &entry.node == node) {
            return Ok(Some((path.clone(), entry.clone())));
        }
    }
    Ok(None)
}

/// Resolved alias overlay carried alongside a [`GraphSnapshot`].
/// Maps a [`NodeId`] to the operator-chosen display name with local
/// stores taking precedence over global stores per ADR 0029.
#[derive(
    Clone, Debug, Default, Eq, PartialEq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct AliasOverlay {
    entries: BTreeMap<NodeId, String>,
}

impl AliasOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: &NodeId) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &str)> {
        self.entries.iter().map(|(id, name)| (id, name.as_str()))
    }

    /// Insert `display_name` for `node_id` only if no higher-priority
    /// entry is already present. Returns `true` when the entry was
    /// stored.
    pub fn insert_if_absent(&mut self, node_id: NodeId, display_name: String) -> bool {
        if self.entries.contains_key(&node_id) {
            return false;
        }
        self.entries.insert(node_id, display_name);
        true
    }

    /// Convenience for tests / callers that want unconditional
    /// insertion (last-wins).
    pub fn insert(&mut self, node_id: NodeId, display_name: String) {
        self.entries.insert(node_id, display_name);
    }
}

/// Compute the display label for `node_id` and `harness_title` per
/// ADR 0029's `alias > title > id-suffix` precedence. Returns `None`
/// when no source is available so callers can decide whether to
/// render a short id suffix or leave the column blank.
pub fn resolve_display_label<'a>(
    overlay: Option<&'a AliasOverlay>,
    node_id: &NodeId,
    harness_title: Option<&'a str>,
) -> Option<&'a str> {
    if let Some(overlay) = overlay
        && let Some(alias) = overlay.get(node_id)
    {
        return Some(alias);
    }
    harness_title
}

/// Convert a [`NodeId`] back into the [`DeclaredEndpoint`] encoding
/// used as the alias node key. Mirrors
/// [`declared_endpoint_from_node_id`] so callers don't reach across
/// modules for the conversion.
pub fn alias_node_from_node_id(id: &NodeId) -> DeclaredEndpoint {
    declared_endpoint_from_node_id(id)
}

fn validate_document(document: &AliasesDocument) -> Result<(), AliasParseError> {
    let Some(section) = &document.aliases else {
        return Ok(());
    };

    if section.schema_version != ALIASES_SCHEMA_VERSION {
        return Err(AliasParseError::UnsupportedSchemaVersion(
            section.schema_version,
        ));
    }

    let mut seen = BTreeSet::new();
    for entry in &section.entries {
        if entry.display_name.trim().is_empty() {
            return Err(AliasParseError::EmptyDisplayName(format!(
                "{:?}",
                entry.node
            )));
        }
        let key = endpoint_key(&entry.node);
        if !seen.insert(key.clone()) {
            return Err(AliasParseError::DuplicateNode(key));
        }
    }

    Ok(())
}

fn load_document_for_write(
    path: &Path,
) -> Result<(toml_edit::DocumentMut, AliasesDocument), AliasWriteError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(AliasWriteError::Read {
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    let edit_document = if text.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| AliasWriteError::Parse {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            })?
    };
    let parsed = parse_aliases_document(&text).map_err(|err| AliasWriteError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    Ok((edit_document, parsed))
}

fn write_entries_if_changed(
    path: &Path,
    document: &mut toml_edit::DocumentMut,
    mut entries: Vec<AliasEntry>,
    changed: bool,
) -> Result<AliasWriteOutcome, AliasWriteError> {
    entries.sort_by(|left, right| endpoint_key(&left.node).cmp(&endpoint_key(&right.node)));
    let entry_count = entries.len();

    if changed {
        if entries.is_empty() {
            document.as_table_mut().remove("aliases");
        } else {
            replace_aliases_section(document, entries)?;
        }
        write_document(path, document)?;
    }

    Ok(AliasWriteOutcome {
        path: path.to_path_buf(),
        changed,
        entry_count,
    })
}

fn write_document(path: &Path, document: &toml_edit::DocumentMut) -> Result<(), AliasWriteError> {
    if document.as_table().is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AliasWriteError::Write {
                path: path.to_path_buf(),
                source: err,
            }),
        }
    } else {
        write_atomic(path, &document.to_string()).map_err(|err| AliasWriteError::Write {
            path: path.to_path_buf(),
            source: err,
        })
    }
}

fn replace_aliases_section(
    document: &mut toml_edit::DocumentMut,
    entries: Vec<AliasEntry>,
) -> Result<(), AliasWriteError> {
    let aliases_document = AliasesDocument {
        aliases: Some(AliasesSection {
            schema_version: ALIASES_SCHEMA_VERSION,
            entries,
        }),
    };
    let text = to_toml(&aliases_document).map_err(|err| AliasWriteError::Serialize {
        message: err.to_string(),
    })?;
    let mut replacement =
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| AliasWriteError::Serialize {
                message: format!("serialized aliases section did not parse: {err}"),
            })?;
    document["aliases"] = replacement
        .as_table_mut()
        .remove("aliases")
        .ok_or_else(|| AliasWriteError::Serialize {
            message: "serialized aliases section was missing".to_string(),
        })?;
    Ok(())
}

fn endpoint_key(endpoint: &DeclaredEndpoint) -> String {
    serde_json::to_string(endpoint).unwrap_or_else(|_| format!("{endpoint:?}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AliasParseError {
    MalformedToml(String),
    UnsupportedSchemaVersion(u32),
    EmptyDisplayName(String),
    DuplicateNode(String),
}

impl fmt::Display for AliasParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToml(err) => write!(f, "malformed aliases TOML: {err}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported aliases.schema_version `{version}`")
            }
            Self::EmptyDisplayName(node) => {
                write!(f, "alias entry for {node} has empty display_name")
            }
            Self::DuplicateNode(key) => write!(f, "duplicate alias entry for node `{key}`"),
        }
    }
}

impl std::error::Error for AliasParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasSerializeError(String);

impl fmt::Display for AliasSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to serialize aliases TOML: {}", self.0)
    }
}

impl std::error::Error for AliasSerializeError {}

#[derive(Debug)]
pub enum AliasWriteError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    Serialize { message: String },
    Write { path: PathBuf, source: io::Error },
    EmptyDisplayName,
}

impl fmt::Display for AliasWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Serialize { message } => {
                write!(f, "failed to serialize alias entries: {message}")
            }
            Self::Write { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
            Self::EmptyDisplayName => write!(f, "alias display_name must not be empty"),
        }
    }
}

impl std::error::Error for AliasWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { .. } | Self::Serialize { .. } | Self::EmptyDisplayName => None,
        }
    }
}

#[cfg(test)]
#[path = "aliases_tests.rs"]
mod tests;
