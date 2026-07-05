//! In-memory PRs projection renderer (P11-011b / ADR 0082).
//!
//! Emits one row per `forge_pr` node. Iterates
//! `snapshot.nodes` and `snapshot.candidate_links` directly.

use std::collections::HashMap;
use std::path::Path;

use super::render::{
    PRS_COLUMNS, RenderOptions, current_epoch, format_relative_age, header_label,
    node_short_id_from_display, strip_branch_prefix, unique_prefix_len,
};
use super::table::agent_session_key_for_label;
use crate::filter::{MuxStateKey, SessionMatchInputs};
use crate::model::{
    AgentSessionNode, BranchId, ForgePrNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, NodeId, RelationKind, path_is_ancestor_of, pick_preferred,
};

/// `(repo_common_dir, refname)` identifies a branch structurally.
type BranchKey = (String, String);

#[derive(Debug, Clone)]
struct PrRow<'a> {
    node_id_display: String,
    node: &'a ForgePrNode,
}

#[derive(Debug, Clone)]
struct AgentRow<'a> {
    node_id: String,
    node: &'a AgentSessionNode,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_pr_rows_from_snapshot(
    snapshot: &GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let prs = collect_pr_rows(snapshot);
    let preferred_branch = collect_preferred_branch_per_pr(snapshot);
    let checkout_roots_per_branch = collect_checkout_roots_per_branch(snapshot);
    let agents = collect_agents_with_cwd(snapshot);
    let candidate_counts = crate::tui::rows::collect_agent_mux_candidate_counts(snapshot);

    let body_full_ids: Vec<String> = prs
        .iter()
        .map(|p| node_short_id_from_display(&p.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(PRS_COLUMNS, key))
            .collect(),
    );

    let filter_active = options.filter.has_narrowing_predicates();
    for (row, full_short) in prs.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let branch = preferred_branch.get(&row.node_id_display);
        let attached = branch
            .and_then(|b| checkout_roots_per_branch.get(b))
            .map(|roots| attached_agents_for_roots(&agents, roots, &candidate_counts, options))
            .unwrap_or_default();
        if filter_active && attached.is_empty() {
            continue;
        }
        let ctx = CellCtx {
            row,
            short_id,
            branch,
            attached: &attached,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    rows
}

fn attached_agents_for_roots(
    agents: &[AgentRow<'_>],
    roots: &[String],
    candidate_counts: &HashMap<String, usize>,
    options: &RenderOptions,
) -> Vec<String> {
    let filter_active = options.filter.has_narrowing_predicates();
    let mut labels: Vec<String> = Vec::new();
    for agent in agents {
        let Some(cwd) = agent.node.cwd.as_deref() else {
            continue;
        };
        let cwd_path = Path::new(cwd);
        let under_root = roots
            .iter()
            .any(|root| path_is_ancestor_of(Path::new(root), cwd_path));
        if !under_root {
            continue;
        }
        if filter_active {
            let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
            let matches = options.filter.matches_session(&SessionMatchInputs {
                harness_key: &agent.node.harness_key,
                now_epoch: options.now_epoch,
                last_active_epoch: agent.node.last_active_epoch,
                mux_state: MuxStateKey::from_candidate_count(candidate_count),
            });
            if !matches {
                continue;
            }
        }
        labels.push(format!(
            "{}:{}",
            agent.node.harness_key,
            agent_session_key_for_label(&agent.node.id.session_key)
        ));
    }
    labels
}

struct CellCtx<'a> {
    row: &'a PrRow<'a>,
    short_id: &'a str,
    branch: Option<&'a BranchKey>,
    attached: &'a [String],
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    let pr = ctx.row.node;
    match key {
        "id" => ctx.short_id.to_string(),
        "pr" => {
            let state = pr.state.as_deref().unwrap_or("?");
            let draft = if pr.is_draft { " draft" } else { "" };
            format!("{}/{}#{} ({state}{draft})", pr.owner, pr.repo, pr.number)
        }
        "state" => pr.state.clone().unwrap_or_else(dash),
        "draft" => {
            if pr.is_draft {
                "draft".to_string()
            } else {
                dash()
            }
        }
        "branch" => ctx
            .branch
            .map(|(_repo, refname)| strip_branch_prefix(refname).to_string())
            .unwrap_or_else(dash),
        "repo" => format!("{}/{}", pr.owner, pr.repo),
        "updated" => pr
            .updated_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(dash),
        "attached" => {
            if ctx.attached.is_empty() {
                dash()
            } else {
                ctx.attached.join(", ")
            }
        }
        _ => dash(),
    }
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_pr_rows(snapshot: &GraphSnapshot) -> Vec<PrRow<'_>> {
    let mut rows: Vec<PrRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(PrRow {
                node_id_display: NodeId::ForgePr(pr.id.clone()).to_string(),
                node: pr,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| a.node_id_display.cmp(&b.node_id_display));
    rows
}

/// Per-PR preferred `branch_has_forge_pr` candidate's target
/// branch (structural key). Pick is by `pick_preferred`'s
/// ordering. Production discovery emits these with source=PR,
/// target=Branch (despite the relation name suggesting the
/// opposite); P10-004 already pinned that orientation.
fn collect_preferred_branch_per_pr(snapshot: &GraphSnapshot) -> HashMap<String, BranchKey> {
    let mut per_pr: HashMap<String, Vec<&GraphLink>> = HashMap::new();
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
            id: NodeId::Branch(_),
        } = &link.target
        else {
            continue;
        };
        per_pr
            .entry(link.source.to_string())
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (pr_id, candidates) in per_pr {
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        if let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &best.target
        {
            let BranchId { repo, refname } = branch_id;
            out.insert(pr_id, (repo.common_dir.clone(), refname.clone()));
        }
    }
    out
}

/// All active `checked_out_branch` candidates joined to their
/// source checkout's `root`, grouped by target branch.
/// Takes every matching candidate, not just the preferred one
/// per checkout.
fn collect_checkout_roots_per_branch(snapshot: &GraphSnapshot) -> HashMap<BranchKey, Vec<String>> {
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
    let mut out: HashMap<BranchKey, Vec<String>> = HashMap::new();
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
        out.entry((branch_id.repo.common_dir.clone(), branch_id.refname.clone()))
            .or_default()
            .push(root.clone());
    }
    out
}

fn collect_agents_with_cwd(snapshot: &GraphSnapshot) -> Vec<AgentRow<'_>> {
    let mut rows: Vec<AgentRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) if agent.cwd.is_some() => Some(AgentRow {
                node_id: NodeId::AgentSession(agent.id.clone()).to_string(),
                node: agent,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| {
        a.node
            .id
            .harness_key
            .cmp(&b.node.id.harness_key)
            .then_with(|| a.node.id.state_scope.cmp(&b.node.id.state_scope))
            .then_with(|| a.node.id.session_key.cmp(&b.node.id.session_key))
    });
    rows
}
