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
//!
//! Per ADR 0066, the adapter also reads agent-deck's per-profile
//! `state.db` SQLite databases under
//! `<agent-deck-root>/../profiles/<profile>/state.db` and uses the
//! `instances.title` column to label each workspace. Matching is by
//! folder-suffix ↔ instance-id-prefix on the 8-hex conductor id.
//! Read-only opens so we never compete with agent-deck for the
//! write lock; any I/O or schema error falls back to the folder
//! basename.

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
    /// Directory holding per-profile `state.db` SQLite databases
    /// (ADR 0066). Defaults to `<root parent>/profiles` matching
    /// agent-deck's on-disk layout; tests override directly.
    profiles_root: PathBuf,
    git_probe: GitProbe,
}

impl AgentDeckDiscovery {
    /// `multi_repo_worktrees_root` should point at
    /// `~/.agent-deck/multi-repo-worktrees/`. Discovery enumerates
    /// the immediate child `<id>` directories and probes each one
    /// for symlinked git members.
    pub fn new(multi_repo_worktrees_root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = multi_repo_worktrees_root.into();
        let profiles_root = default_profiles_root(&root);
        Self {
            root,
            profiles_root,
            git_probe: GitProbe::new(),
        }
    }

    /// Override the profiles-root directory the title lookup walks.
    /// Used by tests; production callers rely on the
    /// `<root parent>/profiles` default.
    pub fn with_profiles_root(mut self, profiles_root: impl Into<PathBuf>) -> Self {
        self.profiles_root = profiles_root.into();
        self
    }
}

fn default_profiles_root(multi_repo_worktrees_root: &Path) -> PathBuf {
    multi_repo_worktrees_root.parent().map_or_else(
        || PathBuf::from("profiles"),
        |parent| parent.join("profiles"),
    )
}

impl DiscoveryProvider for AgentDeckDiscovery {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        let epoch = crate::discovery::current_epoch();
        if !self.root.is_dir() {
            return Ok(GraphFragment::empty());
        }

        let title_map = read_instance_titles(&self.profiles_root);

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
            fragments.push(self.discover_workspace(&workspace_path, &title_map)?);
        }
        let mut fragment = GraphFragment::from(merge_fragments(fragments));
        crate::discovery::stamp_fragment(
            &mut fragment,
            crate::discovery::providers::AGENT_DECK,
            epoch,
        );
        Ok(fragment)
    }
}

/// Walk `profiles_root` and pull `(id_prefix, title)` pairs from
/// every readable `state.db`. Returns an empty map if the directory
/// or any database is missing or unreadable — see ADR 0066's
/// tolerance rule. First profile to register a prefix wins; the
/// directory walk is sorted so the outcome is deterministic.
fn read_instance_titles(profiles_root: &Path) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(profiles_root) else {
        return out;
    };
    let mut profile_dirs: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry.file_type().ok()?.is_dir().then(|| entry.path())
        })
        .collect();
    profile_dirs.sort();

    for profile_dir in profile_dirs {
        let db_path = profile_dir.join("state.db");
        if !db_path.is_file() {
            continue;
        }
        read_titles_from_db(&db_path, &mut out);
    }
    out
}

fn read_titles_from_db(db_path: &Path, out: &mut BTreeMap<String, String>) {
    use rusqlite::{Connection, OpenFlags};

    let Ok(conn) = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return;
    };
    let Ok(mut stmt) = conn.prepare("SELECT id, title FROM instances") else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return;
    };
    for row in rows.flatten() {
        let (id, title) = row;
        let title = title.trim();
        if title.is_empty() {
            continue;
        }
        let prefix = id_prefix(&id);
        if prefix.is_empty() {
            continue;
        }
        // First-hit-wins per ADR 0066. Profiles are walked in
        // sorted order, so the tiebreaker is deterministic.
        out.entry(prefix.to_string())
            .or_insert_with(|| title.to_string());
    }
}

fn id_prefix(id: &str) -> &str {
    id.split('-').next().unwrap_or(id)
}

/// The 8-hex conductor id agent-deck uses to key into `instances`.
/// For folders shaped `<title-slug>-<8hex>` we want the trailing
/// segment; for bare `<8hex>` folders we want the whole name.
fn workspace_folder_id_suffix(folder_name: &str) -> &str {
    folder_name.rsplit('-').next().unwrap_or(folder_name)
}

impl AgentDeckDiscovery {
    fn discover_workspace(
        &self,
        workspace_path: &Path,
        title_map: &BTreeMap<String, String>,
    ) -> Result<GraphFragment> {
        let Some(folder_basename) = workspace_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        else {
            return Ok(GraphFragment::empty());
        };
        // Per ADR 0066: prefer agent-deck's instance title when one
        // exists for this folder's id-suffix; fall back to the folder
        // basename so unregistered folders keep today's display.
        let name = title_map
            .get(workspace_folder_id_suffix(&folder_basename))
            .cloned()
            .unwrap_or_else(|| folder_basename.clone());

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
            let repo_id = RepoId::new(crate::discovery::path_to_string(&probe.common_dir));
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

        let workspace_root = crate::discovery::path_to_string(workspace_path);
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
        Ok(GraphFragment::from(fragment))
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
    let logical_path = crate::discovery::path_to_string(&member.logical_path);
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "logical_path".to_string(),
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
            adapter: crate::discovery::providers::AGENT_DECK.to_string(),
            evidence: Some("agent-deck multi-repo-worktrees symlink".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn canonicalized_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
#[path = "agent_deck_tests.rs"]
mod tests;
