//! Atelier workspace metadata discovery.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::discovery::git::{GitProbe, fragment_from_probe};
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint, LinkState, NodeId, Provenance,
    RelationKind, SourceMetadata, WorkspaceId, WorkspaceNode,
};

pub const ATELIER_CONFIG_FILENAME: &str = "atelier.toml";

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

            if let Some(probe) = self.git_probe.probe(&repo_root)? {
                let repo_id =
                    NodeId::Repo(crate::model::RepoId::new(path_string(&probe.common_dir)));
                repo_links.push(atelier_workspace_repo_link(
                    workspace.clone(),
                    repo_id,
                    &repo.name,
                    "atelier.toml repo entry",
                ));
                child_fragments.push(fragment_from_probe(&probe));
            } else {
                repo_links.push(atelier_workspace_repo_link(
                    workspace.clone(),
                    NodeId::Repo(crate::model::RepoId::new(path_string(
                        &workspace_root.join(&repo.path),
                    ))),
                    &repo.name,
                    "atelier.toml repo entry without discovered checkout",
                ));
            }
        }

        let mut snapshot = merge_fragments(child_fragments);
        snapshot.nodes.push(workspace_node);
        snapshot.candidate_links.extend(repo_links);
        snapshot.canonicalize();
        Ok(snapshot_fragment(snapshot))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    repo_name: &str,
    evidence: &str,
) -> GraphLink {
    let relation = RelationKind::WorkspaceContainsRepo;
    let relation_name = relation_name(&relation);
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "repo_name".to_string(),
        serde_json::Value::String(repo_name.to_string()),
    );

    GraphLink {
        id: format!("atelier:{source}:{relation_name}:{target}:{repo_name}"),
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
    use std::fs;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::{DiscoveryContext, discover_local_at_roots};
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

        let snapshot = discover_local_at_roots([fixture.root()]).expect("local discovery succeeds");

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
