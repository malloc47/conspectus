//! Sessions row-tree builder.
//!
//! Implements the locked sessions-view rules from
//! `docs/tui-sessions-mockup.md` / phase-08:
//!
//! - Workspace → repo → worktree → agent session lineage.
//! - The worktree level renders only when its repo has ≥ 2
//!   worktrees inside the visible set.
//! - Agent sessions with ≥ 2 active `LinkedToMux` candidates expose
//!   an expandable sub-tree of candidate-mux child rows. The
//!   resolver-preferred candidate is marked.
//! - No per-row activity indicator.
//! - Paths render with `~` shortening.
//! - Sessions without a resolved repo/worktree land in the
//!   "Ungrouped" bucket (one synthetic group at the top level,
//!   regardless of `SessionsGrouping`).
//!
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{
    AgentSessionNode, CheckoutId, CheckoutNode, GraphLink, GraphNode, GraphSnapshot,
    MuxSessionNode, NodeId, RelationKind, RepoId, RepoNode, WorkspaceId,
};
use crate::tui::SessionsGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxCandidateRow, MuxIndicator, Row, RowId, RowKind, RowTree,
    ViewLabel, format_recency, harness_label, shorten_home,
};

/// Inputs to the sessions builder. Tests construct these directly;
/// the runtime fills them from the resolved snapshot, the parsed
/// `RunConfig`, and the current wall clock.
#[derive(Debug, Clone)]
pub struct SessionsBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub grouping: SessionsGrouping,
    /// User home directory used for `~`-shortening. Pass `None` to
    /// disable shortening (paths render in full).
    pub home: Option<&'a Path>,
    /// Wall-clock epoch (Unix seconds) used to compute recency.
    /// Pass `None` to leave recency blank — useful for tests.
    pub now: Option<i64>,
    /// Process launch-time cwd. Used to mark the deepest matching
    /// group row with `is_launch_context = true` so the renderer
    /// and selection state machine can highlight the operator's
    /// orientation. Pass `None` to disable the highlight (tests).
    pub cwd: Option<&'a Path>,
}

/// Build the sessions row tree. Pure: depends only on the inputs,
/// no I/O, no clock reads, no env access.
pub fn build_sessions_tree(inputs: SessionsBuildInputs<'_>) -> RowTree {
    let index = SessionsIndex::new(inputs.snapshot);
    let mut tree = RowTree {
        view: ViewLabel::Sessions,
        ..RowTree::default()
    };

    // Bucket sessions by their grouping key. The key shape depends
    // on `inputs.grouping`; the "Ungrouped" bucket catches sessions
    // whose worktree/repo lookup fails.
    let mut buckets: BTreeMap<GroupKey, Vec<SessionEntry<'_>>> = BTreeMap::new();
    let mut ungrouped: Vec<SessionEntry<'_>> = Vec::new();

    for (session_id, session) in &index.agent_sessions {
        let entry = SessionEntry {
            id: session_id.clone(),
            node: session,
        };
        match resolve_group_key(&entry, &index, inputs.grouping) {
            Some(key) => buckets.entry(key).or_default().push(entry),
            None => ungrouped.push(entry),
        }
    }

    let mut session_short_ids =
        ShortIds::from_sessions(buckets.values().flatten().chain(&ungrouped));

    let mut ctx = EmitCtx {
        tree: &mut tree,
        index: &index,
        short_ids: &mut session_short_ids,
        home: inputs.home,
        now: inputs.now,
        grouping: inputs.grouping,
    };

    // Iterate worktree-keyed buckets while deduplicating their
    // ancestor headers. Buckets ordered by (workspace, repo,
    // worktree) come out adjacent for the same (workspace, repo)
    // pair, so we just track the most recent header keys and emit
    // each only on change.
    let mut last_workspace: Option<Option<String>> = None;
    let mut last_repo: Option<RepoId> = None;
    for (key, sessions) in buckets {
        emit_checkout_bucket(&mut ctx, key, sessions, &mut last_workspace, &mut last_repo);
    }

    if !ungrouped.is_empty() {
        emit_ungrouped(&mut ctx, ungrouped);
    }

    mark_launch_context(&mut tree, inputs.cwd);

    tree
}

/// Walk every group row and set `is_launch_context = true` on the
/// one whose underlying path is the deepest ancestor of `cwd`.
/// Skipped when `cwd` is absent or no row matches.
fn mark_launch_context(tree: &mut RowTree, cwd: Option<&Path>) {
    let Some(cwd) = cwd else {
        return;
    };
    let mut best_idx: Option<usize> = None;
    let mut best_depth: usize = 0;
    for (idx, row) in tree.rows.iter().enumerate() {
        let RowKind::Group(group) = &row.kind else {
            continue;
        };
        let Some(path) = group
            .primary_node
            .as_ref()
            .and_then(node_id_path)
            .map(PathBuf::from)
        else {
            continue;
        };
        if !path_is_ancestor_of(&path, cwd) {
            continue;
        }
        let depth = path.components().count();
        if depth > best_depth || best_idx.is_none() {
            best_idx = Some(idx);
            best_depth = depth;
        }
    }
    if let Some(idx) = best_idx
        && let RowKind::Group(group) = &mut tree.rows[idx].kind
    {
        group.is_launch_context = true;
    }
}

fn node_id_path(id: &NodeId) -> Option<&str> {
    match id {
        NodeId::Workspace(ws) => Some(ws.root.as_str()),
        NodeId::Repo(repo) => Some(repo_display_path_from_common_dir(&repo.common_dir)),
        NodeId::Checkout(wt) => Some(wt.root.as_str()),
        _ => None,
    }
}

/// Component-wise prefix match so `/a/b` does **not** count as an
/// ancestor of `/a/barbecue`.
fn path_is_ancestor_of(ancestor: &Path, descendant: &Path) -> bool {
    let mut anc_iter = ancestor.components();
    let mut desc_iter = descendant.components();
    loop {
        match (anc_iter.next(), desc_iter.next()) {
            (Some(a), Some(d)) if a == d => continue,
            (Some(_), Some(_)) => return false,
            (Some(_), None) => return false,
            (None, _) => return true,
        }
    }
}

/// Bundle of mutable + read-only context threaded through the emit
/// helpers so each helper isn't an 8-argument signature.
struct EmitCtx<'a, 'snap> {
    tree: &'a mut RowTree,
    index: &'a SessionsIndex<'snap>,
    short_ids: &'a mut ShortIds,
    home: Option<&'a Path>,
    now: Option<i64>,
    grouping: SessionsGrouping,
}

// -----------------------------------------------------------------------------
// Index built once per build call
// -----------------------------------------------------------------------------

struct SessionsIndex<'a> {
    snapshot: &'a GraphSnapshot,
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    repos: BTreeMap<NodeId, &'a RepoNode>,
    worktrees: BTreeMap<NodeId, &'a CheckoutNode>,
    /// `(source, relation)` → all *active* candidate links. Mirrors
    /// `SnapshotView::by_source_relation` in `output::table` but
    /// scoped to the TUI's needs.
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
}

impl<'a> SessionsIndex<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut repos = BTreeMap::new();
        let mut worktrees = BTreeMap::new();

        for node in &snapshot.nodes {
            let id = node.id();
            match node {
                GraphNode::AgentSession(session) => {
                    agent_sessions.insert(id, session);
                }
                GraphNode::MuxSession(mux) => {
                    mux_sessions.insert(id, mux);
                }
                GraphNode::Repo(repo) => {
                    repos.insert(id, repo);
                }
                GraphNode::Checkout(worktree) => {
                    worktrees.insert(id, worktree);
                }
                _ => {}
            }
        }

        let mut by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>> =
            BTreeMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, crate::model::LinkState::Active) {
                continue;
            }
            by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
        }

        Self {
            snapshot,
            agent_sessions,
            mux_sessions,
            repos,
            worktrees,
            by_source_relation,
        }
    }

    /// Find the deepest worktree node whose root contains `cwd`.
    /// Mirrors the behavior of `output::table::session_checkout_root`.
    fn checkout_for_cwd(&self, cwd: &str) -> Option<(&CheckoutId, &CheckoutNode)> {
        let cwd = Path::new(cwd);
        self.worktrees
            .iter()
            .filter_map(|(id, node)| match id {
                NodeId::Checkout(wt_id) if path_is_ancestor_of(Path::new(&wt_id.root), cwd) => {
                    Some((wt_id, *node))
                }
                _ => None,
            })
            .max_by_key(|(wt_id, _)| Path::new(&wt_id.root).components().count())
    }

    /// Count worktrees that belong to `repo`. Used to apply the
    /// "show worktree level only when ≥ 2 worktrees" rule.
    fn checkout_count_for_repo(&self, repo: &RepoId) -> usize {
        self.worktrees
            .keys()
            .filter(|id| match id {
                NodeId::Checkout(wt_id) => &wt_id.repo == repo,
                _ => false,
            })
            .count()
    }

    /// Resolve the workspace that contains `repo`, if the graph
    /// records one via `WorkspaceContainsRepo`. Returns the first
    /// preferred-by-provenance link's target.
    fn workspace_for_repo(&self, repo: &NodeId) -> Option<&WorkspaceId> {
        for ((source, relation), links) in &self.by_source_relation {
            if *relation != RelationKind::WorkspaceContainsRepo {
                continue;
            }
            for link in links {
                if let crate::model::LinkEndpoint::Node { id } = &link.target
                    && id == repo
                    && let NodeId::Workspace(ws_id) = source
                {
                    return Some(ws_id);
                }
            }
        }
        None
    }

    /// Resolve the workspace context directly associated with a
    /// session. This uses resolved `AssociatedWith` relationships so
    /// workspace membership can come from logical/canonical workspace
    /// member paths rather than only repo-level membership evidence.
    fn workspace_for_session(&self, session: &NodeId) -> Option<WorkspaceId> {
        self.snapshot
            .resolved_relationships
            .iter()
            .find_map(|relationship| {
                if relationship.source != *session
                    || relationship.relation != RelationKind::AssociatedWith
                {
                    return None;
                }
                match &relationship.target {
                    NodeId::Workspace(workspace) => Some(workspace.clone()),
                    _ => None,
                }
            })
    }

    fn repo_display_path(&self, repo_id: &RepoId) -> String {
        let node_id = NodeId::Repo(repo_id.clone());
        self.repos
            .get(&node_id)
            .and_then(|repo| repo.source_paths.first())
            .cloned()
            .unwrap_or_else(|| repo_display_path_from_common_dir(&repo_id.common_dir).to_string())
    }

    /// Active `LinkedToMux` candidate links sourced at this agent
    /// session, de-duplicated by target mux. Multiple evidence links
    /// to the same mux should not make the row look ambiguous.
    fn mux_candidates_for_session(&self, session: &NodeId) -> Vec<&'a GraphLink> {
        let Some(links) = self
            .by_source_relation
            .get(&(session.clone(), RelationKind::LinkedToMux))
        else {
            return Vec::new();
        };

        let mut by_target: BTreeMap<NodeId, Vec<&GraphLink>> = BTreeMap::new();
        for link in links {
            let Some(target) = link.target_node_id() else {
                continue;
            };
            by_target.entry(target.clone()).or_default().push(*link);
        }

        by_target
            .into_values()
            .filter_map(|links| pick_preferred_link(&links))
            .collect()
    }
}

// -----------------------------------------------------------------------------
// Grouping
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct GroupKey {
    /// Optional workspace root (only populated under `Graph`
    /// grouping when a workspace is present).
    workspace: Option<String>,
    /// Repo common-dir.
    repo: String,
    /// Human-oriented repo path. Prefer a checkout/source path over
    /// the git common-dir identity so group labels do not show `/.git`.
    repo_display_path: String,
    /// Repo id (kept alongside `repo` for the `RowId::Group(repo)`
    /// stable identity).
    repo_id: RepoId,
    /// Worktree key. `None` when the grouping mode doesn't include
    /// a worktree level.
    worktree: Option<String>,
}

#[derive(Clone, Debug)]
struct SessionEntry<'a> {
    id: NodeId,
    node: &'a AgentSessionNode,
}

fn resolve_group_key(
    entry: &SessionEntry<'_>,
    index: &SessionsIndex<'_>,
    grouping: SessionsGrouping,
) -> Option<GroupKey> {
    let cwd = entry.node.cwd.as_deref()?;
    let (worktree_id, _worktree) = index.checkout_for_cwd(cwd)?;
    let repo_id = worktree_id.repo.clone();
    let repo_node_id = NodeId::Repo(repo_id.clone());
    let workspace = match grouping {
        SessionsGrouping::Graph => index
            .workspace_for_session(&entry.id)
            .or_else(|| index.workspace_for_repo(&repo_node_id).cloned())
            .map(|ws| ws.root),
        // Repo/Worktree/ScanRoot collapse the workspace level.
        // ScanRoot fallback to repo grouping until the runtime
        // wires scan roots into the builder.
        SessionsGrouping::Repo | SessionsGrouping::Checkout | SessionsGrouping::ScanRoot => None,
    };

    let worktree = match grouping {
        SessionsGrouping::Checkout => Some(worktree_id.root.clone()),
        // For graph/repo grouping, the worktree level is decided by
        // the builder's repo-fan-out rule (≥ 2 worktrees) later.
        // We always include the worktree key here so the grouping
        // is stable; the rendering pass collapses it as needed.
        SessionsGrouping::Graph | SessionsGrouping::Repo | SessionsGrouping::ScanRoot => {
            Some(worktree_id.root.clone())
        }
    };

    Some(GroupKey {
        workspace,
        repo: repo_id.common_dir.clone(),
        repo_display_path: index.repo_display_path(&repo_id),
        repo_id,
        worktree,
    })
}

/// Emit one worktree-keyed bucket while reusing the workspace and
/// repo group rows from prior buckets when those keys haven't
/// changed. Mutates `last_workspace` / `last_repo` to track the
/// most recent header emitted.
fn emit_checkout_bucket(
    ctx: &mut EmitCtx<'_, '_>,
    key: GroupKey,
    mut sessions: Vec<SessionEntry<'_>>,
    last_workspace: &mut Option<Option<String>>,
    last_repo: &mut Option<RepoId>,
) {
    sessions.sort_by(|a, b| compare_sessions(a, b));

    let workspace_changed = last_workspace.as_ref() != Some(&key.workspace);
    let repo_changed = workspace_changed || last_repo.as_ref() != Some(&key.repo_id);

    let mut depth: u8 = 0;
    if key.workspace.is_some() {
        if workspace_changed && let Some(workspace_root) = &key.workspace {
            push_workspace_row(ctx.tree, depth, workspace_root, ctx.home);
        }
        depth = depth.saturating_add(1);
    }

    if repo_changed {
        push_repo_row(ctx.tree, depth, &key, ctx.home);
    }
    let repo_depth = depth;

    let checkout_should_render = matches!(ctx.grouping, SessionsGrouping::Checkout)
        || ctx.index.checkout_count_for_repo(&key.repo_id) >= 2;
    let session_depth = if checkout_should_render && let Some(wt_root) = &key.worktree {
        push_checkout_row(
            ctx.tree,
            repo_depth.saturating_add(1),
            &key.repo_id,
            wt_root,
            ctx.home,
        );
        repo_depth.saturating_add(2)
    } else {
        repo_depth.saturating_add(1)
    };

    for entry in sessions {
        emit_session(ctx, session_depth, entry);
    }

    *last_workspace = Some(key.workspace);
    *last_repo = Some(key.repo_id);
}

fn push_workspace_row(tree: &mut RowTree, depth: u8, workspace_root: &str, home: Option<&Path>) {
    let node_id = NodeId::Workspace(WorkspaceId {
        root: workspace_root.to_string(),
    });
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(workspace_root, home),
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

fn push_repo_row(tree: &mut RowTree, depth: u8, key: &GroupKey, home: Option<&Path>) {
    let node_id = NodeId::Repo(key.repo_id.clone());
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(&key.repo_display_path, home),
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

fn repo_display_path_from_common_dir(common_dir: &str) -> &str {
    common_dir.strip_suffix("/.git").unwrap_or(common_dir)
}

fn push_checkout_row(
    tree: &mut RowTree,
    depth: u8,
    repo: &RepoId,
    worktree_root: &str,
    home: Option<&Path>,
) {
    let wt_id = CheckoutId::new(repo.clone(), worktree_root.to_string());
    let node_id = NodeId::Checkout(wt_id);
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(worktree_root, home),
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

fn emit_ungrouped(ctx: &mut EmitCtx<'_, '_>, mut sessions: Vec<SessionEntry<'_>>) {
    sessions.sort_by(|a, b| compare_sessions(a, b));
    ctx.tree.rows.push(Row {
        id: RowId::Synthetic("ungrouped"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "Ungrouped".to_string(),
            primary_node: None,
            is_launch_context: false,
        }),
    });
    for entry in sessions {
        emit_session(ctx, 1, entry);
    }
}

fn emit_session(ctx: &mut EmitCtx<'_, '_>, depth: u8, entry: SessionEntry<'_>) {
    let candidates = ctx.index.mux_candidates_for_session(&entry.id);
    let preferred = pick_preferred_link(&candidates);
    let mux_state = match candidates.len() {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    };
    let expandable = candidates.len() >= 2;
    let short_id = ctx.short_ids.short_id(&entry.id);
    let cwd_display = entry
        .node
        .cwd
        .as_deref()
        .map(|cwd| shorten_home(cwd, ctx.home));

    ctx.tree.rows.push(Row {
        id: RowId::AgentSession(entry.id.clone()),
        depth,
        expandable,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: entry.node.id.clone(),
            short_id,
            harness_label: harness_label(&entry.node.harness_key),
            cwd_display,
            recency: format_recency(ctx.now, entry.node.last_active_epoch),
            activity_epoch: entry.node.last_active_epoch,
            mux_state,
            preview: entry.node.last_message_preview.clone(),
            title: entry.node.title.clone(),
            alias: ctx
                .index
                .snapshot
                .aliases
                .get(&entry.id)
                .map(str::to_string),
            primary_node: entry.id.clone(),
        }),
    });

    if expandable {
        let preferred_target = preferred.and_then(|link| link.target_node_id().cloned());
        for link in candidates {
            let Some(target) = link.target_node_id() else {
                continue;
            };
            let mux_id = match target {
                NodeId::MuxSession(id) => id.clone(),
                _ => continue,
            };
            let mux_node = ctx.index.mux_sessions.get(target).copied();
            let mux_label = mux_node
                .map(mux_session_label)
                .unwrap_or_else(|| format!("{}:{}", mux_id.native_id, mux_id.native_id));
            let is_preferred = preferred_target.as_ref() == Some(target);
            ctx.tree.rows.push(Row {
                id: RowId::AgentSessionMuxCandidate {
                    agent: entry.id.clone(),
                    mux: target.clone(),
                },
                depth: depth.saturating_add(1),
                expandable: false,
                kind: RowKind::AgentSessionMuxCandidate(MuxCandidateRow {
                    mux: mux_id,
                    mux_label,
                    is_preferred,
                    primary_node: target.clone(),
                }),
            });
        }
    }
}

fn mux_session_label(node: &MuxSessionNode) -> String {
    let native = if node.native_id.chars().count() > 36 {
        let head: String = node.native_id.chars().take(28).collect();
        let tail: String = node
            .native_id
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("{head}…{tail}")
    } else {
        node.native_id.clone()
    };
    format!("{}:{native}", node.backend)
}

/// Sessions within a group sort by recency desc (None last), then
/// alphabetical by harness then short id for a stable tie-breaker.
fn compare_sessions(a: &SessionEntry<'_>, b: &SessionEntry<'_>) -> std::cmp::Ordering {
    b.node
        .last_active_epoch
        .cmp(&a.node.last_active_epoch)
        .then_with(|| a.node.harness_key.cmp(&b.node.harness_key))
        .then_with(|| a.id.cmp(&b.id))
}

fn pick_preferred_link<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links.to_vec();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

// -----------------------------------------------------------------------------
// Short ids
// -----------------------------------------------------------------------------

/// Tracks the per-tree short-id prefix length so all rows share a
/// consistent floor and grow only as collisions force. Mirrors
/// `output::table::unique_prefix_len` but tailored to the agent
/// session set.
struct ShortIds {
    prefix_len: usize,
    table: BTreeMap<NodeId, String>,
}

impl ShortIds {
    const FLOOR: usize = 6;

    fn from_sessions<'a, I>(sessions: I) -> Self
    where
        I: Iterator<Item = &'a SessionEntry<'a>>,
    {
        let full_ids: Vec<(NodeId, String)> = sessions
            .map(|entry| {
                let full = crate::output::table::node_short_id(&entry.id);
                (entry.id.clone(), full)
            })
            .collect();

        let prefix_len = unique_prefix_len(&full_ids);
        let mut table = BTreeMap::new();
        for (id, full) in full_ids {
            let take = prefix_len.min(full.len());
            table.insert(id, full[..take].to_string());
        }
        Self { prefix_len, table }
    }

    fn short_id(&self, id: &NodeId) -> String {
        if let Some(found) = self.table.get(id) {
            return found.clone();
        }
        // Fallback: compute on the fly at the same prefix length.
        // Should not happen in normal use, but the API is total.
        let full = crate::output::table::node_short_id(id);
        let take = self.prefix_len.min(full.len());
        full[..take].to_string()
    }
}

fn unique_prefix_len(ids: &[(NodeId, String)]) -> usize {
    if ids.len() <= 1 {
        return ShortIds::FLOOR;
    }
    let cap = ids
        .iter()
        .map(|(_, s)| s.len())
        .max()
        .unwrap_or(ShortIds::FLOOR);
    for len in ShortIds::FLOOR..=cap {
        let mut seen = std::collections::HashSet::new();
        let mut unique = true;
        for (_, full) in ids {
            let take = len.min(full.len());
            if !seen.insert(&full[..take]) {
                unique = false;
                break;
            }
        }
        if unique {
            return len;
        }
    }
    cap
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink,
        GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId,
        RepoNode, WorkspaceId, WorkspaceNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    fn repo(common_dir: &str) -> GraphNode {
        GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
    }

    fn repo_with_source(common_dir: &str, source_path: &str) -> GraphNode {
        let mut repo = RepoNode::new(RepoId::new(common_dir));
        repo.source_paths.push(source_path.to_string());
        GraphNode::Repo(repo)
    }

    fn worktree(repo_common: &str, root: &str) -> GraphNode {
        GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common), root.to_string()),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
        })
    }

    fn workspace(root: &str) -> GraphNode {
        GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: None,
            name: None,
        })
    }

    fn agent_session(
        harness: &str,
        scope: &str,
        key: &str,
        cwd: Option<&str>,
        title: Option<&str>,
        preview: Option<&str>,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: title.map(str::to_string),
            last_message_preview: preview.map(str::to_string),
            last_active_epoch: None,
        })
    }

    fn agent_session_with_activity(
        harness: &str,
        scope: &str,
        key: &str,
        cwd: Option<&str>,
        activity_epoch: i64,
    ) -> GraphNode {
        let mut node = agent_session(harness, scope, key, cwd, None, None);
        if let GraphNode::AgentSession(session) = &mut node {
            session.last_active_epoch = Some(activity_epoch);
        }
        node
    }

    fn mux_node(backend: &str, native_id: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native_id),
            backend: backend.to_string(),
            native_id: native_id.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux(
        session: &NodeId,
        mux: &NodeId,
        provenance: Provenance,
        suffix: &str,
    ) -> GraphLink {
        GraphLink {
            id: format!("session-mux-{suffix}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn associated_with_workspace(session: &NodeId, root: &str) -> GraphLink {
        let workspace = NodeId::Workspace(WorkspaceId::new(root));
        GraphLink {
            id: format!("test:{session}:associated_with:{workspace}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: workspace },
            relation: RelationKind::AssociatedWith,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn build(inputs: SessionsBuildInputs<'_>) -> RowTree {
        build_sessions_tree(inputs)
    }

    #[test]
    fn empty_snapshot_produces_empty_tree() {
        let snapshot = GraphSnapshot::empty();
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });
        assert!(tree.rows.is_empty());
        assert_eq!(tree.view, ViewLabel::Sessions);
    }

    #[test]
    fn single_session_with_one_worktree_collapses_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            Some("first message"),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        // Expect: repo row, then session row. No worktree row.
        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        assert!(matches!(tree.rows[0].kind, RowKind::Group(_)));
        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert_eq!(tree.rows[0].depth, 0);

        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
        assert_eq!(tree.rows[1].depth, 1);
    }

    #[test]
    fn repo_group_prefers_source_path_over_git_common_dir() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/proj/.git",
            "/home/op/src/proj",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: Some(Path::new("/home/op/src/proj")),
        });

        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert!(group.is_launch_context);
    }

    #[test]
    fn repo_group_strips_git_suffix_when_source_path_is_missing() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj/.git"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: Some(Path::new("/home/op/src/proj")),
        });

        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert!(group.is_launch_context);
    }

    #[test]
    fn session_nested_inside_checkout_groups_under_checkout() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/proj/.git",
            "/home/op/src/proj",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj/crates/core"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
    }

    #[test]
    fn graph_grouping_uses_session_workspace_context() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(workspace("/home/op/ws"));
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/proj/.git",
            "/home/op/src/proj",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj/crates/core"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&session_id, "/home/op/ws"));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        assert_eq!(tree.rows.len(), 3, "{:#?}", tree.rows);
        let workspace_group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(workspace_group.display_path, "~/ws");
        assert!(matches!(
            workspace_group.primary_node,
            Some(NodeId::Workspace(_))
        ));
        assert_eq!(tree.rows[1].depth, 1, "repo should sit under workspace");
        assert_eq!(tree.rows[2].depth, 2, "session should sit under repo");
    }

    #[test]
    fn repo_grouping_excludes_workspace_context() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(workspace("/home/op/ws"));
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/proj/.git",
            "/home/op/src/proj",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj/crates/core"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&session_id, "/home/op/ws"));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Repo,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert!(matches!(group.primary_node, Some(NodeId::Repo(_))));
        assert_eq!(tree.rows[0].depth, 0);
        assert_eq!(tree.rows[1].depth, 1);
    }

    #[test]
    fn two_worktrees_in_same_repo_show_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/wt/featx"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "def",
            Some("/home/op/wt/featx"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        // Expect a single repo row at depth 0, followed by two
        // worktree rows at depth 1 each owning their session rows
        // at depth 2. The dedup pass collapses what would otherwise
        // be a per-bucket repo header repeat (was T8-001).
        let kinds: Vec<&RowKind> = tree.rows.iter().map(|r| &r.kind).collect();
        let repo_count = kinds
            .iter()
            .filter(|k| {
                matches!(
                    k,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Repo(_)),
                        ..
                    })
                )
            })
            .count();
        assert_eq!(
            repo_count, 1,
            "expected exactly one repo group row, kinds={kinds:#?}"
        );
        let worktree_count = kinds
            .iter()
            .filter(|k| {
                matches!(
                    k,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Checkout(_)),
                        ..
                    })
                )
            })
            .count();
        assert_eq!(
            worktree_count, 2,
            "expected two worktree group rows, kinds={kinds:#?}"
        );
        // Repo row sits above the two worktree subtrees.
        let repo_pos = tree
            .rows
            .iter()
            .position(|r| {
                matches!(
                    &r.kind,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Repo(_)),
                        ..
                    })
                )
            })
            .unwrap();
        let first_worktree_pos = tree
            .rows
            .iter()
            .position(|r| {
                matches!(
                    &r.kind,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Checkout(_)),
                        ..
                    })
                )
            })
            .unwrap();
        assert!(repo_pos < first_worktree_pos);
    }

    #[test]
    fn unmuxed_session_has_unmuxed_indicator_and_is_a_leaf() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row present");
        match &session_row.kind {
            RowKind::AgentSession(row) => {
                assert_eq!(row.mux_state, MuxIndicator::Unmuxed);
            }
            _ => unreachable!(),
        }
        assert!(!session_row.expandable);
    }

    #[test]
    fn session_row_uses_agent_activity_epoch_for_recency() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            1_000_000 - 120,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(session) => Some(session),
                _ => None,
            })
            .expect("session row present");
        assert_eq!(session_row.activity_epoch, Some(1_000_000 - 120));
        assert_eq!(session_row.recency.as_deref(), Some("2m"));
    }

    #[test]
    fn sessions_sort_by_most_recent_activity_within_group() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "old",
            Some("/home/op/src/proj"),
            1_000,
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "missing",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "new",
            Some("/home/op/src/proj"),
            2_000,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(2_500),
            cwd: None,
        });

        let session_keys: Vec<&str> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(session_keys, vec!["new", "old", "missing"]);
    }

    #[test]
    fn single_mux_link_yields_attached_indicator() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row");
        match &row.kind {
            RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
            _ => unreachable!(),
        }
        assert!(!row.expandable);
    }

    #[test]
    fn two_mux_links_yield_ambiguous_and_expandable_with_candidate_children() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        // editor is StrongDiscovered, so it should be the preferred
        // candidate; scratch is Discovered.
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &editor,
            Provenance::StrongDiscovered,
            "1",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &scratch,
            Provenance::Discovered,
            "2",
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row");
        match &session_row.kind {
            RowKind::AgentSession(s) => {
                assert_eq!(s.mux_state, MuxIndicator::Ambiguous { candidate_count: 2 });
            }
            _ => unreachable!(),
        }
        assert!(session_row.expandable);

        let candidate_rows: Vec<&MuxCandidateRow> = tree
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::AgentSessionMuxCandidate(row) => Some(row),
                _ => None,
            })
            .collect();
        assert_eq!(candidate_rows.len(), 2, "two candidate child rows");
        let preferred = candidate_rows
            .iter()
            .find(|c| c.is_preferred)
            .expect("a preferred candidate is marked");
        assert_eq!(preferred.mux_label, "tmux:editor");
        let alt = candidate_rows
            .iter()
            .find(|c| !c.is_preferred)
            .expect("an alternate candidate is present");
        assert_eq!(alt.mux_label, "tmux:scratch");
    }

    #[test]
    fn duplicate_mux_target_links_do_not_make_session_ambiguous() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "s1"));
        let mux = NodeId::MuxSession(MuxSessionId::new("editor"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent_session(
            "claude-code",
            "/state",
            "s1",
            Some("/repo"),
            None,
            None,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux,
            Provenance::StrongDiscovered,
            "hook",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux,
            Provenance::Discovered,
            "cwd",
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row");
        match &session_row.kind {
            RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
            _ => unreachable!(),
        }
        assert!(!session_row.expandable);
        assert!(
            !tree
                .rows
                .iter()
                .any(|row| matches!(row.kind, RowKind::AgentSessionMuxCandidate(_)))
        );
    }

    #[test]
    fn orphan_session_lands_in_ungrouped_bucket() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent_session("codex", "/state", "abc", None, None, None));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        assert_eq!(tree.rows.len(), 2);
        match &tree.rows[0].kind {
            RowKind::Group(g) => assert_eq!(g.display_path, "Ungrouped"),
            _ => panic!("first row should be the Ungrouped group"),
        }
        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
    }

    #[test]
    fn session_row_carries_title_preview_and_short_id() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "claude-code",
            "/state",
            "xyz",
            Some("/home/op/src/proj"),
            Some("Phase 8 mockup"),
            Some("could you give me a bit more co…"),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::AgentSession(s) => Some(s),
                _ => None,
            })
            .expect("session row");
        assert_eq!(row.harness_label, "claude");
        assert_eq!(row.title.as_deref(), Some("Phase 8 mockup"));
        assert_eq!(
            row.preview.as_deref(),
            Some("could you give me a bit more co…")
        );
        assert_eq!(row.cwd_display.as_deref(), Some("~/src/proj"));
        assert_eq!(row.short_id.len(), 6, "short id floor at 6 chars");
    }

    #[test]
    fn session_row_alias_overrides_title_in_display_label() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            None,
            Some("harness title"),
            None,
        ));
        let id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        snapshot.aliases.insert(id, "ingest-refactor".to_string());

        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        let row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::AgentSession(s) => Some(s),
                _ => None,
            })
            .expect("session row");
        assert_eq!(row.alias.as_deref(), Some("ingest-refactor"));
        assert_eq!(row.title.as_deref(), Some("harness title"));
        assert_eq!(row.display_label(), Some("ingest-refactor"));
    }

    #[test]
    fn worktree_grouping_always_shows_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Checkout,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });

        // With explicit worktree grouping, the worktree row is
        // always present even though there's a single worktree.
        let has_worktree_group = tree.rows.iter().any(|r| {
            matches!(
                r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Checkout(_)),
                    ..
                })
            )
        });
        assert!(
            has_worktree_group,
            "explicit worktree grouping should show the worktree level"
        );
    }

    #[test]
    fn mark_launch_context_marks_deepest_ancestor_group() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/wt/featx"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "a",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "b",
            Some("/home/op/wt/featx"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            // Launching from inside the featx worktree should mark
            // the featx worktree row (more specific than the repo
            // row, which is also an ancestor).
            cwd: Some(std::path::Path::new("/home/op/wt/featx/src")),
        });

        let marked: Vec<&GroupRow> = tree
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Group(g) if g.is_launch_context => Some(g),
                _ => None,
            })
            .collect();
        assert_eq!(marked.len(), 1, "exactly one row should be marked");
        let marked = marked[0];
        match &marked.primary_node {
            Some(NodeId::Checkout(wt)) => assert_eq!(wt.root, "/home/op/wt/featx"),
            other => panic!("expected featx worktree marked, got {other:?}"),
        }
    }

    #[test]
    fn mark_launch_context_with_no_match_leaves_all_rows_unmarked() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "a",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: Some(std::path::Path::new("/tmp/elsewhere")),
        });
        for row in &tree.rows {
            if let RowKind::Group(g) = &row.kind {
                assert!(
                    !g.is_launch_context,
                    "no row should be marked when cwd is unrelated; got marked: {g:?}"
                );
            }
        }
    }

    #[test]
    fn mark_launch_context_disabled_when_cwd_is_none() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "a",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
        });
        for row in &tree.rows {
            if let RowKind::Group(g) = &row.kind {
                assert!(!g.is_launch_context);
            }
        }
    }

    #[test]
    fn path_is_ancestor_of_respects_component_boundaries() {
        use std::path::Path;
        assert!(path_is_ancestor_of(Path::new("/a/b"), Path::new("/a/b")));
        assert!(path_is_ancestor_of(Path::new("/a/b"), Path::new("/a/b/c")));
        assert!(!path_is_ancestor_of(
            Path::new("/a/b"),
            Path::new("/a/barbecue")
        ));
        assert!(!path_is_ancestor_of(Path::new("/x"), Path::new("/y")));
    }
}
