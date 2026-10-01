//! In-memory forks projection renderer (ADR 0082).
//!
//! Emits one row per `fork` node. Iterates `snapshot.nodes`
//! and `snapshot.candidate_links` directly — no SQLite
//! materialization. The cell-level output shape matches the
//! deleted pre-P11-011b SQL renderer byte-for-byte so the
//! existing `output::table` snapshot tests stay green.

use std::collections::HashMap;

use super::render::{
    FORKS_COLUMNS, RenderOptions, header_label, node_short_id_from_display, unique_prefix_len,
};
use super::table::short_session_id;
use crate::filter::{MuxStateKey, SessionMatchInputs};
use crate::model::{
    ForkNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, NodeId, RelationKind,
    pick_preferred,
};

#[derive(Debug, Clone)]
struct ForkRow<'a> {
    /// Display-form `NodeId` of the fork — keys the parent and
    /// children lookups and feeds the short id hash.
    node_id_display: String,
    node: &'a ForkNode,
}

#[derive(Debug, Clone)]
struct ParentInfo {
    label: String,
}

#[derive(Debug, Clone)]
struct ChildAgent {
    /// Display-form `NodeId` of the child agent.
    node_id: String,
    harness_key: String,
    last_active_epoch: Option<i64>,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_fork_rows_from_snapshot(
    snapshot: &GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let forks = collect_fork_rows(snapshot);
    let parents = collect_parent_session_per_fork(snapshot);
    let child_counts = collect_child_session_counts_per_fork(snapshot);

    let body_full_ids: Vec<String> = forks
        .iter()
        .map(|f| node_short_id_from_display(&f.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(FORKS_COLUMNS, key))
            .collect(),
    );

    let filter_active = options.filter.has_narrowing_predicates();
    // Resolved child-agent metadata is only needed when filter
    // is active — without a narrowing predicate the fork row
    // keeps the total `children` count (which includes
    // unresolved-target candidates) and never drops.
    let resolved_children = if filter_active {
        Some(collect_resolved_child_agents_per_fork(snapshot))
    } else {
        None
    };
    let candidate_counts = if filter_active {
        Some(
            crate::model::SnapshotIndex::new(snapshot)
                .agent_mux_candidate_counts()
                .clone(),
        )
    } else {
        None
    };

    for (row, full_short) in forks.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let child_count = if filter_active {
            let visible = visible_child_count(
                row,
                resolved_children
                    .as_ref()
                    .expect("populated when filter_active"),
                candidate_counts
                    .as_ref()
                    .expect("populated when filter_active"),
                options,
            );
            if visible == 0 {
                continue;
            }
            visible
        } else {
            child_counts.get(&row.node_id_display).copied().unwrap_or(0)
        };
        let ctx = CellCtx {
            row,
            short_id,
            parent: parents.get(&row.node_id_display),
            child_count,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    rows
}

fn visible_child_count(
    fork: &ForkRow<'_>,
    resolved: &HashMap<String, Vec<ChildAgent>>,
    candidate_counts: &HashMap<String, usize>,
    options: &RenderOptions,
) -> usize {
    let Some(children) = resolved.get(&fork.node_id_display) else {
        return 0;
    };
    children
        .iter()
        .filter(|child| {
            let candidate_count = candidate_counts.get(&child.node_id).copied().unwrap_or(0);
            options.filter.matches_session(&SessionMatchInputs {
                harness_key: &child.harness_key,
                now_epoch: options.now_epoch,
                last_active_epoch: child.last_active_epoch,
                mux_state: MuxStateKey::from_candidate_count(candidate_count),
            })
        })
        .count()
}

struct CellCtx<'a> {
    row: &'a ForkRow<'a>,
    short_id: &'a str,
    parent: Option<&'a ParentInfo>,
    child_count: usize,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match key {
        "id" => ctx.short_id.to_string(),
        "fork" => match &ctx.row.node.name {
            Some(name) => format!("{}:{}", ctx.row.node.provider, name),
            None => ctx.row.node.provider_source_key.clone(),
        },
        "provider" => ctx.row.node.provider.clone(),
        "scope" => ctx.row.node.scope.clone().unwrap_or_else(dash),
        "parent" => ctx.parent.map_or_else(dash, |p| p.label.clone()),
        "children" => {
            if ctx.child_count == 0 {
                dash()
            } else {
                ctx.child_count.to_string()
            }
        }
        "capabilities" => {
            if ctx.row.node.capabilities.is_empty() {
                dash()
            } else {
                ctx.row.node.capabilities.join(", ")
            }
        }
        _ => dash(),
    }
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_fork_rows(snapshot: &GraphSnapshot) -> Vec<ForkRow<'_>> {
    let mut rows: Vec<ForkRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Fork(fork) => Some(ForkRow {
                node_id_display: NodeId::Fork(fork.id.clone()).to_string(),
                node: fork,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| a.node_id_display.cmp(&b.node_id_display));
    rows
}

/// Per-fork preferred `parent_session` candidate's rendered label.
/// Resolved agent_session targets render as the short session_key;
/// unresolved targets render as `?<short native_id>`; anything
/// else yields no entry.
fn collect_parent_session_per_fork(snapshot: &GraphSnapshot) -> HashMap<String, ParentInfo> {
    let mut per_fork: HashMap<String, Vec<&GraphLink>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !is_active_fork_link(link, RelationKind::ParentSession) {
            continue;
        }
        per_fork
            .entry(link.source.to_string())
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (fork_id, candidates) in per_fork {
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        let label = match &best.target {
            LinkEndpoint::Node {
                id: NodeId::AgentSession(agent_id),
            } => Some(short_session_id(&agent_id.session_key)),
            LinkEndpoint::Unresolved { evidence } => {
                Some(evidence.native_id.as_deref().map_or_else(
                    || "?".to_string(),
                    |native| format!("?{}", short_session_id(native)),
                ))
            }
            LinkEndpoint::Node { .. } => None,
        };
        if let Some(label) = label {
            out.insert(fork_id, ParentInfo { label });
        }
    }
    out
}

/// Per-fork list of resolved child agent sessions — the subset of
/// `child_session` candidates whose target is a known
/// `agent_session` node. Used only when a `RowFilter` is active;
/// unresolved-target children are intentionally excluded (a
/// `RowFilter`'s predicates all need session-level metadata that
/// isn't available for those).
fn collect_resolved_child_agents_per_fork(
    snapshot: &GraphSnapshot,
) -> HashMap<String, Vec<ChildAgent>> {
    let agent_lookup: HashMap<String, &crate::model::AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => {
                Some((NodeId::AgentSession(agent.id.clone()).to_string(), agent))
            }
            _ => None,
        })
        .collect();
    let mut per_fork: HashMap<String, Vec<ChildAgent>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !is_active_fork_link(link, RelationKind::ChildSession) {
            continue;
        }
        let LinkEndpoint::Node {
            id: target_id @ NodeId::AgentSession(_),
        } = &link.target
        else {
            continue;
        };
        let target_display = target_id.to_string();
        let Some(agent) = agent_lookup.get(&target_display) else {
            continue;
        };
        per_fork
            .entry(link.source.to_string())
            .or_default()
            .push(ChildAgent {
                node_id: target_display,
                harness_key: agent.harness_key.clone(),
                last_active_epoch: agent.last_active_epoch,
            });
    }
    for children in per_fork.values_mut() {
        children.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    }
    per_fork
}

/// Per-fork count of active `child_session` candidates targeting
/// an `agent_session` node OR an unresolved endpoint. Mirrors
/// `fork_child_session_count`'s filter.
fn collect_child_session_counts_per_fork(snapshot: &GraphSnapshot) -> HashMap<String, usize> {
    let mut out: HashMap<String, usize> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !is_active_fork_link(link, RelationKind::ChildSession) {
            continue;
        }
        let counts = matches!(
            &link.target,
            LinkEndpoint::Node {
                id: NodeId::AgentSession(_)
            } | LinkEndpoint::Unresolved { .. }
        );
        if counts {
            *out.entry(link.source.to_string()).or_insert(0) += 1;
        }
    }
    out
}

fn is_active_fork_link(link: &GraphLink, relation: RelationKind) -> bool {
    matches!(link.state, LinkState::Active)
        && link.relation == relation
        && matches!(link.source, NodeId::Fork(_))
}
