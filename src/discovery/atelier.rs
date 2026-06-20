//! Atelier workspace metadata discovery.
//!
//! Atelier workspaces are represented on disk by an `atelier.toml` file at the
//! workspace root. Conspectus treats that file as a read-only provider manifest:
//! the `[workspace]` table names the workspace, and each `[[repos]]` entry names
//! a repository that belongs to it. Atelier stores the source path in
//! `repo.path`, while materialized checkouts are expected to live directly under
//! the workspace root using `repo.name`; discovery probes that checkout first and
//! falls back to the source path when the checkout is not present.
//!
//! Fork metadata comes from `.atelier/forks/index.toml`. Conspectus parses the
//! subset of the index needed to describe workspace-scoped forks, their source
//! repositories, associated worktrees, branches, parent fork relationships, and
//! harness/session capabilities. Unknown TOML fields are intentionally ignored so
//! new Atelier metadata can be added without breaking discovery.
//!
//! This module is the provider boundary. It preserves Atelier provenance in
//! provider names, source metadata, and stable link identifiers, but maps the
//! underlying concepts into Conspectus graph nodes and relation kinds as soon as
//! possible. Downstream resolution and rendering should be able to reason over
//! the generic graph model without understanding Atelier's implementation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::discovery::git::{GitProbe, fragment_from_probe};
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence, ForkId, ForkNode, Freshness,
    GraphLink, GraphNode, LinkEndpoint, LinkState, Metadata, NodeId, Provenance, RelationKind,
    RepoId, RepoNode, SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
};

pub const ATELIER_CONFIG_FILENAME: &str = "atelier.toml";
pub const ATELIER_FORK_INDEX_PATH: &str = ".atelier/forks/index.toml";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtelierWorkspaceDiscovery {
    git_probe: GitProbe,
}

impl AtelierWorkspaceDiscovery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DiscoveryProvider for AtelierWorkspaceDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut fragments = Vec::new();

        for root in context.roots() {
            if let Some(config_path) = find_atelier_config(root) {
                fragments.push(self.discover_config(&config_path)?);
            }
        }

        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

impl AtelierWorkspaceDiscovery {
    fn discover_config(&self, config_path: &Path) -> Result<GraphFragment> {
        let workspace_root = config_path
            .parent()
            .context("atelier config path has no parent")?
            .canonicalize()
            .with_context(|| {
                format!(
                    "failed to canonicalize Atelier workspace root: {}",
                    config_path.display()
                )
            })?;
        let config = AtelierWorkspaceConfig::load(config_path)?;
        let workspace_id = WorkspaceId::new(path_string(&workspace_root));
        let workspace_node = GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: path_string(&workspace_root),
            provider: Some("atelier".to_string()),
            name: Some(config.workspace.name.clone()),
        });
        let workspace = NodeId::Workspace(workspace_id);
        let mut child_fragments = Vec::new();
        let mut repo_links = Vec::new();

        for repo in &config.repos {
            let repo_root = workspace_root.join(&repo.name);
            let provider_source_path = absolutize(&workspace_root, &repo.path);
            let member_path_kind = workspace_member_path_kind(&repo_root);

            if let Some(probe) = self.git_probe.probe(&repo_root)? {
                let repo_id =
                    NodeId::Repo(crate::model::RepoId::new(path_string(&probe.common_dir)));
                let canonical_checkout_root = canonicalized_or_original(&probe.worktree_root);
                repo_links.push(atelier_workspace_repo_link(
                    workspace.clone(),
                    repo_id,
                    WorkspaceRepoMemberMetadata {
                        repo_name: &repo.name,
                        logical_path: &repo_root,
                        provider_source_path: &provider_source_path,
                        canonical_checkout_root: Some(&canonical_checkout_root),
                        member_path_kind,
                    },
                    "atelier.toml repo entry",
                ));
                child_fragments.push(fragment_from_probe(&probe));
            } else {
                repo_links.push(atelier_workspace_repo_link(
                    workspace.clone(),
                    NodeId::Repo(crate::model::RepoId::new(path_string(
                        &provider_source_path,
                    ))),
                    WorkspaceRepoMemberMetadata {
                        repo_name: &repo.name,
                        logical_path: &repo_root,
                        provider_source_path: &provider_source_path,
                        canonical_checkout_root: None,
                        member_path_kind,
                    },
                    "atelier.toml repo entry without discovered checkout",
                ));
            }
        }

        let mut snapshot = merge_fragments(child_fragments);
        snapshot.nodes.push(workspace_node);
        snapshot.candidate_links.extend(repo_links);

        let fork_records =
            AtelierForkIndex::load(&workspace_root)?.into_provider_records(&workspace_root);
        let fork_fragment = fork_records_fragment(&workspace, &fork_records);
        snapshot.nodes.extend(fork_fragment.nodes);
        snapshot
            .candidate_links
            .extend(fork_fragment.candidate_links);
        snapshot.diagnostics.extend(fork_fragment.diagnostics);

        snapshot.canonicalize();
        Ok(snapshot_fragment(snapshot))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AtelierWorkspaceConfig {
    pub workspace: AtelierWorkspaceMetadata,
    #[serde(default)]
    pub repos: Vec<AtelierRepoEntry>,
}

impl AtelierWorkspaceConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct AtelierForkIndex {
    #[serde(default)]
    pub forks: Vec<AtelierForkEntry>,
}

impl AtelierForkIndex {
    pub fn path_for(workspace_root: &Path) -> PathBuf {
        workspace_root.join(ATELIER_FORK_INDEX_PATH)
    }

    pub fn load(workspace_root: &Path) -> Result<Self> {
        let path = Self::path_for(workspace_root);

        if !path.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn into_provider_records(self, workspace_root: &Path) -> Vec<AtelierForkRecord> {
        self.forks
            .into_iter()
            .map(|entry| AtelierForkRecord::from_entry(workspace_root, entry))
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct AtelierForkEntry {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub created_epoch: i64,
    pub mode: AtelierForkMode,
    pub root: PathBuf,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub state: AtelierForkState,
    #[serde(default)]
    pub repos: Vec<AtelierForkRepoEntry>,
    #[serde(default)]
    pub harness: Vec<AtelierForkHarnessEntry>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AtelierForkMode {
    Worktree,
    Selected,
    Research,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AtelierForkState {
    #[default]
    Inherit,
    Shared,
    Isolated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct AtelierForkRepoEntry {
    pub name: String,
    pub source: PathBuf,
    pub parent_worktree: PathBuf,
    #[serde(default)]
    pub fork_worktree: Option<PathBuf>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub forked: bool,
    #[serde(default)]
    pub link: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct AtelierForkHarnessEntry {
    pub key: String,
    #[serde(default)]
    pub source_session: Option<String>,
    #[serde(default)]
    pub fork_session: Option<String>,
    pub capability: AtelierHarnessCapability,
    #[serde(default)]
    pub degraded_warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AtelierHarnessCapability {
    Native,
    Approximate,
    Unsupported,
    Fresh,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtelierForkRecord {
    pub provider: String,
    pub source_key: String,
    pub name: String,
    pub parent: Option<String>,
    pub created_epoch: i64,
    pub mode: AtelierForkMode,
    pub root: PathBuf,
    pub read_only: bool,
    pub state: AtelierForkState,
    pub repos: Vec<AtelierForkRepoEntry>,
    pub harness: Vec<AtelierForkHarnessEntry>,
}

impl AtelierForkRecord {
    fn from_entry(workspace_root: &Path, entry: AtelierForkEntry) -> Self {
        let root = absolutize(workspace_root, &entry.root);

        Self {
            provider: "atelier".to_string(),
            source_key: entry.name.clone(),
            name: entry.name,
            parent: entry.parent,
            created_epoch: entry.created_epoch,
            mode: entry.mode,
            root,
            read_only: entry.read_only,
            state: entry.state,
            repos: entry.repos,
            harness: entry.harness,
        }
    }
}

pub fn fork_records_fragment(workspace: &NodeId, records: &[AtelierForkRecord]) -> GraphFragment {
    let mut fragment = GraphFragment::empty();

    for record in records {
        let fork_id = ForkId::new(format!("atelier:{}", record.source_key));
        let fork = NodeId::Fork(fork_id.clone());
        fragment.nodes.push(GraphNode::Fork(ForkNode {
            id: fork_id,
            provider: record.provider.clone(),
            provider_source_key: record.source_key.clone(),
            name: Some(record.name.clone()),
            scope: Some("workspace".to_string()),
            capabilities: fork_capabilities(record),
        }));
        fragment.candidate_links.push(atelier_link(
            fork.clone(),
            LinkEndpoint::Node {
                id: workspace.clone(),
            },
            RelationKind::ForksWorkspace,
            "fork belongs to Atelier workspace",
            record_metadata(record, None),
        ));
        fragment.candidate_links.push(atelier_link(
            fork.clone(),
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "path".to_string(),
                    harness_key: None,
                    native_id: None,
                    state_scope: None,
                    path: Some(path_string(&record.root)),
                    metadata: record_metadata(record, None),
                },
            },
            RelationKind::RootedAtPath,
            "fork root path",
            record_metadata(record, None),
        ));

        if let Some(parent) = &record.parent {
            fragment.candidate_links.push(atelier_link(
                fork.clone(),
                LinkEndpoint::Node {
                    id: NodeId::Fork(ForkId::new(format!("atelier:{parent}"))),
                },
                RelationKind::ParentFork,
                "fork parent",
                record_metadata(record, None),
            ));
        }

        for repo in &record.repos {
            let repo_id = RepoId::new(path_string(&repo.source));
            let repo_node = NodeId::Repo(repo_id.clone());
            fragment
                .nodes
                .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
            fragment.candidate_links.push(atelier_link(
                fork.clone(),
                LinkEndpoint::Node {
                    id: repo_node.clone(),
                },
                RelationKind::ForksRepo,
                "fork repo membership",
                record_metadata(record, Some(repo)),
            ));

            if let Some(fork_worktree) = &repo.fork_worktree {
                let worktree_id = CheckoutId::new(repo_id.clone(), path_string(fork_worktree));
                fragment.nodes.push(GraphNode::Checkout(CheckoutNode {
                    id: worktree_id.clone(),
                    root: path_string(fork_worktree),
                    git_dir: None,
                    current_branch: repo
                        .branch
                        .as_ref()
                        .map(|branch| BranchId::new(repo_id.clone(), branch.clone())),
                }));
                fragment.candidate_links.push(atelier_link(
                    fork.clone(),
                    LinkEndpoint::Node {
                        id: NodeId::Checkout(worktree_id),
                    },
                    RelationKind::CreatedCheckout,
                    "fork created checkout",
                    record_metadata(record, Some(repo)),
                ));
            }

            if repo.link {
                let worktree_id =
                    CheckoutId::new(repo_id.clone(), path_string(&repo.parent_worktree));
                fragment.nodes.push(GraphNode::Checkout(CheckoutNode {
                    id: worktree_id.clone(),
                    root: path_string(&repo.parent_worktree),
                    git_dir: None,
                    current_branch: None,
                }));
                fragment.candidate_links.push(atelier_link(
                    fork.clone(),
                    LinkEndpoint::Node {
                        id: NodeId::Checkout(worktree_id),
                    },
                    RelationKind::ReferencedCheckout,
                    "fork referenced parent checkout",
                    record_metadata(record, Some(repo)),
                ));
            }

            if let Some(branch) = &repo.branch {
                let branch_id = BranchId::new(repo_id.clone(), branch.clone());
                fragment.nodes.push(GraphNode::Branch(BranchNode {
                    id: branch_id.clone(),
                    refname: branch.clone(),
                    current_commit: None,
                    upstream: None,
                }));
                fragment.candidate_links.push(atelier_link(
                    fork.clone(),
                    LinkEndpoint::Node {
                        id: NodeId::Branch(branch_id),
                    },
                    if repo.forked {
                        RelationKind::CreatedBranch
                    } else {
                        RelationKind::AssociatedBranch
                    },
                    "fork branch metadata",
                    record_metadata(record, Some(repo)),
                ));
            }
        }

        for harness in &record.harness {
            let metadata = harness_lineage_metadata(record, harness);

            if let Some(parent_session) = &harness.source_session {
                fragment.candidate_links.push(harness_lineage_link(
                    fork.clone(),
                    RelationKind::ParentSession,
                    harness,
                    parent_session,
                    &record.root,
                    metadata.clone(),
                    "fork harness parent session",
                ));
            }

            if let Some(child_session) = &harness.fork_session {
                fragment.candidate_links.push(harness_lineage_link(
                    fork.clone(),
                    RelationKind::ChildSession,
                    harness,
                    child_session,
                    &record.root,
                    metadata,
                    "fork harness child session",
                ));
            }
        }
    }

    snapshot_fragment(fragment.into_snapshot())
}

fn harness_lineage_link(
    fork: NodeId,
    relation: RelationKind,
    harness: &AtelierForkHarnessEntry,
    session_id: &str,
    fork_root: &Path,
    fields: Metadata,
    evidence: &str,
) -> GraphLink {
    let relation_name = relation_name(&relation);
    GraphLink {
        id: format!(
            "atelier:{fork}:{relation_name}:{harness_key}:{session_id}",
            harness_key = harness.key,
        ),
        source: fork,
        target: LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some(harness.key.clone()),
                native_id: Some(session_id.to_string()),
                state_scope: None,
                path: Some(path_string(fork_root)),
                metadata: fields.clone(),
            },
        },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: lineage_confidence(harness.capability),
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "atelier".to_string(),
            evidence: Some(evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn lineage_confidence(capability: AtelierHarnessCapability) -> Confidence {
    match capability {
        AtelierHarnessCapability::Native => Confidence::High,
        AtelierHarnessCapability::Approximate => Confidence::Medium,
        AtelierHarnessCapability::Unsupported | AtelierHarnessCapability::Fresh => Confidence::Low,
    }
}

fn harness_lineage_metadata(
    record: &AtelierForkRecord,
    harness: &AtelierForkHarnessEntry,
) -> Metadata {
    let mut fields = record_metadata(record, None);
    fields.insert(
        "harness_key".to_string(),
        serde_json::Value::String(harness.key.clone()),
    );
    fields.insert(
        "lineage_kind".to_string(),
        serde_json::Value::String(lineage_kind_name(harness).to_string()),
    );
    fields.insert(
        "lineage_fidelity".to_string(),
        serde_json::Value::String(lineage_fidelity_name(harness.capability).to_string()),
    );

    if let Some(warning) = &harness.degraded_warning {
        fields.insert(
            "degraded_warning".to_string(),
            serde_json::Value::String(warning.clone()),
        );
    }

    fields
}

/// ADR 0018 operation vocabulary. Atelier lineage is fork-shaped unless the
/// provider intentionally started the harness fresh.
fn lineage_kind_name(harness: &AtelierForkHarnessEntry) -> &'static str {
    match harness.capability {
        AtelierHarnessCapability::Fresh => "fresh",
        _ => "fork",
    }
}

/// ADR 0018 attribution-fidelity vocabulary, derived from the per-harness
/// capability Atelier records in the fork index.
fn lineage_fidelity_name(capability: AtelierHarnessCapability) -> &'static str {
    match capability {
        AtelierHarnessCapability::Native => "native",
        AtelierHarnessCapability::Approximate => "approximate",
        AtelierHarnessCapability::Unsupported => "unsupported",
        AtelierHarnessCapability::Fresh => "fresh",
    }
}

fn fork_capabilities(record: &AtelierForkRecord) -> Vec<String> {
    let mut capabilities = vec![format!("mode:{:?}", record.mode).to_lowercase()];

    if record.read_only {
        capabilities.push("read_only".to_string());
    }

    capabilities.push(format!("state:{:?}", record.state).to_lowercase());
    capabilities
}

fn atelier_link(
    source: NodeId,
    target: LinkEndpoint,
    relation: RelationKind,
    evidence: &str,
    fields: Metadata,
) -> GraphLink {
    let target_key = match &target {
        LinkEndpoint::Node { id } => id.to_string(),
        LinkEndpoint::Unresolved { evidence } => evidence
            .path
            .clone()
            .or_else(|| evidence.native_id.clone())
            .unwrap_or_else(|| "unresolved".to_string()),
    };
    let relation_name = relation_name(&relation);
    GraphLink {
        id: format!("atelier:{source}:{relation_name}:{target_key}"),
        source,
        target,
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "atelier".to_string(),
            evidence: Some(evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn record_metadata(record: &AtelierForkRecord, repo: Option<&AtelierForkRepoEntry>) -> Metadata {
    let mut fields = Metadata::new();
    fields.insert(
        "fork_name".to_string(),
        serde_json::Value::String(record.name.clone()),
    );
    fields.insert(
        "fork_mode".to_string(),
        serde_json::Value::String(format!("{:?}", record.mode).to_lowercase()),
    );
    fields.insert(
        "created_epoch".to_string(),
        serde_json::Value::Number(record.created_epoch.into()),
    );

    if let Some(repo) = repo {
        fields.insert(
            "repo_name".to_string(),
            serde_json::Value::String(repo.name.clone()),
        );
        fields.insert("forked".to_string(), serde_json::Value::Bool(repo.forked));
        fields.insert("link".to_string(), serde_json::Value::Bool(repo.link));
    }

    fields
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AtelierWorkspaceMetadata {
    pub name: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub versioned: bool,
    #[serde(default, rename = "repo-backend")]
    pub repo_backend: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AtelierRepoEntry {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

fn find_atelier_config(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().ok()?;
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start
    };

    loop {
        let candidate = current.join(ATELIER_CONFIG_FILENAME);

        if candidate.is_file() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}

fn atelier_workspace_repo_link(
    source: NodeId,
    target: NodeId,
    member: WorkspaceRepoMemberMetadata<'_>,
    evidence: &str,
) -> GraphLink {
    let relation = RelationKind::WorkspaceContainsRepo;
    let relation_name = relation_name(&relation);
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "repo_name".to_string(),
        serde_json::Value::String(member.repo_name.to_string()),
    );
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(path_string(member.logical_path)),
    );
    fields.insert(
        "provider_source_path".to_string(),
        serde_json::Value::String(path_string(member.provider_source_path)),
    );
    if let Some(canonical_checkout_root) = member.canonical_checkout_root {
        fields.insert(
            "canonical_checkout_root".to_string(),
            serde_json::Value::String(path_string(canonical_checkout_root)),
        );
    }
    fields.insert(
        "member_path_kind".to_string(),
        serde_json::Value::String(member.member_path_kind.to_string()),
    );

    GraphLink {
        id: format!(
            "atelier:{source}:{relation_name}:{target}:{repo_name}",
            repo_name = member.repo_name
        ),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "atelier".to_string(),
            evidence: Some(evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

#[derive(Clone, Copy, Debug)]
struct WorkspaceRepoMemberMetadata<'a> {
    repo_name: &'a str,
    logical_path: &'a Path,
    provider_source_path: &'a Path,
    canonical_checkout_root: Option<&'a Path>,
    member_path_kind: &'static str,
}

fn workspace_member_path_kind(path: &Path) -> &'static str {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => "symlink",
        Ok(metadata) if metadata.is_dir() => "directory",
        Ok(_) => "other",
        Err(_) => "provider_declared_unresolved",
    }
}

fn snapshot_fragment(snapshot: crate::model::GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
        node_provenance: BTreeMap::new(),
    }
}

fn relation_name(relation: &RelationKind) -> String {
    serde_json::to_string(relation)
        .expect("relation serializes")
        .trim_matches('"')
        .to_string()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn absolutize(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn canonicalized_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::{DiscoveryContext, LocalDiscoveryConfig, discover_local_with};
    use crate::model::{GraphNode, RelationKind};

    #[test]
    fn parses_minimal_atelier_workspace_config() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("atelier.toml");
        fs::write(
            &path,
            r#"
[workspace]
name = "demo"
"#,
        )
        .expect("write config");

        let config = AtelierWorkspaceConfig::load(&path).expect("parse config");

        assert_eq!(config.workspace.name, "demo");
        assert!(config.repos.is_empty());
    }

    #[test]
    fn malformed_atelier_workspace_config_returns_parse_error() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("atelier.toml");
        fs::write(&path, "not = [valid").expect("write bad config");

        let error = AtelierWorkspaceConfig::load(&path).expect_err("parse should fail");

        assert!(error.to_string().contains("parsing"));
    }

    #[test]
    fn atelier_workspace_discovery_emits_workspace_and_repo_membership() {
        let fixture = AtelierFixture::new();
        fixture.write_config(
            r#"
[workspace]
name = "demo"

[[repos]]
name = "repo-a"
path = "/source/repo-a"

[[repos]]
name = "repo-b"
path = "/source/repo-b"
"#,
        );
        fixture.init_repo("repo-a");
        fixture.init_repo("repo-b");

        let snapshot = discover_local_with([fixture.root()], LocalDiscoveryConfig::empty())
            .expect("local discovery succeeds");

        let atelier_workspaces = snapshot
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node,
                    GraphNode::Workspace(workspace)
                        if workspace.provider.as_deref() == Some("atelier")
                )
            })
            .count();
        let workspace_links = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::WorkspaceContainsRepo
                    && link.source_metadata.adapter == "atelier"
            })
            .count();

        assert_eq!(atelier_workspaces, 1);
        assert_eq!(workspace_links, 2);

        let repo_a_fields = snapshot
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::WorkspaceContainsRepo
                    && link.source_metadata.adapter == "atelier"
                    && link.source_metadata.fields.get("repo_name")
                        == Some(&serde_json::Value::String("repo-a".to_string()))
            })
            .expect("repo-a workspace membership link")
            .source_metadata
            .fields
            .clone();
        assert_eq!(
            repo_a_fields.get("logical_path"),
            Some(&serde_json::Value::String(path_string(
                &fixture.root().join("repo-a")
            )))
        );
        assert_eq!(
            repo_a_fields.get("provider_source_path"),
            Some(&serde_json::Value::String("/source/repo-a".to_string()))
        );
        assert_eq!(
            repo_a_fields.get("canonical_checkout_root"),
            Some(&serde_json::Value::String(path_string(
                &fixture
                    .root()
                    .join("repo-a")
                    .canonicalize()
                    .expect("canonical repo-a")
            )))
        );
        assert_eq!(
            repo_a_fields.get("member_path_kind"),
            Some(&serde_json::Value::String("directory".to_string()))
        );
    }

    #[test]
    fn nested_paths_find_parent_atelier_workspace_config() {
        let fixture = AtelierFixture::new();
        fixture.write_config(
            r#"
[workspace]
name = "demo"

[[repos]]
name = "repo-a"
path = "/source/repo-a"
"#,
        );
        let nested = fixture.root().join("repo-a").join("src");
        fixture.init_repo("repo-a");
        fs::create_dir_all(&nested).expect("create nested path");

        let fragment = AtelierWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([nested]).expect("context"))
            .expect("atelier discovery succeeds");

        assert!(
            fragment
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::Workspace(_)))
        );
    }

    #[test]
    fn missing_fork_index_loads_as_empty() {
        let temp = TempDir::new().expect("temp dir");

        let index = AtelierForkIndex::load(temp.path()).expect("load missing index");

        assert!(index.forks.is_empty());
    }

    #[test]
    fn parses_worktree_selected_research_and_standalone_fork_records() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = AtelierForkIndex::path_for(temp.path());
        fs::create_dir_all(index_path.parent().expect("index parent")).expect("create parent");
        fs::write(
            &index_path,
            r#"
[[forks]]
name = "alpha"
created-epoch = 1
mode = "worktree"
root = ".atelier/forks/alpha"
state = "isolated"

[[forks.repos]]
name = "repo-a"
source = "/src/repo-a"
parent-worktree = "/workspace/repo-a"
fork-worktree = ".atelier/forks/alpha/repo-a"
branch = "fork/alpha/repo-a"
forked = true

[[forks.harness]]
key = "codex"
source-session = "parent-session"
fork-session = "child-session"
capability = "native"

[[forks]]
name = "beta"
parent = "alpha"
created-epoch = 2
mode = "selected"
root = ".atelier/forks/beta"
read-only = true

[[forks.repos]]
name = "repo-b"
source = "/src/repo-b"
parent-worktree = "/workspace/repo-b"
link = true

[[forks]]
name = "research"
created-epoch = 3
mode = "research"
root = ".atelier/forks/research"

[[forks]]
name = "standalone"
created-epoch = 4
mode = "worktree"
root = "/tmp/standalone"

[[forks.repos]]
name = "repo-c"
source = "/src/repo-c"
parent-worktree = "/workspace/repo-c"
"#,
        )
        .expect("write fork index");

        let records = AtelierForkIndex::load(temp.path())
            .expect("load fork index")
            .into_provider_records(temp.path());

        assert_eq!(records.len(), 4);
        assert_eq!(records[0].provider, "atelier");
        assert_eq!(records[0].source_key, "alpha");
        assert_eq!(records[0].mode, AtelierForkMode::Worktree);
        assert_eq!(
            records[0].repos[0].fork_worktree.as_deref(),
            Some(Path::new(".atelier/forks/alpha/repo-a"))
        );
        assert_eq!(records[1].mode, AtelierForkMode::Selected);
        assert_eq!(records[1].parent.as_deref(), Some("alpha"));
        assert_eq!(records[2].mode, AtelierForkMode::Research);
        assert_eq!(records[3].root, PathBuf::from("/tmp/standalone"));
    }

    #[test]
    fn harness_lineage_emits_unresolved_parent_and_child_session_evidence() {
        let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
        let record = AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "alpha".to_string(),
            name: "alpha".to_string(),
            parent: None,
            created_epoch: 1,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from("/workspace/.atelier/forks/alpha"),
            read_only: false,
            state: AtelierForkState::Isolated,
            repos: Vec::new(),
            harness: vec![AtelierForkHarnessEntry {
                key: "codex".to_string(),
                source_session: Some("parent-session".to_string()),
                fork_session: Some("child-session".to_string()),
                capability: AtelierHarnessCapability::Native,
                degraded_warning: None,
            }],
        };

        let fragment = fork_records_fragment(&workspace, &[record]);
        let parent_link = lineage_link(&fragment, RelationKind::ParentSession);
        let child_link = lineage_link(&fragment, RelationKind::ChildSession);

        let parent_endpoint = unresolved_endpoint(parent_link);
        assert_eq!(parent_endpoint.node_type, "agent_session");
        assert_eq!(parent_endpoint.harness_key.as_deref(), Some("codex"));
        assert_eq!(parent_endpoint.native_id.as_deref(), Some("parent-session"));
        assert_eq!(
            parent_endpoint.path.as_deref(),
            Some("/workspace/.atelier/forks/alpha")
        );
        assert_eq!(
            parent_endpoint.metadata.get("lineage_kind"),
            Some(&serde_json::Value::String("fork".to_string()))
        );
        assert_eq!(
            parent_endpoint.metadata.get("lineage_fidelity"),
            Some(&serde_json::Value::String("native".to_string()))
        );

        assert_eq!(parent_link.confidence, Confidence::High);
        assert_eq!(parent_link.provenance, Provenance::StrongDiscovered);

        assert_eq!(
            unresolved_endpoint(child_link).native_id.as_deref(),
            Some("child-session")
        );
    }

    #[test]
    fn each_lineage_capability_maps_to_unresolved_endpoint() {
        let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
        let cases = [
            (
                AtelierHarnessCapability::Native,
                "fork",
                "native",
                Confidence::High,
            ),
            (
                AtelierHarnessCapability::Approximate,
                "fork",
                "approximate",
                Confidence::Medium,
            ),
            (
                AtelierHarnessCapability::Unsupported,
                "fork",
                "unsupported",
                Confidence::Low,
            ),
            (
                AtelierHarnessCapability::Fresh,
                "fresh",
                "fresh",
                Confidence::Low,
            ),
        ];

        for (capability, kind_name, fidelity_name, expected_confidence) in cases {
            let record = AtelierForkRecord {
                provider: "atelier".to_string(),
                source_key: format!("fork-{kind_name}"),
                name: format!("fork-{kind_name}"),
                parent: None,
                created_epoch: 1,
                mode: AtelierForkMode::Worktree,
                root: PathBuf::from(format!("/workspace/.atelier/forks/fork-{kind_name}")),
                read_only: false,
                state: AtelierForkState::Inherit,
                repos: Vec::new(),
                harness: vec![AtelierForkHarnessEntry {
                    key: "codex".to_string(),
                    source_session: Some(format!("parent-{kind_name}")),
                    fork_session: Some(format!("child-{kind_name}")),
                    capability,
                    degraded_warning: Some("provider degraded".to_string()),
                }],
            };
            let fragment = fork_records_fragment(&workspace, &[record]);
            let parent_link = lineage_link(&fragment, RelationKind::ParentSession);

            assert_eq!(
                parent_link.confidence, expected_confidence,
                "capability {capability:?} should map to {expected_confidence:?}"
            );
            assert_eq!(
                parent_link
                    .source_metadata
                    .fields
                    .get("lineage_kind")
                    .and_then(serde_json::Value::as_str),
                Some(kind_name)
            );
            assert_eq!(
                parent_link
                    .source_metadata
                    .fields
                    .get("lineage_fidelity")
                    .and_then(serde_json::Value::as_str),
                Some(fidelity_name)
            );
            assert_eq!(
                parent_link
                    .source_metadata
                    .fields
                    .get("degraded_warning")
                    .and_then(serde_json::Value::as_str),
                Some("provider degraded")
            );
        }
    }

    #[test]
    fn fresh_session_without_source_emits_only_child_link() {
        let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
        let record = AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "fresh-fork".to_string(),
            name: "fresh-fork".to_string(),
            parent: None,
            created_epoch: 1,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from("/workspace/.atelier/forks/fresh"),
            read_only: false,
            state: AtelierForkState::Isolated,
            repos: Vec::new(),
            harness: vec![AtelierForkHarnessEntry {
                key: "codex".to_string(),
                source_session: None,
                fork_session: Some("brand-new".to_string()),
                capability: AtelierHarnessCapability::Fresh,
                degraded_warning: None,
            }],
        };

        let fragment = fork_records_fragment(&workspace, &[record]);

        assert!(
            fragment
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::ParentSession),
            "fresh sessions without source_session must not invent a parent link"
        );

        let child = lineage_link(&fragment, RelationKind::ChildSession);
        assert_eq!(
            unresolved_endpoint(child).metadata.get("lineage_kind"),
            Some(&serde_json::Value::String("fresh".to_string()))
        );
    }

    #[test]
    fn harness_lineage_does_not_emit_placeholder_session_nodes() {
        let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
        let record = AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "alpha".to_string(),
            name: "alpha".to_string(),
            parent: None,
            created_epoch: 1,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from("/workspace/.atelier/forks/alpha"),
            read_only: false,
            state: AtelierForkState::Isolated,
            repos: Vec::new(),
            harness: vec![AtelierForkHarnessEntry {
                key: "codex".to_string(),
                source_session: Some("parent-session".to_string()),
                fork_session: Some("child-session".to_string()),
                capability: AtelierHarnessCapability::Native,
                degraded_warning: None,
            }],
        };

        let fragment = fork_records_fragment(&workspace, &[record]);

        assert!(
            !fragment
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::AgentSession(_))),
            "atelier must not fabricate AgentSession nodes from lineage evidence"
        );
    }

    fn lineage_link(fragment: &GraphFragment, relation: RelationKind) -> &GraphLink {
        fragment
            .candidate_links
            .iter()
            .find(|link| link.relation == relation)
            .unwrap_or_else(|| panic!("expected lineage link with relation {relation:?}"))
    }

    fn unresolved_endpoint(link: &GraphLink) -> &UnresolvedEndpoint {
        match &link.target {
            LinkEndpoint::Unresolved { evidence } => evidence,
            LinkEndpoint::Node { .. } => {
                panic!("expected unresolved endpoint, got concrete node target")
            }
        }
    }

    #[test]
    fn malformed_fork_index_returns_parse_error() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = AtelierForkIndex::path_for(temp.path());
        fs::create_dir_all(index_path.parent().expect("index parent")).expect("create parent");
        fs::write(
            &index_path,
            r#"
[[forks]]
name = "bad"
created-epoch = 1
mode = "not-a-mode"
root = ".atelier/forks/bad"
"#,
        )
        .expect("write bad fork index");

        let error = AtelierForkIndex::load(temp.path()).expect_err("parse should fail");

        assert!(error.to_string().contains("parsing"));
    }

    struct AtelierFixture {
        temp: TempDir,
    }

    impl AtelierFixture {
        fn new() -> Self {
            Self {
                temp: TempDir::new().expect("temp dir"),
            }
        }

        fn root(&self) -> &Path {
            self.temp.path()
        }

        fn write_config(&self, text: &str) {
            fs::write(self.root().join("atelier.toml"), text).expect("write atelier config");
        }

        fn init_repo(&self, name: &str) {
            let root = self.root().join(name);
            fs::create_dir(&root).expect("create repo dir");
            git(&root, &["init", "--initial-branch", "main"]);
            git(&root, &["config", "user.name", "Conspectus Test"]);
            git(
                &root,
                &["config", "user.email", "conspectus@example.invalid"],
            );
            fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
            git(&root, &["add", "README.md"]);
            git(&root, &["commit", "-m", "initial"]);
        }
    }

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git command");

        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
