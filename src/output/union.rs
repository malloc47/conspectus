//! In-memory union projection renderer (P11-011b / ADR 0082).
//!
//! Emits one row per `agent_session` followed by one row per
//! `mux_session`, with per-row-kind cell rendering driven by a
//! shared column set.

use std::collections::{BTreeSet, HashMap};

use super::render::{
    self, RenderOptions, UNION_COLUMNS, header_label, node_short_id_from_display, unique_prefix_len,
};
use super::table::agent_session_key_for_label;
use crate::filter::{MuxStateKey, SessionMatchInputs};
use crate::model::{
    AgentSessionNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode,
    NodeId, RelationKind, pick_preferred,
};

type SessionKey = (String, String, String);

fn session_key_of(agent: &AgentSessionNode) -> SessionKey {
    (
        agent.id.harness_key.clone(),
        agent.id.state_scope.clone(),
        agent.id.session_key.clone(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnionKind {
    Agent,
    Mux,
}

struct UnionRow<'a> {
    node_id_display: String,
    kind: UnionKind,
    agent: Option<&'a AgentSessionNode>,
    mux: Option<&'a MuxSessionNode>,
    alias_display_name: Option<String>,
}

#[derive(Debug, Clone)]
struct PreferredMux {
    backend: String,
    native_id: String,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_union_rows_from_snapshot(
    snapshot: &GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let rows_data = collect_union_rows(snapshot);
    let preferred_mux = collect_preferred_mux_per_agent(snapshot);

    let body_full_ids: Vec<String> = rows_data
        .iter()
        .map(|r| node_short_id_from_display(&r.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(UNION_COLUMNS, key))
            .collect(),
    );

    let filter_active = options.filter.has_narrowing_predicates();
    for (row, full_short) in rows_data.iter().zip(body_full_ids.iter()) {
        if filter_active {
            match row.kind {
                UnionKind::Agent => {
                    if !agent_matches_filter(row, &preferred_mux, options) {
                        continue;
                    }
                }
                UnionKind::Mux => continue,
            }
        }
        let short_id = &full_short[..id_len];
        let preferred = row
            .agent
            .map(session_key_of)
            .and_then(|k| preferred_mux.get(&k));
        let ctx = CellCtx {
            row,
            short_id,
            preferred_mux: preferred,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    rows
}

fn agent_matches_filter(
    row: &UnionRow<'_>,
    preferred_mux: &HashMap<SessionKey, PreferredMux>,
    options: &RenderOptions,
) -> bool {
    let Some(agent) = row.agent else {
        return false;
    };
    let candidate_count = preferred_mux
        .get(&session_key_of(agent))
        .map_or(0, |m| m.candidate_count);
    options.filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.harness_key,
        now_epoch: options.now_epoch,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
}

struct CellCtx<'a> {
    row: &'a UnionRow<'a>,
    short_id: &'a str,
    preferred_mux: Option<&'a PreferredMux>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match (key, ctx.row.kind) {
        ("id", _) => ctx.short_id.to_string(),
        ("kind", UnionKind::Agent) => "agent".to_string(),
        ("kind", UnionKind::Mux) => "mux".to_string(),
        ("label", UnionKind::Agent) => match ctx.row.agent {
            Some(agent) => format!(
                "{}:{}",
                agent.harness_key,
                agent_session_key_for_label(&agent.id.session_key)
            ),
            None => dash(),
        },
        ("label", UnionKind::Mux) => match ctx.row.mux {
            Some(mux) => format!("{}:{}", mux.backend, mux.native_id),
            None => dash(),
        },
        ("cwd", UnionKind::Agent) => ctx
            .row
            .agent
            .and_then(|a| a.cwd.clone())
            .unwrap_or_else(dash),
        ("cwd", UnionKind::Mux) => ctx.row.mux.and_then(|m| m.cwd.clone()).unwrap_or_else(dash),
        ("relationship", UnionKind::Agent) => match ctx.preferred_mux {
            Some(m) => format!(
                "mux={}:{} [{}]",
                m.backend,
                m.native_id,
                render::indicator_from_tags(&m.provenance, &m.confidence, m.candidate_count > 1)
            ),
            None => "mux=—".to_string(),
        },
        ("preview", UnionKind::Agent) => ctx
            .row
            .agent
            .and_then(|a| a.last_message_preview.clone())
            .unwrap_or_else(dash),
        ("title", UnionKind::Agent) => ctx
            .row
            .alias_display_name
            .clone()
            .or_else(|| ctx.row.agent.and_then(|a| a.title.clone()))
            .unwrap_or_else(dash),
        _ => dash(),
    }
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_union_rows(snapshot: &GraphSnapshot) -> Vec<UnionRow<'_>> {
    let mut agents: Vec<UnionRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => {
                let display = NodeId::AgentSession(agent.id.clone()).to_string();
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(agent.id.clone()))
                    .map(|s| s.to_string());
                Some(UnionRow {
                    node_id_display: display,
                    kind: UnionKind::Agent,
                    agent: Some(agent),
                    mux: None,
                    alias_display_name: alias,
                })
            }
            _ => None,
        })
        .collect();
    agents.sort_by(|a, b| a.node_id_display.cmp(&b.node_id_display));

    let mut muxes: Vec<UnionRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(UnionRow {
                node_id_display: NodeId::MuxSession(mux.id.clone()).to_string(),
                kind: UnionKind::Mux,
                agent: None,
                mux: Some(mux),
                alias_display_name: None,
            }),
            _ => None,
        })
        .collect();
    muxes.sort_by(|a, b| a.node_id_display.cmp(&b.node_id_display));

    agents.extend(muxes);
    agents
}

/// Per-agent preferred `linked_to_mux` candidate joined to its
/// mux's structural columns. `candidate_count` is the number of
/// distinct active mux targets the source agent claims.
fn collect_preferred_mux_per_agent(snapshot: &GraphSnapshot) -> HashMap<SessionKey, PreferredMux> {
    let agent_lookup: HashMap<NodeId, &AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => Some((NodeId::AgentSession(agent.id.clone()), agent)),
            _ => None,
        })
        .collect();
    let mux_lookup: HashMap<NodeId, &MuxSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some((NodeId::MuxSession(mux.id.clone()), mux)),
            _ => None,
        })
        .collect();
    let mut per_source: HashMap<NodeId, Vec<&GraphLink>> = HashMap::new();
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
        let LinkEndpoint::Node {
            id: NodeId::MuxSession(_),
        } = &link.target
        else {
            continue;
        };
        per_source
            .entry(link.source.clone())
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (source, candidates) in per_source {
        let Some(agent) = agent_lookup.get(&source) else {
            continue;
        };
        let candidate_count: usize = candidates
            .iter()
            .filter_map(|link| match &link.target {
                LinkEndpoint::Node { id } => Some(id.to_string()),
                LinkEndpoint::Unresolved { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        let LinkEndpoint::Node {
            id: target_id @ NodeId::MuxSession(_),
        } = &best.target
        else {
            continue;
        };
        let Some(mux) = mux_lookup.get(target_id) else {
            continue;
        };
        out.insert(
            session_key_of(agent),
            PreferredMux {
                backend: mux.backend.clone(),
                native_id: mux.native_id.clone(),
                provenance: best.provenance.snake_case().to_string(),
                confidence: best.confidence.snake_case().to_string(),
                candidate_count,
            },
        );
    }
    out
}
