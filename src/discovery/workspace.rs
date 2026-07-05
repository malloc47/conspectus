//! Generic workspace inference from explicit scan roots.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::discovery::atelier::ATELIER_CONFIG_FILENAME;
use crate::discovery::git::{GitProbe, fragment_from_probe};
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint, LinkState, NodeId, Provenance,
    RelationKind, SourceMetadata, WorkspaceId, WorkspaceNode,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenericWorkspaceDiscovery {
    git_probe: GitProbe,
}

impl GenericWorkspaceDiscovery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DiscoveryProvider for GenericWorkspaceDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let epoch = crate::discovery::current_epoch();
        let mut fragments = Vec::new();

        for root in context.roots() {
            fragments.push(self.discover_root(root)?);
        }

        let mut fragment = GraphFragment::from(merge_fragments(fragments));
        crate::discovery::stamp_fragment(
            &mut fragment,
            crate::discovery::providers::GENERIC_WORKSPACE,
            epoch,
        );
        Ok(fragment)
    }
}

impl GenericWorkspaceDiscovery {
    fn discover_root(&self, root: &Path) -> Result<GraphFragment> {
        if self.git_probe.probe(root)?.is_some() {
            return Ok(GraphFragment::empty());
        }
        if provider_workspace_claims_root(root) {
            return Ok(GraphFragment::empty());
        }

        let mut child_fragments = Vec::new();
        let mut repo_members = Vec::new();

        for entry in fs::read_dir(root)
            .with_context(|| format!("failed to read scan root: {}", root.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_symlink() && !path.exists() {
                continue;
            }
            let Some(probe) = self.git_probe.probe(&path)? else {
                continue;
            };

            repo_members.push(WorkspaceMember {
                repo: NodeId::Repo(crate::model::RepoId::new(crate::discovery::path_to_string(
                    &probe.common_dir,
                ))),
                logical_path: path,
                canonical_checkout_root: canonicalized_or_original(&probe.worktree_root),
                path_kind: if file_type.is_symlink() {
                    WorkspaceMemberPathKind::Symlink
                } else {
                    WorkspaceMemberPathKind::Directory
                },
            });
            child_fragments.push(fragment_from_probe(&probe));
        }

        if repo_members.len() < 2 {
            return Ok(GraphFragment::empty());
        }

        let workspace_id = WorkspaceId::new(crate::discovery::path_to_string(root));
        let workspace_node = GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: crate::discovery::path_to_string(root),
            provider: None,
            name: root
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .filter(|name| !name.is_empty()),
        });
        let workspace = NodeId::Workspace(workspace_id);
        let mut fragment = merge_fragments(child_fragments);
        fragment.nodes.push(workspace_node);

        let repo_counts = repo_counts(&repo_members);
        for member in repo_members {
            let duplicate_target = repo_counts.get(&member.repo).copied().unwrap_or(0) > 1;
            fragment.candidate_links.push(workspace_repo_link(
                workspace.clone(),
                member,
                "multiple git repos under explicit scan root",
                duplicate_target,
            ));
        }

        fragment.canonicalize();
        Ok(GraphFragment::from(fragment))
    }
}

fn provider_workspace_claims_root(root: &Path) -> bool {
    root.join(ATELIER_CONFIG_FILENAME).is_file()
}

fn repo_counts(members: &[WorkspaceMember]) -> BTreeMap<NodeId, usize> {
    let mut counts = BTreeMap::new();
    for member in members {
        *counts.entry(member.repo.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceMember {
    repo: NodeId,
    logical_path: PathBuf,
    canonical_checkout_root: PathBuf,
    path_kind: WorkspaceMemberPathKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceMemberPathKind {
    Directory,
    Symlink,
}

impl WorkspaceMemberPathKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Symlink => "symlink",
        }
    }
}

fn workspace_repo_link(
    source: NodeId,
    member: WorkspaceMember,
    evidence: &str,
    duplicate_target: bool,
) -> GraphLink {
    let relation = RelationKind::WorkspaceContainsRepo;
    let relation_name = relation_name(&relation);
    let target = member.repo;
    let logical_path = crate::discovery::path_to_string(&member.logical_path);
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        crate::model::source_field::LOGICAL_PATH.to_string(),
        serde_json::Value::String(logical_path.clone()),
    );
    fields.insert(
        "canonical_checkout_root".to_string(),
        serde_json::Value::String(crate::discovery::path_to_string(
            &member.canonical_checkout_root,
        )),
    );
    fields.insert(
        "member_path_kind".to_string(),
        serde_json::Value::String(member.path_kind.as_str().to_string()),
    );

    GraphLink {
        id: if duplicate_target {
            format!("generic_workspace:{source}:{relation_name}:{target}:{logical_path}")
        } else {
            format!("generic_workspace:{source}:{relation_name}:{target}")
        },
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::Convention,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: crate::discovery::providers::GENERIC_WORKSPACE.to_string(),
            evidence: Some(evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn relation_name(relation: &RelationKind) -> String {
    serde_json::to_string(relation)
        .expect("relation serializes")
        .trim_matches('"')
        .to_string()
}

fn canonicalized_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
