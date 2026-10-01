//! In-memory mux projection renderer (ADR 0082).

use std::collections::{HashMap, HashSet};

use super::render::{
    self, MUX_COLUMNS, RenderOptions, current_epoch, format_relative_age, header_label,
    node_short_id_from_display, unique_prefix_len,
};
use super::table::agent_session_key_for_label;
use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode,
    NodeId, RelationKind, pick_preferred,
};

/// `(harness_key, state_scope, session_key)` triple identifying an
/// attached agent session.
type SessionKey = (String, String, String);

fn session_key_of(agent: &AgentSessionNode) -> SessionKey {
    (
        agent.id.harness_key.clone(),
        agent.id.state_scope.clone(),
        agent.id.session_key.clone(),
    )
}

#[derive(Debug, Clone)]
struct MuxRow<'a> {
    node_id_display: String,
    node: &'a MuxSessionNode,
}

#[derive(Debug, Clone)]
struct AttachedAgent {
    label: String,
    provenance: String,
    confidence: String,
    preview: Option<String>,
    session_key: SessionKey,
    last_active_epoch: Option<i64>,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_mux_rows_from_snapshot(
    snapshot: &GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let muxes = collect_mux_rows(snapshot);
    let attachments = collect_attachment_lookup(snapshot);
    let ambiguity = collect_per_agent_ambiguity(snapshot);

    let body_full_ids: Vec<String> = muxes
        .iter()
        .map(|m| node_short_id_from_display(&m.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(MUX_COLUMNS, key))
            .collect(),
    );

    let empty_attached: Vec<AttachedAgent> = Vec::new();
    for (row, full_short) in muxes.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let all_attached = attachments
            .get(&row.node_id_display)
            .unwrap_or(&empty_attached);
        let visible_attached: Vec<AttachedAgent> = all_attached
            .iter()
            .filter(|agent| agent_matches_filter(agent, &ambiguity, options, &options.filter))
            .cloned()
            .collect();
        if !mux_matches_filter(all_attached, &visible_attached, &options.filter) {
            continue;
        }
        let ctx = CellCtx {
            row,
            short_id,
            attached: if visible_attached.is_empty() {
                None
            } else {
                Some(visible_attached.as_slice())
            },
            ambiguity: &ambiguity,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    rows
}

fn agent_matches_filter(
    agent: &AttachedAgent,
    ambiguity: &HashMap<SessionKey, usize>,
    options: &RenderOptions,
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = ambiguity.get(&agent.session_key).copied().unwrap_or(0);
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.session_key.0,
        now_epoch: options.now_epoch,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
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

struct CellCtx<'a> {
    row: &'a MuxRow<'a>,
    short_id: &'a str,
    attached: Option<&'a [AttachedAgent]>,
    ambiguity: &'a HashMap<SessionKey, usize>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    let mux = ctx.row.node;
    match key {
        "id" => ctx.short_id.to_string(),
        "mux" => format!("{}:{}", mux.backend, mux.native_id),
        "cwd" => mux.cwd.clone().unwrap_or_else(dash),
        "agents" => match ctx.attached {
            Some(entries) if !entries.is_empty() => entries
                .iter()
                .map(|a| {
                    let ambiguous = ctx.ambiguity.get(&a.session_key).copied().unwrap_or(0) > 1;
                    format!(
                        "{} [{}]",
                        a.label,
                        render::indicator_from_tags(&a.provenance, &a.confidence, ambiguous)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            _ => dash(),
        },
        "attached-count" => match ctx.attached {
            Some(entries) if !entries.is_empty() => entries.len().to_string(),
            _ => dash(),
        },
        "activity" => mux
            .activity_epoch
            .map_or_else(dash, |epoch| format_relative_age(epoch, current_epoch())),
        "created" => mux
            .created_epoch
            .map_or_else(dash, |epoch| format_relative_age(epoch, current_epoch())),
        "preview" => ctx
            .attached
            .and_then(|entries| entries.iter().find_map(|a| a.preview.clone()))
            .unwrap_or_else(dash),
        _ => dash(),
    }
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_mux_rows(snapshot: &GraphSnapshot) -> Vec<MuxRow<'_>> {
    let mut rows: Vec<MuxRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(MuxRow {
                node_id_display: NodeId::MuxSession(mux.id.clone()).to_string(),
                node: mux,
            }),
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| a.node_id_display.cmp(&b.node_id_display));
    rows
}

/// Active `linked_to_mux` candidate links joined to their source
/// agent session, grouped by mux `node_id`. Per source session we
/// keep the `pick_preferred` winner.
fn collect_attachment_lookup(snapshot: &GraphSnapshot) -> HashMap<String, Vec<AttachedAgent>> {
    /// `(mux_id, source_session)`.
    type Key = (String, NodeId);

    let agent_lookup: HashMap<NodeId, &AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => Some((NodeId::AgentSession(agent.id.clone()), agent)),
            _ => None,
        })
        .collect();

    // Group all active linked_to_mux candidates by (mux_id, source_session).
    let mut per_source: HashMap<Key, Vec<&GraphLink>> = HashMap::new();
    // Preserve first-seen ordering so the rendered `agents` cell
    // emits attached agents in BTreeMap-by-source-NodeId order.
    let mut order: Vec<Key> = Vec::new();
    // Pre-sort the candidates by (source, link_id) so first-seen
    // ordering is stable across snapshots.
    let mut active_links: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| matches!(link.relation, RelationKind::LinkedToMux))
        .filter(|link| matches!(link.source, NodeId::AgentSession(_)))
        .filter(|link| {
            matches!(
                link.target,
                LinkEndpoint::Node {
                    id: NodeId::MuxSession(_)
                }
            )
        })
        .collect();
    active_links.sort_by(|a, b| {
        let NodeId::AgentSession(ai) = &a.source else {
            unreachable!()
        };
        let NodeId::AgentSession(bi) = &b.source else {
            unreachable!()
        };
        ai.harness_key
            .cmp(&bi.harness_key)
            .then_with(|| ai.state_scope.cmp(&bi.state_scope))
            .then_with(|| ai.session_key.cmp(&bi.session_key))
            .then_with(|| a.id.cmp(&b.id))
    });
    for link in active_links {
        // Only include attachments whose source agent session is
        // present in the snapshot (mirror the resolved graph join on
        // node_agent_sessions).
        if !agent_lookup.contains_key(&link.source) {
            continue;
        }
        let LinkEndpoint::Node {
            id: target_id @ NodeId::MuxSession(_),
        } = &link.target
        else {
            continue;
        };
        let key = (target_id.to_string(), link.source.clone());
        if !per_source.contains_key(&key) {
            order.push(key.clone());
        }
        per_source.entry(key).or_default().push(link);
    }

    let mut out: HashMap<String, Vec<AttachedAgent>> = HashMap::new();
    for key in order {
        let candidates = per_source.remove(&key).expect("filled in first pass");
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        let NodeId::AgentSession(agent_id) = &best.source else {
            continue;
        };
        let agent = agent_lookup
            .get(&best.source)
            .expect("agent present per JOIN check above");
        let label = format!(
            "{}:{}",
            agent_id.harness_key,
            agent_session_key_for_label(&agent_id.session_key)
        );
        out.entry(key.0).or_default().push(AttachedAgent {
            label,
            provenance: best.provenance.snake_case().to_string(),
            confidence: best.confidence.snake_case().to_string(),
            preview: agent.last_message_preview.clone(),
            session_key: session_key_of(agent),
            last_active_epoch: agent.last_active_epoch,
        });
    }
    out
}

/// Per-source counts of distinct active `linked_to_mux` mux
/// targets. Multiple evidence links to the same mux are
/// corroboration, not ambiguity.
fn collect_per_agent_ambiguity(snapshot: &GraphSnapshot) -> HashMap<SessionKey, usize> {
    let agent_lookup: HashMap<NodeId, &AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => Some((NodeId::AgentSession(agent.id.clone()), agent)),
            _ => None,
        })
        .collect();
    let mut per_agent: HashMap<NodeId, HashSet<String>> = HashMap::new();
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
            .entry(link.source.clone())
            .or_default()
            .insert(target_id.to_string());
    }
    let mut out = HashMap::new();
    for (source, targets) in per_agent {
        if let Some(agent) = agent_lookup.get(&source) {
            out.insert(session_key_of(agent), targets.len());
        }
    }
    out
}
