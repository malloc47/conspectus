//! In-memory mux-view row-tree builder (ADR 0082).
//!
//! The mux view is mux-session oriented: one row per mux
//! session, with compact metrics about known attached agent
//! sessions. The common zero-or-one-agent case stays flat;
//! muxes linked to multiple agents expose those agents as
//! child rows for drill-down.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, MuxSessionId, MuxSessionNode, NodeId, PinBinding, PinCandidate, PinId, RelationKind,
    RepoId, path_is_ancestor_of,
};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, MuxSessionRow, Row, RowId, RowKind, RowTree,
    ViewLabel, format_recency, harness_label, mux_process_harnesses, mux_program,
    mux_program_harness, shorten_home,
};
use crate::tui::{MuxGrouping, Sort};

pub struct MuxBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
    pub grouping: MuxGrouping,
    pub sort: Sort,
    /// Which recency signal `Sort::Recency` orders mux rows by.
    /// Ignored under `Sort::Hierarchy`.
    pub mux_recency: crate::tui::MuxRecency,
}

#[derive(Clone, Debug)]
struct MuxData<'a> {
    node_id: String,
    id: MuxSessionId,
    node: &'a MuxSessionNode,
}

impl MuxData<'_> {
    fn effective_cwd(&self) -> Option<&str> {
        self.node
            .active_pane_current_path
            .as_deref()
            .or(self.node.cwd.as_deref())
    }
}

#[derive(Clone, Debug)]
struct AttachedAgent {
    node_id: String,
    id: AgentSessionId,
    harness_key: String,
    cwd: Option<String>,
    title: Option<String>,
    alias: Option<String>,
    preview: Option<String>,
    last_active_epoch: Option<i64>,
    candidate_count: usize,
}

pub fn build_mux_tree(inputs: MuxBuildInputs<'_>) -> RowTree {
    let snapshot = inputs.snapshot;
    let now = inputs.now;

    let muxes = collect_muxes(snapshot);
    let attachments = collect_attached_agents(snapshot);
    let pins: Vec<&PinCandidate> = snapshot.pins.iter().collect();
    let process_harnesses = mux_process_harnesses(snapshot);

    let mut pin_by_mux: HashMap<NodeId, &PinCandidate> = HashMap::new();
    for pin in &pins {
        match &pin.binding {
            Some(PinBinding::Bound { mux, .. } | PinBinding::StaleMux { mux }) => {
                pin_by_mux.insert(NodeId::MuxSession(mux.clone()), pin);
            }
            Some(PinBinding::Unbound) | None => {}
        }
    }

    let mut node_ids: Vec<String> = muxes.iter().map(|mux| mux.node_id.clone()).collect();
    for attached in attachments.values() {
        node_ids.extend(attached.iter().map(|agent| agent.node_id.clone()));
    }
    let full_ids: Vec<String> = node_ids
        .iter()
        .map(|id| node_short_id_from_display(id))
        .collect();
    let id_len = unique_prefix_len(&full_ids);
    let short_ids: HashMap<&str, String> = node_ids
        .iter()
        .zip(full_ids.iter())
        .map(|(node_id, full)| (node_id.as_str(), full[..id_len].to_string()))
        .collect();

    let mut tree = RowTree {
        view: ViewLabel::Mux,
        ..RowTree::default()
    };

    // Build each visible mux as a (parent, children) group so the
    // grouping phase can keep parents and their child agent rows
    // adjacent and the optional float-top sort can move them as a
    // unit.
    let mut groups: Vec<MuxGroup> = Vec::new();
    for mux in &muxes {
        let attached = attachments.get(&mux.node_id).cloned().unwrap_or_default();
        let visible_attached: Vec<AttachedAgent> = attached
            .iter()
            .filter(|agent| agent_matches_filter(agent, now, &inputs.filter))
            .cloned()
            .collect();

        if !mux_matches_filter(&attached, &visible_attached, &inputs.filter) {
            continue;
        }

        let latest_agent_epoch = visible_attached
            .iter()
            .filter_map(|agent| agent.last_active_epoch)
            .max();
        let activity_epoch = latest_epoch(mux.node.activity_epoch, latest_agent_epoch);
        let ambiguous_count = visible_attached
            .iter()
            .filter(|agent| agent.candidate_count > 1)
            .count();
        let node_id = NodeId::MuxSession(mux.id.clone());
        let single_session_preview = if visible_attached.len() == 1 {
            visible_attached[0]
                .preview
                .clone()
                .filter(|preview| !preview.is_empty())
        } else {
            None
        };
        let attached_count = visible_attached.len();
        let cwd = mux.effective_cwd().map(std::string::ToString::to_string);

        let parent_row = MuxSessionRow {
            mux: mux.id.clone(),
            backend: mux.node.backend.clone(),
            native_id: mux.node.native_id.clone(),
            client_attached: mux.node.client_attached,
            cwd_display: cwd.as_deref().map(|cwd| shorten_home(cwd, inputs.home)),
            attached_count,
            ambiguous_count,
            recency: format_recency(now, activity_epoch),
            activity_epoch,
            created_epoch: mux.node.created_epoch,
            last_attached_epoch: mux.node.last_attached_epoch,
            agent_labels: agent_labels(&visible_attached),
            program: mux_program(mux.node, pin_by_mux.get(&node_id).copied()),
            program_harness: mux_program_harness(
                mux.node,
                pin_by_mux.get(&node_id).copied(),
                process_harnesses.get(&node_id).map(String::as_str),
            ),
            single_session_preview,
            pin_id: pin_by_mux.get(&node_id).map(|pin| pin.id.clone()),
            primary_node: node_id.clone(),
        };

        let children: Vec<AttachedAgent> = if attached_count > 1 {
            visible_attached.clone()
        } else {
            Vec::new()
        };

        groups.push(MuxGroup {
            parent_row,
            parent_node_id: node_id,
            children,
            attached_count,
            cwd,
        });
    }

    for pin in pins
        .iter()
        .filter(|pin| matches!(pin.binding, Some(PinBinding::Unbound) | None))
    {
        groups.push(placeholder_mux_group_for_pin(pin, inputs.home));
    }

    match inputs.grouping {
        MuxGrouping::Session | MuxGrouping::Host => {
            emit_flat(&mut tree, groups, &inputs, &short_ids, now);
        }
        MuxGrouping::Repo => {
            // Pins group sits above the repo-grouped muxes so
            // the operator sees the deck's pinned work first.
            emit_pins_group_for_mux(&mut tree, &pins, &groups, &short_ids, inputs.home, now);

            let path_index = PathIndex::from_snapshot(snapshot);
            emit_repo_grouped(&mut tree, groups, &inputs, &short_ids, &path_index, now);
        }
    }

    tree
}

fn emit_pins_group_for_mux(
    tree: &mut RowTree,
    pins: &[&PinCandidate],
    groups: &[MuxGroup],
    short_ids: &HashMap<&str, String>,
    home: Option<&Path>,
    now: Option<i64>,
) {
    if pins.is_empty() {
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

    let group_by_mux: HashMap<NodeId, &MuxGroup> = groups
        .iter()
        .map(|group| (group.parent_node_id.clone(), group))
        .collect();
    let mut emitted_muxes: HashSet<NodeId> = HashSet::new();

    for pin in pins {
        let bound_mux = match &pin.binding {
            Some(PinBinding::Bound { mux, .. } | PinBinding::StaleMux { mux }) => {
                Some(NodeId::MuxSession(mux.clone()))
            }
            Some(PinBinding::Unbound) | None => None,
        };
        if let Some(mux_id) = bound_mux
            && emitted_muxes.insert(mux_id.clone())
            && let Some(group) = group_by_mux.get(&mux_id)
        {
            (*group).clone().push_into(tree, 1, short_ids, home, now);
            continue;
        }

        placeholder_mux_group_for_pin(pin, home).push_into(tree, 1, short_ids, home, now);
    }
}

fn placeholder_mux_group_for_pin(pin: &PinCandidate, home: Option<&Path>) -> MuxGroup {
    let pin_node = NodeId::Pin(PinId::new(pin.id.clone()));
    let native_id = pin.mux.socket_name.as_ref().map_or_else(
        || pin.mux.name.clone(),
        |socket| format!("{socket}:{}", pin.mux.name),
    );
    MuxGroup {
        parent_row: MuxSessionRow {
            mux: MuxSessionId::new(pin.mux.native_id()),
            backend: pin.mux.backend.clone(),
            native_id,
            client_attached: None,
            cwd_display: Some(shorten_home(&pin.cwd, home)),
            attached_count: 0,
            ambiguous_count: 0,
            recency: None,
            activity_epoch: None,
            created_epoch: None,
            last_attached_epoch: None,
            agent_labels: vec![harness_label(&pin.harness)],
            program: None,
            program_harness: None,
            single_session_preview: Some(shorten_home(&pin.cwd, home)),
            pin_id: Some(pin.id.clone()),
            primary_node: pin_node.clone(),
        },
        parent_node_id: pin_node,
        children: Vec::new(),
        attached_count: 0,
        cwd: Some(pin.cwd.clone()),
    }
}

/// Per-mux work product collected by the build loop.
#[derive(Clone, Debug)]
struct MuxGroup {
    parent_row: MuxSessionRow,
    parent_node_id: NodeId,
    children: Vec<AttachedAgent>,
    attached_count: usize,
    cwd: Option<String>,
}

impl MuxGroup {
    fn push_into(
        self,
        tree: &mut RowTree,
        depth: u8,
        short_ids: &HashMap<&str, String>,
        home: Option<&Path>,
        now: Option<i64>,
    ) {
        let MuxGroup {
            parent_row,
            parent_node_id,
            children,
            attached_count,
            cwd: _,
        } = self;
        tree.rows.push(Row {
            id: RowId::MuxSession(parent_node_id),
            depth,
            expandable: attached_count > 1,
            kind: RowKind::MuxSession(parent_row),
        });
        let child_depth = depth.saturating_add(1);
        for agent in children {
            tree.rows.push(agent_row(
                &agent,
                child_depth,
                short_ids
                    .get(agent.node_id.as_str())
                    .cloned()
                    .unwrap_or_default(),
                home,
                now,
            ));
        }
    }
}

fn emit_flat(
    tree: &mut RowTree,
    mut groups: Vec<MuxGroup>,
    inputs: &MuxBuildInputs<'_>,
    short_ids: &HashMap<&str, String>,
    now: Option<i64>,
) {
    sort_mux_groups(&mut groups, inputs);
    for group in groups {
        group.push_into(tree, 0, short_ids, inputs.home, now);
    }
}

fn emit_repo_grouped(
    tree: &mut RowTree,
    groups: Vec<MuxGroup>,
    inputs: &MuxBuildInputs<'_>,
    short_ids: &HashMap<&str, String>,
    path_index: &PathIndex<'_>,
    now: Option<i64>,
) {
    let mut buckets: BTreeMap<RepoBucketKey, Vec<MuxGroup>> = BTreeMap::new();
    let mut ungrouped: Vec<MuxGroup> = Vec::new();

    for group in groups {
        let resolved = group
            .cwd
            .as_deref()
            .and_then(|cwd| path_index.checkout_for_path(Path::new(cwd)));
        match resolved {
            Some(checkout_id) => {
                let key = RepoBucketKey {
                    repo_id: checkout_id.repo.clone(),
                    display_path: path_index.repo_display_path(&checkout_id.repo),
                };
                buckets.entry(key).or_default().push(group);
            }
            None => ungrouped.push(group),
        }
    }

    for (key, mut groups) in buckets {
        let RepoBucketKey {
            repo_id,
            display_path,
        } = key;
        push_repo_header(tree, 0, &repo_id, &display_path, inputs.home);
        sort_mux_groups(&mut groups, inputs);
        for group in groups {
            group.push_into(tree, 1, short_ids, inputs.home, now);
        }
    }

    if !ungrouped.is_empty() {
        tree.rows.push(Row {
            id: RowId::Synthetic("ungrouped"),
            depth: 0,
            expandable: true,
            kind: RowKind::Group(GroupRow {
                display_path: "Ungrouped".to_string(),
                primary_node: None,
                is_launch_context: false,
            }),
        });
        sort_mux_groups(&mut ungrouped, inputs);
        for group in ungrouped {
            group.push_into(tree, 1, short_ids, inputs.home, now);
        }
    }
}

/// Recency ordering key for a mux row under the chosen basis.
/// Activity blends in attached-agent activity (its
/// `activity_epoch` is already the blended value); created and
/// last-attached read the raw session-lifecycle epochs. A missing
/// signal sorts oldest (`None` < `Some`), so sessions lacking the
/// chosen epoch fall to the bottom of the recency order deterministically.
fn recency_key(row: &MuxSessionRow, basis: crate::tui::MuxRecency) -> Option<i64> {
    match basis {
        crate::tui::MuxRecency::Activity => row.activity_epoch,
        crate::tui::MuxRecency::Created => row.created_epoch,
        crate::tui::MuxRecency::LastAttached => row.last_attached_epoch,
    }
}

fn sort_mux_groups(groups: &mut [MuxGroup], inputs: &MuxBuildInputs<'_>) {
    groups.sort_by(|left, right| {
        let pinned = usize::from(left.parent_row.pin_id.is_none())
            .cmp(&usize::from(right.parent_row.pin_id.is_none()));
        if pinned != std::cmp::Ordering::Equal {
            return pinned;
        }
        let left_float = usize::from(left.attached_count == 0);
        let right_float = usize::from(right.attached_count == 0);
        let float_order = if inputs.filter.float_attached_muxes_top {
            left_float.cmp(&right_float)
        } else {
            std::cmp::Ordering::Equal
        };
        float_order.then_with(|| match inputs.sort {
            Sort::Hierarchy => std::cmp::Ordering::Equal,
            Sort::Recency => recency_key(&right.parent_row, inputs.mux_recency)
                .cmp(&recency_key(&left.parent_row, inputs.mux_recency))
                .then_with(|| left.parent_row.native_id.cmp(&right.parent_row.native_id))
                .then_with(|| left.parent_node_id.cmp(&right.parent_node_id)),
        })
    });
}

fn push_repo_header(
    tree: &mut RowTree,
    depth: u8,
    repo_id: &RepoId,
    display_path: &str,
    home: Option<&Path>,
) {
    let node_id = NodeId::Repo(repo_id.clone());
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(display_path, home),
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoBucketKey {
    repo_id: RepoId,
    display_path: String,
}

impl Ord for RepoBucketKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.display_path
            .cmp(&other.display_path)
            .then_with(|| self.repo_id.common_dir.cmp(&other.repo_id.common_dir))
    }
}

impl PartialOrd for RepoBucketKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) struct PathIndex<'a> {
    snapshot: &'a GraphSnapshot,
    checkouts: Vec<(PathBuf, CheckoutId)>,
}

impl<'a> PathIndex<'a> {
    pub(super) fn from_snapshot(snapshot: &'a GraphSnapshot) -> Self {
        let mut checkouts = Vec::new();
        for node in &snapshot.nodes {
            if let GraphNode::Checkout(c) = node {
                checkouts.push((PathBuf::from(&c.id.root), c.id.clone()));
            }
        }
        Self {
            snapshot,
            checkouts,
        }
    }

    pub(super) fn checkout_for_path(&self, path: &Path) -> Option<&CheckoutId> {
        self.checkouts
            .iter()
            .filter(|(root, _)| path_is_ancestor_of(root, path))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, id)| id)
    }

    pub(super) fn repo_display_path(&self, repo: &RepoId) -> String {
        let common_dir = repo
            .common_dir
            .strip_suffix("/.git")
            .unwrap_or(&repo.common_dir)
            .to_string();
        for node in &self.snapshot.nodes {
            if let GraphNode::Repo(r) = node
                && r.id == *repo
                && let Some(path) = r
                    .source_paths
                    .iter()
                    .find(|p| !p.contains("/.agent-deck/multi-repo-worktrees/"))
            {
                return path.clone();
            }
        }
        common_dir
    }
}

fn latest_epoch(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn agent_labels(agents: &[AttachedAgent]) -> Vec<String> {
    let mut labels: Vec<String> = agents
        .iter()
        .map(|agent| harness_label(&agent.harness_key))
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

fn mux_matches_filter(
    all_attached: &[AttachedAgent],
    visible_attached: &[AttachedAgent],
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    if !visible_attached.is_empty() {
        return true;
    }
    let RowFilter {
        harness,
        max_age,
        mux_state,
        float_muxed_sessions_top: _,
        float_attached_muxes_top: _,
    } = filter;
    harness.is_none()
        && max_age.is_none()
        && mux_state
            .as_ref()
            .is_some_and(|mux_state| mux_state.values().contains(&MuxStateKey::Unmuxed))
        && all_attached.is_empty()
}

fn agent_matches_filter(agent: &AttachedAgent, now: Option<i64>, filter: &RowFilter) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.harness_key,
        now_epoch: now,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(agent.candidate_count),
    })
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_muxes(snapshot: &GraphSnapshot) -> Vec<MuxData<'_>> {
    let mut rows: Vec<MuxData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(MuxData {
                node_id: NodeId::MuxSession(mux.id.clone()).to_string(),
                id: mux.id.clone(),
                node: mux,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    rows
}

/// Attached agents filter through the resolver. Only
/// `LinkedToMux` candidates whose `link_id` appears in
/// `snapshot.resolved_relationships.selected_link_id` for the
/// matching relation are surfaced — drops false-positive
/// attachments under non-winning cwd evidence.
fn collect_attached_agents(snapshot: &GraphSnapshot) -> HashMap<String, Vec<AttachedAgent>> {
    // Consult the shared SnapshotIndex instead
    // of maintaining a local `collect_agent_mux_candidate_counts`
    // copy. The clone is cheap for typical graph sizes.
    let candidate_counts = crate::model::SnapshotIndex::new(snapshot)
        .agent_mux_candidate_counts()
        .clone();
    let selected_link_ids: HashSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::LinkedToMux))
        .filter_map(|r| r.selected_link_id.as_deref())
        .collect();
    let agent_lookup: HashMap<NodeId, &AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => Some((NodeId::AgentSession(agent.id.clone()), agent)),
            _ => None,
        })
        .collect();

    let mut collected: Vec<(String, AttachedAgent)> = Vec::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::LinkedToMux) {
            continue;
        }
        if !selected_link_ids.contains(link.id.as_str()) {
            continue;
        }
        let NodeId::AgentSession(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: target_id @ NodeId::MuxSession(_),
        } = &link.target
        else {
            continue;
        };
        let Some(agent) = agent_lookup.get(&link.source) else {
            continue;
        };
        let agent_node_id = NodeId::AgentSession(agent.id.clone()).to_string();
        let alias = snapshot
            .aliases
            .get(&NodeId::AgentSession(agent.id.clone()))
            .map(std::string::ToString::to_string);
        collected.push((
            target_id.to_string(),
            AttachedAgent {
                node_id: agent_node_id.clone(),
                id: agent.id.clone(),
                harness_key: agent.harness_key.clone(),
                cwd: agent.cwd.clone(),
                title: agent.title.clone(),
                alias,
                preview: agent.last_message_preview.clone(),
                last_active_epoch: agent.last_active_epoch,
                candidate_count: candidate_counts.get(&agent_node_id).copied().unwrap_or(0),
            },
        ));
    }

    // Dedupe + group + sort by mux id, then attached agent id.
    let mut out: HashMap<String, Vec<AttachedAgent>> = HashMap::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (mux_id, agent) in collected {
        if !seen.insert((mux_id.clone(), agent.node_id.clone())) {
            continue;
        }
        out.entry(mux_id).or_default().push(agent);
    }
    for agents in out.values_mut() {
        agents.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    }
    out
}

fn agent_row(
    agent: &AttachedAgent,
    depth: u8,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
            pin_id: None,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.last_active_epoch),
            activity_epoch: agent.last_active_epoch,
            mux_state: mux_indicator(agent.candidate_count),
            preview: agent.preview.clone(),
            title: agent.title.clone(),
            alias: agent.alias.clone(),
            title_disambiguates: false,
            primary_node: node_id,
        }),
    }
}

fn mux_indicator(candidate_count: usize) -> MuxIndicator {
    match candidate_count {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    }
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
