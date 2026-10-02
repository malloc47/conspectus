//! In-memory union-view row-tree builder (ADR 0082).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::filter::RowFilter;
use crate::model::{
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
    PinBinding, RelationKind,
};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    MuxSessionRow, Row, RowId, RowKind, RowTree, ViewLabel, format_recency, shorten_home,
};

pub struct UnionBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
enum UnionData<'a> {
    Agent(super::AgentData<'a>),
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
    let candidate_counts = crate::model::SnapshotIndex::new(snapshot)
        .agent_mux_candidate_counts()
        .clone();
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
                if !super::session_matches_filter(agent, &candidate_counts, now, &inputs.filter) {
                    continue;
                }
                tree.rows.push(super::agent_row(
                    agent,
                    0, // Union rows always sit at depth 0.
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
                        created_epoch: mux.node.created_epoch,
                        last_attached_epoch: mux.node.last_attached_epoch,
                        agent_labels: Vec::new(),
                        program: super::mux_program(mux.node, None),
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
                    .map(std::string::ToString::to_string);
                rows.push(UnionData::Agent(super::AgentData {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode, PinBinding,
        PinCandidate, PinMuxRef, Provenance,
    };

    fn mux(name: &str) -> GraphNode {
        GraphNode::MuxSession(
            MuxSessionNode::new(
                MuxSessionId::new(format!("tmux:{name}")),
                "tmux".to_string(),
                name.to_string(),
            )
            .with_activity_epoch(1_700_000_050),
        )
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
