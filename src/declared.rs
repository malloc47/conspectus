//! Declared-link TOML file models.
//!
//! These types model the durable `[declared]` config section from ADR 0014.
//! Loading them does not imply graph discovery or persistence; later phases
//! convert validated entries into `GraphLink` evidence and write them back.

use std::collections::BTreeSet;
use std::fmt;
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
    Worktree {
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

fn endpoint_project_root(endpoint: &DeclaredEndpoint, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => repo_root(common_dir, snapshot),
        DeclaredEndpoint::Worktree { root, .. } | DeclaredEndpoint::Workspace { root } => {
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
        _ => None,
    })
}

fn fork_root(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    snapshot.candidate_links.iter().find_map(|link| {
        if &link.source == id
            && link.relation == RootedAtPath
            && let LinkEndpoint::Unresolved { evidence } = &link.target
        {
            return evidence.path.clone();
        }
        None
    })
}

fn branch_for_pr(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    snapshot.candidate_links.iter().find_map(|link| {
        if &link.source == id
            && link.relation == RelationKind::BranchHasForgePr
            && let LinkEndpoint::Node {
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
            GraphNode::Worktree(worktree) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, Confidence, ForgePrId, ForkId, ForkNode,
        Freshness, GraphLink, LinkState, MuxSessionId, MuxSessionNode, RepoId, RepoNode,
        SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode, WorktreeId, WorktreeNode,
    };
    use tempfile::TempDir;

    fn sample_document() -> DeclaredDocument {
        DeclaredDocument {
            declared: Some(DeclaredSection {
                schema_version: DECLARED_SCHEMA_VERSION,
                links: vec![
                    DeclaredLink {
                        id: "codex-alpha-to-editor".to_string(),
                        relation: RelationKind::LinkedToMux,
                        state: DeclaredLinkState::Active,
                        source: DeclaredEndpoint::AgentSession {
                            harness_key: "codex".to_string(),
                            state_scope: "/home/me/.codex".to_string(),
                            session_key: "alpha".to_string(),
                        },
                        target: DeclaredEndpoint::MuxSession {
                            native_id: "tmux:editor".to_string(),
                        },
                        reason: None,
                        overridden_by: None,
                        label: Some("alpha editor".to_string()),
                    },
                    DeclaredLink {
                        id: "ignore-old-editor".to_string(),
                        relation: RelationKind::LinkedToMux,
                        state: DeclaredLinkState::Ignored,
                        source: DeclaredEndpoint::AgentSession {
                            harness_key: "codex".to_string(),
                            state_scope: "/home/me/.codex".to_string(),
                            session_key: "alpha".to_string(),
                        },
                        target: DeclaredEndpoint::MuxSession {
                            native_id: "tmux:old-editor".to_string(),
                        },
                        reason: Some("stale".to_string()),
                        overridden_by: None,
                        label: None,
                    },
                ],
            }),
        }
    }

    #[test]
    fn declared_document_round_trips_through_toml() {
        let document = sample_document();

        let encoded = to_toml(&document).expect("serialize");
        let decoded = parse_declared_document(&encoded).expect("parse");

        assert_eq!(decoded, document);
        assert!(encoded.contains("[[declared.links]]"));
        assert!(encoded.contains("type = \"agent_session\""));
    }

    #[test]
    fn missing_declared_section_yields_empty_document() {
        let document = parse_declared_document("[session]\nprojection = \"agent\"\n")
            .expect("parse session-only config");

        assert!(document.links().is_empty());
    }

    #[test]
    fn unknown_keys_are_ignored_for_forward_compatibility() {
        let document = parse_declared_document(
            r#"
            [declared]
            schema_version = 1
            future = "ignored"

            [[declared.links]]
            id = "repo-to-worktree"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "worktree", repo_common_dir = "/repo/.git", root = "/repo" }
            target = { type = "repo", common_dir = "/repo/.git" }
            future_link_key = true
            "#,
        )
        .expect("parse with unknown keys");

        assert_eq!(document.links().len(), 1);
    }

    #[test]
    fn all_endpoint_shapes_parse() {
        let document = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "repo-worktree"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "worktree", repo_common_dir = "/repo/.git", root = "/repo" }
            target = { type = "repo", common_dir = "/repo/.git" }

            [[declared.links]]
            id = "workspace-repo"
            relation = "workspace_contains_repo"
            state = "active"
            source = { type = "workspace", root = "/workspace" }
            target = { type = "repo", common_dir = "/workspace/repo/.git" }

            [[declared.links]]
            id = "branch-pr"
            relation = "branch_has_forge_pr"
            state = "active"
            source = { type = "forge_pr", provider = "github", host = "github.com", owner = "octo", repo = "repo", number = 7 }
            target = { type = "branch", repo_common_dir = "/repo/.git", refname = "refs/heads/main" }

            [[declared.links]]
            id = "fork-session"
            relation = "associated_with"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "fork", provider_source_key = "atelier:alpha" }
            "#,
        )
        .expect("parse endpoints");

        assert_eq!(document.links().len(), 4);
    }

    #[test]
    fn malformed_declared_toml_is_an_error() {
        let err = parse_declared_document("[declared\n").expect_err("malformed");

        assert!(matches!(err, DeclaredParseError::MalformedToml(_)));
    }

    #[test]
    fn unsupported_schema_version_is_an_error() {
        let err = parse_declared_document(
            r#"
            [declared]
            schema_version = 99
            "#,
        )
        .expect_err("unsupported version");

        assert_eq!(err, DeclaredParseError::UnsupportedSchemaVersion(99));
    }

    #[test]
    fn duplicate_declared_ids_are_an_error() {
        let err = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "same"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:a" }

            [[declared.links]]
            id = "same"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s2" }
            target = { type = "mux_session", native_id = "tmux:b" }
            "#,
        )
        .expect_err("duplicate id");

        assert_eq!(err, DeclaredParseError::DuplicateId("same".to_string()));
    }

    #[test]
    fn overridden_links_must_name_replacement() {
        let err = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "old"
            relation = "linked_to_mux"
            state = "overridden"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:a" }
            "#,
        )
        .expect_err("missing overridden_by");

        assert_eq!(
            err,
            DeclaredParseError::MissingOverriddenBy("old".to_string())
        );
    }

    #[test]
    fn store_selection_uses_repo_project_config_when_endpoint_is_repo_rooted() {
        let temp = TempDir::new().expect("temp");
        let repo_root = temp.path().join("repo");
        let common_dir = repo_root.join(".git");
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::Repo(RepoNode {
                id: RepoId::new(path_string(&common_dir)),
                common_dir: path_string(&common_dir),
                source_paths: vec![path_string(&repo_root)],
                remotes: Vec::new(),
            })],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::Repo {
                common_dir: path_string(&common_dir),
            },
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.kind, DeclaredStoreKind::Project);
        assert_eq!(selection.path, repo_root.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn store_selection_reuses_existing_workspace_config_for_nested_worktree() {
        let temp = TempDir::new().expect("temp");
        let workspace = temp.path().join("workspace");
        let repo = workspace.join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        std::fs::write(workspace.join(PROJECT_CONFIG_FILENAME), "").expect("config");
        let snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::Workspace(WorkspaceNode {
                    id: WorkspaceId::new(path_string(&workspace)),
                    root: path_string(&workspace),
                    provider: None,
                    name: None,
                }),
                GraphNode::Worktree(WorktreeNode {
                    id: WorktreeId::new(
                        RepoId::new(path_string(repo.join(".git"))),
                        path_string(&repo),
                    ),
                    root: path_string(&repo),
                    git_dir: None,
                    current_branch: None,
                }),
            ],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::Worktree {
                repo_common_dir: path_string(repo.join(".git")),
                root: path_string(&repo),
            },
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.path, workspace.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn store_selection_uses_user_config_for_orphan_agent_and_mux_only_links() {
        let temp = TempDir::new().expect("temp");
        let xdg = temp.path().join("xdg");
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::AgentSession {
                harness_key: "codex".to_string(),
                state_scope: "/state".to_string(),
                session_key: "s1".to_string(),
            },
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &GraphSnapshot::empty(),
            &ConfigLoader::new()
                .with_home(temp.path())
                .with_xdg_config_home(&xdg),
        )
        .expect("selection");

        assert_eq!(selection.kind, DeclaredStoreKind::User);
        assert_eq!(
            selection.path,
            xdg.join(crate::config::USER_CONFIG_RELATIVE)
        );
    }

    #[test]
    fn store_selection_uses_session_cwd_when_it_sits_under_known_worktree() {
        let temp = TempDir::new().expect("temp");
        let repo = temp.path().join("repo");
        let child = repo.join("nested");
        let snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::Worktree(WorktreeNode {
                    id: WorktreeId::new(
                        RepoId::new(path_string(repo.join(".git"))),
                        path_string(&repo),
                    ),
                    root: path_string(&repo),
                    git_dir: None,
                    current_branch: None,
                }),
                GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "/state", "s1"),
                    harness_key: "codex".to_string(),
                    cwd: Some(path_string(&child)),
                    title: None,
                }),
            ],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::AgentSession {
                harness_key: "codex".to_string(),
                state_scope: "/state".to_string(),
                session_key: "s1".to_string(),
            },
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn store_selection_uses_mux_cwd_when_it_sits_under_known_worktree() {
        let temp = TempDir::new().expect("temp");
        let repo = temp.path().join("repo");
        let child = repo.join("nested");
        let snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::Worktree(WorktreeNode {
                    id: WorktreeId::new(
                        RepoId::new(path_string(repo.join(".git"))),
                        path_string(&repo),
                    ),
                    root: path_string(&repo),
                    git_dir: None,
                    current_branch: None,
                }),
                GraphNode::MuxSession(MuxSessionNode {
                    id: MuxSessionId::new("tmux:editor"),
                    backend: "tmux".to_string(),
                    native_id: "tmux:editor".to_string(),
                    cwd: Some(path_string(&child)),
                    activity_epoch: None,
                    created_epoch: None,
                }),
            ],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &DeclaredEndpoint::AgentSession {
                harness_key: "codex".to_string(),
                state_scope: "/state".to_string(),
                session_key: "s1".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn store_selection_uses_branch_repo_root_for_branch_pr_links() {
        let temp = TempDir::new().expect("temp");
        let repo = temp.path().join("repo");
        let common_dir = repo.join(".git");
        let branch_id = BranchId::new(RepoId::new(path_string(&common_dir)), "refs/heads/main");
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::Repo(RepoNode {
                id: RepoId::new(path_string(&common_dir)),
                common_dir: path_string(&common_dir),
                source_paths: vec![path_string(&repo)],
                remotes: Vec::new(),
            })],
            candidate_links: vec![GraphLink {
                id: "pr-branch".to_string(),
                source: NodeId::ForgePr(pr_id.clone()),
                target: LinkEndpoint::Node {
                    id: NodeId::Branch(branch_id),
                },
                relation: RelationKind::BranchHasForgePr,
                provenance: crate::model::Provenance::StrongDiscovered,
                confidence: Confidence::High,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            }],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::ForgePr {
                provider: "github".to_string(),
                host: "github.com".to_string(),
                owner: "octo".to_string(),
                repo: "repo".to_string(),
                number: 7,
            },
            &DeclaredEndpoint::Branch {
                repo_common_dir: path_string(&common_dir),
                refname: "refs/heads/main".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn store_selection_uses_fork_root_when_under_known_workspace() {
        let temp = TempDir::new().expect("temp");
        let workspace = temp.path().join("workspace");
        let fork_root_path = workspace.join("forks/alpha");
        let fork_id = ForkId::new("atelier:alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::Workspace(WorkspaceNode {
                    id: WorkspaceId::new(path_string(&workspace)),
                    root: path_string(&workspace),
                    provider: None,
                    name: None,
                }),
                GraphNode::Fork(ForkNode {
                    id: fork_id.clone(),
                    provider: "atelier".to_string(),
                    provider_source_key: "atelier:alpha".to_string(),
                    name: Some("alpha".to_string()),
                    scope: None,
                    capabilities: Vec::new(),
                }),
            ],
            candidate_links: vec![GraphLink {
                id: "fork-root".to_string(),
                source: NodeId::Fork(fork_id),
                target: LinkEndpoint::Unresolved {
                    evidence: UnresolvedEndpoint {
                        node_type: "path".to_string(),
                        harness_key: None,
                        native_id: None,
                        state_scope: None,
                        path: Some(path_string(&fork_root_path)),
                        metadata: Default::default(),
                    },
                },
                relation: RootedAtPath,
                provenance: crate::model::Provenance::StrongDiscovered,
                confidence: Confidence::High,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            }],
            ..GraphSnapshot::empty()
        };
        let selection = select_store_for_declaration(
            &DeclaredEndpoint::Fork {
                provider_source_key: "atelier:alpha".to_string(),
            },
            &DeclaredEndpoint::MuxSession {
                native_id: "tmux:editor".to_string(),
            },
            &snapshot,
            &ConfigLoader::new().with_home(temp.path()),
        )
        .expect("selection");

        assert_eq!(selection.path, workspace.join(PROJECT_CONFIG_FILENAME));
    }

    fn path_string(path: impl AsRef<Path>) -> String {
        path.as_ref().to_string_lossy().to_string()
    }
}
