//! Declared-link TOML file models.
//!
//! These types model the durable `[declared]` config section from ADR 0014.
//! Loading them does not imply graph discovery or persistence; later phases
//! convert validated entries into `GraphLink` evidence and write them back.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use crate::model::{
    GraphNode, GraphSnapshot, LinkEndpoint, NodeId, RelationKind, RelationKind::RootedAtPath,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredStoreSelection {
    pub kind: DeclaredStoreKind,
    pub path: PathBuf,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeclaredStoreKind {
    Project,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredWriteOutcome {
    pub path: PathBuf,
    pub changed: bool,
    pub link_count: usize,
}

pub fn select_store_for_declaration(
    source: &DeclaredEndpoint,
    target: &DeclaredEndpoint,
    snapshot: &GraphSnapshot,
    loader: &ConfigLoader,
) -> Option<DeclaredStoreSelection> {
    if let Some(root) =
        endpoint_project_root(source, snapshot).or_else(|| endpoint_project_root(target, snapshot))
    {
        let path = loader
            .locate_project_config(&root)
            .unwrap_or_else(|| root.join(PROJECT_CONFIG_FILENAME));
        return Some(DeclaredStoreSelection {
            kind: DeclaredStoreKind::Project,
            path,
        });
    }

    loader
        .user_config_path()
        .map(|path| DeclaredStoreSelection {
            kind: DeclaredStoreKind::User,
            path,
        })
}

/// Convert a discovered [`NodeId`] back into a [`DeclaredEndpoint`].
///
/// Used by `conspectus declared confirm` / `ignore` to derive a
/// declared link's endpoints from a candidate link's source / target
/// in the current graph.
pub fn declared_endpoint_from_node_id(id: &NodeId) -> DeclaredEndpoint {
    match id {
        NodeId::Repo(repo) => DeclaredEndpoint::Repo {
            common_dir: repo.common_dir.clone(),
        },
        NodeId::Checkout(checkout) => DeclaredEndpoint::Checkout {
            repo_common_dir: checkout.repo.common_dir.clone(),
            root: checkout.root.clone(),
        },
        NodeId::Workspace(workspace) => DeclaredEndpoint::Workspace {
            root: workspace.root.clone(),
        },
        NodeId::AgentSession(session) => DeclaredEndpoint::AgentSession {
            harness_key: session.harness_key.clone(),
            state_scope: session.state_scope.clone(),
            session_key: session.session_key.clone(),
        },
        NodeId::MuxSession(mux) => DeclaredEndpoint::MuxSession {
            native_id: mux.native_id.clone(),
        },
        NodeId::Pin(pin) => DeclaredEndpoint::Pin { id: pin.id.clone() },
        NodeId::RuntimeProcess(process) => DeclaredEndpoint::RuntimeProcess {
            observation_key: process.observation_key.clone(),
        },
        NodeId::Branch(branch) => DeclaredEndpoint::Branch {
            repo_common_dir: branch.repo.common_dir.clone(),
            refname: branch.refname.clone(),
        },
        NodeId::Fork(fork) => DeclaredEndpoint::Fork {
            provider_source_key: fork.provider_source_key.clone(),
        },
        NodeId::ForgePr(pr) => DeclaredEndpoint::ForgePr {
            provider: pr.provider.clone(),
            host: pr.host.clone(),
            owner: pr.owner.clone(),
            repo: pr.repo.clone(),
            number: pr.number,
        },
    }
}

/// Load every declared link with `id` from `paths` and return the
/// first match together with the file it came from. Used by `override`
/// so it can read the existing declaration, flip its state, and write
/// it back to the same store.
pub fn load_declared_link_by_id(
    paths: &[PathBuf],
    id: &str,
) -> Result<Option<(PathBuf, DeclaredLink)>, DeclaredParseError> {
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        let document = parse_declared_document(&text)?;
        if let Some(link) = document.links().iter().find(|link| link.id == id) {
            return Ok(Some((path.clone(), link.clone())));
        }
    }
    Ok(None)
}

pub fn upsert_declared_link(
    path: impl AsRef<Path>,
    link: DeclaredLink,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    let path = path.as_ref();
    let (mut document, declared) = load_document_for_write(path)?;
    let mut links = declared.links().to_vec();
    let mut changed = true;

    if let Some(existing) = links.iter_mut().find(|existing| existing.id == link.id) {
        changed = existing != &link;
        *existing = link;
    } else {
        links.push(link);
    }

    write_declared_links_if_changed(path, &mut document, links, changed)
}

pub fn remove_declared_link(
    path: impl AsRef<Path>,
    id: &str,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    let path = path.as_ref();
    let (mut document, declared) = load_document_for_write(path)?;
    let mut links = declared.links().to_vec();
    let original_len = links.len();
    links.retain(|link| link.id != id);
    let changed = links.len() != original_len;

    write_declared_links_if_changed(path, &mut document, links, changed)
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

fn load_document_for_write(
    path: &Path,
) -> Result<(toml_edit::DocumentMut, DeclaredDocument), DeclaredWriteError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(DeclaredWriteError::Read {
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    let edit_document = if text.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| DeclaredWriteError::Parse {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            })?
    };
    let declared = parse_declared_document(&text).map_err(|err| DeclaredWriteError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    Ok((edit_document, declared))
}

fn write_declared_links_if_changed(
    path: &Path,
    document: &mut toml_edit::DocumentMut,
    mut links: Vec<DeclaredLink>,
    changed: bool,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    links.sort_by(|left, right| left.id.cmp(&right.id));
    let link_count = links.len();

    if changed {
        if links.is_empty() {
            document.as_table_mut().remove("declared");
        } else {
            replace_declared_section(document, links)?;
        }
        write_document(path, document)?;
    }

    Ok(DeclaredWriteOutcome {
        path: path.to_path_buf(),
        changed,
        link_count,
    })
}

/// Persist `document` to `path`. If the document is empty (no
/// top-level keys remain after `[declared]` was stripped), remove the
/// file instead so the store leaves no dangling header behind.
fn write_document(
    path: &Path,
    document: &toml_edit::DocumentMut,
) -> Result<(), DeclaredWriteError> {
    if document.as_table().is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(DeclaredWriteError::Write {
                path: path.to_path_buf(),
                source: err,
            }),
        }
    } else {
        write_atomic(path, &document.to_string()).map_err(|err| DeclaredWriteError::Write {
            path: path.to_path_buf(),
            source: err,
        })
    }
}

fn replace_declared_section(
    document: &mut toml_edit::DocumentMut,
    links: Vec<DeclaredLink>,
) -> Result<(), DeclaredWriteError> {
    let declared_document = DeclaredDocument {
        declared: Some(DeclaredSection {
            schema_version: DECLARED_SCHEMA_VERSION,
            links,
        }),
    };
    let text = to_toml(&declared_document).map_err(|err| DeclaredWriteError::Serialize {
        message: err.to_string(),
    })?;
    let mut replacement =
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| DeclaredWriteError::Serialize {
                message: format!("serialized declared section did not parse: {err}"),
            })?;
    document["declared"] = replacement
        .as_table_mut()
        .remove("declared")
        .ok_or_else(|| DeclaredWriteError::Serialize {
            message: "serialized declared section was missing".to_string(),
        })?;
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");

    for attempt in 0..100 {
        let temp_path = parent.join(format!(".{file_name}.tmp-{}-{attempt}", std::process::id()));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };
        if let Err(err) = file
            .write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        drop(file);
        if let Err(err) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate temporary file next to {}",
            path.display()
        ),
    ))
}

fn endpoint_project_root(endpoint: &DeclaredEndpoint, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => repo_root(common_dir, snapshot),
        DeclaredEndpoint::Checkout { root, .. } | DeclaredEndpoint::Workspace { root } => {
            Some(PathBuf::from(root))
        }
        DeclaredEndpoint::Branch {
            repo_common_dir, ..
        } => repo_root(repo_common_dir, snapshot),
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => {
            let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
                harness_key.clone(),
                state_scope.clone(),
                session_key.clone(),
            ));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::MuxSession { native_id } => {
            let id = NodeId::MuxSession(crate::model::MuxSessionId::new(native_id.clone()));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::Pin { id } => snapshot
            .pins
            .iter()
            .find(|pin| pin.id == *id)
            .and_then(|pin| nearest_known_root(Path::new(&pin.cwd), snapshot)),
        DeclaredEndpoint::RuntimeProcess { observation_key } => {
            let id = NodeId::RuntimeProcess(crate::model::RuntimeProcessId::new(
                observation_key.clone(),
            ));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => {
            let id = NodeId::Fork(crate::model::ForkId::new(provider_source_key.clone()));
            fork_root(&id, snapshot).and_then(|root| nearest_known_root(Path::new(&root), snapshot))
        }
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            let id = NodeId::ForgePr(crate::model::ForgePrId::new(
                provider.clone(),
                host.clone(),
                owner.clone(),
                repo.clone(),
                *number,
            ));
            branch_for_pr(&id, snapshot)
                .and_then(|repo_common_dir| repo_root(&repo_common_dir, snapshot))
        }
    }
}

fn repo_root(common_dir: &str, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::Repo(repo) if repo.id.common_dir == common_dir => repo
            .source_paths
            .first()
            .map(PathBuf::from)
            .or_else(|| common_dir_parent(common_dir)),
        _ => None,
    })
}

fn common_dir_parent(common_dir: &str) -> Option<PathBuf> {
    let path = Path::new(common_dir);
    if path.file_name().is_some_and(|name| name == ".git") {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn node_cwd(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(session) if NodeId::AgentSession(session.id.clone()) == *id => {
            session.cwd.clone()
        }
        GraphNode::MuxSession(mux) if NodeId::MuxSession(mux.id.clone()) == *id => mux.cwd.clone(),
        GraphNode::RuntimeProcess(process) if NodeId::RuntimeProcess(process.id.clone()) == *id => {
            process.cwd.clone()
        }
        _ => None,
    })
}

fn fork_root(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    // H-HYG-006 wave 7: consult SnapshotIndex instead of a
    // linear scan. The index is (re)built here per call; a
    // future shared-index passthrough optimization can remove
    // the rebuild if the helper becomes hot.
    let index = crate::model::SnapshotIndex::new(snapshot);
    index.links_for(id, RootedAtPath).iter().find_map(|link| {
        if let LinkEndpoint::Unresolved { evidence } = &link.target {
            return evidence.path.clone();
        }
        None
    })
}

fn branch_for_pr(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    // H-HYG-006 wave 7: same shape as fork_root.
    let index = crate::model::SnapshotIndex::new(snapshot);
    index
        .links_for(id, RelationKind::BranchHasForgePr)
        .iter()
        .find_map(|link| {
            if let LinkEndpoint::Node {
                id: NodeId::Branch(branch),
            } = &link.target
            {
                return Some(branch.repo.common_dir.clone());
            }
            None
        })
}

fn nearest_known_root(path: &Path, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    let mut candidates = known_project_roots(snapshot);
    candidates.sort_by_key(|root| std::cmp::Reverse(root.as_os_str().len()));
    candidates
        .into_iter()
        .find(|root| path == root || path.starts_with(root))
}

fn known_project_roots(snapshot: &GraphSnapshot) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for node in &snapshot.nodes {
        match node {
            GraphNode::Repo(repo) => {
                roots.extend(repo.source_paths.iter().map(PathBuf::from));
                if let Some(parent) = common_dir_parent(&repo.common_dir) {
                    roots.insert(parent);
                }
            }
            GraphNode::Checkout(worktree) => {
                roots.insert(PathBuf::from(&worktree.root));
            }
            GraphNode::Workspace(workspace) => {
                roots.insert(PathBuf::from(&workspace.root));
            }
            _ => {}
        }
    }
    roots.into_iter().collect()
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
#[path = "declared_tests.rs"]
mod tests;
