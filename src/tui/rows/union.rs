//! In-memory union-view row-tree builder (P11-011c / ADR 0082).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot, LinkEndpoint, LinkState,
    MuxSessionId, MuxSessionNode, NodeId, PinBinding, RelationKind,
};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, MuxIndicator, MuxSessionRow, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
};

pub struct UnionBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
enum UnionData<'a> {
    Agent(AgentData<'a>),
    Mux(MuxData<'a>),
}

impl UnionData<'_> {
    fn node_id(&self) -> &str {
        match self {
            Self::Agent(row) => &row.node_id,
            Self::Mux(row) => &row.node_id,
        }
    }
}

#[derive(Clone, Debug)]
struct AgentData<'a> {
    node_id: String,
    id: AgentSessionId,
    node: &'a AgentSessionNode,
    alias: Option<String>,
}

#[derive(Clone, Debug)]
struct MuxData<'a> {
    node_id: String,
    id: MuxSessionId,
    node: &'a MuxSessionNode,
    attached_count: usize,
}

impl MuxData<'_> {
    fn effective_cwd(&self) -> Option<&str> {
        self.node
            .active_pane_current_path
            .as_deref()
            .or(self.node.cwd.as_deref())
    }
}

pub fn build_union_tree(inputs: UnionBuildInputs<'_>) -> RowTree {
    let snapshot = inputs.snapshot;
    let now: Option<i64> = None;

    let rows = collect_union_rows(snapshot);
    let candidate_counts = collect_agent_mux_candidate_counts(snapshot);
    let pin_id_by_bound_mux = collect_pin_id_by_bound_mux(snapshot);

    let full_ids: Vec<String> = rows
        .iter()
        .map(|row| node_short_id_from_display(row.node_id()))
        .collect();
    let id_len = unique_prefix_len(&full_ids);
    let short_ids: HashMap<&str, String> = rows
        .iter()
        .zip(full_ids.iter())
        .map(|(row, full)| (row.node_id(), full[..id_len].to_string()))
        .collect();

    let mut tree = RowTree {
        view: ViewLabel::Union,
        ..RowTree::default()
    };

    for row in &rows {
        match row {
            UnionData::Agent(agent) => {
                if !session_matches_filter(agent, &candidate_counts, now, &inputs.filter) {
                    continue;
                }
                tree.rows.push(agent_row(
                    agent,
                    &candidate_counts,
                    short_ids
                        .get(agent.node_id.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    inputs.home,
                    now,
                ));
            }
            UnionData::Mux(mux) => {
                if inputs.filter.has_narrowing_predicates() {
                    continue;
                }
                let node_id = NodeId::MuxSession(mux.id.clone());
                tree.rows.push(Row {
                    id: RowId::MuxSession(node_id.clone()),
                    depth: 0,
                    expandable: false,
                    kind: RowKind::MuxSession(MuxSessionRow {
                        mux: mux.id.clone(),
                        backend: mux.node.backend.clone(),
                        native_id: mux.node.native_id.clone(),
                        client_attached: mux.node.client_attached,
                        cwd_display: mux
                            .effective_cwd()
                            .map(|cwd| shorten_home(cwd, inputs.home)),
                        attached_count: mux.attached_count,
                        ambiguous_count: 0,
                        recency: format_recency(now, mux.node.activity_epoch),
                        activity_epoch: mux.node.activity_epoch,
                        agent_labels: Vec::new(),
                        single_session_preview: None,
                        pin_id: pin_id_by_bound_mux.get(&mux.node.native_id).cloned(),
                        primary_node: node_id,
                    }),
                });
            }
        }
    }

    tree
}

fn agent_row(
    agent: &AgentData<'_>,
    candidate_counts: &HashMap<String, usize>,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth: 0,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
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
            pin_id: None,
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

/// Sort key keeps rows stable by recency, kind, then node id.
fn collect_union_rows(snapshot: &GraphSnapshot) -> Vec<UnionData<'_>> {
    let attached_counts = collect_mux_attached_counts(snapshot);
    let mut rows: Vec<UnionData<'_>> = Vec::new();
    for node in &snapshot.nodes {
        match node {
            GraphNode::AgentSession(agent) => {
                let id = agent.id.clone();
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(id.clone()))
                    .map(|s| s.to_string());
                rows.push(UnionData::Agent(AgentData {
                    node_id: NodeId::AgentSession(id.clone()).to_string(),
                    id,
                    node: agent,
                    alias,
                }));
            }
            GraphNode::MuxSession(mux) => {
                let display = NodeId::MuxSession(mux.id.clone()).to_string();
                let attached_count = attached_counts.get(&display).copied().unwrap_or(0);
                rows.push(UnionData::Mux(MuxData {
                    node_id: display,
                    id: mux.id.clone(),
                    node: mux,
                    attached_count,
                }));
            }
            _ => {}
        }
    }
    rows.sort_by(|a, b| {
        let a_epoch = match a {
            UnionData::Agent(agent) => agent.node.last_active_epoch.unwrap_or(0),
            UnionData::Mux(_) => 0,
        };
        let b_epoch = match b {
            UnionData::Agent(agent) => agent.node.last_active_epoch.unwrap_or(0),
            UnionData::Mux(_) => 0,
        };
        let a_kind = match a {
            UnionData::Agent(_) => 0,
            UnionData::Mux(_) => 1,
        };
        let b_kind = match b {
            UnionData::Agent(_) => 0,
            UnionData::Mux(_) => 1,
        };
        b_epoch
            .cmp(&a_epoch)
            .then_with(|| a_kind.cmp(&b_kind))
            .then_with(|| a.node_id().cmp(b.node_id()))
    });
    rows
}

fn collect_mux_attached_counts(snapshot: &GraphSnapshot) -> HashMap<String, usize> {
    let mut per_mux: HashMap<String, HashSet<String>> = HashMap::new();
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
            id: target_id @ NodeId::MuxSession(_),
        } = &link.target
        else {
            continue;
        };
        per_mux
            .entry(target_id.to_string())
            .or_default()
            .insert(link.source.to_string());
    }
    per_mux.into_iter().map(|(k, v)| (k, v.len())).collect()
}

fn collect_pin_id_by_bound_mux(snapshot: &GraphSnapshot) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    for pin in &snapshot.pins {
        let bound = matches!(pin.binding, Some(PinBinding::Bound { .. }));
        if !bound {
            continue;
        }
        let native_id = pin.mux.native_id();
        if let Some(bare) = strip_backend_prefix(&native_id) {
            out.insert(bare.to_string(), pin.id.clone());
        }
    }
    out
}

fn strip_backend_prefix(pin_mux_native_id: &str) -> Option<&str> {
    pin_mux_native_id.strip_prefix("tmux:")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode, PinBinding,
        PinCandidate, PinMuxRef, Provenance,
    };

    fn mux(name: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{name}")),
            backend: "tmux".to_string(),
            native_id: name.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: Some(1_700_000_050),
            created_epoch: None,
        })
    }

    #[test]
    fn union_view_paints_pin_id_on_bound_mux_rows() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux("editor"));
        snapshot.nodes.push(mux("scratch"));
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

        let tree = build_union_tree(UnionBuildInputs {
            snapshot: &snapshot,
            home: None,
            filter: RowFilter::default(),
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
    fn strip_backend_prefix_normalizes_pin_mux_native_id() {
        assert_eq!(super::strip_backend_prefix("tmux:editor"), Some("editor"));
        assert_eq!(
            super::strip_backend_prefix("tmux:scratch:editor"),
            Some("scratch:editor"),
        );
        assert_eq!(super::strip_backend_prefix("zellij:foo"), None);
        assert_eq!(super::strip_backend_prefix("editor"), None);
    }
}
