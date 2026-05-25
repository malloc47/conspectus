//! Read-only typed index over a [`GraphSnapshot`].
//!
//! Per ADR 0035, the views that slice the snapshot (TUI sessions tree, CLI
//! `table` projections, `node show`, the detail pane, and future ADR 0031
//! per-view filters) all funnel their joins through this layer instead of
//! re-walking `snapshot.nodes` and `snapshot.candidate_links` themselves.
//!
//! The index is one allocation per snapshot build: typed maps from
//! [`NodeId`] to each node kind, plus a `(source, relation)` map over active
//! candidate links. Selectors expose the joins that more than one view was
//! already coding by hand.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{
    AgentSessionNode, BranchNode, CheckoutId, CheckoutNode, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId, RelationKind,
    RepoId, RepoNode, WorkspaceId, WorkspaceNode,
};

/// Read-only typed index over a snapshot. See module docs.
pub struct SnapshotIndex<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    pub mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    pub repos: BTreeMap<NodeId, &'a RepoNode>,
    pub checkouts: BTreeMap<NodeId, &'a CheckoutNode>,
    pub workspaces: BTreeMap<NodeId, &'a WorkspaceNode>,
    pub branches: BTreeMap<NodeId, &'a BranchNode>,
    pub forks: BTreeMap<NodeId, &'a ForkNode>,
    pub forge_prs: BTreeMap<NodeId, &'a ForgePrNode>,
    /// `(source, relation)` → all *active* candidate links for that pair,
    /// in the order they appear in `snapshot.candidate_links`.
    /// Non-active (ignored / overridden) links are filtered out; consumers
    /// that need them walk `snapshot.candidate_links` directly.
    pub by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
}

impl<'a> SnapshotIndex<'a> {
    pub fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut repos = BTreeMap::new();
        let mut checkouts = BTreeMap::new();
        let mut workspaces = BTreeMap::new();
        let mut branches = BTreeMap::new();
        let mut forks = BTreeMap::new();
        let mut forge_prs = BTreeMap::new();

        for node in &snapshot.nodes {
            let id = node.id();
            match node {
                GraphNode::AgentSession(n) => {
                    agent_sessions.insert(id, n);
                }
                GraphNode::MuxSession(n) => {
                    mux_sessions.insert(id, n);
                }
                GraphNode::Repo(n) => {
                    repos.insert(id, n);
                }
                GraphNode::Checkout(n) => {
                    checkouts.insert(id, n);
                }
                GraphNode::Workspace(n) => {
                    workspaces.insert(id, n);
                }
                GraphNode::Branch(n) => {
                    branches.insert(id, n);
                }
                GraphNode::Fork(n) => {
                    forks.insert(id, n);
                }
                GraphNode::ForgePr(n) => {
                    forge_prs.insert(id, n);
                }
            }
        }

        let mut by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>> =
            BTreeMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, LinkState::Active) {
                continue;
            }
            by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
        }

        Self {
            snapshot,
            agent_sessions,
            mux_sessions,
            repos,
            checkouts,
            workspaces,
            branches,
            forks,
            forge_prs,
            by_source_relation,
        }
    }

    /// First-place candidate for `(source, relation)`, ranked by
    /// [`pick_preferred`].
    pub fn preferred_link(&self, source: &NodeId, relation: RelationKind) -> Option<&'a GraphLink> {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .and_then(|links| pick_preferred(links))
    }

    /// Active candidate links for `(source, relation)`, in stable order.
    pub fn candidates_for(&self, source: &NodeId, relation: RelationKind) -> &[&'a GraphLink] {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Active `LinkedToMux` candidates sourced at this agent session,
    /// de-duplicated by target mux. Multiple evidence links pointing at the
    /// same mux collapse to one (the preferred among them) so a row backed
    /// by redundant evidence does not look ambiguous.
    pub fn mux_candidates_for_session(&self, session: &NodeId) -> Vec<&'a GraphLink> {
        let Some(links) = self
            .by_source_relation
            .get(&(session.clone(), RelationKind::LinkedToMux))
        else {
            return Vec::new();
        };

        let mut by_target: BTreeMap<NodeId, Vec<&GraphLink>> = BTreeMap::new();
        for link in links {
            let Some(target) = link.target_node_id() else {
                continue;
            };
            by_target.entry(target.clone()).or_default().push(*link);
        }

        by_target
            .into_values()
            .filter_map(|links| pick_preferred(&links))
            .collect()
    }

    /// Deepest [`CheckoutNode`] whose root is an ancestor of `path` (in
    /// path-component terms — so `/a/b` is not an ancestor of `/a/barbecue`).
    pub fn checkout_for_path(&self, path: &str) -> Option<(&CheckoutId, &'a CheckoutNode)> {
        let path = Path::new(path);
        self.checkouts
            .iter()
            .filter_map(|(id, node)| match id {
                NodeId::Checkout(wt_id) if path_is_ancestor_of(Path::new(&wt_id.root), path) => {
                    Some((wt_id, *node))
                }
                _ => None,
            })
            .max_by_key(|(wt_id, _)| Path::new(&wt_id.root).components().count())
    }

    /// Number of checkouts whose [`CheckoutId::repo`] is `repo`.
    pub fn checkout_count_for_repo(&self, repo: &RepoId) -> usize {
        self.checkouts
            .keys()
            .filter(|id| match id {
                NodeId::Checkout(wt_id) => &wt_id.repo == repo,
                _ => false,
            })
            .count()
    }

    /// Workspace containing `repo` via any active `WorkspaceContainsRepo`
    /// link. Returns the first match in `by_source_relation` order.
    pub fn workspace_for_repo(&self, repo: &NodeId) -> Option<&WorkspaceId> {
        for ((source, relation), links) in &self.by_source_relation {
            if *relation != RelationKind::WorkspaceContainsRepo {
                continue;
            }
            for link in links {
                if let LinkEndpoint::Node { id } = &link.target
                    && id == repo
                    && let NodeId::Workspace(ws_id) = source
                {
                    return Some(ws_id);
                }
            }
        }
        None
    }

    /// Workspace directly associated with a session through a resolved
    /// `AssociatedWith` relationship. Reads the resolved layer rather than
    /// candidate links so logical/canonical workspace membership wins over
    /// per-repo evidence.
    pub fn workspace_for_session(&self, session: &NodeId) -> Option<WorkspaceId> {
        self.snapshot.resolved_relationships.iter().find_map(|rel| {
            if rel.source != *session || rel.relation != RelationKind::AssociatedWith {
                return None;
            }
            match &rel.target {
                NodeId::Workspace(ws) => Some(ws.clone()),
                _ => None,
            }
        })
    }

    /// Display path for a repo, per ADR 0034: for a non-bare repo the
    /// canonical checkout root (the parent of `common_dir`), for a bare
    /// repo the first observed source path, falling back to the raw
    /// common dir.
    pub fn repo_display_path(&self, repo_id: &RepoId) -> String {
        let common_dir = &repo_id.common_dir;
        if let Some(canonical) = common_dir.strip_suffix("/.git") {
            return canonical.to_string();
        }
        let node_id = NodeId::Repo(repo_id.clone());
        self.repos
            .get(&node_id)
            .and_then(|repo| repo.source_paths.first())
            .cloned()
            .unwrap_or_else(|| common_dir.clone())
    }
}

/// Rank active candidate links by provenance precedence, then confidence,
/// then link id, and return the first. Non-active links are skipped, so
/// the helper is safe to call on either a list filtered ahead of time
/// (via `SnapshotIndex::candidates_for`) or a raw `Vec<&GraphLink>`.
pub fn pick_preferred<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links
        .iter()
        .copied()
        .filter(|link| matches!(link.state, LinkState::Active))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

/// Component-wise prefix match so `/a/b` does **not** count as an ancestor
/// of `/a/barbecue`.
pub fn path_is_ancestor_of(ancestor: &Path, descendant: &Path) -> bool {
    let mut anc_iter = ancestor.components();
    let mut desc_iter = descendant.components();
    loop {
        match (anc_iter.next(), desc_iter.next()) {
            (Some(a), Some(d)) if a == d => continue,
            (Some(_), Some(_)) => return false,
            (Some(_), None) => return false,
            (None, _) => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, GraphLink, GraphNode, GraphSnapshot,
        LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId, Provenance, RelationKind,
        RepoId, RepoNode,
    };

    fn repo(common_dir: &str) -> RepoNode {
        RepoNode::new(RepoId {
            common_dir: common_dir.to_string(),
        })
    }

    fn checkout(repo: &str, root: &str) -> GraphNode {
        GraphNode::checkout(
            CheckoutId::new(
                RepoId {
                    common_dir: repo.to_string(),
                },
                root.to_string(),
            ),
            root,
        )
    }

    fn agent(session_key: &str, harness: &str) -> AgentSessionNode {
        AgentSessionNode {
            id: AgentSessionId::new(harness, "default", session_key),
            harness_key: harness.to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
        }
    }

    fn mux(native_id: &str) -> MuxSessionNode {
        MuxSessionNode {
            id: MuxSessionId {
                native_id: native_id.to_string(),
            },
            backend: "tmux".to_string(),
            native_id: native_id.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            activity_epoch: None,
            created_epoch: None,
        }
    }

    fn linked_to_mux(
        link_id: &str,
        source: NodeId,
        target: NodeId,
        provenance: Provenance,
    ) -> GraphLink {
        GraphLink::new(
            link_id,
            source,
            LinkEndpoint::Node { id: target },
            RelationKind::LinkedToMux,
            provenance,
        )
    }

    #[test]
    fn checkout_for_path_picks_deepest_ancestor() {
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::Repo(repo("/r/.git")));
        snap.nodes.push(checkout("/r/.git", "/r"));
        snap.nodes.push(checkout("/r/.git", "/r/sub"));

        let idx = SnapshotIndex::new(&snap);
        let (deep, _) = idx.checkout_for_path("/r/sub/deeper").expect("ancestor");
        assert_eq!(deep.root, "/r/sub");

        let (shallow, _) = idx.checkout_for_path("/r/elsewhere").expect("ancestor");
        assert_eq!(shallow.root, "/r");

        assert!(idx.checkout_for_path("/unrelated").is_none());
    }

    #[test]
    fn mux_candidates_dedupe_by_target() {
        let mut snap = GraphSnapshot::empty();
        let session = agent("s1", "claude");
        let session_id = NodeId::AgentSession(session.id.clone());
        let mux_node = mux("tmux:0");
        let mux_id = NodeId::MuxSession(mux_node.id.clone());
        snap.nodes.push(GraphNode::AgentSession(session));
        snap.nodes.push(GraphNode::MuxSession(mux_node));
        snap.candidate_links.push(linked_to_mux(
            "a",
            session_id.clone(),
            mux_id.clone(),
            Provenance::Discovered,
        ));
        snap.candidate_links.push(linked_to_mux(
            "b",
            session_id.clone(),
            mux_id.clone(),
            Provenance::StrongDiscovered,
        ));

        let idx = SnapshotIndex::new(&snap);
        let candidates = idx.mux_candidates_for_session(&session_id);
        assert_eq!(candidates.len(), 1, "duplicate targets collapse");
        assert_eq!(candidates[0].id, "b", "preferred by stronger provenance");
    }

    #[test]
    fn by_source_relation_skips_non_active_links() {
        let mut snap = GraphSnapshot::empty();
        let session = agent("s1", "claude");
        let session_id = NodeId::AgentSession(session.id.clone());
        let mux_node = mux("tmux:0");
        let mux_id = NodeId::MuxSession(mux_node.id.clone());
        snap.nodes.push(GraphNode::AgentSession(session));
        snap.nodes.push(GraphNode::MuxSession(mux_node));
        let mut ignored = linked_to_mux(
            "ignored",
            session_id.clone(),
            mux_id.clone(),
            Provenance::Discovered,
        );
        ignored.state = LinkState::Ignored { reason: None };
        snap.candidate_links.push(ignored);

        let idx = SnapshotIndex::new(&snap);
        assert!(
            idx.candidates_for(&session_id, RelationKind::LinkedToMux)
                .is_empty()
        );
    }
}
