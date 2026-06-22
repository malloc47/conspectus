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
mod tests {
    use super::*;
    use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
    use tempfile::TempDir;

    fn sample_entry(session_key: &str, display_name: &str) -> AliasEntry {
        AliasEntry {
            node: DeclaredEndpoint::AgentSession {
                harness_key: "codex".to_string(),
                state_scope: "/home/me/.codex".to_string(),
                session_key: session_key.to_string(),
            },
            display_name: display_name.to_string(),
            reason: None,
        }
    }

    #[test]
    fn aliases_document_round_trips_through_toml() {
        let document = AliasesDocument {
            aliases: Some(AliasesSection {
                schema_version: ALIASES_SCHEMA_VERSION,
                entries: vec![
                    sample_entry("alpha", "ingest-refactor"),
                    AliasEntry {
                        node: DeclaredEndpoint::MuxSession {
                            native_id: "editor".to_string(),
                        },
                        display_name: "editor".to_string(),
                        reason: Some("matches harness title intentionally".to_string()),
                    },
                ],
            }),
        };

        let encoded = to_toml(&document).expect("serialize");
        let decoded = parse_aliases_document(&encoded).expect("parse");

        assert_eq!(decoded, document);
        assert!(encoded.contains("[[aliases.entries]]"));
        assert!(encoded.contains("display_name"));
        assert!(encoded.contains("type = \"agent_session\""));
    }

    #[test]
    fn missing_aliases_section_yields_empty_document() {
        let document =
            parse_aliases_document("[session]\nprojection = \"agent\"\n").expect("parse");
        assert!(document.entries().is_empty());
    }

    #[test]
    fn unknown_keys_are_ignored_for_forward_compatibility() {
        let document = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1
            future = "ignored"

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "x"
            future_alias_key = true
            "#,
        )
        .expect("parse with unknown keys");
        assert_eq!(document.entries().len(), 1);
    }

    #[test]
    fn unsupported_schema_version_is_an_error() {
        let err = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 99
            "#,
        )
        .expect_err("unsupported version");
        assert_eq!(err, AliasParseError::UnsupportedSchemaVersion(99));
    }

    #[test]
    fn empty_display_name_is_an_error() {
        let err = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "   "
            "#,
        )
        .expect_err("empty display name");
        assert!(matches!(err, AliasParseError::EmptyDisplayName(_)));
    }

    #[test]
    fn duplicate_node_is_an_error() {
        let err = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "first"

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "second"
            "#,
        )
        .expect_err("duplicate node");
        assert!(matches!(err, AliasParseError::DuplicateNode(_)));
    }

    #[test]
    fn upsert_creates_file_and_parent_dirs() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("xdg").join(USER_CONFIG_RELATIVE);
        let outcome = upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("write");
        assert!(outcome.changed);
        assert_eq!(outcome.entry_count, 1);

        let parsed =
            parse_aliases_document(&fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(parsed.entries().len(), 1);
        assert_eq!(parsed.entries()[0].display_name, "name");
    }

    #[test]
    fn upsert_preserves_unrelated_sections() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");

        upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("write");

        let text = fs::read_to_string(&path).expect("read");
        assert!(text.contains("[session]"));
        assert!(text.contains("projection = \"mux\""));
        assert!(text.contains("[aliases]"));
        assert!(text.contains("[[aliases.entries]]"));
    }

    #[test]
    fn upsert_replaces_entry_for_same_node() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        upsert_alias_entry(&path, sample_entry("alpha", "old")).expect("seed");

        let outcome = upsert_alias_entry(&path, sample_entry("alpha", "new")).expect("replace");

        assert!(outcome.changed);
        let parsed =
            parse_aliases_document(&fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(parsed.entries().len(), 1);
        assert_eq!(parsed.entries()[0].display_name, "new");
    }

    #[test]
    fn upsert_skips_unchanged_replacement() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        let entry = sample_entry("alpha", "name");
        upsert_alias_entry(&path, entry.clone()).expect("seed");

        let outcome = upsert_alias_entry(&path, entry).expect("same");

        assert!(!outcome.changed);
        assert_eq!(outcome.entry_count, 1);
    }

    #[test]
    fn upsert_rejects_empty_display_name() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        let err = upsert_alias_entry(&path, sample_entry("alpha", "   ")).expect_err("empty name");
        assert!(matches!(err, AliasWriteError::EmptyDisplayName));
        assert!(!path.exists());
    }

    #[test]
    fn remove_drops_entry_and_deletes_file_when_section_empties() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("seed");
        assert!(path.is_file());

        let outcome =
            remove_alias_entry(&path, &sample_entry("alpha", "name").node).expect("remove");

        assert!(outcome.changed);
        assert_eq!(outcome.entry_count, 0);
        assert!(!path.exists());
    }

    #[test]
    fn remove_preserves_unrelated_sections_when_section_empties() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");
        upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("seed alias");

        let outcome =
            remove_alias_entry(&path, &sample_entry("alpha", "name").node).expect("remove");

        assert!(outcome.changed);
        assert_eq!(outcome.entry_count, 0);
        let text = fs::read_to_string(&path).expect("read");
        assert!(
            text.contains("[session]"),
            "session section preserved:\n{text}"
        );
        assert!(
            !text.contains("[aliases]"),
            "aliases section pruned:\n{text}"
        );
    }

    #[test]
    fn remove_missing_entry_does_not_create_file() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("missing").join(PROJECT_CONFIG_FILENAME);

        let outcome =
            remove_alias_entry(&path, &sample_entry("alpha", "x").node).expect("remove missing");

        assert!(!outcome.changed);
        assert_eq!(outcome.entry_count, 0);
        assert!(!path.exists());
    }

    #[test]
    fn upsert_reports_malformed_existing_toml_without_mutating() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);
        let original = "[aliases\n";
        fs::write(&path, original).expect("seed");

        let err = upsert_alias_entry(&path, sample_entry("alpha", "name")).expect_err("malformed");

        assert!(matches!(err, AliasWriteError::Parse { .. }));
        assert_eq!(fs::read_to_string(&path).expect("read"), original);
    }

    #[test]
    fn overlay_insert_if_absent_respects_existing_entry() {
        let mut overlay = AliasOverlay::new();
        let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
            "codex", "/state", "alpha",
        ));
        assert!(overlay.insert_if_absent(id.clone(), "first".to_string()));
        assert_eq!(overlay.get(&id), Some("first"));
        assert!(!overlay.insert_if_absent(id.clone(), "second".to_string()));
        assert_eq!(overlay.get(&id), Some("first"));
    }

    #[test]
    fn resolve_display_label_prefers_alias_over_title() {
        let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
            "codex", "/state", "alpha",
        ));
        let mut overlay = AliasOverlay::new();
        overlay.insert(id.clone(), "alias".to_string());

        assert_eq!(
            resolve_display_label(Some(&overlay), &id, Some("title")),
            Some("alias")
        );
        assert_eq!(
            resolve_display_label(Some(&overlay), &id, None),
            Some("alias")
        );
        assert_eq!(
            resolve_display_label(None, &id, Some("title")),
            Some("title")
        );
        assert_eq!(resolve_display_label(None, &id, None), None);
    }

    #[test]
    fn load_alias_entry_for_node_returns_first_matching_store() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join(PROJECT_CONFIG_FILENAME);
        let user = temp.path().join(USER_CONFIG_RELATIVE);
        upsert_alias_entry(&project, sample_entry("alpha", "project-name")).expect("project");
        upsert_alias_entry(&user, sample_entry("alpha", "user-name")).expect("user");

        let (found_path, entry) = load_alias_entry_for_node(
            &[project.clone(), user.clone()],
            &sample_entry("alpha", "x").node,
        )
        .expect("ok")
        .expect("found");

        assert_eq!(found_path, project);
        assert_eq!(entry.display_name, "project-name");
    }

    #[test]
    fn load_alias_entry_for_missing_node_returns_none() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join(PROJECT_CONFIG_FILENAME);

        let result =
            load_alias_entry_for_node(&[path], &sample_entry("alpha", "x").node).expect("ok");
        assert!(result.is_none());
    }

    // Touch `ConfigLoader` to keep import-pruning tools honest — this
    // module's discovery wiring lives in `crate::discovery::aliases`
    // which exercises `ConfigLoader::user_config_path` and friends.
    #[test]
    fn config_loader_remains_a_dependency_marker() {
        let _ = ConfigLoader::new();
        let _ = alias_node_from_node_id(&NodeId::AgentSession(crate::model::AgentSessionId::new(
            "codex", "/state", "alpha",
        )));
    }
}
