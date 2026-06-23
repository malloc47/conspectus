//! In-memory forks-view row-tree builder (P11-011c / ADR 0082).
//!
//! Lists forks as parents and nests resolved child agent sessions
//! when they are present in the graph.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionId, AgentSessionNode, ForkId, ForkNode, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, NodeId, RelationKind,
};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, ForkRow, MuxIndicator, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
};

pub struct ForksBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
}

/// Connection-based inputs surviving until P11-011d retires
/// `App::database`. Thin wrapper that reads the snapshot back
/// out of SQLite then delegates to [`build_forks_tree`].
pub struct ForksBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

pub fn build_forks_tree_from_conn(
    inputs: ForksBuildInputsFromConn<'_>,
) -> rusqlite::Result<RowTree> {
    let snapshot = crate::query::read_snapshot(inputs.conn)?;
    let _ = (inputs.now, inputs.filter); // Sessions filter doesn't apply in the forks default render.
    Ok(build_forks_tree(ForksBuildInputs {
        snapshot: &snapshot,
        home: inputs.home,
    }))
}

#[derive(Clone, Debug)]
struct ForkData<'a> {
    node_id: String,
    id: ForkId,
    node: &'a ForkNode,
}

#[derive(Clone, Debug)]
struct AgentData<'a> {
    node_id: String,
    id: AgentSessionId,
    node: &'a AgentSessionNode,
    alias: Option<String>,
}

pub fn build_forks_tree(inputs: ForksBuildInputs<'_>) -> RowTree {
    let snapshot = inputs.snapshot;
    let filter = RowFilter::default();
    let now: Option<i64> = None;

    let mut forks = collect_forks(snapshot);
    forks.sort_by(|a, b| {
        a.node
            .provider
            .cmp(&b.node.provider)
            .then_with(|| a.node.provider_source_key.cmp(&b.node.provider_source_key))
    });

    let agents = collect_agents(snapshot);
    let agents_by_node: HashMap<&str, &AgentData<'_>> = agents
        .iter()
        .map(|agent| (agent.node_id.as_str(), agent))
        .collect();

    let candidate_counts = collect_agent_mux_candidate_counts(snapshot);
    let child_counts = collect_child_counts(snapshot);
    let child_links = collect_resolved_child_links(snapshot);
    let parent_labels = collect_parent_labels(snapshot);

    let mut node_ids: Vec<String> = forks.iter().map(|fork| fork.node_id.clone()).collect();
    node_ids.extend(agents.iter().map(|agent| agent.node_id.clone()));
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
        view: ViewLabel::Forks,
        ..RowTree::default()
    };

    for fork in &forks {
        let visible_children: Vec<&AgentData<'_>> = child_links
            .get(&fork.node_id)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|node_id| agents_by_node.get(node_id.as_str()).copied())
            .filter(|agent| session_matches_filter(agent, &candidate_counts, now, &filter))
            .collect();
        if filter.has_narrowing_predicates() && visible_children.is_empty() {
            continue;
        }

        let node_id = NodeId::Fork(fork.id.clone());
        tree.rows.push(Row {
            id: RowId::Fork(node_id.clone()),
            depth: 0,
            expandable: !visible_children.is_empty(),
            kind: RowKind::Fork(ForkRow {
                fork_label: fork
                    .node
                    .name
                    .as_ref()
                    .map(|name| format!("{}:{name}", fork.node.provider))
                    .unwrap_or_else(|| fork.node.provider_source_key.clone()),
                provider: fork.node.provider.clone(),
                scope: fork.node.scope.clone(),
                parent_label: parent_labels.get(&fork.node_id).cloned(),
                child_count: child_counts.get(&fork.node_id).copied().unwrap_or(0),
                primary_node: node_id,
            }),
        });

        for agent in visible_children {
            tree.rows.push(agent_row(
                agent,
                1,
                &candidate_counts,
                short_ids
                    .get(agent.node_id.as_str())
                    .cloned()
                    .unwrap_or_default(),
                inputs.home,
                now,
            ));
        }
    }

    tree
}

fn agent_row(
    agent: &AgentData<'_>,
    depth: u8,
    candidate_counts: &HashMap<String, usize>,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
            pin_id: None,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.node.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.node.last_active_epoch),
            activity_epoch: agent.node.last_active_epoch,
            mux_state: mux_indicator(candidate_count),
            preview: agent.node.last_message_preview.clone(),
            title: agent.node.title.clone(),
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

fn session_matches_filter(
    agent: &AgentData<'_>,
    candidate_counts: &HashMap<String, usize>,
    now: Option<i64>,
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.id.harness_key,
        now_epoch: now,
        last_active_epoch: agent.node.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_forks(snapshot: &GraphSnapshot) -> Vec<ForkData<'_>> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Fork(fork) => Some(ForkData {
                node_id: NodeId::Fork(fork.id.clone()).to_string(),
                id: fork.id.clone(),
                node: fork,
            }),
            _ => None,
        })
        .collect()
}

fn collect_agents(snapshot: &GraphSnapshot) -> Vec<AgentData<'_>> {
    let mut agents: Vec<AgentData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => {
                let id = agent.id.clone();
                let node_id_display = NodeId::AgentSession(id.clone()).to_string();
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(id.clone()))
                    .map(|s| s.to_string());
                Some(AgentData {
                    node_id: node_id_display,
                    id,
                    node: agent,
                    alias,
                })
            }
            _ => None,
        })
        .collect();
    agents.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    agents
}

fn collect_child_counts(snapshot: &GraphSnapshot) -> HashMap<String, usize> {
    let mut out: HashMap<String, usize> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::ChildSession) {
            continue;
        }
        let NodeId::Fork(_) = &link.source else {
            continue;
        };
        let counts = matches!(
            &link.target,
            LinkEndpoint::Node {
                id: NodeId::AgentSession(_),
            } | LinkEndpoint::Unresolved { .. }
        );
        if counts {
            *out.entry(link.source.to_string()).or_insert(0) += 1;
        }
    }
    out
}

/// H-UI-008: surface only resolver-blessed children. Each
/// fork's `child_session` candidate must have a corresponding
/// `ResolvedRelationship` with `selected_link_id`.
fn collect_resolved_child_links(snapshot: &GraphSnapshot) -> BTreeMap<String, Vec<String>> {
    let selected_link_ids: HashSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::ChildSession))
        .filter_map(|r| r.selected_link_id.as_deref())
        .collect();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::ChildSession) {
            continue;
        }
        let NodeId::Fork(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: target_id @ NodeId::AgentSession(_),
        } = &link.target
        else {
            continue;
        };
        if !selected_link_ids.contains(link.id.as_str()) {
            continue;
        }
        out.entry(link.source.to_string())
            .or_default()
            .push(target_id.to_string());
    }
    for ids in out.values_mut() {
        ids.sort();
    }
    out
}

/// H-UI-008: per-fork parent label. The atelier adapter emits
/// at most one ParentSession candidate per fork; we surface
/// resolved targets via `selected_link_id` lookup and
/// unresolved targets directly so `?{native_id}` lineage gaps
/// remain visible.
fn collect_parent_labels(snapshot: &GraphSnapshot) -> HashMap<String, String> {
    let selected_link_ids: HashSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::ParentSession))
        .filter_map(|r| r.selected_link_id.as_deref())
        .collect();
    let mut sorted_links: Vec<&crate::model::GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| matches!(link.relation, RelationKind::ParentSession))
        .filter(|link| matches!(link.source, NodeId::Fork(_)))
        .filter(|link| {
            // Either resolved or unresolved-target; if it's a
            // resolved-node target, it must be in the
            // resolver's selected set.
            matches!(&link.target, LinkEndpoint::Unresolved { .. })
                || selected_link_ids.contains(link.id.as_str())
        })
        .collect();
    sorted_links.sort_by(|a, b| {
        a.source
            .to_string()
            .cmp(&b.source.to_string())
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut out: HashMap<String, String> = HashMap::new();
    for link in sorted_links {
        let fork_node_id = link.source.to_string();
        if out.contains_key(&fork_node_id) {
            continue;
        }
        let label = match &link.target {
            LinkEndpoint::Node {
                id: NodeId::AgentSession(agent_id),
            } => Some(short_session_label(&agent_id.session_key)),
            LinkEndpoint::Unresolved { evidence } => evidence
                .native_id
                .as_deref()
                .map(short_session_label)
                .map(|label| format!("?{label}")),
            _ => None,
        };
        if let Some(label) = label {
            out.insert(fork_node_id, label);
        }
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

fn short_session_label(value: &str) -> String {
    value.chars().take(8).collect()
}
