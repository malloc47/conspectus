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

fn absolutize(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
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
