//! Sessions row-tree builder.
//!
//! Implements the locked sessions-view rules from
//! `docs/tui-sessions-mockup.md` / phase-08:
//!
//! - `graph` grouping renders workspace → repo → worktree → agent
//!   session lineage and is the default.
//! - `repo` grouping stays location-first.
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
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionNode, CheckoutId, GraphLink, GraphNode, GraphSnapshot, LinkState, MuxSessionNode,
    NodeId, PinBinding, PinCandidate, RelationKind, RepoId, WorkspaceId, path_is_ancestor_of,
    pick_preferred,
};
use crate::tui::SessionsGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxCandidateRow, MuxIndicator, PinRow, Row, RowId, RowKind, RowTree,
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
    /// Active row filter for the sessions view (ADR 0031). Sessions
    /// that fail the predicate are dropped before bucketing so empty
    /// groups never emit. An empty filter (the [`RowFilter::default`]
    /// value) admits every session.
    pub filter: RowFilter,
}

/// SQLite-backed inputs (P10-011 / ADR 0043). Mirrors
/// [`SessionsBuildInputs`] but takes a `Connection` instead of a
/// `&GraphSnapshot`.
#[derive(Debug)]
pub struct SessionsBuildInputsFromConn<'a> {
    pub conn: &'a rusqlite::Connection,
    pub grouping: SessionsGrouping,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub cwd: Option<&'a Path>,
    pub filter: RowFilter,
}

/// SQLite-backed sessions row-tree builder (P10-011 / ADR 0043).
///
/// Consumes from SQLite via [`crate::query::read_snapshot`] per-call
/// and delegates to [`build_sessions_tree`]. The
/// grouping / bucketing / launch-context-highlight / candidate-mux
/// expansion logic (ADR 0024 and follow-ups) lives in the typed
/// builder below and is preserved end-to-end — see the rationale on
/// `crate::tui::detail::build_node_detail_from_conn` for why this
/// migration follows the bridge pattern rather than per-section SQL.
///
/// Runtime TUI code uses this entry point; the snapshot-taking
/// [`build_sessions_tree`] remains for fixture-heavy tests and typed
/// assembly reuse.
pub fn build_sessions_tree_from_conn(
    inputs: SessionsBuildInputsFromConn<'_>,
) -> rusqlite::Result<RowTree> {
    let snapshot = crate::query::read_snapshot(inputs.conn)?;
    Ok(build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: inputs.grouping,
        home: inputs.home,
        now: inputs.now,
        cwd: inputs.cwd,
        filter: inputs.filter,
    }))
}

/// Build the sessions row tree. Pure: depends only on the inputs,
/// no I/O, no clock reads, no env access.
pub fn build_sessions_tree(inputs: SessionsBuildInputs<'_>) -> RowTree {
    let data = SessionsData::new(inputs.snapshot);
    let filter = &inputs.filter;
    let nest_lineage = should_nest_lineage(inputs.grouping);
    let nested_lineage_ids = if nest_lineage {
        data.nested_lineage_ids(filter, inputs.now)
    } else {
        HashSet::new()
    };
    let mut tree = RowTree {
        view: ViewLabel::Sessions,
        ..RowTree::default()
    };

    if matches!(inputs.grouping, SessionsGrouping::None) {
        let mut sessions: Vec<_> = data
            .agent_sessions
            .iter()
            .filter_map(|(session_id, session)| {
                if nested_lineage_ids.contains(session_id) {
                    return None;
                }
                let mux_state = MuxStateKey::from_candidate_count(
                    data.mux_candidates_for_session(session_id).len(),
                );
                if !session_matches_filter(session, mux_state, inputs.now, filter) {
                    return None;
                }
                Some(SessionEntry {
                    id: session_id.clone(),
                    node: session,
                    mux_state,
                })
            })
            .collect();
        sessions.sort_by(|a, b| compare_sessions(a, b, filter.float_muxed_sessions_top));

        let mut session_short_ids = ShortIds::from_sessions(sessions.iter());
        let mut ctx = EmitCtx {
            tree: &mut tree,
            data: &data,
            short_ids: &mut session_short_ids,
            filter,
            home: inputs.home,
            now: inputs.now,
            grouping: inputs.grouping,
            float_muxed_top: filter.float_muxed_sessions_top,
            nest_lineage,
            lineage_stack: HashSet::new(),
        };
        for entry in sessions {
            emit_session(&mut ctx, 0, entry);
        }
        return tree;
    }

    // Bucket sessions by their grouping key. The key shape depends
    // on `inputs.grouping`; the "Ungrouped" bucket catches sessions
    // whose worktree/repo lookup fails. The active filter (ADR 0031)
    // is applied per session before bucketing so empty groups never
    // emit a header.
    let mut buckets: BTreeMap<GroupKey, Vec<SessionEntry<'_>>> = BTreeMap::new();
    let mut ungrouped: Vec<SessionEntry<'_>> = Vec::new();

    for (session_id, session) in &data.agent_sessions {
        if nested_lineage_ids.contains(session_id) {
            continue;
        }
        let mux_state =
            MuxStateKey::from_candidate_count(data.mux_candidates_for_session(session_id).len());
        if !session_matches_filter(session, mux_state, inputs.now, filter) {
            continue;
        }
        let entry = SessionEntry {
            id: session_id.clone(),
            node: session,
            mux_state,
        };
        match resolve_group_key(&entry, &data, inputs.grouping) {
            Some(key) => buckets.entry(key).or_default().push(entry),
            None => ungrouped.push(entry),
        }
    }

    let mut session_short_ids =
        ShortIds::from_sessions(buckets.values().flatten().chain(&ungrouped));

    let mut ctx = EmitCtx {
        tree: &mut tree,
        data: &data,
        short_ids: &mut session_short_ids,
        filter,
        home: inputs.home,
        now: inputs.now,
        grouping: inputs.grouping,
        float_muxed_top: filter.float_muxed_sessions_top,
        nest_lineage,
        lineage_stack: HashSet::new(),
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

    emit_unbound_pins(&mut tree, &data, inputs.home);

    mark_launch_context(&mut tree, inputs.cwd);

    tree
}

/// Emit a synthetic "Pins" group with one `RowKind::Pin` child per
/// `PinCandidate` whose binding is `Unbound` or `StaleMux`. Bound
/// pins flow through the regular agent-session rows (with `pin_id`
/// set); pins with no resolver-determined binding (i.e. `binding ==
/// None`, which only happens when the resolver hasn't run) are
/// skipped so we don't render pre-resolved noise.
fn emit_unbound_pins(tree: &mut RowTree, data: &SessionsData<'_>, home: Option<&Path>) {
    let unbound: Vec<&PinCandidate> = data
        .snapshot
        .pins
        .iter()
        .filter(|pin| {
            matches!(
                pin.binding,
                Some(PinBinding::Unbound) | Some(PinBinding::StaleMux { .. })
            )
        })
        .collect();
    if unbound.is_empty() {
        return;
    }

    tree.rows.push(Row {
        id: RowId::Synthetic("pins"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "Pins".to_string(),
            primary_node: None,
            is_launch_context: false,
        }),
    });

    for pin in unbound {
        let state_label = match pin.binding {
            Some(PinBinding::StaleMux { .. }) => "stale-mux",
            _ => "unbound",
        };
        tree.rows.push(Row {
            id: RowId::Pin {
                pin_id: pin.id.clone(),
            },
            depth: 1,
            expandable: false,
            kind: RowKind::Pin(PinRow {
                pin_id: pin.id.clone(),
                display_name: pin.display_name.clone(),
                harness_label: harness_label(&pin.harness),
                cwd_display: shorten_home(&pin.cwd, home),
                mux_label: format!("{}:{}", pin.mux.backend, pin.mux.name),
                state_label,
            }),
        });
    }
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

fn should_nest_lineage(grouping: SessionsGrouping) -> bool {
    matches!(grouping, SessionsGrouping::Graph)
}

fn session_matches_filter(
    session: &AgentSessionNode,
    mux_state: MuxStateKey,
    now: Option<i64>,
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }

    let match_inputs = SessionMatchInputs {
        harness_key: &session.harness_key,
        now_epoch: now,
        last_active_epoch: session.last_active_epoch,
        mux_state,
    };
    filter.matches_session(&match_inputs)
}

fn node_id_path(id: &NodeId) -> Option<&str> {
    match id {
        NodeId::Workspace(ws) => Some(ws.root.as_str()),
        NodeId::Repo(repo) => Some(repo_display_path_from_common_dir(&repo.common_dir)),
        NodeId::Checkout(wt) => Some(wt.root.as_str()),
        _ => None,
    }
}

/// Bundle of mutable + read-only context threaded through the emit
/// helpers so each helper isn't an 8-argument signature.
struct EmitCtx<'a, 'snap> {
    tree: &'a mut RowTree,
    data: &'a SessionsData<'snap>,
    short_ids: &'a mut ShortIds,
    filter: &'a RowFilter,
    home: Option<&'a Path>,
    now: Option<i64>,
    grouping: SessionsGrouping,
    float_muxed_top: bool,
    nest_lineage: bool,
    lineage_stack: HashSet<NodeId>,
}

struct SessionsData<'a> {
    snapshot: &'a GraphSnapshot,
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    repos: BTreeMap<NodeId, &'a crate::model::RepoNode>,
    checkouts: BTreeMap<NodeId, &'a crate::model::CheckoutNode>,
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
    /// Parent `NodeId` → child sessions `(NodeId, &AgentSessionNode)`.
    /// Populated from resolved active `ParentSession` links.
    lineage_children: BTreeMap<NodeId, Vec<(NodeId, &'a AgentSessionNode)>>,
    /// Child `NodeId` → parent session `NodeId`, used to suppress
    /// nested rows from the top-level bucket only when the parent
    /// is visible in the current build.
    lineage_parent: BTreeMap<NodeId, NodeId>,
    /// Bound session `NodeId` → pin id. Built once from
    /// `snapshot.pins` so the per-row emit can attach the pin
    /// marker (ADR 0057 / H-PIN-016) without re-walking the pin
    /// vector for every session.
    pin_id_by_bound_session: BTreeMap<NodeId, String>,
}

impl<'a> SessionsData<'a> {
    fn lineage_children_for(&self, parent: &NodeId) -> Option<&[(NodeId, &'a AgentSessionNode)]> {
        self.lineage_children.get(parent).map(Vec::as_slice)
    }

    fn nested_lineage_ids(&self, filter: &RowFilter, now: Option<i64>) -> HashSet<NodeId> {
        let mut ids = HashSet::new();
        for (child, parent) in &self.lineage_parent {
            if self.is_lineage_cycle_member(child) {
                continue;
            }
            let Some(parent_node) = self.agent_sessions.get(parent) else {
                continue;
            };
            let mux_state =
                MuxStateKey::from_candidate_count(self.mux_candidates_for_session(parent).len());
            if session_matches_filter(parent_node, mux_state, now, filter) {
                ids.insert(child.clone());
            }
        }
        ids
    }

    fn is_lineage_cycle_member(&self, start: &NodeId) -> bool {
        let mut seen = HashSet::new();
        let mut current = start;
        while let Some(parent) = self.lineage_parent.get(current) {
            if parent == start {
                return true;
            }
            if !seen.insert(parent) {
                return false;
            }
            current = parent;
        }
        false
    }
}

impl<'a> SessionsData<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut repos = BTreeMap::new();
        let mut checkouts = BTreeMap::new();

        for node in &snapshot.nodes {
            let id = node.id();
            match node {
                GraphNode::AgentSession(n) => {
                    agent_sessions.insert(id, n);
                }
                GraphNode::MuxSession(n) => {
                    mux_sessions.insert(id, n);
                }
                GraphNode::Repo(n) => {
                    repos.insert(id, n);
                }
                GraphNode::Checkout(n) => {
                    checkouts.insert(id, n);
                }
                _ => {}
            }
        }

        let mut by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>> =
            BTreeMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, LinkState::Active) {
                continue;
            }
            by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
        }

        let mut lineage_children: BTreeMap<NodeId, Vec<(NodeId, &AgentSessionNode)>> =
            BTreeMap::new();
        let mut lineage_parent = BTreeMap::new();

        for link in &snapshot.candidate_links {
            if link.relation != RelationKind::ParentSession
                || !matches!(link.state, LinkState::Active)
            {
                continue;
            }
            let NodeId::AgentSession(child_id) = &link.source else {
                continue;
            };
            let Some(NodeId::AgentSession(parent_id)) = link.target_node_id() else {
                continue;
            };
            let Some(child_node) = agent_sessions.get(&NodeId::AgentSession(child_id.clone()))
            else {
                continue;
            };
            let child_node_id = link.source.clone();
            let parent_node_id = NodeId::AgentSession(parent_id.clone());
            if !agent_sessions.contains_key(&parent_node_id) {
                continue;
            }
            lineage_children
                .entry(parent_node_id.clone())
                .or_default()
                .push((child_node_id.clone(), *child_node));
            lineage_parent.insert(child_node_id, parent_node_id);
        }

        let mut pin_id_by_bound_session = BTreeMap::new();
        for pin in &snapshot.pins {
            if let Some(PinBinding::Bound { session, .. }) = &pin.binding {
                pin_id_by_bound_session
                    .insert(NodeId::AgentSession(session.clone()), pin.id.clone());
            }
        }

        Self {
            snapshot,
            agent_sessions,
            mux_sessions,
            repos,
            checkouts,
            by_source_relation,
            lineage_children,
            lineage_parent,
            pin_id_by_bound_session,
        }
    }

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
            .filter_map(|links| pick_preferred(&links))
            .collect()
    }

    fn checkout_for_path(
        &self,
        path: &str,
    ) -> Option<(CheckoutId, &'a crate::model::CheckoutNode)> {
        let path = Path::new(path);
        self.checkouts
            .iter()
            .filter_map(|(id, node)| match id {
                NodeId::Checkout(wt_id) if path_is_ancestor_of(Path::new(&wt_id.root), path) => {
                    Some((wt_id.clone(), *node))
                }
                _ => None,
            })
            .max_by_key(|(wt_id, _)| Path::new(&wt_id.root).components().count())
    }

    fn checkout_count_for_repo(&self, repo: &RepoId) -> usize {
        self.checkouts
            .keys()
            .filter(|id| match id {
                NodeId::Checkout(wt_id) => &wt_id.repo == repo,
                _ => false,
            })
            .count()
    }

    fn workspace_for_repo(&self, repo: &NodeId) -> Option<&WorkspaceId> {
        for ((source, relation), links) in &self.by_source_relation {
            if *relation != RelationKind::WorkspaceContainsRepo {
                continue;
            }
            if links.iter().any(|link| link.target_node_id() == Some(repo))
                && let NodeId::Workspace(ws) = source
            {
                return Some(ws);
            }
        }
        None
    }

    fn workspace_for_session(&self, session: &NodeId) -> Option<&WorkspaceId> {
        self.by_source_relation
            .get(&(session.clone(), RelationKind::AssociatedWith))
            .and_then(|links| {
                links.iter().find_map(|link| match link.target_node_id()? {
                    NodeId::Workspace(ws) => Some(ws),
                    _ => None,
                })
            })
    }

    fn repo_display_path(&self, repo: &RepoId) -> String {
        let node_id = NodeId::Repo(repo.clone());
        let common_dir = repo_display_path_from_common_dir(&repo.common_dir).to_string();
        self.repos
            .get(&node_id)
            .and_then(|repo| {
                repo.source_paths
                    .iter()
                    .find(|path| !path.contains("/.agent-deck/multi-repo-worktrees/"))
            })
            .cloned()
            .unwrap_or(common_dir)
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
    /// Coarse mux-state for the session, derived once from
    /// `mux_candidates_for_session`. Used by the comparator when the
    /// "float muxed sessions to top" toggle is active so the sort key
    /// is consistent across all call sites without re-walking the
    /// graph at compare time.
    mux_state: MuxStateKey,
}

fn resolve_group_key(
    entry: &SessionEntry<'_>,
    data: &SessionsData<'_>,
    grouping: SessionsGrouping,
) -> Option<GroupKey> {
    let cwd = entry.node.cwd.as_deref()?;
    let (worktree_id, _worktree) = data.checkout_for_path(cwd)?;
    let repo_id = worktree_id.repo.clone();
    let repo_node_id = NodeId::Repo(repo_id.clone());
    let workspace = match grouping {
        SessionsGrouping::Graph => data
            .workspace_for_session(&entry.id)
            .or_else(|| data.workspace_for_repo(&repo_node_id))
            .map(|ws| ws.root.clone()),
        // Repo/Worktree/ScanRoot collapse the workspace level.
        // ScanRoot fallback to repo grouping until the runtime
        // wires scan roots into the builder.
        SessionsGrouping::Repo
        | SessionsGrouping::Checkout
        | SessionsGrouping::ScanRoot
        | SessionsGrouping::None => None,
    };

    let worktree = match grouping {
        SessionsGrouping::Checkout => Some(worktree_id.root.clone()),
        // For graph/repo grouping, the worktree level is decided by
        // the builder's repo-fan-out rule (≥ 2 worktrees) later.
        // We always include the worktree key here so the grouping
        // is stable; the rendering pass collapses it as needed.
        SessionsGrouping::Graph
        | SessionsGrouping::Repo
        | SessionsGrouping::ScanRoot
        | SessionsGrouping::None => Some(worktree_id.root.clone()),
    };

    Some(GroupKey {
        workspace,
        repo: repo_id.common_dir.clone(),
        repo_display_path: data.repo_display_path(&repo_id),
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
    sessions.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));

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
        || ctx.data.checkout_count_for_repo(&key.repo_id) >= 2;
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
    sessions.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));
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
    let candidates = ctx.data.mux_candidates_for_session(&entry.id);
    let preferred = pick_preferred(&candidates);
    let mux_state = match candidates.len() {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    };
    let mut lineage_children = visible_lineage_children(ctx, &entry.id);
    lineage_children.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));
    let has_lineage_children = !lineage_children.is_empty();
    let expandable = candidates.len() >= 2 || has_lineage_children;
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
            project_display: project_display_for_session(&entry, ctx.data, ctx.grouping),
            recency: format_recency(ctx.now, entry.node.last_active_epoch),
            activity_epoch: entry.node.last_active_epoch,
            mux_state,
            preview: entry.node.last_message_preview.clone(),
            title: entry.node.title.clone(),
            alias: ctx.data.snapshot.aliases.get(&entry.id).map(str::to_string),
            primary_node: entry.id.clone(),
            pin_id: ctx.data.pin_id_by_bound_session.get(&entry.id).cloned(),
        }),
    });

    if has_lineage_children {
        let inserted = ctx.lineage_stack.insert(entry.id.clone());
        for child in lineage_children {
            emit_session(ctx, depth.saturating_add(1), child);
        }
        if inserted {
            ctx.lineage_stack.remove(&entry.id);
        }
    }

    if candidates.len() >= 2 {
        let preferred_target = preferred.and_then(|link| link.target_node_id().cloned());
        for link in candidates {
            let Some(target) = link.target_node_id() else {
                continue;
            };
            let mux_id = match target {
                NodeId::MuxSession(id) => id.clone(),
                _ => continue,
            };
            let mux_node = ctx.data.mux_sessions.get(target).copied();
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

fn visible_lineage_children<'snap>(
    ctx: &EmitCtx<'_, 'snap>,
    parent: &NodeId,
) -> Vec<SessionEntry<'snap>> {
    if !ctx.nest_lineage {
        return Vec::new();
    }
    let Some(children) = ctx.data.lineage_children_for(parent) else {
        return Vec::new();
    };

    children
        .iter()
        .filter_map(|(child_id, child_node)| {
            if child_id == parent || ctx.lineage_stack.contains(child_id) {
                return None;
            }
            let mux_state = MuxStateKey::from_candidate_count(
                ctx.data.mux_candidates_for_session(child_id).len(),
            );
            if !session_matches_filter(child_node, mux_state, ctx.now, ctx.filter) {
                return None;
            }
            Some(SessionEntry {
                id: child_id.clone(),
                node: child_node,
                mux_state,
            })
        })
        .collect()
}

fn project_display_for_session(
    entry: &SessionEntry<'_>,
    data: &SessionsData<'_>,
    grouping: SessionsGrouping,
) -> Option<String> {
    if !matches!(grouping, SessionsGrouping::None) {
        return None;
    }
    let cwd = entry.node.cwd.as_deref()?;
    data.checkout_for_path(cwd)
        .and_then(|(worktree_id, _worktree)| {
            let project_path = data.repo_display_path(&worktree_id.repo);
            project_name_from_path(&project_path)
        })
        .or_else(|| project_name_from_path(cwd))
}

fn project_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
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
/// When `float_muxed_top` is set, sessions with an attached mux
/// candidate sort before sessions without one, with the existing
/// within-group order preserved inside each of the two resulting
/// halves.
fn compare_sessions(
    a: &SessionEntry<'_>,
    b: &SessionEntry<'_>,
    float_muxed_top: bool,
) -> std::cmp::Ordering {
    if float_muxed_top {
        let muxed_priority = |state: MuxStateKey| match state {
            MuxStateKey::Attached | MuxStateKey::Ambiguous => 0,
            MuxStateKey::Unmuxed => 1,
        };
        let ord = muxed_priority(a.mux_state).cmp(&muxed_priority(b.mux_state));
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    b.node
        .last_active_epoch
        .cmp(&a.node.last_active_epoch)
        .then_with(|| a.node.harness_key.cmp(&b.node.harness_key))
        .then_with(|| a.id.cmp(&b.id))
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
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RelationKind, RepoId, RepoNode, SessionKind, WorkspaceId, WorkspaceNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    /// Parity guard for the SQLite-backed entry point: the two
    /// builders should produce identical `RowTree`s for the same
    /// fixture. Catches drift if a future story refactors only one
    /// path. See `crate::tui::detail` for the same pattern on the
    /// detail pane.
    #[test]
    fn from_conn_matches_snapshot_path_for_basic_fixture() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                repo("/r/.git"),
                worktree("/r/.git", "/r"),
                agent_session("codex", "/state", "alpha", Some("/r/sub"), None, None),
            ],
            ..GraphSnapshot::empty()
        };
        let direct = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Repo,
            home: Some(home().as_path()),
            now: Some(1_700_000_000),
            cwd: None,
            filter: RowFilter::default(),
        });
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let via_conn = build_sessions_tree_from_conn(SessionsBuildInputsFromConn {
            conn: &conn,
            grouping: SessionsGrouping::Repo,
            home: Some(home().as_path()),
            now: Some(1_700_000_000),
            cwd: None,
            filter: RowFilter::default(),
        })
        .expect("from_conn ok");
        assert_eq!(direct, via_conn);
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
            session_kind: None,
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
            client_attached: None,
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
    fn none_grouping_emits_flat_recency_sorted_session_rows_with_project_labels() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/alpha/.git",
            "/home/op/src/alpha",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/alpha/.git", "/home/op/src/alpha"));
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/beta/.git",
            "/home/op/src/beta",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/beta/.git", "/home/op/src/beta"));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "older",
            Some("/home/op/src/alpha"),
            100,
        ));
        snapshot.nodes.push(agent_session_with_activity(
            "claude-code",
            "/state",
            "newer",
            Some("/home/op/src/beta"),
            200,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::None,
            home: Some(home().as_path()),
            now: Some(300),
            cwd: None,
            filter: RowFilter::default(),
        });

        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        assert!(tree.rows.iter().all(|row| row.depth == 0));
        assert!(
            tree.rows
                .iter()
                .all(|row| matches!(row.kind, RowKind::AgentSession(_)))
        );
        let first = match &tree.rows[0].kind {
            RowKind::AgentSession(session) => session,
            other => panic!("expected session row, got {other:?}"),
        };
        let second = match &tree.rows[1].kind {
            RowKind::AgentSession(session) => session,
            other => panic!("expected session row, got {other:?}"),
        };
        assert_eq!(first.session.session_key, "newer");
        assert_eq!(first.project_display.as_deref(), Some("beta"));
        assert_eq!(second.session.session_key, "older");
        assert_eq!(second.project_display.as_deref(), Some("alpha"));
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
        });

        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert!(group.is_launch_context);
    }

    #[test]
    fn repo_group_prefers_canonical_over_non_canonical_source_path() {
        // Regression: when the only known `source_path` is a
        // non-canonical worktree (e.g. an agent-deck multi-repo
        // checkout that was probed before the canonical clone), the
        // repo row should still label with the canonical path derived
        // from the git common dir — otherwise the canonical
        // checkout appears visually nested *under* the agent-deck
        // path when both worktrees fan out as group rows.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/proj/.git",
            "/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj",
        ));
        snapshot.nodes.push(worktree(
            "/home/op/src/proj/.git",
            "/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj",
        ));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj/.git", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "a",
            Some("/home/op/.agent-deck/multi-repo-worktrees/feat-x/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session(
            "claude-code",
            "/state",
            "b",
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
            filter: RowFilter::default(),
        });

        let repo_row = tree
            .rows
            .iter()
            .find(|row| matches!(&row.kind, RowKind::Group(g) if matches!(&g.primary_node, Some(NodeId::Repo(_)))))
            .expect("repo group row");
        let group = match &repo_row.kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
    fn float_muxed_sessions_top_lifts_attached_above_unmuxed() {
        let muxed_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "muxed-old"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        // Newest session is unmuxed — it should drop below the older
        // muxed session when the bool is set, but stay on top within
        // its own (unmuxed) half of the split.
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "unmuxed-new",
            Some("/home/op/src/proj"),
            2_000,
        ));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "muxed-old",
            Some("/home/op/src/proj"),
            1_000,
        ));
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "unmuxed-old",
            Some("/home/op/src/proj"),
            500,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.candidate_links.push(linked_to_mux(
            &muxed_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
        let snapshot = resolve_snapshot(snapshot);

        let baseline = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(3_000),
            cwd: None,
            filter: RowFilter::default(),
        });
        let baseline_keys: Vec<&str> = baseline
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            baseline_keys,
            vec!["unmuxed-new", "muxed-old", "unmuxed-old"],
            "baseline sort is recency desc"
        );

        let floated = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(3_000),
            cwd: None,
            filter: RowFilter {
                float_muxed_sessions_top: true,
                ..RowFilter::default()
            },
        });
        let floated_keys: Vec<&str> = floated
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(session) => Some(session.session.session_key.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            floated_keys,
            vec!["muxed-old", "unmuxed-new", "unmuxed-old"],
            "muxed session rises above the newer unmuxed ones, \
             and within the unmuxed half the recency order is preserved"
        );
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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
            filter: RowFilter::default(),
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

    // ---- ADR 0031: filter predicate integration ----

    fn three_harness_snapshot() -> GraphSnapshot {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        // claude session, recent
        snapshot.nodes.push(agent_session_with_activity(
            "claude-code",
            "/state",
            "c1",
            Some("/home/op/src/proj"),
            1_000_000,
        ));
        // codex session, stale (8 days old vs `now = 1_000_000`)
        let eight_days = 8 * 24 * 60 * 60;
        snapshot.nodes.push(agent_session_with_activity(
            "codex",
            "/state",
            "x1",
            Some("/home/op/src/proj"),
            1_000_000 - eight_days,
        ));
        // opencode session, recent
        snapshot.nodes.push(agent_session_with_activity(
            "opencode",
            "/state",
            "o1",
            Some("/home/op/src/proj"),
            1_000_000 - 600,
        ));
        resolve_snapshot(snapshot)
    }

    fn session_rows(tree: &RowTree) -> Vec<&AgentSessionRow> {
        tree.rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn filter_harness_narrows_to_matching_sessions() {
        let snapshot = three_harness_snapshot();
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
                ..RowFilter::default()
            },
        });
        let sessions = session_rows(&tree);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].harness_label, "claude");
    }

    #[test]
    fn filter_max_age_drops_stale_sessions() {
        let snapshot = three_harness_snapshot();
        let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                max_age: Some(week),
                ..RowFilter::default()
            },
        });
        // codex (8d old) drops; claude and opencode remain.
        let labels: Vec<&str> = session_rows(&tree)
            .iter()
            .map(|s| s.harness_label.as_str())
            .collect();
        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"claude"));
        assert!(labels.contains(&"opencode"));
        assert!(!labels.contains(&"codex"));
    }

    #[test]
    fn filter_mux_state_unmuxed_admits_unmuxed_sessions_only() {
        let snapshot = three_harness_snapshot();
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                mux_state: Some(crate::filter::MuxStateFilter::from_values([
                    crate::filter::MuxStateKey::Unmuxed,
                ])),
                ..RowFilter::default()
            },
        });
        // None of the fixture sessions have mux links, so all three pass.
        assert_eq!(session_rows(&tree).len(), 3);

        // Asking for attached-only drops all three.
        let attached_only = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                mux_state: Some(crate::filter::MuxStateFilter::from_values([
                    crate::filter::MuxStateKey::Attached,
                ])),
                ..RowFilter::default()
            },
        });
        assert!(session_rows(&attached_only).is_empty());
    }

    #[test]
    fn filter_intersection_of_all_dimensions() {
        let snapshot = three_harness_snapshot();
        let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values([
                    "claude-code",
                    "codex",
                ])),
                max_age: Some(week),
                mux_state: Some(crate::filter::MuxStateFilter::from_values([
                    crate::filter::MuxStateKey::Unmuxed,
                ])),
                ..RowFilter::default()
            },
        });
        // claude-code passes harness+age+mux; codex fails the age cut.
        let sessions = session_rows(&tree);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].harness_label, "claude");
    }

    #[test]
    fn filter_emptying_set_drops_entire_tree() {
        let snapshot = three_harness_snapshot();
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: Some(1_000_000),
            cwd: None,
            filter: RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["aider"])),
                ..RowFilter::default()
            },
        });
        // No matching session means no rows at all — group headers
        // skip emission when their bucket is empty.
        assert!(tree.rows.is_empty(), "{:#?}", tree.rows);
    }

    #[test]
    fn filter_skips_empty_groups_so_no_orphan_headers_render() {
        // Two repos, but the filter only matches a session in repo A.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/projA"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/projA", "/home/op/src/projA"));
        snapshot.nodes.push(repo("/home/op/src/projB"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/projB", "/home/op/src/projB"));
        snapshot.nodes.push(agent_session(
            "claude-code",
            "/state",
            "a",
            Some("/home/op/src/projA"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "b",
            Some("/home/op/src/projB"),
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
            filter: RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
                ..RowFilter::default()
            },
        });
        // Only one group header should render (projA), not both.
        let group_count = tree
            .rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Group(_)))
            .count();
        assert_eq!(group_count, 1, "{:#?}", tree.rows);
        assert_eq!(session_rows(&tree).len(), 1);
    }

    // --- Subagent nesting tests ---

    fn agent_session_with_kind(
        harness: &str,
        scope: &str,
        key: &str,
        cwd: Option<&str>,
        title: Option<&str>,
        session_kind: Option<SessionKind>,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: title.map(str::to_string),
            last_message_preview: None,
            last_active_epoch: None,
            session_kind,
        })
    }

    fn parent_session_link(child: &NodeId, parent: &NodeId) -> GraphLink {
        GraphLink {
            id: format!("test:lineage:{child}:parent_session:{parent}"),
            source: child.clone(),
            target: LinkEndpoint::Node { id: parent.clone() },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata {
                adapter: "opencode".to_string(),
                evidence: Some("opencode session.parent_id unknown".to_string()),
                fields: Default::default(),
            },
            state: LinkState::Active,
        }
    }

    fn checkout_node(repo_common_dir: &str, root: &str) -> GraphNode {
        let repo_id = RepoId {
            common_dir: repo_common_dir.to_string(),
        };
        GraphNode::Checkout(CheckoutNode {
            id: CheckoutId {
                repo: repo_id,
                root: root.to_string(),
            },
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
        })
    }

    #[test]
    fn subagent_sessions_are_nested_under_parent_in_row_tree() {
        let parent = agent_session_with_kind(
            "opencode",
            "/state",
            "parent",
            Some("/work/repo"),
            Some("Parent session"),
            None,
        );
        let subagent = agent_session_with_kind(
            "opencode",
            "/state",
            "sub",
            Some("/work/repo"),
            Some("(@explore subagent) Find files"),
            Some(SessionKind::Subagent),
        );
        let parent_id = parent.id();
        let sub_id = subagent.id();

        let checkout = checkout_node("/work/repo/.git", "/work/repo");
        let resolves = crate::resolve::resolve_snapshot;

        let snapshot = resolves(GraphSnapshot {
            nodes: vec![parent, subagent, checkout],
            candidate_links: vec![parent_session_link(&sub_id, &parent_id)],
            ..GraphSnapshot::empty()
        });

        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        // Parent should be at depth 2 (workspace → repo → agent)
        // Subagent should be at depth 3 (under parent, but since no mux,
        // it's just depth+1)
        let parent_row = tree.rows.iter().find(
            |r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "parent"),
        );
        let subagent_row = tree.rows.iter().find(
            |r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "sub"),
        );

        assert!(parent_row.is_some(), "parent session should appear in tree");
        assert!(subagent_row.is_some(), "subagent should appear in tree");

        let parent_row = parent_row.unwrap();
        let subagent_row = subagent_row.unwrap();

        assert!(
            parent_row.expandable,
            "parent should be expandable (has subagent child)"
        );
        assert!(
            subagent_row.depth > parent_row.depth,
            "subagent depth {subagent_depth} should be > parent depth {parent_depth}, rows: {rows:#?}",
            subagent_depth = subagent_row.depth,
            parent_depth = parent_row.depth,
            rows = tree.rows
        );
    }

    #[test]
    fn graph_grouping_nests_resolved_lineage_for_regular_sessions() {
        let parent =
            agent_session_with_kind("codex", "/state", "parent", Some("/work/repo"), None, None);
        let child =
            agent_session_with_kind("codex", "/state", "child", Some("/work/repo"), None, None);
        let parent_id = parent.id();
        let child_id = child.id();
        let checkout = checkout_node("/work/repo/.git", "/work/repo");
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![parent, child, checkout],
            candidate_links: vec![parent_session_link(&child_id, &parent_id)],
            ..GraphSnapshot::empty()
        });

        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let parent_row = tree.rows.iter().find(
            |r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "parent"),
        );
        let child_row = tree.rows.iter().find(
            |r| matches!(&r.kind, RowKind::AgentSession(s) if s.session.session_key == "child"),
        );

        let parent_row = parent_row.expect("parent session row");
        let child_row = child_row.expect("child session row");
        assert!(parent_row.expandable, "{:#?}", tree.rows);
        assert!(
            child_row.depth > parent_row.depth,
            "child should nest under parent in graph grouping: {:#?}",
            tree.rows
        );
    }

    #[test]
    fn repo_grouping_keeps_regular_lineage_sessions_flat_by_location() {
        let parent = agent_session_with_kind(
            "claude-code",
            "/state",
            "parent",
            Some("/work/repo"),
            None,
            None,
        );
        let child = agent_session_with_kind(
            "claude-code",
            "/state",
            "child",
            Some("/work/repo"),
            None,
            None,
        );
        let parent_id = parent.id();
        let child_id = child.id();
        let checkout = checkout_node("/work/repo/.git", "/work/repo");
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![parent, child, checkout],
            candidate_links: vec![parent_session_link(&child_id, &parent_id)],
            ..GraphSnapshot::empty()
        });

        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Repo,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let session_rows: Vec<_> = tree
            .rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
            .collect();
        assert_eq!(session_rows.len(), 2, "{:#?}", tree.rows);
        assert_eq!(session_rows[0].depth, session_rows[1].depth);
        assert!(
            session_rows.iter().all(|row| !row.expandable),
            "repo grouping should not expose lineage disclosure rows: {:#?}",
            tree.rows
        );
    }

    // ---- ADR 0057 / H-PIN-016 pin row integration ---------------

    fn pin_candidate(
        id: &str,
        harness: &str,
        cwd: &str,
        mux_name: &str,
        provenance: Provenance,
        binding: Option<PinBinding>,
    ) -> crate::model::PinCandidate {
        crate::model::PinCandidate {
            id: id.to_string(),
            display_name: id.to_string(),
            harness: harness.to_string(),
            cwd: cwd.to_string(),
            mux: crate::model::PinMuxRef {
                backend: "tmux".to_string(),
                name: mux_name.to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance,
            store_path: "/tmp/conspectus.toml".to_string(),
            binding,
        }
    }

    fn build_tree(snapshot: &GraphSnapshot) -> RowTree {
        build(SessionsBuildInputs {
            snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        })
    }

    #[test]
    fn unbound_pin_emits_synthetic_pins_group_and_row() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(pin_candidate(
            "ingest",
            "codex",
            "/home/op/work/repo",
            "ingest",
            Provenance::LocalPin,
            Some(PinBinding::Unbound),
        ));

        let tree = build_tree(&snapshot);

        // Synthetic group + the one pin row.
        let pin_group_idx = tree
            .rows
            .iter()
            .position(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"))
            .expect("pins group emitted");
        assert!(matches!(&tree.rows[pin_group_idx].kind, RowKind::Group(_)));

        let pin_row = tree
            .rows
            .iter()
            .find(|row| matches!(&row.id, RowId::Pin { pin_id } if pin_id == "ingest"))
            .expect("pin row emitted");
        match &pin_row.kind {
            RowKind::Pin(row) => {
                assert_eq!(row.display_name, "ingest");
                assert_eq!(row.harness_label, "codex");
                assert_eq!(row.cwd_display, "~/work/repo");
                assert_eq!(row.mux_label, "tmux:ingest");
                assert_eq!(row.state_label, "unbound");
            }
            other => panic!("expected RowKind::Pin, got {other:?}"),
        }
        assert_eq!(pin_row.depth, 1);
    }

    #[test]
    fn stale_mux_pin_uses_stale_mux_state_label() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(pin_candidate(
            "ingest",
            "codex",
            "/home/op/work/repo",
            "ingest",
            Provenance::LocalPin,
            Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:ingest"),
            }),
        ));

        let tree = build_tree(&snapshot);
        let row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Pin(pin) => Some(pin.clone()),
                _ => None,
            })
            .expect("pin row");
        assert_eq!(row.state_label, "stale-mux");
    }

    #[test]
    fn bound_pin_marks_existing_agent_session_row() {
        // The resolver synthesizes a LinkedToMux candidate when a pin
        // binds; we mimic that here by:
        // - adding the session and mux nodes
        // - adding a LinkedToMux candidate (the discovered one)
        // - declaring the pin with `binding = Bound`
        // Bound pins do NOT appear under the synthetic "Pins" group;
        // they ride the regular agent-session row with `pin_id` set.
        let session_id = AgentSessionId::new("codex", "/state", "alpha");
        let mux_id = MuxSessionId::new("tmux:ingest");
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                repo("/home/op/work/repo/.git"),
                worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
                agent_session(
                    "codex",
                    "/state",
                    "alpha",
                    Some("/home/op/work/repo"),
                    None,
                    None,
                ),
                mux_node("tmux", "tmux:ingest"),
            ],
            ..GraphSnapshot::empty()
        };
        snapshot.candidate_links.push(linked_to_mux(
            &NodeId::AgentSession(session_id.clone()),
            &NodeId::MuxSession(mux_id.clone()),
            Provenance::StrongDiscovered,
            "discovered",
        ));
        snapshot.pins.push(pin_candidate(
            "ingest",
            "codex",
            "/home/op/work/repo",
            "ingest",
            Provenance::LocalPin,
            Some(PinBinding::Bound {
                mux: mux_id,
                session: session_id.clone(),
            }),
        ));

        let tree = build_tree(&snapshot);

        // No synthetic "Pins" group for a bound pin.
        assert!(
            !tree
                .rows
                .iter()
                .any(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins")),
            "bound pins must not surface under the synthetic Pins group: {:#?}",
            tree.rows
        );

        let session_row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::AgentSession(s) if s.session == session_id => Some(s.clone()),
                _ => None,
            })
            .expect("agent session row emitted");
        assert_eq!(session_row.pin_id.as_deref(), Some("ingest"));
    }

    #[test]
    fn mixed_pin_states_emit_only_unbound_in_synthetic_group() {
        // One bound pin (marker on its session row) + two unbound
        // pins (rendered under the Pins group).
        let session_id = AgentSessionId::new("codex", "/state", "alpha");
        let mux_id = MuxSessionId::new("tmux:bound");
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                repo("/home/op/work/repo/.git"),
                worktree("/home/op/work/repo/.git", "/home/op/work/repo"),
                agent_session(
                    "codex",
                    "/state",
                    "alpha",
                    Some("/home/op/work/repo"),
                    None,
                    None,
                ),
                mux_node("tmux", "tmux:bound"),
            ],
            ..GraphSnapshot::empty()
        };
        snapshot.candidate_links.push(linked_to_mux(
            &NodeId::AgentSession(session_id.clone()),
            &NodeId::MuxSession(mux_id.clone()),
            Provenance::StrongDiscovered,
            "discovered",
        ));
        snapshot.pins.extend([
            pin_candidate(
                "bound-one",
                "codex",
                "/home/op/work/repo",
                "bound",
                Provenance::LocalPin,
                Some(PinBinding::Bound {
                    mux: mux_id,
                    session: session_id.clone(),
                }),
            ),
            pin_candidate(
                "free-one",
                "codex",
                "/home/op/other",
                "free-one",
                Provenance::LocalPin,
                Some(PinBinding::Unbound),
            ),
            pin_candidate(
                "free-two",
                "claude-code",
                "/home/op/another",
                "free-two",
                Provenance::GlobalPin,
                Some(PinBinding::Unbound),
            ),
        ]);

        let tree = build_tree(&snapshot);

        let pin_rows: Vec<_> = tree
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Pin(row) => Some(row.pin_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(pin_rows, vec!["free-one", "free-two"]);

        let bound_marker = tree.rows.iter().any(|row| {
            matches!(
                &row.kind,
                RowKind::AgentSession(s) if s.pin_id.as_deref() == Some("bound-one")
            )
        });
        assert!(bound_marker, "bound pin marker missing from session row");
    }

    #[test]
    fn resolver_drives_pin_rows_end_to_end() {
        // Smoke test that wires discovery + resolver together: an
        // unbound pin (no matching mux in the snapshot) should
        // surface as a `RowKind::Pin` after `resolve_snapshot` runs.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(pin_candidate(
            "ingest",
            "codex",
            "/home/op/work/repo",
            "missing-mux",
            Provenance::LocalPin,
            None,
        ));

        let resolved = resolve_snapshot(snapshot);
        // Sanity: the resolver populated the binding to Unbound.
        assert!(matches!(
            resolved.pins[0].binding,
            Some(PinBinding::Unbound)
        ));

        let tree = build(SessionsBuildInputs {
            snapshot: &resolved,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let has_pin_row = tree.rows.iter().any(|r| matches!(&r.kind, RowKind::Pin(_)));
        assert!(
            has_pin_row,
            "expected a pin row after resolver run: {:#?}",
            tree.rows
        );
    }
}
