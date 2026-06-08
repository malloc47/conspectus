//! Agent-deck multi-repo workspace discovery.
//!
//! Agent-deck composes multi-repo working trees under a fixed
//! location: `~/.agent-deck/multi-repo-worktrees/<id>/`, where each
//! immediate child of `<id>/` is a symlink to a git checkout
//! elsewhere on disk. The directory itself is not a git checkout —
//! it only exists as the symlink container.
//!
//! Conspectus emits one `Workspace` node per `<id>` directory with
//! `provider = Some("agent-deck")` and `name = Some(<id>)`, plus a
//! `WorkspaceContainsRepo` candidate link per symlink that probes
//! as a git repo. The `Repo` identity uses the symlink target's
//! canonical git common dir so an existing `Repo` node from an
//! ordinary scan (`~/src/foo`) merges with the agent-deck membership.
//!
//! Non-symlink children are skipped — agent-deck's contract is
//! "symlinks to repos elsewhere," and a bare directory at that
//! depth is provider-foreign noise the operator did not author
//! through agent-deck. A `<id>/` directory with fewer than two
//! resolved repo members is also skipped: a single-repo workspace
//! has no multi-repo composition to surface, and the row's cwd
//! already conveys the underlying checkout.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::discovery::git::{GitProbe, fragment_from_probe};
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint, LinkState, NodeId, Provenance,
    RelationKind, RepoId, SourceMetadata, WorkspaceId, WorkspaceNode,
};

/// Provider identifier carried on `WorkspaceNode.provider` and
/// embedded into emitted candidate-link ids so agent-deck
/// membership stays distinct from generic / atelier workspaces
/// even if a path collides.
pub const AGENT_DECK_PROVIDER: &str = "agent-deck";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentDeckDiscovery {
    root: PathBuf,
    git_probe: GitProbe,
}

impl AgentDeckDiscovery {
    /// `multi_repo_worktrees_root` should point at
    /// `~/.agent-deck/multi-repo-worktrees/`. Discovery enumerates
    /// the immediate child `<id>` directories and probes each one
    /// for symlinked git members.
    pub fn new(multi_repo_worktrees_root: impl Into<PathBuf>) -> Self {
        Self {
            root: multi_repo_worktrees_root.into(),
            git_probe: GitProbe::new(),
        }
    }
}

impl DiscoveryProvider for AgentDeckDiscovery {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        if !self.root.is_dir() {
            return Ok(GraphFragment::empty());
        }

        let mut fragments = Vec::new();
        for entry in fs::read_dir(&self.root).with_context(|| {
            format!(
                "failed to read agent-deck worktrees root: {}",
                self.root.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            let workspace_path = entry.path();
            fragments.push(self.discover_workspace(&workspace_path)?);
        }
        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

impl AgentDeckDiscovery {
    fn discover_workspace(&self, workspace_path: &Path) -> Result<GraphFragment> {
        let Some(name) = workspace_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        else {
            return Ok(GraphFragment::empty());
        };

        let mut child_fragments = Vec::new();
        let mut members = Vec::new();

        for entry in fs::read_dir(workspace_path).with_context(|| {
            format!(
                "failed to read agent-deck workspace: {}",
                workspace_path.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_symlink() {
                continue;
            }
            let logical_path = entry.path();
            if !logical_path.exists() {
                continue;
            }
            let Some(probe) = self.git_probe.probe(&logical_path)? else {
                continue;
            };
            let repo_id = RepoId::new(path_string(&probe.common_dir));
            members.push(AgentDeckMember {
                repo: NodeId::Repo(repo_id),
                logical_path,
                canonical_checkout_root: canonicalized_or_original(&probe.worktree_root),
            });
            child_fragments.push(fragment_from_probe(&probe));
        }

        if members.len() < 2 {
            return Ok(GraphFragment::empty());
        }

        let workspace_root = path_string(workspace_path);
        let workspace_id = WorkspaceId::new(workspace_root.clone());
        let workspace_node = GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: workspace_root,
            provider: Some(AGENT_DECK_PROVIDER.to_string()),
            name: Some(name),
        });
        let workspace = NodeId::Workspace(workspace_id);

        let member_counts = repo_counts(&members);
        let mut fragment = merge_fragments(child_fragments);
        fragment.nodes.push(workspace_node);
        for member in members {
            let duplicate_target = member_counts.get(&member.repo).copied().unwrap_or(0) > 1;
            fragment.candidate_links.push(workspace_repo_link(
                workspace.clone(),
                member,
                duplicate_target,
            ));
        }
        fragment.canonicalize();
        Ok(snapshot_fragment(fragment))
    }
}

fn repo_counts(members: &[AgentDeckMember]) -> BTreeMap<NodeId, usize> {
    let mut counts = BTreeMap::new();
    for member in members {
        *counts.entry(member.repo.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentDeckMember {
    repo: NodeId,
    logical_path: PathBuf,
    canonical_checkout_root: PathBuf,
}

fn workspace_repo_link(
    source: NodeId,
    member: AgentDeckMember,
    duplicate_target: bool,
) -> GraphLink {
    let relation = RelationKind::WorkspaceContainsRepo;
    let relation_name = relation.snake_case();
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
        serde_json::Value::String("symlink".to_string()),
    );
    GraphLink {
        id: if duplicate_target {
            format!("agent_deck:{source}:{relation_name}:{target}:{logical_path}")
        } else {
            format!("agent_deck:{source}:{relation_name}:{target}")
        },
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "agent_deck".to_string(),
            evidence: Some("agent-deck multi-repo-worktrees symlink".to_string()),
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

    struct GitRepoFixture {
        root: PathBuf,
    }

    impl GitRepoFixture {
        fn init_at(parent: &Path, name: &str) -> Self {
            let root = parent.join(name);
            init_repo(&root);
            Self { root }
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

    fn fragment_for(root: &Path) -> GraphFragment {
        AgentDeckDiscovery::new(root)
            .discover(&DiscoveryContext::from_root(root))
            .expect("agent-deck discovery succeeds")
    }

    #[test]
    fn missing_root_emits_empty_fragment() {
        let temp = TempDir::new().expect("temp dir");
        let missing = temp.path().join("does-not-exist");
        let fragment = fragment_for(&missing);
        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
    }

    #[test]
    fn two_symlink_workspace_emits_provider_workspace_and_membership() {
        let temp = TempDir::new().expect("temp dir");
        let worktrees_root = temp.path().join("multi-repo-worktrees");
        let workspace_id_dir = worktrees_root.join("abc");
        let external = TempDir::new().expect("external dir");
        let repo_a = GitRepoFixture::init_at(external.path(), "atelier");
        let repo_b = GitRepoFixture::init_at(external.path(), "conspectus");
        fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
        symlink_dir(repo_a.path(), &workspace_id_dir.join("atelier"));
        symlink_dir(repo_b.path(), &workspace_id_dir.join("conspectus"));

        let fragment = fragment_for(&worktrees_root);

        let workspaces: Vec<&WorkspaceNode> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::Workspace(w) => Some(w),
                _ => None,
            })
            .collect();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].provider.as_deref(), Some(AGENT_DECK_PROVIDER));
        assert_eq!(workspaces[0].name.as_deref(), Some("abc"));
        assert_eq!(workspaces[0].root, path_string(&workspace_id_dir));

        let membership: Vec<&GraphLink> = fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .collect();
        assert_eq!(membership.len(), 2);
        for link in &membership {
            assert_eq!(link.provenance, Provenance::StrongDiscovered);
            assert_eq!(
                link.source_metadata.fields.get("member_path_kind"),
                Some(&serde_json::Value::String("symlink".to_string()))
            );
            assert!(link.source_metadata.fields.contains_key("logical_path"));
        }
    }

    #[test]
    fn one_symlink_workspace_is_skipped() {
        let temp = TempDir::new().expect("temp dir");
        let worktrees_root = temp.path().join("multi-repo-worktrees");
        let workspace_id_dir = worktrees_root.join("solo");
        let external = TempDir::new().expect("external dir");
        let repo = GitRepoFixture::init_at(external.path(), "only");
        fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
        symlink_dir(repo.path(), &workspace_id_dir.join("only"));

        let fragment = fragment_for(&worktrees_root);

        assert!(
            fragment
                .nodes
                .iter()
                .all(|node| !matches!(node, GraphNode::Workspace(_)))
        );
        assert!(
            fragment
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::WorkspaceContainsRepo)
        );
    }

    #[test]
    fn broken_symlinks_are_skipped_but_others_count() {
        let temp = TempDir::new().expect("temp dir");
        let worktrees_root = temp.path().join("multi-repo-worktrees");
        let workspace_id_dir = worktrees_root.join("broken-mix");
        let external = TempDir::new().expect("external dir");
        let repo_a = GitRepoFixture::init_at(external.path(), "repo-a");
        let repo_b = GitRepoFixture::init_at(external.path(), "repo-b");
        fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
        symlink_dir(repo_a.path(), &workspace_id_dir.join("repo-a"));
        symlink_dir(repo_b.path(), &workspace_id_dir.join("repo-b"));
        symlink_dir(
            &workspace_id_dir.join("missing-target"),
            &workspace_id_dir.join("broken"),
        );

        let fragment = fragment_for(&worktrees_root);

        let membership: Vec<&GraphLink> = fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .collect();
        assert_eq!(membership.len(), 2);
    }

    #[test]
    fn non_symlink_child_directories_are_ignored() {
        let temp = TempDir::new().expect("temp dir");
        let worktrees_root = temp.path().join("multi-repo-worktrees");
        let workspace_id_dir = worktrees_root.join("dir-only");
        let external = TempDir::new().expect("external dir");
        let linked_repo = GitRepoFixture::init_at(external.path(), "linked");
        fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
        symlink_dir(linked_repo.path(), &workspace_id_dir.join("linked"));
        // Real subdirectory that happens to be a git repo — agent-deck
        // does not author this and the adapter must not pick it up.
        let _inline_repo = GitRepoFixture::init_at(&workspace_id_dir, "inline");

        let fragment = fragment_for(&worktrees_root);

        // Only one symlinked member → below the ≥2 threshold → no
        // workspace emitted. The presence of the inline directory
        // must not promote the count.
        assert!(
            fragment
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::WorkspaceContainsRepo)
        );
    }

    #[test]
    fn multiple_workspaces_under_root_are_independent() {
        let temp = TempDir::new().expect("temp dir");
        let worktrees_root = temp.path().join("multi-repo-worktrees");
        let external = TempDir::new().expect("external dir");
        let repo_a = GitRepoFixture::init_at(external.path(), "atelier");
        let repo_b = GitRepoFixture::init_at(external.path(), "conspectus");
        let repo_c = GitRepoFixture::init_at(external.path(), "tooling");
        let ws_one = worktrees_root.join("one");
        let ws_two = worktrees_root.join("two");
        fs::create_dir_all(&ws_one).expect("ws one");
        fs::create_dir_all(&ws_two).expect("ws two");
        symlink_dir(repo_a.path(), &ws_one.join("atelier"));
        symlink_dir(repo_b.path(), &ws_one.join("conspectus"));
        symlink_dir(repo_b.path(), &ws_two.join("conspectus"));
        symlink_dir(repo_c.path(), &ws_two.join("tooling"));

        let fragment = fragment_for(&worktrees_root);

        let names: Vec<&str> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::Workspace(w) => w.name.as_deref(),
                _ => None,
            })
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["one", "two"]);
    }
}
