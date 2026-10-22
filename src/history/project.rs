//! Project history into the graph (ADR 0140).
//!
//! Each record gets a presence from a targeted check, never from
//! absence in a scan: a mux is gone when the latest successful probe
//! of its server didn't list it, and a checkout or repo when its path
//! is missing under an existing parent. Records that aren't live
//! become nodes with that presence and no live-only relations, so the
//! detail pane, search, Reveal, exports, and the History view work on
//! them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use crate::model::{
    AgentSessionId, BranchId, CheckoutId, CheckoutNode, CheckoutRecord, Confidence, Diagnostic,
    Freshness, GraphLink, GraphNode, GraphSnapshot, HistorySnapshot, LinkEndpoint, MuxRecord,
    MuxSessionId, MuxSessionNode, NodeId, NodePresence, NodeProvenance, Provenance, RelationKind,
    RepoId, RepoNode, RepoRecord, WorktreeMeta, effective_socket, probe_key,
};

use super::HistoryLoad;
use super::recorder::HISTORY_ADAPTER;

/// Fold `load` into `snapshot`: presence for every record, nodes for
/// the ones that aren't live, the history-derived relations, and the
/// records themselves in [`GraphSnapshot::history`].
pub fn project(snapshot: &mut GraphSnapshot, load: HistoryLoad, now: i64) {
    let dir = String::from("history");
    for problem in &load.problems {
        snapshot.diagnostics.push(Diagnostic::Config {
            path: dir.clone(),
            message: problem.clone(),
        });
    }
    let mut history = load.history;
    history.presence.clear();
    project_into(snapshot, &mut history, now);
    snapshot.history = history;
}

/// The projection proper, over records already in memory. Fixtures and
/// tests call it directly.
pub fn project_into(snapshot: &mut GraphSnapshot, history: &mut HistorySnapshot, now: i64) {
    let mut out = Projection::default();
    let live_muxes = match_live_muxes(snapshot, &history.muxes);

    for record in &history.muxes {
        if let Some(node_id) = live_muxes.get(&record.meta.id) {
            out.fallback_attribution(snapshot, record, node_id);
            continue;
        }
        let presence = mux_presence(snapshot, record);
        history.presence.insert(record.meta.id.clone(), presence);
        out.ghost_mux(snapshot, record, presence);
    }
    mark_exited_zellij(snapshot, history, &mut out);

    for record in &history.repos {
        let Some(presence) =
            path_presence(Path::new(&record.recipe.common_dir), record.meta.last_seen)
        else {
            continue;
        };
        history.presence.insert(record.meta.id.clone(), presence);
        out.gone_repo(snapshot, record, presence);
    }
    for record in &history.checkouts {
        let Some(presence) = path_presence(Path::new(&record.recipe.root), record.meta.last_seen)
        else {
            continue;
        };
        history.presence.insert(record.meta.id.clone(), presence);
        out.gone_checkout(snapshot, record, presence);
    }
    out.rebuilt_from(snapshot, &history.checkouts);
    out.apply(snapshot, now);
}

#[derive(Default)]
struct Projection {
    nodes: Vec<GraphNode>,
    links: Vec<GraphLink>,
    presence: BTreeMap<NodeId, NodePresence>,
}

impl Projection {
    fn has_node(&self, snapshot: &GraphSnapshot, id: &NodeId) -> bool {
        snapshot.nodes.iter().any(|node| node.has_id(id))
            || self.nodes.iter().any(|node| node.has_id(id))
    }

    fn ghost_mux(&mut self, snapshot: &GraphSnapshot, record: &MuxRecord, presence: NodePresence) {
        let recipe = &record.recipe;
        let native_id = match effective_socket(recipe.socket.as_deref()) {
            Some(socket) => format!("{socket}:{}", recipe.name),
            None => recipe.name.clone(),
        };
        let id = MuxSessionId::new(recipe.native_id());
        let node_id = NodeId::MuxSession(id.clone());
        if self.has_node(snapshot, &node_id) {
            return;
        }
        let mut node = MuxSessionNode::new(id, recipe.backend.clone(), native_id);
        node.cwd.clone_from(&recipe.cwd);
        node.socket = effective_socket(recipe.socket.as_deref()).map(str::to_string);
        if let Some(incarnation) = &recipe.incarnation {
            node.created_epoch = incarnation.created;
            node.backend_session_id.clone_from(&incarnation.session_id);
        }
        node.active_pane_start_command = recipe.argv.as_ref().map(|argv| argv.join(" "));
        self.nodes.push(GraphNode::MuxSession(node));
        self.presence.insert(node_id.clone(), presence);
        if let Some(hosted) = recipe.last_hosted()
            && let Some(session) = session_node_id(snapshot, &hosted.harness, &hosted.session_id)
        {
            self.links.push(history_link(
                format!("history:{}:last-hosted", record.meta.id),
                node_id,
                session,
                RelationKind::MuxLastHosted,
                "the session this mux hosted last",
            ));
        }
    }

    /// ADR 0102's stickiness for every mux: the session a live mux
    /// hosted last stays attributed to it as the weakest evidence,
    /// unless it went quiet before the mux was created.
    fn fallback_attribution(&mut self, snapshot: &GraphSnapshot, record: &MuxRecord, mux: &NodeId) {
        let Some(hosted) = record.recipe.last_hosted() else {
            return;
        };
        let Some(session_id) = session_node_id(snapshot, &hosted.harness, &hosted.session_id)
        else {
            return;
        };
        let created = mux_node(snapshot, mux).and_then(|node| node.created_epoch);
        let active = snapshot.nodes.iter().find_map(|node| match node {
            GraphNode::AgentSession(session)
                if NodeId::AgentSession(session.id.clone()) == session_id =>
            {
                session.last_active_epoch
            }
            _ => None,
        });
        if let (Some(created), Some(active)) = (created, active)
            && active < created
        {
            return;
        }
        let mut link = history_link(
            format!("history:{}:hosted", record.meta.id),
            session_id,
            mux.clone(),
            RelationKind::LinkedToMux,
            "the session this mux hosted last",
        );
        link.freshness = Freshness::Stale;
        self.links.push(link);
    }

    fn gone_repo(&mut self, snapshot: &GraphSnapshot, record: &RepoRecord, presence: NodePresence) {
        let id = RepoId::new(record.recipe.common_dir.clone());
        let node_id = NodeId::Repo(id.clone());
        if self.has_node(snapshot, &node_id) {
            return;
        }
        let mut node = RepoNode::new(id);
        node.remotes = record
            .recipe
            .remotes
            .iter()
            .map(|remote| format!("{}={}", remote.name, remote.url))
            .collect();
        node.source_paths = record.recipe.primary_root.iter().cloned().collect();
        self.nodes.push(GraphNode::Repo(node));
        self.presence.insert(node_id, presence);
    }

    fn gone_checkout(
        &mut self,
        snapshot: &GraphSnapshot,
        record: &CheckoutRecord,
        presence: NodePresence,
    ) {
        let recipe = &record.recipe;
        let repo = RepoId::new(recipe.common_dir().to_string());
        let id = CheckoutId::new(repo.clone(), recipe.root.clone());
        let node_id = NodeId::Checkout(id.clone());
        if self.has_node(snapshot, &node_id) {
            return;
        }
        let mut node = CheckoutNode::new(id, recipe.root.clone()).with_worktree(if recipe.linked {
            WorktreeMeta::linked()
        } else {
            WorktreeMeta::primary()
        });
        node.current_branch = recipe
            .branch
            .as_ref()
            .map(|branch| BranchId::new(repo.clone(), format!("refs/heads/{branch}")));
        node.head_commit.clone_from(&recipe.head);
        self.nodes.push(GraphNode::Checkout(node));
        self.presence.insert(node_id.clone(), presence);
        // Which repo a checkout belongs to is structure, not liveness,
        // so the gone checkout still files under its repo.
        let repo_node = NodeId::Repo(repo);
        if self.has_node(snapshot, &repo_node) {
            self.links.push(history_link(
                format!("history:{}:belongs-to-repo", record.meta.id),
                node_id,
                repo_node,
                RelationKind::BelongsToRepo,
                "recorded checkout of this repo",
            ));
        }
    }

    fn rebuilt_from(&mut self, snapshot: &GraphSnapshot, checkouts: &[CheckoutRecord]) {
        for record in checkouts {
            let Some(old_id) = &record.meta.rebuilt_from else {
                continue;
            };
            let Some(old) = checkouts.iter().find(|r| &r.meta.id == old_id) else {
                continue;
            };
            let node = |r: &CheckoutRecord| {
                NodeId::Checkout(CheckoutId::new(
                    RepoId::new(r.recipe.common_dir().to_string()),
                    r.recipe.root.clone(),
                ))
            };
            let (new, old) = (node(record), node(old));
            if self.has_node(snapshot, &new) && self.has_node(snapshot, &old) {
                self.links.push(history_link(
                    format!("history:{}:rebuilt-from", record.meta.id),
                    new,
                    old,
                    RelationKind::RebuiltFrom,
                    "rebuilt by conspectus",
                ));
            }
        }
    }

    fn apply(self, snapshot: &mut GraphSnapshot, now: i64) {
        for node in self.nodes {
            snapshot.node_provenance.insert(
                node.id(),
                NodeProvenance {
                    provider: HISTORY_ADAPTER.to_string(),
                    freshness_epoch: Some(now),
                },
            );
            snapshot.nodes.push(node);
        }
        for mut link in self.links {
            link.source_metadata.freshness_epoch = Some(now);
            snapshot.candidate_links.push(link);
        }
        snapshot.node_presence.extend(self.presence);
        snapshot.canonicalize();
    }
}

/// Which live mux node each mux record matches: by incarnation first,
/// then by name on the same server, each node at most once.
fn match_live_muxes(snapshot: &GraphSnapshot, records: &[MuxRecord]) -> BTreeMap<String, NodeId> {
    let live: Vec<&MuxSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) if !snapshot.node_presence.contains_key(&node.id()) => {
                Some(mux)
            }
            _ => None,
        })
        .filter(|mux| !exited(snapshot, mux))
        .collect();
    let mut taken: BTreeSet<NodeId> = BTreeSet::new();
    let mut matched = BTreeMap::new();
    let same_server = |record: &MuxRecord, mux: &MuxSessionNode| {
        record.recipe.backend == mux.backend
            && effective_socket(record.recipe.socket.as_deref())
                == effective_socket(mux.socket.as_deref())
    };
    for record in records {
        let Some(incarnation) = record
            .recipe
            .incarnation
            .as_ref()
            .filter(|inc| inc.is_complete())
        else {
            continue;
        };
        if let Some(mux) = live.iter().find(|mux| {
            same_server(record, mux)
                && !taken.contains(&NodeId::MuxSession(mux.id.clone()))
                && mux.backend_session_id == incarnation.session_id
                && mux.created_epoch == incarnation.created
        }) {
            let id = NodeId::MuxSession(mux.id.clone());
            taken.insert(id.clone());
            matched.insert(record.meta.id.clone(), id);
        }
    }
    for record in records {
        if matched.contains_key(&record.meta.id) {
            continue;
        }
        if let Some(mux) = live.iter().find(|mux| {
            same_server(record, mux)
                && !taken.contains(&NodeId::MuxSession(mux.id.clone()))
                && mux.session_name() == record.recipe.name
        }) {
            let id = NodeId::MuxSession(mux.id.clone());
            taken.insert(id.clone());
            matched.insert(record.meta.id.clone(), id);
        }
    }
    matched
}

/// The live mux node a record matches now, by incarnation or name.
pub fn live_mux_for(snapshot: &GraphSnapshot, record: &MuxRecord) -> Option<NodeId> {
    match_live_muxes(snapshot, std::slice::from_ref(record)).remove(&record.meta.id)
}

/// Gone when the latest probe of the mux's server succeeded without
/// listing it; unknown when that probe failed, the backend is missing,
/// or the server was never probed.
fn mux_presence(snapshot: &GraphSnapshot, record: &MuxRecord) -> NodePresence {
    let last_seen = record.meta.last_seen;
    let probe = snapshot.mux_probes.get(&probe_key(
        &record.recipe.backend,
        record.recipe.socket.as_deref(),
    ));
    match probe {
        Some(probe) if probe.status.succeeded() => NodePresence::Gone { last_seen },
        _ => NodePresence::Unknown { last_seen },
    }
}

fn exited(snapshot: &GraphSnapshot, mux: &MuxSessionNode) -> bool {
    snapshot
        .mux_probes
        .get(&probe_key(&mux.backend, mux.socket.as_deref()))
        .is_some_and(|probe| probe.exited.iter().any(|name| name == mux.session_name()))
}

/// A zellij session zellij lists as exited is gone: it can be
/// resurrected, but nothing runs in it (ADR 0140).
fn mark_exited_zellij(
    snapshot: &GraphSnapshot,
    history: &mut HistorySnapshot,
    out: &mut Projection,
) {
    for node in &snapshot.nodes {
        let GraphNode::MuxSession(mux) = node else {
            continue;
        };
        if !exited(snapshot, mux) {
            continue;
        }
        let record = history.muxes.iter().find(|record| {
            record.recipe.backend == mux.backend && record.recipe.name == mux.session_name()
        });
        let last_seen = record.map_or(0, |record| record.meta.last_seen);
        let presence = NodePresence::Gone { last_seen };
        if let Some(record) = record {
            history.presence.insert(record.meta.id.clone(), presence);
        }
        out.presence.insert(node.id(), presence);
    }
}

/// Live (`None`) when `path` exists, gone when it is missing under an
/// existing parent, unknown when the parent is missing too (an
/// unmounted drive) or the check fails.
fn path_presence(path: &Path, last_seen: i64) -> Option<NodePresence> {
    match fs::symlink_metadata(path) {
        Ok(_) => None,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let parent_exists = path
                .parent()
                .is_some_and(|parent| fs::metadata(parent).is_ok_and(|meta| meta.is_dir()));
            Some(if parent_exists {
                NodePresence::Gone { last_seen }
            } else {
                NodePresence::Unknown { last_seen }
            })
        }
        Err(_) => Some(NodePresence::Unknown { last_seen }),
    }
}

fn session_node_id(snapshot: &GraphSnapshot, harness: &str, session_key: &str) -> Option<NodeId> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(session)
            if session.id.harness_key == harness && session.id.session_key == session_key =>
        {
            Some(NodeId::AgentSession(AgentSessionId::clone(&session.id)))
        }
        _ => None,
    })
}

fn mux_node<'a>(snapshot: &'a GraphSnapshot, id: &NodeId) -> Option<&'a MuxSessionNode> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::MuxSession(mux) if NodeId::MuxSession(mux.id.clone()) == *id => Some(mux),
        _ => None,
    })
}

fn history_link(
    id: String,
    source: NodeId,
    target: NodeId,
    relation: RelationKind,
    evidence: &str,
) -> GraphLink {
    let mut link = GraphLink::new(
        id,
        source,
        LinkEndpoint::Node { id: target },
        relation,
        Provenance::Cached,
    );
    link.confidence = Confidence::Low;
    link.freshness = Freshness::Unknown;
    link.source_metadata.adapter = HISTORY_ADAPTER.to_string();
    link.source_metadata.evidence = Some(evidence.to_string());
    link
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
