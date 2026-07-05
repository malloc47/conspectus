//! In-memory mux-view row-tree builder (P11-011d / ADR 0082).
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
    ViewLabel, format_recency, harness_label, shorten_home,
};
use crate::tui::{MuxGrouping, Sort};

pub struct MuxBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
    pub grouping: MuxGrouping,
    pub sort: Sort,
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

    let mut pin_id_by_mux: HashMap<NodeId, String> = HashMap::new();
    for pin in &pins {
        match &pin.binding {
            Some(PinBinding::Bound { mux, .. }) | Some(PinBinding::StaleMux { mux }) => {
                pin_id_by_mux.insert(NodeId::MuxSession(mux.clone()), pin.id.clone());
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
        let cwd = mux.effective_cwd().map(|s| s.to_string());

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
            agent_labels: agent_labels(&visible_attached),
            single_session_preview,
            pin_id: pin_id_by_mux.get(&node_id).cloned(),
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
            Some(PinBinding::Bound { mux, .. }) | Some(PinBinding::StaleMux { mux }) => {
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
            agent_labels: vec![harness_label(&pin.harness)],
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
            Sort::Recency => right
                .parent_row
                .activity_epoch
                .cmp(&left.parent_row.activity_epoch)
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

/// H-UI-008: attached agents filter through the resolver. Only
/// `LinkedToMux` candidates whose `link_id` appears in
/// `snapshot.resolved_relationships.selected_link_id` for the
/// matching relation are surfaced — drops false-positive
/// attachments under non-winning cwd evidence.
fn collect_attached_agents(snapshot: &GraphSnapshot) -> HashMap<String, Vec<AttachedAgent>> {
    let candidate_counts = collect_agent_mux_candidate_counts(snapshot);
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
            .map(|s| s.to_string());
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

fn collect_agent_mux_candidate_counts(snapshot: &GraphSnapshot) -> HashMap<String, usize> {
    let mut per_agent: HashMap<String, HashSet<String>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::LinkedToMux) {
            continue;
        }
        let NodeId::AgentSession(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node { id: target_id } = &link.target else {
            continue;
        };
        per_agent
            .entry(link.source.to_string())
            .or_default()
            .insert(target_id.to_string());
    }
    per_agent.into_iter().map(|(k, v)| (k, v.len())).collect()
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
mod tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutNode, Confidence, Freshness, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RelationKind, RepoNode, SourceMetadata,
    };
    use crate::resolve::resolve_snapshot;

    fn mux_node(native: &str) -> GraphNode {
        mux_node_with_paths(native, Some(format!("/p/{native}")), None)
    }

    fn mux_node_with_activity(native: &str, activity_epoch: i64) -> GraphNode {
        let mut node = mux_node(native);
        if let GraphNode::MuxSession(mux) = &mut node {
            mux.activity_epoch = Some(activity_epoch);
        }
        node
    }

    fn mux_node_with_paths(
        native: &str,
        cwd: Option<String>,
        active_pane_current_path: Option<String>,
    ) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: Some(1_700_000_050),
            created_epoch: None,
        })
    }

    fn session_node(key: &str, cwd: &str) -> GraphNode {
        session_node_with_preview(key, cwd, None)
    }

    fn session_node_with_preview(
        key: &str,
        cwd: &str,
        last_message_preview: Option<String>,
    ) -> GraphNode {
        session_node_with_activity(key, cwd, last_message_preview, 1_700_000_100)
    }

    fn session_node_with_activity(
        key: &str,
        cwd: &str,
        last_message_preview: Option<String>,
        last_active_epoch: i64,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", key),
            harness_key: "codex".to_string(),
            cwd: Some(cwd.to_string()),
            title: None,
            last_message_preview,
            last_active_epoch: Some(last_active_epoch),
            session_kind: None,
        })
    }

    #[test]
    fn mux_view_emits_one_row_per_mux_with_agent_labels() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(session_node_with_preview(
            "abcdef123456",
            "/p/editor",
            Some("running cargo test".to_string()),
        ));
        let session = crate::model::NodeId::AgentSession(AgentSessionId::new(
            "codex",
            "/state",
            "abcdef123456",
        ));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: mux },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        assert_eq!(
            tree.rows.len(),
            1,
            "mux view should not emit agent children"
        );
        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert_eq!(row.attached_count, 1);
        assert_eq!(row.agent_labels, vec!["codex"]);
        assert_eq!(row.recency.as_deref(), Some("1m"));
        assert_eq!(
            row.single_session_preview.as_deref(),
            Some("running cargo test"),
            "single-session mux should carry the agent's last-message preview"
        );
    }

    #[test]
    fn mux_view_prefers_active_pane_cwd_for_display() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node_with_paths(
            "shell",
            Some("/started/here".to_string()),
            Some("/moved/there".to_string()),
        ));

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert_eq!(row.cwd_display.as_deref(), Some("/moved/there"));
    }

    #[test]
    fn mux_view_nests_session_rows_when_multiple_agents_link_to_one_mux() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot
            .nodes
            .push(session_node("abcdef123456", "/p/editor"));
        snapshot
            .nodes
            .push(session_node("123456abcdef", "/p/editor"));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        for key in ["abcdef123456", "123456abcdef"] {
            snapshot.candidate_links.push(GraphLink {
                id: format!("session-mux-{key}"),
                source: crate::model::NodeId::AgentSession(AgentSessionId::new(
                    "codex", "/state", key,
                )),
                target: LinkEndpoint::Node { id: mux.clone() },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        assert_eq!(tree.rows.len(), 3);
        assert!(tree.rows[0].expandable);
        let RowKind::MuxSession(mux_row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(
            mux_row.single_session_preview.is_none(),
            "multi-session mux should rely on child rows, not the inline preview"
        );
        assert_eq!(tree.rows[1].depth, 1);
        assert_eq!(tree.rows[2].depth, 1);
        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
        assert!(matches!(tree.rows[2].kind, RowKind::AgentSession(_)));
    }

    #[test]
    fn mux_view_drops_non_winner_linked_to_mux_candidate() {
        // H-UI-008: the mux view's attached-agents list filters
        // through `resolved_relationships`. A `LinkedToMux`
        // candidate that the resolver did not pick (e.g. a weaker
        // cwd evidence pointing at a different mux) must not
        // surface as an attached agent under either mux.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(mux_node("scratch"));
        snapshot.nodes.push(session_node("session-x", "/p/editor"));
        let session =
            crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "session-x"));
        let editor = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        let scratch = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
        // Editor link is StrongDiscovered (winner); scratch link
        // is Discovered (loses the LinkedToMux slot).
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux-editor".to_string(),
            source: session.clone(),
            target: LinkEndpoint::Node { id: editor },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux-scratch".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: scratch },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let mux_rows: Vec<_> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux),
                _ => None,
            })
            .collect();
        let editor_row = mux_rows
            .iter()
            .find(|mux| mux.native_id == "editor")
            .expect("editor row");
        let scratch_row = mux_rows
            .iter()
            .find(|mux| mux.native_id == "scratch")
            .expect("scratch row");
        assert_eq!(
            editor_row.attached_count, 1,
            "editor (resolver winner) keeps the attachment",
        );
        assert_eq!(
            scratch_row.attached_count, 0,
            "scratch (non-winner) must not surface a false-positive attachment",
        );
    }

    #[test]
    fn mux_view_counts_same_target_evidence_as_one_attachment() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(session_node("session-x", "/p/editor"));
        let session =
            crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "session-x"));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));

        for id in ["session-mux-log", "session-mux-process"] {
            snapshot.candidate_links.push(GraphLink {
                id: id.to_string(),
                source: session.clone(),
                target: LinkEndpoint::Node { id: mux.clone() },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::StrongDiscovered,
                confidence: Confidence::High,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert_eq!(row.attached_count, 1);
        assert_eq!(
            row.ambiguous_count, 0,
            "multiple evidence links to the same mux target are corroboration, not ambiguity",
        );
        assert_eq!(tree.rows.len(), 1);
    }

    #[test]
    fn mux_view_omits_single_session_preview_when_unattached() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("solo"));

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(row.single_session_preview.is_none());
        assert_eq!(row.attached_count, 0);
    }

    #[test]
    fn mux_view_omits_preview_when_attached_agent_has_none() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot
            .nodes
            .push(session_node("abcdef123456", "/p/editor"));
        let session = crate::model::NodeId::AgentSession(AgentSessionId::new(
            "codex",
            "/state",
            "abcdef123456",
        ));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: mux },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(row.single_session_preview.is_none());
        assert_eq!(row.attached_count, 1);
    }

    #[test]
    fn float_attached_muxes_top_lifts_attached_above_unattached() {
        // Three muxes: alpha and gamma are unattached, beta has one
        // agent session linked to it. Default row order is by
        // node_id (alpha, beta, gamma); with the bool set we expect
        // beta first and the alpha/gamma order preserved after it.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("alpha"));
        snapshot.nodes.push(mux_node("beta"));
        snapshot.nodes.push(mux_node("gamma"));
        snapshot.nodes.push(session_node("agent-1", "/p/beta"));
        let agent =
            crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "agent-1"));
        let beta = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:beta"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-beta".to_string(),
            source: agent,
            target: LinkEndpoint::Node { id: beta },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);

        let native_ids = |tree: &RowTree| -> Vec<String> {
            tree.rows
                .iter()
                .filter_map(|row| match &row.kind {
                    RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
                    _ => None,
                })
                .collect()
        };

        let baseline = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });
        assert_eq!(
            native_ids(&baseline),
            vec!["alpha", "beta", "gamma"],
            "baseline order is alphabetical by node_id"
        );

        let floated = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter {
                float_attached_muxes_top: true,
                ..RowFilter::default()
            },
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });
        assert_eq!(
            native_ids(&floated),
            vec!["beta", "alpha", "gamma"],
            "beta rises and alpha/gamma keep their relative order"
        );
    }

    #[test]
    fn recency_sort_orders_muxes_by_latest_attached_agent_activity() {
        let now = 1_700_000_000;
        let old = now - 3 * 86_400;
        let fresh = now - 60;

        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node_with_activity("alpha", old));
        snapshot.nodes.push(mux_node_with_activity("beta", fresh));
        snapshot.nodes.push(session_node_with_activity(
            "old-agent",
            "/p/alpha",
            None,
            old,
        ));
        snapshot.nodes.push(session_node_with_activity(
            "fresh-agent",
            "/p/beta",
            None,
            fresh,
        ));
        for (key, mux_native) in [("old-agent", "alpha"), ("fresh-agent", "beta")] {
            snapshot.candidate_links.push(GraphLink {
                id: format!("session-{mux_native}"),
                source: crate::model::NodeId::AgentSession(AgentSessionId::new(
                    "codex", "/state", key,
                )),
                target: LinkEndpoint::Node {
                    id: crate::model::NodeId::MuxSession(MuxSessionId::new(format!(
                        "tmux:{mux_native}"
                    ))),
                },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }

        let snapshot = resolve_snapshot(snapshot);

        let native_ids = |tree: &RowTree| -> Vec<String> {
            tree.rows
                .iter()
                .filter_map(|row| match &row.kind {
                    RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
                    _ => None,
                })
                .collect()
        };

        let hierarchy = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(now),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });
        assert_eq!(native_ids(&hierarchy), vec!["alpha", "beta"]);

        let recency = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(now),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Recency,
        });
        assert_eq!(native_ids(&recency), vec!["beta", "alpha"]);
    }

    #[test]
    fn repo_grouping_buckets_muxes_under_repo_headers() {
        let mut snapshot = GraphSnapshot::empty();
        // Two repos, three muxes total: two under /p/foo, one under
        // /p/bar. A fourth mux has no resolvable cwd and lands in
        // the Ungrouped synthetic bucket.
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
                "/p/foo/.git",
            ))));
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
                "/p/bar/.git",
            ))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/foo/.git"),
                "/p/foo".to_string(),
            ),
            root: "/p/foo".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/bar/.git"),
                "/p/bar".to_string(),
            ),
            root: "/p/bar".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-a",
            Some("/p/foo".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-b",
            Some("/p/foo/sub".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "bar-a",
            Some("/p/bar".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "orphan",
            Some("/elsewhere".to_string()),
            None,
        ));

        let snapshot = resolve_snapshot(snapshot);

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Repo,
            sort: Sort::Hierarchy,
        });

        let row_summary: Vec<(u8, String)> = tree
            .rows
            .iter()
            .map(|row| {
                let label = match &row.kind {
                    RowKind::MuxSession(mux) => mux.native_id.clone(),
                    RowKind::Group(g) => g.display_path.clone(),
                    other => format!("{other:?}"),
                };
                (row.depth, label)
            })
            .collect();

        // Bucket order is repo display path asc, then Ungrouped last.
        // Expected shape:
        //   depth 0: /p/bar
        //     depth 1: bar-a
        //   depth 0: /p/foo
        //     depth 1: foo-a
        //     depth 1: foo-b
        //   depth 0: Ungrouped
        //     depth 1: orphan
        assert_eq!(row_summary[0], (0, "/p/bar".to_string()));
        assert_eq!(row_summary[1], (1, "bar-a".to_string()));
        assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
        assert_eq!(row_summary[3], (1, "foo-a".to_string()));
        assert_eq!(row_summary[4], (1, "foo-b".to_string()));
        assert_eq!(row_summary[5], (0, "Ungrouped".to_string()));
        assert_eq!(row_summary[6], (1, "orphan".to_string()));
        assert_eq!(row_summary.len(), 7, "no extra rows: {row_summary:?}");
    }

    #[test]
    fn mux_view_paints_pin_id_on_bound_mux_rows() {
        // A pin bound to `tmux:editor` should leave its `pin_id` on
        // the mux row so the renderer can paint the bound-pin glyph
        // (regardless of grouping). Other muxes stay None. We set
        // `binding` directly rather than running the resolver
        // because the resolver test fixtures and production
        // discovery encode `MuxSessionNode.native_id` differently
        // (resolver-test fixtures prefix `tmux:`, production
        // discovery emits the bare name); the mux-view code path
        // we're testing only cares about the value already stored
        // on the pin.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(mux_node("scratch"));
        snapshot.pins.push(PinCandidate {
            id: "code".to_string(),
            display_name: "Code Review".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p/work".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "editor".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/work/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Bound {
                mux: MuxSessionId::new("tmux:editor"),
                session: AgentSessionId::new("claude-code", "/state", "session"),
            }),
        });

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let mut pin_ids: Vec<(String, Option<String>)> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some((mux.native_id.clone(), mux.pin_id.clone())),
                _ => None,
            })
            .collect();
        pin_ids.sort();
        assert_eq!(
            pin_ids,
            vec![
                ("editor".to_string(), Some("code".to_string())),
                ("scratch".to_string(), None),
            ],
        );
    }

    #[test]
    fn mux_view_flat_grouping_floats_pinned_mux_rows_to_top() {
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("newer"));
        snapshot.nodes.push(mux_node("older"));
        snapshot.pins.push(PinCandidate {
            id: "old-pin".to_string(),
            display_name: "Old Pin".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/older".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "older".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/older/.conspectus.toml".to_string(),
            binding: Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:older"),
            }),
        });

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let muxes: Vec<_> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux),
                _ => None,
            })
            .collect();
        assert_eq!(muxes.len(), 2, "{:#?}", tree.rows);
        assert_eq!(muxes[0].native_id, "older");
        assert_eq!(muxes[0].pin_id.as_deref(), Some("old-pin"));
        assert_eq!(muxes[1].native_id, "newer");
    }

    #[test]
    fn mux_view_emits_pins_group_at_top_under_repo_grouping() {
        // Repo grouping introduces header rows. The Pins group
        // should sit above every repo bucket so the operator sees
        // pinned work first.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: crate::model::RepoId::new("/p/foo/.git"),
            common_dir: "/p/foo/.git".to_string(),
            source_paths: vec!["/p/foo".to_string()],
            remotes: vec![],
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/foo/.git"),
                "/p/foo".to_string(),
            ),
            root: "/p/foo".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-a",
            Some("/p/foo".to_string()),
            None,
        ));
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest Pin".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/foo".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/foo/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Unbound),
        });

        let snapshot = resolve_snapshot(snapshot);

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Repo,
            sort: Sort::Hierarchy,
        });

        let row_summary: Vec<(u8, String)> = tree
            .rows
            .iter()
            .map(|row| {
                let label = match &row.kind {
                    RowKind::MuxSession(mux) => format!("mux:{}", mux.native_id),
                    RowKind::Group(g) => g.display_path.clone(),
                    RowKind::Pin(p) => format!("pin:{}", p.pin_id),
                    other => format!("{other:?}"),
                };
                (row.depth, label)
            })
            .collect();
        // Pins group first, then the repo bucket. The unbound pin
        // also appears in its natural repo location as a placeholder
        // mux row, mirroring bound pins appearing both in Pins and
        // in place.
        assert_eq!(row_summary[0], (0, "Pins".to_string()));
        assert_eq!(row_summary[1], (1, "mux:ingest".to_string()));
        assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
        assert_eq!(row_summary[3], (1, "mux:ingest".to_string()));
        assert_eq!(row_summary[4], (1, "mux:foo-a".to_string()));
    }

    #[test]
    fn mux_view_repo_grouping_renders_bound_pins_as_mux_rows() {
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: crate::model::RepoId::new("/p/foo/.git"),
            common_dir: "/p/foo/.git".to_string(),
            source_paths: vec!["/p/foo".to_string()],
            remotes: vec![],
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/foo/.git"),
                "/p/foo".to_string(),
            ),
            root: "/p/foo".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-a",
            Some("/p/foo".to_string()),
            None,
        ));
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest Pin".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/foo".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "foo-a".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/foo/.conspectus.toml".to_string(),
            binding: Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:foo-a"),
            }),
        });

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Repo,
            sort: Sort::Hierarchy,
        });

        let pins_group_idx = tree
            .rows
            .iter()
            .position(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"))
            .expect("Pins group present");
        let pins_children: Vec<_> = tree
            .rows
            .iter()
            .skip(pins_group_idx + 1)
            .take_while(|row| row.depth > 0)
            .collect();
        assert_eq!(pins_children.len(), 1, "{:#?}", tree.rows);
        match &pins_children[0].kind {
            RowKind::MuxSession(mux) => {
                assert_eq!(mux.native_id, "foo-a");
                assert_eq!(mux.pin_id.as_deref(), Some("ingest"));
            }
            other => panic!("expected pinned mux row, got {other:?}"),
        }
    }

    #[test]
    fn mux_view_flat_grouping_renders_unbound_pin_as_placeholder_mux_row() {
        // Flat groupings float pinned mux entities directly rather
        // than inserting a synthetic Pins group header. Unbound pins
        // still get a mux-shaped placeholder row so the operator can
        // launch them from the mux view before tmux exists.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(PinCandidate {
            id: "code".to_string(),
            display_name: "Pin".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "editor".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Unbound),
        });

        let snapshot = resolve_snapshot(snapshot);

        let tree = build_mux_tree(MuxBuildInputs {
            snapshot: &snapshot,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
            sort: Sort::Hierarchy,
        });

        let has_pins_group = tree
            .rows
            .iter()
            .any(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"));
        assert!(
            !has_pins_group,
            "session/host groupings should not emit a Pins group: {:#?}",
            tree.rows,
        );
        let mux = tree
            .rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::MuxSession(mux) if mux.pin_id.as_deref() == Some("code") => Some(mux),
                _ => None,
            })
            .expect("placeholder mux row");
        assert_eq!(mux.native_id, "editor");
        assert_eq!(mux.agent_labels, vec!["claude".to_string()]);
        assert_eq!(mux.single_session_preview.as_deref(), Some("/p"));
        assert!(matches!(&mux.primary_node, NodeId::Pin(pin) if pin.id == "code"));
    }
}
