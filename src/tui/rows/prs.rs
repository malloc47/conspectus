//! In-memory PRs-view row-tree builder (P11-011c / ADR 0082).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionId, AgentSessionNode, ForgePrId, ForgePrNode, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, NodeId, RelationKind, path_is_ancestor_of,
};
use crate::output::render::{node_short_id_from_display, strip_branch_prefix, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, MuxIndicator, PrRow, Row, RowId, RowKind, RowTree, ViewLabel, format_recency,
    harness_label, shorten_home,
};

pub struct PrsBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
}

/// Connection-based inputs surviving until P11-011d.
pub struct PrsBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

pub fn build_prs_tree_from_conn(inputs: PrsBuildInputsFromConn<'_>) -> rusqlite::Result<RowTree> {
    let snapshot = crate::query::read_snapshot(inputs.conn)?;
    let _ = (inputs.now, inputs.filter);
    Ok(build_prs_tree(PrsBuildInputs {
        snapshot: &snapshot,
        home: inputs.home,
    }))
}

#[derive(Clone, Debug)]
struct PrData<'a> {
    node_id: String,
    id: ForgePrId,
    node: &'a ForgePrNode,
}

#[derive(Clone, Debug)]
struct BranchLink {
    branch_node_id: String,
    refname: String,
}

#[derive(Clone, Debug)]
struct AgentData<'a> {
    node_id: String,
    id: AgentSessionId,
    node: &'a AgentSessionNode,
    alias: Option<String>,
}

pub fn build_prs_tree(inputs: PrsBuildInputs<'_>) -> RowTree {
    let snapshot = inputs.snapshot;
    let filter = RowFilter::default();
    let now: Option<i64> = None;

    let prs = collect_prs(snapshot);
    let agents = collect_agents(snapshot);
    let candidate_counts = collect_agent_mux_candidate_counts(snapshot);
    let branches = collect_preferred_branch_per_pr(snapshot);
    let checkout_roots = collect_checkout_roots_per_branch(snapshot);

    let mut node_ids: Vec<String> = prs.iter().map(|pr| pr.node_id.clone()).collect();
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
        view: ViewLabel::Prs,
        ..RowTree::default()
    };

    for pr in &prs {
        let branch = branches.get(&pr.node_id);
        let roots = branch
            .and_then(|branch| checkout_roots.get(&branch.branch_node_id))
            .cloned()
            .unwrap_or_default();
        let visible_agents: Vec<&AgentData<'_>> = agents
            .iter()
            .filter(|agent| agent_attached_to_roots(agent, &roots))
            .filter(|agent| session_matches_filter(agent, &candidate_counts, now, &filter))
            .collect();
        if filter.has_narrowing_predicates() && visible_agents.is_empty() {
            continue;
        }

        let node_id = NodeId::ForgePr(pr.id.clone());
        tree.rows.push(Row {
            id: RowId::Pr(node_id.clone()),
            depth: 0,
            expandable: !visible_agents.is_empty(),
            kind: RowKind::Pr(PrRow {
                pr_number: pr.node.number,
                repo_display: format!("{}/{}#{}", pr.node.owner, pr.node.repo, pr.node.number),
                state: pr.node.state.clone(),
                is_draft: pr.node.is_draft,
                branch_name: branch
                    .map(|branch| branch.refname.as_str())
                    .map(strip_branch_prefix)
                    .map(str::to_string),
                updated_recency: format_recency(now, pr.node.updated_epoch),
                attached_count: visible_agents.len(),
                url: pr.node.url.clone(),
                primary_node: node_id,
            }),
        });

        for agent in visible_agents {
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

fn agent_attached_to_roots(agent: &AgentData<'_>, roots: &[String]) -> bool {
    let Some(cwd) = agent.node.cwd.as_deref() else {
        return false;
    };
    let cwd = Path::new(cwd);
    roots
        .iter()
        .any(|root| path_is_ancestor_of(Path::new(root), cwd))
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

fn collect_prs(snapshot: &GraphSnapshot) -> Vec<PrData<'_>> {
    let mut prs: Vec<PrData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(PrData {
                node_id: NodeId::ForgePr(pr.id.clone()).to_string(),
                id: pr.id.clone(),
                node: pr,
            }),
            _ => None,
        })
        .collect();
    prs.sort_by(|a, b| {
        a.node
            .owner
            .cmp(&b.node.owner)
            .then_with(|| a.node.repo.cmp(&b.node.repo))
            .then_with(|| b.node.number.cmp(&a.node.number))
    });
    prs
}

fn collect_agents(snapshot: &GraphSnapshot) -> Vec<AgentData<'_>> {
    let mut agents: Vec<AgentData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) if agent.cwd.is_some() => {
                let id = agent.id.clone();
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(id.clone()))
                    .map(|s| s.to_string());
                Some(AgentData {
                    node_id: NodeId::AgentSession(id.clone()).to_string(),
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

/// H-UI-008: the preferred branch per PR comes from the
/// resolver's `branch_has_forge_pr` winner. Production
/// discovery emits these with source=ForgePr, target=Branch.
fn collect_preferred_branch_per_pr(snapshot: &GraphSnapshot) -> HashMap<String, BranchLink> {
    let selected_link_ids: HashSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::BranchHasForgePr))
        .filter_map(|r| r.selected_link_id.as_deref())
        .collect();
    let mut out = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::BranchHasForgePr) {
            continue;
        }
        let NodeId::ForgePr(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &link.target
        else {
            continue;
        };
        if !selected_link_ids.contains(link.id.as_str()) {
            continue;
        }
        let branch_node_id = NodeId::Branch(branch_id.clone()).to_string();
        out.entry(link.source.to_string()).or_insert(BranchLink {
            branch_node_id,
            refname: branch_id.refname.clone(),
        });
    }
    out
}

fn collect_checkout_roots_per_branch(snapshot: &GraphSnapshot) -> HashMap<String, Vec<String>> {
    let checkout_roots: HashMap<NodeId, String> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Checkout(checkout) => {
                Some((NodeId::Checkout(checkout.id.clone()), checkout.root.clone()))
            }
            _ => None,
        })
        .collect();
    let mut sorted: Vec<(String, String)> = Vec::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::CheckedOutBranch) {
            continue;
        }
        let NodeId::Checkout(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &link.target
        else {
            continue;
        };
        let Some(root) = checkout_roots.get(&link.source) else {
            continue;
        };
        let branch_node_id = NodeId::Branch(branch_id.clone()).to_string();
        sorted.push((branch_node_id, root.clone()));
    }
    sorted.sort();
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (branch_node_id, root) in sorted {
        out.entry(branch_node_id).or_default().push(root);
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
