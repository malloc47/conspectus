//! Generic workspace inference from explicit scan roots.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

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
        let mut fragments = Vec::new();

        for root in context.roots() {
            fragments.push(self.discover_root(root)?);
        }

        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

impl GenericWorkspaceDiscovery {
    fn discover_root(&self, root: &Path) -> Result<GraphFragment> {
        if self.git_probe.probe(root)?.is_some() {
            return Ok(GraphFragment::empty());
        }

        let mut child_fragments = Vec::new();
        let mut repo_ids = Vec::new();

        for entry in fs::read_dir(root)
            .with_context(|| format!("failed to read scan root: {}", root.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            let Some(probe) = self.git_probe.probe(&path)? else {
                continue;
            };

            repo_ids.push(NodeId::Repo(crate::model::RepoId::new(path_string(
                &probe.common_dir,
            ))));
            child_fragments.push(fragment_from_probe(&probe));
        }

        if repo_ids.len() < 2 {
            return Ok(GraphFragment::empty());
        }

        let workspace_id = WorkspaceId::new(path_string(root));
        let workspace_node = GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: path_string(root),
            provider: None,
            name: root
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .filter(|name| !name.is_empty()),
        });
        let workspace = NodeId::Workspace(workspace_id);
        let mut fragment = merge_fragments(child_fragments);
        fragment.nodes.push(workspace_node);

        for repo in repo_ids {
            fragment.candidate_links.push(workspace_repo_link(
                workspace.clone(),
                repo,
                "multiple git repos under explicit scan root",
            ));
        }

        fragment.canonicalize();
        Ok(snapshot_fragment(fragment))
    }
}

fn workspace_repo_link(source: NodeId, target: NodeId, evidence: &str) -> GraphLink {
    let relation = RelationKind::WorkspaceContainsRepo;
    let relation_name = relation_name(&relation);
    GraphLink {
        id: format!("generic_workspace:{source}:{relation_name}:{target}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::Convention,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "generic_workspace".to_string(),
            evidence: Some(evidence.to_string()),
            fields: Default::default(),
        },
        state: LinkState::Active,
    }
}

fn snapshot_fragment(snapshot: crate::model::GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::{DiscoveryContext, LocalDiscoveryConfig, discover_local_with};

    #[test]
    fn standalone_repo_root_does_not_fabricate_workspace() {
        let repo = GitRepoFixture::init("repo");

        let fragment = GenericWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([repo.path()]).expect("context"))
            .expect("workspace discovery succeeds");

        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
    }

    #[test]
    fn multi_repo_scan_root_infers_generic_workspace() {
        let temp = TempDir::new().expect("temp dir");
        let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
        let _second = GitRepoFixture::init_at(temp.path(), "repo-b");

        let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
            .expect("local discovery succeeds");

        let workspace_nodes = snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::Workspace(_)))
            .count();
        let repo_nodes = snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::Repo(_)))
            .count();
        let workspace_links = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .count();

        assert_eq!(workspace_nodes, 1);
        assert_eq!(repo_nodes, 2);
        assert_eq!(workspace_links, 2);
    }

    #[test]
    fn single_child_repo_root_stays_repo_only() {
        let temp = TempDir::new().expect("temp dir");
        let _repo = GitRepoFixture::init_at(temp.path(), "repo");

        let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
            .expect("local discovery succeeds");

        assert!(
            snapshot
                .nodes
                .iter()
                .all(|node| !matches!(node, GraphNode::Workspace(_)))
        );
    }

    struct GitRepoFixture {
        root: PathBuf,
        _temp: Option<TempDir>,
    }

    impl GitRepoFixture {
        fn init(name: &str) -> Self {
            let temp = TempDir::new().expect("temp dir");
            let root = temp.path().join(name);
            init_repo(&root);
            Self {
                root,
                _temp: Some(temp),
            }
        }

        fn init_at(parent: &Path, name: &str) -> Self {
            let root = parent.join(name);
            init_repo(&root);
            Self { root, _temp: None }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    fn init_repo(root: &Path) {
        fs::create_dir(root).expect("create repo dir");
        git(root, &["init", "--initial-branch", "main"]);
        git(root, &["config", "user.name", "Conspectus Test"]);
        git(
            root,
            &["config", "user.email", "conspectus@example.invalid"],
        );
        fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
        git(root, &["add", "README.md"]);
        git(root, &["commit", "-m", "initial"]);
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
