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
    AgentSessionId, AgentSessionNode, CheckoutId, GraphLink, GraphNode, GraphSnapshot, LinkState,
    NodeId, PinBinding, PinCandidate, PinId, RelationKind, RepoId, WorkspaceId, WorktreeKind,
    WorktreeMeta, path_is_ancestor_of,
};
use crate::tui::SessionsGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, Row, RowId, RowKind, RowTree, ViewLabel,
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
                    pinned: data.is_pinned_session(session_id),
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
        for pin in &data.snapshot.pins {
            if !matches!(pin.binding, Some(PinBinding::Bound { .. })) {
                emit_pin_placeholder_session_row(&mut ctx, pin, 0);
            }
        }
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
            pinned: data.is_pinned_session(session_id),
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

    // Pins group lives at the top of the grouped sessions view so
    // operators see the deck's pinned work before scrolling through
    // the broader session list. Bound pins render as the realized
    // agent-session row inside this group, not as pin config rows,
    // so selection/detail/formatting all remain session-oriented.
    emit_pins_group(&mut ctx);

    // Mirror bound pins' "appear in both the Pins group AND the
    // natural project bucket" pattern for unbound / stale-mux pins:
    // bucket each non-bound pin's placeholder by the same group key
    // its cwd would produce, pre-seed any buckets that contain only
    // placeholders so the project header still emits, then hand the
    // placeholder list to `emit_checkout_bucket` so it emits after
    // the bucket's sessions at the matching depth.
    let mut placeholders_by_key: BTreeMap<GroupKey, Vec<&PinCandidate>> = BTreeMap::new();
    for pin in &ctx.data.snapshot.pins {
        if matches!(pin.binding, Some(PinBinding::Bound { .. })) {
            continue;
        }
        if let Some(key) = pin_group_key(pin, ctx.data, inputs.grouping) {
            placeholders_by_key.entry(key).or_default().push(pin);
        }
    }
    for key in placeholders_by_key.keys() {
        buckets.entry(key.clone()).or_default();
    }

    // Iterate worktree-keyed buckets while deduplicating their
    // ancestor headers. Buckets ordered by (workspace, repo,
    // worktree) come out adjacent for the same (workspace, repo)
    // pair, so we just track the most recent header keys and emit
    // each only on change.
    let mut previous = PreviousHeader::Nothing;
    for (key, sessions) in buckets {
        let placeholders = placeholders_by_key.remove(&key).unwrap_or_default();
        emit_checkout_bucket(&mut ctx, key, sessions, placeholders, &mut previous);
    }

    if !ungrouped.is_empty() {
        emit_ungrouped(&mut ctx, ungrouped);
    }

    mark_launch_context(&mut tree, inputs.cwd);

    tree
}

/// Emit a synthetic "Pins" group for the grouped sessions view.
/// Bound pins use the realized [`RowKind::AgentSession`] row because
/// this view is session-first: the top bucket should answer "which
/// sessions are pinned?" rather than "which pin declarations exist?".
/// Unbound/stale/pre-resolve pins render as session-shaped
/// placeholder rows: the row still points at the pin node, and a dim
/// placeholder glyph plus the pin's cwd identifies it as a not-running
/// declaration without overloading the row with extra vocabulary.
fn emit_pins_group(ctx: &mut EmitCtx<'_, '_>) {
    if ctx.data.snapshot.pins.is_empty() {
        return;
    }

    ctx.tree.rows.push(Row {
        id: RowId::Synthetic("pins"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "Pins".to_string(),
            primary_node: None,
            is_launch_context: false,
        }),
    });

    for pin in &ctx.data.snapshot.pins {
        if let Some(PinBinding::Bound { session, .. }) = &pin.binding {
            let session_id = NodeId::AgentSession(session.clone());
            if let Some(session_node) = ctx.data.agent_sessions.get(&session_id) {
                let mux_state = MuxStateKey::from_candidate_count(
                    ctx.data.mux_candidates_for_session(&session_id).len(),
                );
                emit_session_row_only(
                    ctx,
                    1,
                    SessionEntry {
                        id: session_id,
                        node: session_node,
                        mux_state,
                        pinned: true,
                    },
                    false,
                );
                continue;
            }
        }

        emit_pin_placeholder_session_row(ctx, pin, 1);
    }
}

fn emit_pin_placeholder_session_row(ctx: &mut EmitCtx<'_, '_>, pin: &PinCandidate, depth: u8) {
    let pin_node = NodeId::Pin(PinId::new(pin.id.clone()));
    ctx.tree.rows.push(Row {
        id: RowId::Pin {
            pin_id: pin.id.clone(),
        },
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: AgentSessionId::new(&pin.harness, format!("pin:{}", pin.id), &pin.id),
            short_id: pin.id.clone(),
            harness_label: harness_label(&pin.harness),
            cwd_display: Some(shorten_home(&pin.cwd, ctx.home)),
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: Some(shorten_home(&pin.cwd, ctx.home)),
            title: None,
            alias: Some(pin.display_name.clone()),
            title_disambiguates: false,
            primary_node: pin_node,
            pin_id: Some(pin.id.clone()),
        }),
    });
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
    /// marker (ADR 0057) without re-walking the pin
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
        // The sessions tree follows resolver winners. When the
        // resolver suppresses a winner (ADR 0077) it keeps the
        // `LinkedToMux` slot alive with `selected_link_id = None`
        // and the candidate set rolled into `competing_link_ids`.
        // Walk each matching slot once:
        // - `Some(winner)`: the resolver picked. Return the
        //   winning link so the row reports `Attached`.
        // - `None`: the resolver explicitly cannot pick. Return
        //   every candidate the slot lists so the row reports
        //   `Ambiguous { candidate_count }` and the operator sees
        //   the fan-out.
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

    fn is_pinned_session(&self, session: &NodeId) -> bool {
        self.pin_id_by_bound_session.contains_key(session)
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
        // Read resolver winners only so the tree row's
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
    /// uses, so the two view headers read identically.
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
    /// True when this session is the live realization of a declared
    /// pin. Flat views float these entities above non-pinned rows.
    pinned: bool,
}

/// Pick the group key an unbound / stale-mux pin's placeholder row
/// should land in, alongside any sessions that bucket to the same
/// key. Mirrors the repo-path arm of `resolve_group_key` so a
/// placeholder shows up under its declared cwd's project bucket — the
/// same way a bound pin's realized session row does. Pins do not
/// carry an `AssociatedWith Workspace` edge, so the workspace and
/// workspace-only grouping arms simply return `None` and leave the
/// pin in the synthetic Pins group only.
fn pin_group_key(
    pin: &PinCandidate,
    data: &SessionsData<'_>,
    grouping: SessionsGrouping,
) -> Option<GroupKey> {
    if matches!(grouping, SessionsGrouping::Workspace) {
        return None;
    }
    let (worktree_id, _worktree) = data.checkout_for_path(&pin.cwd)?;
    let repo_id = worktree_id.repo.clone();
    let repo = Some(RepoBucket {
        common_dir: repo_id.common_dir.clone(),
        repo_display_path: data.repo_display_path(&repo_id),
        repo_id,
    });
    let worktree = Some(worktree_id.root);
    Some(GroupKey {
        workspace: None,
        repo,
        worktree,
    })
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
    let worktree = Some(worktree_id.root);

    Some(GroupKey {
        workspace: None,
        repo,
        worktree,
    })
}

/// The top-level header the previous bucket emitted, so adjacent
/// buckets share it instead of repeating it.
#[derive(PartialEq)]
enum PreviousHeader {
    Nothing,
    Workspace(String),
    Repo(RepoId),
}

/// Emit one bucket while reusing the previous bucket's workspace or
/// repo group row when it is the same. Updates `previous` to the
/// header this bucket sits under.
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
    placeholder_pins: Vec<&PinCandidate>,
    previous: &mut PreviousHeader,
) {
    sessions.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));

    if let Some(workspace_root) = key.workspace {
        if *previous != PreviousHeader::Workspace(workspace_root.clone()) {
            push_workspace_row(ctx.tree, 0, &workspace_root, ctx.data, ctx.home);
        }
        let disambiguating = title_disambiguating_sessions(&sessions);
        for entry in sessions {
            let flag = disambiguating.contains(&entry.id);
            emit_session(ctx, 1, entry, flag);
        }
        for pin in placeholder_pins {
            emit_pin_placeholder_session_row(ctx, pin, 1);
        }
        *previous = PreviousHeader::Workspace(workspace_root);
        return;
    }

    // Repo path. `key.repo` is `Some` here by construction
    // (`resolve_group_key` and `pin_group_key` both build either a
    // workspace-only or a repo bucket, never neither).
    let Some(repo_bucket) = key.repo else {
        return;
    };
    let repo_changed = *previous != PreviousHeader::Repo(repo_bucket.repo_id.clone());

    let depth: u8 = 0;
    if repo_changed {
        push_repo_row(ctx.tree, depth, &repo_bucket, ctx.home);
    }

    let session_bearing_count = ctx
        .session_bearing_worktrees
        .get(&repo_bucket.repo_id)
        .map_or(0, BTreeSet::len);
    let checkout_should_render =
        matches!(ctx.grouping, SessionsGrouping::Checkout) || session_bearing_count >= 2;
    let session_depth = if checkout_should_render && let Some(wt_root) = &key.worktree {
        // Flag linked / locked / prunable worktrees in the
        // checkout group header. A plain primary gets no marker so the
        // common case stays quiet.
        let marker = ctx
            .data
            .checkouts
            .get(&NodeId::Checkout(CheckoutId::new(
                repo_bucket.repo_id.clone(),
                wt_root.clone(),
            )))
            .and_then(|node| node.worktree.as_ref())
            .and_then(worktree_group_marker);
        push_checkout_row(
            ctx.tree,
            depth.saturating_add(1),
            &repo_bucket.repo_id,
            wt_root,
            ctx.home,
            marker,
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
    for pin in placeholder_pins {
        emit_pin_placeholder_session_row(ctx, pin, session_depth);
    }

    *previous = PreviousHeader::Repo(repo_bucket.repo_id);
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
    worktree_marker: Option<String>,
) {
    let wt_id = CheckoutId::new(repo.clone(), worktree_root.to_string());
    let node_id = NodeId::Checkout(wt_id);
    let mut display_path = shorten_home(worktree_root, home);
    if let Some(marker) = worktree_marker {
        display_path.push_str(&format!(" ({marker})"));
    }
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

/// Marker for a checkout group header. Returns `None` for a
/// plain primary worktree (the common, unremarkable case) so the
/// sessions tree stays quiet; flags linked / locked / prunable
/// worktrees, which are the ones worth calling out.
fn worktree_group_marker(meta: &WorktreeMeta) -> Option<String> {
    let mut parts = Vec::new();
    if matches!(meta.kind, WorktreeKind::Linked) {
        parts.push("linked");
    }
    if meta.locked.is_some() {
        parts.push("locked");
    }
    if meta.prunable.is_some() {
        parts.push("prunable");
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
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

/// Compute the set of session ids in `entries` whose row
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
    emit_session_row(ctx, depth, entry, title_disambiguates, true);
}

fn emit_session_row_only(
    ctx: &mut EmitCtx<'_, '_>,
    depth: u8,
    entry: SessionEntry<'_>,
    title_disambiguates: bool,
) {
    emit_session_row(ctx, depth, entry, title_disambiguates, false);
}

fn emit_session_row(
    ctx: &mut EmitCtx<'_, '_>,
    depth: u8,
    entry: SessionEntry<'_>,
    title_disambiguates: bool,
    include_lineage_children: bool,
) {
    let candidates = ctx.data.mux_candidates_for_session(&entry.id);
    let mux_state = match candidates.len() {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    };
    let mut lineage_children = visible_lineage_children(ctx, &entry.id);
    lineage_children.sort_by(|a, b| compare_sessions(a, b, ctx.float_muxed_top));
    let has_lineage_children = include_lineage_children && !lineage_children.is_empty();
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
                pinned: ctx.data.is_pinned_session(child_id),
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

/// Sessions sort pinned rows first, then by recency desc (None last),
/// then alphabetical by harness then short id for a stable tie-breaker.
/// When `float_muxed_top` is set, sessions with an attached mux
/// candidate sort before sessions without one, with the existing
/// within-group order preserved inside each of the two resulting
/// halves.
fn compare_sessions(
    a: &SessionEntry<'_>,
    b: &SessionEntry<'_>,
    float_muxed_top: bool,
) -> std::cmp::Ordering {
    let pinned = usize::from(!a.pinned).cmp(&usize::from(!b.pinned));
    if pinned != std::cmp::Ordering::Equal {
        return pinned;
    }
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
/// `output::render::unique_prefix_len` but tailored to the agent
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
#[path = "sessions_tests.rs"]
mod tests;
