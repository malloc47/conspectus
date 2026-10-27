//! Shortest relationship paths between graph nodes, for opening a search
//! result by the way it relates to what the operator is looking at.
//!
//! The walk is breadth-first over the resolver's winning relationships,
//! treated as undirected: the same edges the explorer's validated zone
//! shows. Candidate and unresolved edges do not count, and neither do the
//! relations that involve runtime processes, because processes are
//! ephemeral and never a destination.
//!
//! When several paths have the same length the choice is deterministic:
//! a path whose first hop uses a structural relation wins over one that
//! does not, and then the path whose node ids sort first.

use std::collections::BTreeMap;

use crate::model::{GraphSnapshot, NodeId, RelationKind};

/// How far a search result may be from the selection and still be opened
/// along a path.
pub const MAX_HOPS: usize = 3;

/// Relations that carry no destination: runtime processes come and go.
fn is_excluded(relation: &RelationKind) -> bool {
    matches!(
        relation,
        RelationKind::MuxContainsProcess
            | RelationKind::ProcessIdentifiesSession
            | RelationKind::ProcessCandidatesSession
    )
}

/// Relations that say what something is part of or running in, which a
/// path should prefer over looser associations.
fn is_structural(relation: &RelationKind) -> bool {
    matches!(
        relation,
        RelationKind::BelongsToRepo
            | RelationKind::RootedIn
            | RelationKind::WorkspaceContainsRepo
            | RelationKind::CheckedOutBranch
            | RelationKind::BranchHasForgePr
            | RelationKind::LinkedToMux
            | RelationKind::PinTargets
            | RelationKind::ParentSession
            | RelationKind::ChildSession
    )
}

/// The shortest paths from one node to every node within a few hops.
#[derive(Clone, Debug, Default)]
pub struct Reachability {
    start: Option<NodeId>,
    /// For each reachable node, the nodes to pass through to get there:
    /// every hop after the start, ending with the node itself.
    paths: BTreeMap<NodeId, Vec<NodeId>>,
}

impl Reachability {
    /// Walk out from `start` for at most `max_hops` hops.
    pub fn from(snapshot: &GraphSnapshot, start: &NodeId, max_hops: usize) -> Self {
        // Neighbors, with whether any relation between the two is structural.
        let mut adjacency: BTreeMap<&NodeId, BTreeMap<&NodeId, bool>> = BTreeMap::new();
        for relationship in &snapshot.resolved_relationships {
            if relationship.selected_link_id.is_none() || is_excluded(&relationship.relation) {
                continue;
            }
            let structural = is_structural(&relationship.relation);
            for (from, to) in [
                (&relationship.source, &relationship.target),
                (&relationship.target, &relationship.source),
            ] {
                let slot = adjacency
                    .entry(from)
                    .or_default()
                    .entry(to)
                    .or_insert(false);
                *slot |= structural;
            }
        }

        // Per node: (first hop was not structural, path). The smaller key
        // wins, which is the tie-break order.
        let mut best: BTreeMap<NodeId, (bool, Vec<NodeId>)> = BTreeMap::new();
        let mut frontier: Vec<NodeId> = vec![start.clone()];
        for depth in 1..=max_hops {
            let mut layer: BTreeMap<NodeId, (bool, Vec<NodeId>)> = BTreeMap::new();
            for node in &frontier {
                let (prefix_flag, prefix) = best
                    .get(node)
                    .map_or((false, Vec::new()), |(flag, path)| (*flag, path.clone()));
                let Some(neighbors) = adjacency.get(node) else {
                    continue;
                };
                for (&neighbor, &structural) in neighbors {
                    if neighbor == start || best.contains_key(neighbor) {
                        continue;
                    }
                    let flag = if depth == 1 { !structural } else { prefix_flag };
                    let mut path = prefix.clone();
                    path.push(neighbor.clone());
                    let better = layer
                        .get(neighbor)
                        .is_none_or(|(known_flag, known)| (flag, &path) < (*known_flag, known));
                    if better {
                        layer.insert(neighbor.clone(), (flag, path));
                    }
                }
            }
            if layer.is_empty() {
                break;
            }
            frontier = layer.keys().cloned().collect();
            best.extend(layer);
        }

        Self {
            start: Some(start.clone()),
            paths: best
                .into_iter()
                .map(|(node, (_, path))| (node, path))
                .collect(),
        }
    }

    /// The node the walk started from.
    pub fn start(&self) -> Option<&NodeId> {
        self.start.as_ref()
    }

    /// The hops to take to reach `node`, the last being `node` itself.
    /// `None` when it is out of range, or is the start.
    pub fn path_to(&self, node: &NodeId) -> Option<&[NodeId]> {
        self.paths.get(node).map(Vec::as_slice)
    }
}

#[cfg(test)]
#[path = "relation_path_tests.rs"]
mod tests;
