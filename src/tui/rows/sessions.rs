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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionNode, CheckoutId, GraphLink, GraphNode, GraphSnapshot, LinkState, NodeId,
    PinBinding, RelationKind, RepoId, WorkspaceId, path_is_ancestor_of,
};
use crate::tui::SessionsGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, PinRow, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
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
        // SessionsGrouping::None never emits repo/worktree headers, so
        // the per-repo worktree-fanout decision isn't consulted on this
        // path. Pass an empty map to satisfy the field.
        let session_bearing_worktrees: BTreeMap<RepoId, BTreeSet<String>> = BTreeMap::new();
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
            session_bearing_worktrees: &session_bearing_worktrees,
        };
        let disambiguating = title_disambiguating_sessions(&sessions);
        for entry in sessions {
            let flag = disambiguating.contains(&entry.id);
            emit_session(&mut ctx, 0, entry, flag);
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

    // Workspace grouping (ADR 0065): every discovered workspace
    // gets a header at top level, even with zero (A)-class sessions
    // associated. Pre-seed an empty bucket per workspace so the
    // emission loop renders the header alongside the session-bearing
    // buckets. `entry().or_default()` preserves any sessions already
    // bucketed under the same workspace key.
    if matches!(inputs.grouping, SessionsGrouping::Workspace) {
        for node in &inputs.snapshot.nodes {
            if let GraphNode::Workspace(ws) = node {
                let key = GroupKey {
                    workspace: Some(ws.id.root.clone()),
                    repo: None,
                    worktree: None,
                };
                buckets.entry(key).or_default();
            }
        }
    }

    let mut session_short_ids =
        ShortIds::from_sessions(buckets.values().flatten().chain(&ungrouped));

    // For each repo, the set of worktrees that contribute a (B)-class
    // (non-workspace) bucket in this build. Used by
    // `emit_checkout_bucket` to decide whether to render a worktree
    // level beneath the repo: collapse for 1, fan out for >= 2.
    //
    // Worktrees with no sessions never appear as a bucket key, so they
    // don't count. Worktrees whose sessions are all (A)-class — routed
    // under their workspace via `AssociatedWith Workspace` — produce
    // workspace-bucket keys with `repo: None`, so they don't count
    // here either. The Sessions view is session-relative; checkouts
    // that aren't backed by sessions in this view don't drive nesting.
    let mut session_bearing_worktrees: BTreeMap<RepoId, BTreeSet<String>> = BTreeMap::new();
    for key in buckets.keys() {
        if key.workspace.is_some() {
            continue;
        }
        if let (Some(repo_bucket), Some(worktree)) = (&key.repo, &key.worktree) {
            session_bearing_worktrees
                .entry(repo_bucket.repo_id.clone())
                .or_default()
                .insert(worktree.clone());
        }
    }

    // Pins group lives at the top of the grouped sessions view so
    // operators see the deck's pinned work before scrolling through
    // the broader session list. Bound pins also stay in their
    // related session group (their agent-session row carries
    // `pin_id`), so the Pins group is additive rather than a
    // replacement.
    emit_pins_group(&mut tree, &data, inputs.home);

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
        session_bearing_worktrees: &session_bearing_worktrees,
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

/// Emit a synthetic "Pins" group with one `RowKind::Pin` child per
/// declared `PinCandidate`, regardless of binding state. Bound pins
/// also remain in their natural location: their agent-session row
/// carries `pin_id = Some(...)` so the renderer paints a pin glyph
/// next to the in-place row. Pre-resolve pins (`binding == None`)
/// are surfaced too so operators see what's declared even if the
/// resolver hasn't run yet — they render with a `(unresolved)`
/// state label.
fn emit_pins_group(tree: &mut RowTree, data: &SessionsData<'_>, home: Option<&Path>) {
    if data.snapshot.pins.is_empty() {
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

    for pin in &data.snapshot.pins {
        let state_label = match &pin.binding {
            Some(PinBinding::Bound { .. }) => "bound",
            Some(PinBinding::StaleMux { .. }) => "stale-mux",
            Some(PinBinding::Unbound) => "unbound",
            None => "unresolved",
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
                harness: pin.harness.clone(),
                cwd: pin.cwd.clone(),
                mux_name: pin.mux.name.clone(),
                mux_socket: pin.mux.socket_name.clone(),
                launch_argv: pin.launch_argv.clone().unwrap_or_default(),
                store_path: pin.store_path.clone(),
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
    /// Per-repo set of worktrees that contribute a (B)-class bucket
    /// to the current build. Drives the "collapse vs. fan out the
    /// worktree level" decision in `emit_checkout_bucket`.
    session_bearing_worktrees: &'a BTreeMap<RepoId, BTreeSet<String>>,
}

struct SessionsData<'a> {
    snapshot: &'a GraphSnapshot,
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    repos: BTreeMap<NodeId, &'a crate::model::RepoNode>,
    checkouts: BTreeMap<NodeId, &'a crate::model::CheckoutNode>,
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
        let mut repos = BTreeMap::new();
        let mut checkouts = BTreeMap::new();

        for node in &snapshot.nodes {
            let id = node.id();
            match node {
                GraphNode::AgentSession(n) => {
                    agent_sessions.insert(id, n);
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
            repos,
            checkouts,
            lineage_children,
            lineage_parent,
            pin_id_by_bound_session,
        }
    }

    fn mux_candidates_for_session(&self, session: &NodeId) -> Vec<&'a GraphLink> {
        // H-UI-008 routed the sessions tree through resolver
        // winners; H-UI-006 (ADR 0077) makes that route honest in
        // the suppression case: the resolver now keeps the
        // `LinkedToMux` slot alive with `selected_link_id = None`
        // and the candidate set rolled into `competing_link_ids`.
        // Walk each matching slot once:
        // - `Some(winner)`: the resolver picked. Return the
        //   winning link so the row reports `Attached`.
        // - `None`: the resolver explicitly cannot pick. Return
        //   every candidate the slot lists so the row reports
        //   `Ambiguous { candidate_count }` and the operator sees
        //   the fan-out.
        // The candidate-fan-out fallback from H-UI-008 retires
        // because the slot now carries the signal directly.
        let mut out: Vec<&'a GraphLink> = Vec::new();
        for rel in self
            .snapshot
            .resolved_relationships
            .iter()
            .filter(|rel| rel.relation == RelationKind::LinkedToMux && rel.source == *session)
        {
            match rel.selected_link_id.as_deref() {
                Some(winner_id) => {
                    if let Some(link) = self.link_by_id(winner_id) {
                        out.push(link);
                    }
                }
                None => {
                    for candidate_id in &rel.competing_link_ids {
                        if let Some(link) = self.link_by_id(candidate_id) {
                            out.push(link);
                        }
                    }
                }
            }
        }
        out
    }

    fn link_by_id(&self, link_id: &str) -> Option<&'a GraphLink> {
        self.snapshot
            .candidate_links
            .iter()
            .find(|link| link.id == link_id)
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

    fn workspace_for_session(&self, session: &NodeId) -> Option<&WorkspaceId> {
        // H-UI-008: read resolver winners only so the tree row's
        // workspace matches the detail pane's validated zone. The
        // resolver keys `AssociatedWith` by `(source, relation,
        // target)`, so a session could theoretically own multiple
        // workspace winners — return the first one in iteration
        // order (resolved_relationships is sorted), which matches
        // the prior single-pick semantics.
        self.snapshot
            .resolved_relationships
            .iter()
            .find(|rel| rel.relation == RelationKind::AssociatedWith && rel.source == *session)
            .and_then(|rel| match &rel.target {
                NodeId::Workspace(ws) => Some(ws),
                _ => None,
            })
    }

    /// Look up a workspace node by root path. Used by the hybrid
    /// Graph header (ADR 0064) to pull the workspace's `name` and
    /// `provider` for the shared `format_workspace_display` helper.
    fn workspace_node(&self, workspace_root: &str) -> Option<&'a crate::model::WorkspaceNode> {
        self.snapshot.nodes.iter().find_map(|node| match node {
            GraphNode::Workspace(w) if w.id.root == workspace_root => Some(w),
            _ => None,
        })
    }

    /// Member display names for a workspace, derived from the
    /// resolver-selected `WorkspaceContainsRepo` candidate links'
    /// `logical_path` source field — same source the Workspaces view
    /// (`H-WS-002`) uses, so the two view headers read identically.
    /// Sorted alphabetically and deduped.
    fn workspace_member_names(&self, workspace_root: &str) -> Vec<String> {
        let workspace_id = NodeId::Workspace(WorkspaceId {
            root: workspace_root.to_string(),
        });
        let mut names: Vec<String> = self
            .snapshot
            .resolved_relationships
            .iter()
            .filter(|rel| {
                rel.relation == RelationKind::WorkspaceContainsRepo && rel.source == workspace_id
            })
            .filter_map(|rel| {
                // ADR 0077: only resolved slots feed the
                // member-name lookup; no-winner slots have no
                // link to anchor a `logical_path` field on.
                let winner_id = rel.selected_link_id.as_deref()?;
                let link = self
                    .snapshot
                    .candidate_links
                    .iter()
                    .find(|cl| cl.id == winner_id)?;
                let logical_path = link.source_metadata.fields.get("logical_path")?.as_str()?;
                Path::new(logical_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            })
            .collect();
        names.sort();
        names.dedup();
        names
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

/// Hybrid Graph bucket key per ADR 0064. A bucket is either:
/// - **Workspace-only** (`workspace = Some`, `repo = None`,
///   `worktree = None`): the session has a direct
///   `AssociatedWith Workspace` edge. Renders under the workspace
///   header with no repo level beneath. Workspace buckets sort
///   ahead of repo buckets at top level.
/// - **Repo** (`workspace = None`, `repo = Some`): the session has
///   no workspace edge and falls through to the existing repo
///   grouping. `worktree` is set when the grouping mode includes
///   a worktree level.
///
/// The custom `Ord` impl puts workspace buckets first within a
/// build so workspace headers render at the top of the Graph
/// view; repo buckets follow alphabetically by repo path.
#[derive(Clone, Debug, Eq, PartialEq)]
struct GroupKey {
    workspace: Option<String>,
    repo: Option<RepoBucket>,
    worktree: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct RepoBucket {
    common_dir: String,
    /// Human-oriented repo path. Prefer a checkout/source path
    /// over the git common-dir identity so group labels don't
    /// show `/.git`.
    repo_display_path: String,
    repo_id: RepoId,
}

impl Ord for GroupKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (&self.workspace, &other.workspace) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self
                .repo
                .cmp(&other.repo)
                .then_with(|| self.worktree.cmp(&other.worktree)),
        }
    }
}

impl PartialOrd for GroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
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

    // Hybrid Graph (ADR 0064) and Workspace grouping (ADR 0065):
    // an (A)-class session — one with a direct `AssociatedWith
    // Workspace` edge — groups under its workspace with no repo or
    // worktree level beneath. This includes agent-deck-shaped
    // launches whose cwd is the workspace composite directory
    // itself; those have no checkout, so the legacy repo-required
    // path would have dropped them into the ungrouped bucket.
    if matches!(
        grouping,
        SessionsGrouping::Graph | SessionsGrouping::Workspace
    ) && let Some(workspace) = data.workspace_for_session(&entry.id)
    {
        return Some(GroupKey {
            workspace: Some(workspace.root.clone()),
            repo: None,
            worktree: None,
        });
    }

    // Workspace grouping (ADR 0065) shows only workspace buckets;
    // (B)-class and unaffiliated sessions land in the ungrouped
    // bucket at the bottom of the tree.
    if matches!(grouping, SessionsGrouping::Workspace) {
        return None;
    }

    // Repo path. Every non-Graph/Workspace grouping and every Graph
    // (B)-class (no workspace edge) session needs a checkout to
    // derive its repo bucket. ScanRoot falls back to repo
    // grouping until the runtime wires scan roots into the
    // builder.
    let (worktree_id, _worktree) = data.checkout_for_path(cwd)?;
    let repo_id = worktree_id.repo.clone();
    let repo = Some(RepoBucket {
        common_dir: repo_id.common_dir.clone(),
        repo_display_path: data.repo_display_path(&repo_id),
        repo_id,
    });

    // For Graph/Repo/ScanRoot/None/Checkout the worktree key is
    // always included so the bucket ordering is stable; the
    // rendering pass collapses the worktree level for Graph and
    // Repo when the repo has a single worktree.
    let worktree = Some(worktree_id.root.clone());

    Some(GroupKey {
        workspace: None,
        repo,
        worktree,
    })
}

/// Emit one bucket while reusing prior workspace / repo group
/// rows when those keys haven't changed. Mutates `last_workspace`
/// and `last_repo` to track the most recent header emitted.
///
/// Two bucket shapes per ADR 0064:
/// - Workspace-only (`key.workspace = Some`, `key.repo = None`):
///   one workspace header at depth 0, sessions directly under it
///   at depth 1.
/// - Repo (`key.repo = Some`, `key.workspace = None`): existing
///   repo (+ optional checkout) emission at depth 0/1/2.
fn emit_checkout_bucket(
    ctx: &mut EmitCtx<'_, '_>,
    key: GroupKey,
    mut sessions: Vec<SessionEntry<'_>>,
    last_workspace: &mut Option<Option<String>>,
    last_repo: &mut Option<RepoId>,
) {
    sessions.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));

    let workspace_changed = last_workspace.as_ref() != Some(&key.workspace);

    if let Some(workspace_root) = &key.workspace {
        if workspace_changed {
            push_workspace_row(ctx.tree, 0, workspace_root, ctx.data, ctx.home);
        }
        let disambiguating = title_disambiguating_sessions(&sessions);
        for entry in sessions {
            let flag = disambiguating.contains(&entry.id);
            emit_session(ctx, 1, entry, flag);
        }
        *last_workspace = Some(key.workspace);
        *last_repo = None;
        return;
    }

    // Repo path. `key.repo` is `Some` here by construction
    // (`resolve_group_key` builds either a workspace-only or a
    // repo bucket, never neither).
    let Some(repo_bucket) = key.repo else {
        return;
    };
    let repo_changed = workspace_changed || last_repo.as_ref() != Some(&repo_bucket.repo_id);

    let depth: u8 = 0;
    if repo_changed {
        push_repo_row(ctx.tree, depth, &repo_bucket, ctx.home);
    }

    let session_bearing_count = ctx
        .session_bearing_worktrees
        .get(&repo_bucket.repo_id)
        .map(BTreeSet::len)
        .unwrap_or(0);
    let checkout_should_render =
        matches!(ctx.grouping, SessionsGrouping::Checkout) || session_bearing_count >= 2;
    let session_depth = if checkout_should_render && let Some(wt_root) = &key.worktree {
        push_checkout_row(
            ctx.tree,
            depth.saturating_add(1),
            &repo_bucket.repo_id,
            wt_root,
            ctx.home,
        );
        depth.saturating_add(2)
    } else {
        depth.saturating_add(1)
    };

    let disambiguating = title_disambiguating_sessions(&sessions);
    for entry in sessions {
        let flag = disambiguating.contains(&entry.id);
        emit_session(ctx, session_depth, entry, flag);
    }

    *last_workspace = Some(key.workspace);
    *last_repo = Some(repo_bucket.repo_id);
}

fn push_workspace_row(
    tree: &mut RowTree,
    depth: u8,
    workspace_root: &str,
    data: &SessionsData<'_>,
    home: Option<&Path>,
) {
    let node_id = NodeId::Workspace(WorkspaceId {
        root: workspace_root.to_string(),
    });

    let workspace = data.workspace_node(workspace_root);
    let label = workspace
        .and_then(|w| w.name.clone())
        .or_else(|| {
            Path::new(workspace_root)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| shorten_home(workspace_root, home));
    let provider = workspace.and_then(|w| w.provider.as_deref());
    let members = data.workspace_member_names(workspace_root);
    let display_path = crate::tui::rows::format_workspace_display(&label, &members, provider);

    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path,
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

fn push_repo_row(tree: &mut RowTree, depth: u8, repo: &RepoBucket, home: Option<&Path>) {
    let node_id = NodeId::Repo(repo.repo_id.clone());
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(&repo.repo_display_path, home),
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
    let disambiguating = title_disambiguating_sessions(&sessions);
    for entry in sessions {
        let flag = disambiguating.contains(&entry.id);
        emit_session(ctx, 1, entry, flag);
    }
}

/// P8-015: compute the set of session ids in `entries` whose row
/// label should incorporate the harness-recorded title. A session
/// qualifies when (a) its `title` attribute is non-empty *and* (b)
/// another session in the same group bucket shares the same rendered
/// harness label. The check is bucket-scoped, not snapshot-scoped,
/// so adding a new same-harness session to a project later refreshes
/// produces a deterministic flip of the existing rows without
/// reordering them.
fn title_disambiguating_sessions(entries: &[SessionEntry<'_>]) -> HashSet<NodeId> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        *counts
            .entry(harness_label(&entry.node.harness_key))
            .or_insert(0) += 1;
    }
    entries
        .iter()
        .filter(|entry| {
            entry
                .node
                .title
                .as_deref()
                .is_some_and(|t| !t.trim().is_empty())
                && counts
                    .get(&harness_label(&entry.node.harness_key))
                    .copied()
                    .unwrap_or(0)
                    >= 2
        })
        .map(|entry| entry.id.clone())
        .collect()
}

fn emit_session(
    ctx: &mut EmitCtx<'_, '_>,
    depth: u8,
    entry: SessionEntry<'_>,
    title_disambiguates: bool,
) {
    let candidates = ctx.data.mux_candidates_for_session(&entry.id);
    let mux_state = match candidates.len() {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    };
    let mut lineage_children = visible_lineage_children(ctx, &entry.id);
    lineage_children.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));
    let has_lineage_children = !lineage_children.is_empty();
    // ADR 0071: ambiguous mux candidates no longer expand a
    // per-session subtree; they surface as a section on the
    // shared-ancestor group's detail pane instead. Sessions stay
    // expandable only when lineage children exist.
    let expandable = has_lineage_children;
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
            title_disambiguates,
            primary_node: entry.id.clone(),
            pin_id: ctx.data.pin_id_by_bound_session.get(&entry.id).cloned(),
        }),
    });

    if has_lineage_children {
        let inserted = ctx.lineage_stack.insert(entry.id.clone());
        // Lineage children are a nested subtree of one parent, not
        // peer rows in the same project group, so the bucket-scoped
        // disambiguation flag never applies — pass `false`.
        for child in lineage_children {
            emit_session(ctx, depth.saturating_add(1), child, false);
        }
        if inserted {
            ctx.lineage_stack.remove(&entry.id);
        }
    }

    // ADR 0071 retired the per-session candidate subtree. The
    // `MuxIndicator::Ambiguous` chip on the session row above
    // still signals the conflict; the detail pane for the
    // session's shared-ancestor group lists the actual muxes.
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

    fn workspace_with_name(root: &str, name: &str) -> GraphNode {
        GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: None,
            name: Some(name.to_string()),
        })
    }

    fn workspace_contains_repo_link(workspace_root: &str, repo_common_dir: &str) -> GraphLink {
        GraphLink {
            id: format!("test:wcr:{workspace_root}:{repo_common_dir}"),
            source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(RepoId::new(repo_common_dir)),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::StrongDiscovered,
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
        // Hybrid Graph (ADR 0064): A-class sessions sit directly
        // beneath their workspace at depth 1 — no intermediate repo
        // level. The repo is still implicit (a session always lives
        // in a checkout), but the workspace is the dominant
        // top-level entity for any session carrying an
        // `AssociatedWith Workspace` edge.
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

        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        let workspace_group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert!(matches!(
            workspace_group.primary_node,
            Some(NodeId::Workspace(_))
        ));
        assert_eq!(tree.rows[0].depth, 0);
        assert_eq!(
            tree.rows[1].depth, 1,
            "session sits directly under workspace — no repo intermediate",
        );
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

    // -----------------------------------------------------------------
    // H-WS-001: strict workspace grouping + weak-membership chip
    // -----------------------------------------------------------------
    //
    // Helper that pulls the session row out of a tree so the chip and
    // grouping depth can be asserted without repeating the matching
    // boilerplate. The session-of-interest in these tests is always
    // the lone AgentSession row.
    fn first_session_row(tree: &RowTree) -> (&Row, &AgentSessionRow) {
        for row in &tree.rows {
            if let RowKind::AgentSession(s) = &row.kind {
                return (row, s);
            }
        }
        panic!("no AgentSession row in tree:\n{:#?}", tree.rows);
    }

    fn find_session_row<'a>(tree: &'a RowTree, key: &str) -> (&'a Row, &'a AgentSessionRow) {
        for row in &tree.rows {
            if let RowKind::AgentSession(s) = &row.kind
                && s.session.session_key == key
            {
                return (row, s);
            }
        }
        panic!("no AgentSession {key:?} in tree:\n{:#?}", tree.rows);
    }

    #[test]
    fn workspace_rooted_session_nests_directly_under_workspace() {
        // Hybrid Graph (ADR 0064): A-class session sits at depth 1
        // directly under the workspace, no repo intermediate.
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
        snapshot.nodes.push(repo_with_source(
            "/home/op/atelier/conspectus/.git",
            "/home/op/atelier/conspectus",
        ));
        snapshot.nodes.push(worktree(
            "/home/op/atelier/conspectus/.git",
            "/home/op/atelier/conspectus",
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/atelier/conspectus/crates/core"),
            None,
            None,
        ));
        snapshot.candidate_links.push(workspace_contains_repo_link(
            "/home/op/atelier",
            "/home/op/atelier/conspectus/.git",
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&session_id, "/home/op/atelier"));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let (row, _) = first_session_row(&tree);
        assert_eq!(
            row.depth, 1,
            "workspace-rooted session sits directly under workspace at depth 1:\n{:#?}",
            tree.rows
        );
        // Workspace header uses the shared format_workspace_display
        // helper: name + member list + provider chip.
        let ws_group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => panic!("expected workspace group row at index 0:\n{:#?}", tree.rows),
        };
        assert!(
            ws_group.display_path.contains("atelier-ws"),
            "workspace header should include the workspace name `atelier-ws`, got `{}`",
            ws_group.display_path,
        );
    }

    #[test]
    fn repo_shared_session_stays_at_repo_level() {
        // Case B — session has no AssociatedWith but its repo is a
        // workspace member. Strict grouping (H-WS-001) puts it under
        // repo, not workspace. The fixture includes an (A)-class
        // session at the workspace root so both classes are present
        // in the tree; we assert each lands at the right depth.
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
        snapshot.nodes.push(repo_with_source(
            "/home/op/src/conspectus/.git",
            "/home/op/src/conspectus",
        ));
        snapshot.nodes.push(worktree(
            "/home/op/src/conspectus/.git",
            "/home/op/src/conspectus",
        ));
        let active_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "active"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "active",
            Some("/home/op/atelier"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&active_id, "/home/op/atelier"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "shared",
            Some("/home/op/src/conspectus/crates/core"),
            None,
            None,
        ));
        snapshot.candidate_links.push(workspace_contains_repo_link(
            "/home/op/atelier",
            "/home/op/src/conspectus/.git",
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

        let (shared_row, _) = find_session_row(&tree, "shared");
        assert_eq!(
            shared_row.depth, 1,
            "(B)-class session should sit under repo, not workspace:\n{:#?}",
            tree.rows
        );

        let (active_row, _) = find_session_row(&tree, "active");
        assert!(
            active_row.depth >= 1,
            "(A)-class session should sit under workspace header:\n{:#?}",
            tree.rows
        );
    }

    #[test]
    fn workspace_root_cwd_groups_under_workspace_without_checkout() {
        // ADR 0064 motivation: agent-deck launches the harness with
        // cwd at the workspace composite directory itself, not
        // inside a member subdir. That cwd has no checkout, so the
        // legacy resolve_group_key (which required a checkout)
        // dropped these sessions into the ungrouped bucket. The
        // hybrid path looks up the workspace edge first and groups
        // the session directly under the workspace.
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "wsroot"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(workspace_with_name(
            "/home/op/.agent-deck/multi-repo-worktrees/abc",
            "abc",
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "wsroot",
            Some("/home/op/.agent-deck/multi-repo-worktrees/abc"),
            None,
            None,
        ));
        snapshot.candidate_links.push(associated_with_workspace(
            &session_id,
            "/home/op/.agent-deck/multi-repo-worktrees/abc",
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

        assert_eq!(
            tree.rows.len(),
            2,
            "workspace header + session, no ungrouped bucket:\n{:#?}",
            tree.rows
        );
        assert!(
            matches!(&tree.rows[0].kind, RowKind::Group(g)
                if matches!(g.primary_node, Some(NodeId::Workspace(_)))),
            "row 0 must be a workspace group, got:\n{:#?}",
            tree.rows[0],
        );
        assert_eq!(tree.rows[0].depth, 0);
        assert!(matches!(&tree.rows[1].kind, RowKind::AgentSession(_)));
        assert_eq!(tree.rows[1].depth, 1);
    }

    #[test]
    fn hybrid_emits_workspace_and_repo_buckets_as_peer_top_level_parents() {
        // ADR 0064 shape: one A-class session whose workspace edge
        // determines a workspace bucket, plus one B-class session in
        // an unrelated repo. The Graph view should emit both at
        // depth 0, with sessions at depth 1 under each, and the
        // workspace bucket sorted before the repo bucket.
        let a_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/atelier", "atelier-ws"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "a",
            Some("/home/op/atelier"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&a_id, "/home/op/atelier"));

        snapshot.nodes.push(repo_with_source(
            "/home/op/src/standalone/.git",
            "/home/op/src/standalone",
        ));
        snapshot.nodes.push(worktree(
            "/home/op/src/standalone/.git",
            "/home/op/src/standalone",
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "b",
            Some("/home/op/src/standalone"),
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

        // Workspace bucket first, repo bucket second. Each bucket
        // contributes one header row at depth 0 plus one session
        // row at depth 1.
        let top_level: Vec<_> = tree.rows.iter().filter(|r| r.depth == 0).collect();
        assert_eq!(
            top_level.len(),
            2,
            "expected 2 top-level parent rows:\n{:#?}",
            tree.rows
        );

        let first = match &top_level[0].kind {
            RowKind::Group(g) => g,
            _ => panic!("expected Group row at top of tree:\n{:#?}", tree.rows),
        };
        assert!(
            matches!(first.primary_node, Some(NodeId::Workspace(_))),
            "workspace bucket should sort before repo bucket, got:\n{:#?}",
            tree.rows,
        );

        let second = match &top_level[1].kind {
            RowKind::Group(g) => g,
            _ => panic!("expected Group row for repo bucket:\n{:#?}", tree.rows),
        };
        assert!(
            matches!(second.primary_node, Some(NodeId::Repo(_))),
            "repo bucket follows workspace bucket:\n{:#?}",
            tree.rows,
        );

        let session_rows: Vec<_> = tree
            .rows
            .iter()
            .filter(|r| matches!(&r.kind, RowKind::AgentSession(_)))
            .collect();
        assert_eq!(
            session_rows.len(),
            2,
            "two sessions, one under each parent:\n{:#?}",
            tree.rows
        );
        for row in &session_rows {
            assert_eq!(
                row.depth, 1,
                "session sits one level under its parent (no repo intermediate under workspace, no checkout-fanout for single-worktree repo):\n{:#?}",
                tree.rows
            );
        }
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
    fn workspace_attributed_worktree_does_not_force_repo_checkout_fanout() {
        // Mirror the snapshot that surfaced this bug: a repo with two
        // discovered checkouts — its canonical checkout and a worktree
        // that lives inside an agent-deck workspace — where only the
        // canonical checkout has a (B)-class session. Sessions whose
        // cwd is the workspace root are (A)-class and group under the
        // workspace, not under the repo. The worktree fanout decision
        // must look at session-bearing repo buckets only, so the repo
        // collapses its single contributing worktree the same way a
        // repo with one discovered checkout does.
        let ws_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "ws"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/config"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/config", "/home/op/src/config"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/config", "/home/op/ws/abc/config"));
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/ws/abc", "abc"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "ws",
            Some("/home/op/ws/abc"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&ws_id, "/home/op/ws/abc"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "canon",
            Some("/home/op/src/config"),
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

        let kinds: Vec<&RowKind> = tree.rows.iter().map(|r| &r.kind).collect();
        let checkout_rows = kinds
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
            checkout_rows, 0,
            "single session-bearing worktree must collapse its checkout level even when a second worktree exists under a workspace:\n{:#?}",
            tree.rows
        );

        let (canon_row, _) = find_session_row(&tree, "canon");
        assert_eq!(
            canon_row.depth, 1,
            "(B)-class session sits directly under its repo:\n{:#?}",
            tree.rows
        );
        let (ws_row, _) = find_session_row(&tree, "ws");
        assert_eq!(
            ws_row.depth, 1,
            "(A)-class session sits directly under its workspace:\n{:#?}",
            tree.rows
        );
    }

    #[test]
    fn workspace_grouping_emits_header_for_every_workspace_even_without_sessions() {
        // ADR 0065: Sessions/Workspace must surface idle workspaces
        // the same way the dropped View::Workspaces did. A workspace
        // node with no (A)-class sessions still gets a header at
        // top level so operators see what workspaces exist on the
        // machine without flipping views.
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/ws/idle", "idle-ws"));
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/ws/busy", "busy-ws"));
        let busy_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "busy"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "busy",
            Some("/home/op/ws/busy"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&busy_id, "/home/op/ws/busy"));

        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Workspace,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let workspace_headers: Vec<&Row> = tree
            .rows
            .iter()
            .filter(|r| {
                matches!(
                    &r.kind,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Workspace(_)),
                        ..
                    })
                )
            })
            .collect();
        assert_eq!(
            workspace_headers.len(),
            2,
            "expected one header per workspace node:\n{:#?}",
            tree.rows
        );
        assert_eq!(workspace_headers[0].depth, 0);
        assert_eq!(workspace_headers[1].depth, 0);
    }

    #[test]
    fn workspace_grouping_drops_b_class_session_into_ungrouped() {
        // ADR 0065: a session whose cwd is inside a workspace
        // member's checkout but not associated with the workspace
        // (no AssociatedWith link) is (B)-class. In Workspace mode
        // there are no repo buckets, so it goes to Ungrouped.
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/ws/abc", "abc"));
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "b",
            Some("/home/op/src/proj"),
            None,
            None,
        ));

        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Workspace,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let ungrouped_header = tree.rows.iter().find(|r| {
            matches!(
                &r.kind,
                RowKind::Group(GroupRow { display_path, primary_node: None, .. })
                if display_path == "Ungrouped"
            )
        });
        assert!(
            ungrouped_header.is_some(),
            "B-class session must land under an Ungrouped header in Workspace mode:\n{:#?}",
            tree.rows
        );

        let repo_rows = tree
            .rows
            .iter()
            .filter(|r| {
                matches!(
                    &r.kind,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Repo(_)),
                        ..
                    })
                )
            })
            .count();
        assert_eq!(
            repo_rows, 0,
            "no repo buckets render in Workspace grouping:\n{:#?}",
            tree.rows
        );

        let (b_row, _) = find_session_row(&tree, "b");
        assert_eq!(
            b_row.depth, 1,
            "B-class session sits under the Ungrouped header at depth 1:\n{:#?}",
            tree.rows
        );
    }

    #[test]
    fn workspace_grouping_groups_a_class_session_under_workspace_header() {
        // ADR 0065 happy path: (A)-class session with an
        // AssociatedWith Workspace edge nests directly under its
        // workspace header at depth 1, mirroring Sessions/Graph.
        let ws_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "ws"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(workspace_with_name("/home/op/ws/abc", "abc"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "ws",
            Some("/home/op/ws/abc"),
            None,
            None,
        ));
        snapshot
            .candidate_links
            .push(associated_with_workspace(&ws_id, "/home/op/ws/abc"));

        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Workspace,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let (ws_row, _) = find_session_row(&tree, "ws");
        assert_eq!(
            ws_row.depth, 1,
            "(A)-class session sits at depth 1 under its workspace:\n{:#?}",
            tree.rows
        );
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
    fn two_mux_links_with_distinct_provenance_resolve_to_one_attached_mux() {
        // H-UI-008: the sessions tree consumes resolver winners,
        // not raw candidate links. With two `LinkedToMux` candidates
        // pointing at different muxes, the resolver picks the
        // higher-provenance candidate; the tree should reflect that
        // single winner as `Attached` rather than presenting both
        // candidates as an ambiguity. Pre-H-UI-008 the tree raised
        // a false-positive `Ambiguous` here because it grouped by
        // candidate target instead of consulting the resolver.
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
        // editor is StrongDiscovered (winner); scratch is
        // Discovered (loses the LinkedToMux slot).
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
            RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
            _ => unreachable!(),
        }
        assert!(!session_row.expandable);
        assert!(
            !tree
                .rows
                .iter()
                .any(|r| matches!(r.kind, RowKind::AgentSessionMuxCandidate(_))),
            "no candidate child rows when the resolver has a single winner",
        );
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
    fn bound_pin_appears_in_pins_group_and_marks_agent_session_row() {
        // The resolver synthesizes a LinkedToMux candidate when a pin
        // binds; we mimic that here by:
        // - adding the session and mux nodes
        // - adding a LinkedToMux candidate (the discovered one)
        // - declaring the pin with `binding = Bound`
        // Bound pins now appear in BOTH the synthetic "Pins" group
        // (top of view) AND the regular agent-session row (with
        // `pin_id` set) so the operator sees the pin in both
        // contexts.
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

        // Bound pin appears in the synthetic Pins group with state
        // "bound".
        let pin_group_row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Pin(row) => Some(row),
                _ => None,
            })
            .expect("pin row in synthetic Pins group");
        assert_eq!(pin_group_row.pin_id, "ingest");
        assert_eq!(pin_group_row.state_label, "bound");

        // The Pins group must come BEFORE the regular session
        // groups so it's the first thing the operator sees.
        let pin_group_idx = tree
            .rows
            .iter()
            .position(|r| matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins"))
            .expect("Pins group present");
        let first_non_pin_group_idx = tree.rows.iter().position(|r| {
            matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins" || *tag == "ungrouped")
                .then_some(false)
                .unwrap_or(
                    matches!(&r.kind, RowKind::Group(_))
                        && !matches!(&r.id, RowId::Synthetic(tag) if *tag == "pins"),
                )
        });
        if let Some(idx) = first_non_pin_group_idx {
            assert!(
                pin_group_idx < idx,
                "Pins group should precede other groups (Pins at {pin_group_idx}, first other at {idx})",
            );
        }

        // The agent-session row still carries the pin marker so
        // the operator sees the pin in-place too.
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
    fn mixed_pin_states_all_emit_in_synthetic_group() {
        // All declared pins surface in the Pins group regardless
        // of binding state; bound pins additionally mark their
        // existing agent-session row with `pin_id` so they're
        // visible in both places.
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
        // Bound pin appears alongside the two unbound ones in the
        // Pins group.
        assert_eq!(pin_rows, vec!["bound-one", "free-one", "free-two"]);

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

    // ----- P8-015: title-disambiguation in the row tree ----------------

    fn session_keys_with_disambiguating_titles(tree: &RowTree) -> Vec<String> {
        tree.rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(s) if s.title_disambiguates => {
                    Some(s.session.session_key.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn session_keys_in_order(tree: &RowTree) -> Vec<String> {
        tree.rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(s) => Some(s.session.session_key.clone()),
                _ => None,
            })
            .collect()
    }

    fn p8_015_snapshot_with_sessions(cwd: &str, specs: &[(&str, &str, Option<&str>)]) -> RowTree {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo(cwd));
        snapshot.nodes.push(worktree(cwd, cwd));
        for (harness, key, title) in specs {
            snapshot.nodes.push(agent_session(
                harness,
                "/state",
                key,
                Some(cwd),
                *title,
                None,
            ));
        }
        let snapshot = resolve_snapshot(snapshot);
        build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        })
    }

    #[test]
    fn title_disambiguation_off_for_single_session_per_harness() {
        // Case 1: a project group with one codex and one opencode
        // session — both already disambiguated by harness label, so
        // neither row should incorporate its title.
        let tree = p8_015_snapshot_with_sessions(
            "/home/op/src/proj",
            &[
                ("codex", "alpha", Some("scratch draft")),
                ("opencode", "beta", Some("review pass")),
            ],
        );
        let flagged = session_keys_with_disambiguating_titles(&tree);
        assert!(
            flagged.is_empty(),
            "no row should be flagged when harness already disambiguates: {flagged:?}"
        );
    }

    #[test]
    fn title_disambiguation_on_for_same_harness_siblings_with_distinct_titles() {
        // Case 2: two codex sessions in the same project group, both
        // carrying distinct titles — both rows are flagged.
        let tree = p8_015_snapshot_with_sessions(
            "/home/op/src/proj",
            &[
                ("codex", "alpha", Some("scratch draft")),
                ("codex", "beta", Some("review pass")),
            ],
        );
        let mut flagged = session_keys_with_disambiguating_titles(&tree);
        flagged.sort();
        assert_eq!(
            flagged,
            vec!["alpha".to_string(), "beta".to_string()],
            "both same-harness siblings should be flagged"
        );
    }

    #[test]
    fn title_disambiguation_only_flags_the_session_with_a_title() {
        // Case 3: two codex sessions in the same project group, but
        // only one has a title — only the titled row is flagged. The
        // untitled row stays clean (no `tree_label`).
        let tree = p8_015_snapshot_with_sessions(
            "/home/op/src/proj",
            &[
                ("codex", "alpha", Some("scratch draft")),
                ("codex", "beta", None),
            ],
        );
        let flagged = session_keys_with_disambiguating_titles(&tree);
        assert_eq!(flagged, vec!["alpha".to_string()]);
        // The untitled row still emits a normal AgentSessionRow — it
        // just doesn't gain a tree label.
        let alpha = tree
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(s) if s.session.session_key == "alpha" => Some(s),
                _ => None,
            })
            .expect("alpha row present");
        let beta = tree
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::AgentSession(s) if s.session.session_key == "beta" => Some(s),
                _ => None,
            })
            .expect("beta row present");
        assert_eq!(alpha.tree_label(), Some("scratch draft"));
        assert_eq!(beta.tree_label(), None);
    }

    #[test]
    fn title_disambiguation_flips_deterministically_on_refresh_without_reordering() {
        // Case 4: a project starts with one codex session whose title
        // is hidden (no collision). A refresh adds a second codex
        // session with a different title; the previously-clean row
        // gains its title and the new row arrives with a title too,
        // both in the same deterministic position the row tree built
        // them in. Sort order across the refresh is preserved (the
        // collision flag never re-keys the sort).
        let first = p8_015_snapshot_with_sessions(
            "/home/op/src/proj",
            &[("codex", "alpha", Some("scratch draft"))],
        );
        assert!(
            session_keys_with_disambiguating_titles(&first).is_empty(),
            "single-session group should not flag titles"
        );
        let pre_order = session_keys_in_order(&first);
        assert_eq!(pre_order, vec!["alpha".to_string()]);

        let second = p8_015_snapshot_with_sessions(
            "/home/op/src/proj",
            &[
                ("codex", "alpha", Some("scratch draft")),
                ("codex", "beta", Some("review pass")),
            ],
        );
        let mut flagged = session_keys_with_disambiguating_titles(&second);
        flagged.sort();
        assert_eq!(
            flagged,
            vec!["alpha".to_string(), "beta".to_string()],
            "both rows should gain the disambiguation flag once a sibling appears"
        );
        let post_order = session_keys_in_order(&second);
        // `alpha` retains its slot; `beta` appends. The session-key
        // sort order is alphabetical, so this is the stable shape.
        assert_eq!(
            post_order,
            vec!["alpha".to_string(), "beta".to_string()],
            "row order must stay deterministic across the refresh"
        );
    }
}
