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
                repo: NodeId::Repo(crate::model::RepoId::new(path_string(&probe.common_dir))),
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
        Ok(snapshot_fragment(fragment))
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
    let logical_path = path_string(&member.logical_path);
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(logical_path.clone()),
    );
    fields.insert(
        "canonical_checkout_root".to_string(),
        serde_json::Value::String(path_string(&member.canonical_checkout_root)),
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
            adapter: "generic_workspace".to_string(),
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

fn canonicalized_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
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
    fn generic_workspace_links_preserve_member_paths() {
        let workspace = TempDir::new().expect("workspace dir");
        let external = TempDir::new().expect("external dir");
        let direct = GitRepoFixture::init_at(workspace.path(), "repo-a");
        let linked_target = GitRepoFixture::init_at(external.path(), "repo-b");
        let linked_logical_path = workspace.path().join("repo-b-link");
        symlink_dir(linked_target.path(), &linked_logical_path);

        let fragment = GenericWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([workspace.path()]).expect("context"))
            .expect("workspace discovery succeeds");

        let links = fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 2);

        let direct_fields = links
            .iter()
            .find(|link| {
                link.source_metadata.fields.get("logical_path")
                    == Some(&serde_json::Value::String(path_string(direct.path())))
            })
            .expect("direct member link")
            .source_metadata
            .fields
            .clone();
        assert_eq!(
            direct_fields.get("member_path_kind"),
            Some(&serde_json::Value::String("directory".to_string()))
        );
        assert_eq!(
            direct_fields.get("canonical_checkout_root"),
            Some(&serde_json::Value::String(path_string(
                &direct.path().canonicalize().expect("direct canonical path")
            )))
        );

        let symlink_fields = links
            .iter()
            .find(|link| {
                link.source_metadata.fields.get("logical_path")
                    == Some(&serde_json::Value::String(path_string(
                        &linked_logical_path,
                    )))
            })
            .expect("symlink member link")
            .source_metadata
            .fields
            .clone();
        assert_eq!(
            symlink_fields.get("member_path_kind"),
            Some(&serde_json::Value::String("symlink".to_string()))
        );
        assert_eq!(
            symlink_fields.get("canonical_checkout_root"),
            Some(&serde_json::Value::String(path_string(
                &linked_target
                    .path()
                    .canonicalize()
                    .expect("linked canonical path")
            )))
        );
    }

    #[test]
    fn generic_workspace_skips_broken_symlink_members() {
        let temp = TempDir::new().expect("temp dir");
        let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
        let _second = GitRepoFixture::init_at(temp.path(), "repo-b");
        symlink_dir(
            &temp.path().join("missing-target"),
            &temp.path().join("broken-link"),
        );

        let fragment = GenericWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([temp.path()]).expect("context"))
            .expect("workspace discovery succeeds");

        let workspace_links = fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .count();

        assert_eq!(workspace_links, 2);
    }

    #[test]
    fn generic_workspace_distinguishes_duplicate_logical_members() {
        let workspace = TempDir::new().expect("workspace dir");
        let external = TempDir::new().expect("external dir");
        let target = GitRepoFixture::init_at(external.path(), "repo");
        let first = workspace.path().join("repo-a");
        let second = workspace.path().join("repo-b");
        symlink_dir(target.path(), &first);
        symlink_dir(target.path(), &second);

        let fragment = GenericWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([workspace.path()]).expect("context"))
            .expect("workspace discovery succeeds");

        let links = fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 2);
        assert_ne!(links[0].id, links[1].id);
        assert!(
            links
                .iter()
                .any(|link| link.id.contains(&path_string(&first))),
            "first logical path should disambiguate a duplicate target: {links:#?}",
        );
        assert!(
            links
                .iter()
                .any(|link| link.id.contains(&path_string(&second))),
            "second logical path should disambiguate a duplicate target: {links:#?}",
        );
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

    #[test]
    fn provider_workspace_metadata_suppresses_generic_inference() {
        let temp = TempDir::new().expect("temp dir");
        let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
        let _second = GitRepoFixture::init_at(temp.path(), "repo-b");
        fs::write(
            temp.path().join(ATELIER_CONFIG_FILENAME),
            r#"
[workspace]
name = "provider-owned"
"#,
        )
        .expect("write atelier config");

        let fragment = GenericWorkspaceDiscovery::new()
            .discover(&DiscoveryContext::from_roots([temp.path()]).expect("context"))
            .expect("workspace discovery succeeds");

        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
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

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create symlink");
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(target, link).expect("create symlink");
    }
}
