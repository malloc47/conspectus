//! Post-merge cross-provider link inference.
//!
//! After every provider has contributed nodes and provider-specific links to a
//! [`GraphSnapshot`], this module derives additional [`GraphLink`] candidates
//! that span providers:
//!
//! - `AgentSession` ↔ `MuxSession` `LinkedToMux` candidates whenever a session
//!   and a mux session share a working directory. Every plausible match is
//!   preserved per ADR 0006; one-to-many ambiguity stays visible until the
//!   resolver picks a preferred relationship.
//! - `AgentSession` ↔ `Fork` `AssociatedWith` candidates when a session's cwd
//!   lives at or below a fork's recorded root path (as captured by the atelier
//!   `RootedAtPath` evidence).
//! - `AgentSession` ↔ `Workspace` `AssociatedWith` candidates when a session's
//!   cwd lives at or below a workspace member path.
//! - `AgentSession` ↔ worktree/checkout `AssociatedWith` candidates when a
//!   session's cwd lives at or below a discovered checkout root.
//!
//! Runtime process observation nodes are created here when active-pane process
//! or fd evidence is available. Other provider-owned unresolved endpoints already
//! present in `candidate_links` are left untouched.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

use crate::model::{
    AgentSessionNode, CheckoutId, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MatchKind, MuxSessionNode, NodeId, Provenance, RelationKind,
    RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SessionKind, SourceMetadata,
};

const ADAPTER_NAME: &str = crate::discovery::providers::CROSS_LINK;
const PROCESS_TREE_MAX_DEPTH: usize = 4;
const SESSION_FILE_ACTIVITY_WINDOW_SECONDS: i64 = 15 * 60;

pub fn infer(snapshot: &mut GraphSnapshot) {
    let process_snapshot = LinuxProcSnapshot;
    infer_with_readers(
        snapshot,
        active_pane_fd_session_evidence,
        Some(&process_snapshot),
    );
}

pub fn infer_without_process_tree(snapshot: &mut GraphSnapshot) {
    infer_with_readers(snapshot, active_pane_fd_session_evidence, None);
}

/// Infer cross-provider links while injecting active-pane fd targets by pid.
///
/// This is primarily a deterministic test/replay seam: production discovery
/// uses [`infer`], which reads `/proc/<pid>/fd` on platforms where that exists.
/// Scenario tests use this helper to exercise the same fd-evidence path without
/// depending on the host process table.
#[doc(hidden)]
pub fn infer_with_fd_paths(
    snapshot: &mut GraphSnapshot,
    fd_paths_by_pid: &BTreeMap<i64, Vec<String>>,
) {
    infer_with_readers(
        snapshot,
        |pid| {
            fd_paths_by_pid
                .get(&pid)
                .map(session_key_evidence_from_fd_paths)
        },
        None,
    );
}

#[doc(hidden)]
pub fn infer_with_process_snapshot(
    snapshot: &mut GraphSnapshot,
    process_snapshot: &dyn ProcessSnapshot,
) {
    infer_with_readers(
        snapshot,
        active_pane_fd_session_evidence,
        Some(process_snapshot),
    );
}

fn infer_with_readers(
    snapshot: &mut GraphSnapshot,
    fd_reader: impl Fn(i64) -> Option<SessionKeyEvidence>,
    process_snapshot: Option<&dyn ProcessSnapshot>,
) {
    let agent_sessions: Vec<&AgentSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session),
            _ => None,
        })
        .collect();
    let mux_sessions: Vec<&MuxSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(mux),
            _ => None,
        })
        .collect();
    let checkout_roots = checkout_roots(snapshot);
    let workspace_member_roots = workspace_member_roots(snapshot);
    let fork_roots = fork_roots(snapshot);
    let process_evidence_by_mux = process_snapshot
        .map(|snapshot| active_pane_process_evidence_by_mux(&mux_sessions, snapshot))
        .unwrap_or_default();
    let (active_mux_sessions, mut process_unresolved_links) = active_mux_sessions(
        &agent_sessions,
        &mux_sessions,
        snapshot,
        &fd_reader,
        process_snapshot.is_some(),
        &process_evidence_by_mux,
    );
    let (mut process_nodes, mut process_links) =
        runtime_process_graph(&agent_sessions, &mux_sessions, &process_evidence_by_mux);
    if process_snapshot.is_none() {
        let (mut fd_process_nodes, mut fd_process_links) =
            fd_runtime_process_graph(&agent_sessions, &mux_sessions, &fd_reader);
        process_nodes.append(&mut fd_process_nodes);
        process_links.append(&mut fd_process_links);
    }

    let mut new_links = Vec::new();
    new_links.append(&mut process_unresolved_links);
    new_links.append(&mut process_links);

    for session in &agent_sessions {
        let Some(session_cwd) = session.cwd.as_deref().map(normalize_path) else {
            continue;
        };

        for mux in &mux_sessions {
            let Some(mux_cwd) = mux.cwd.as_deref().map(normalize_path) else {
                continue;
            };
            if let Some(link) = mux_match(
                session,
                mux,
                &session_cwd,
                &mux_cwd,
                active_mux_sessions.get(&mux.id),
            ) {
                new_links.push(link);
            }
        }

        for (fork, root) in &fork_roots {
            let root_norm = normalize_path(root);
            if path_at_or_under(&session_cwd, &root_norm) {
                new_links.push(fork_association_link(session, fork, root));
            }
        }

        for (workspace, root) in matching_workspaces(&session_cwd, &workspace_member_roots) {
            new_links.push(workspace_association_link(session, workspace, root));
        }

        if let Some((worktree, root)) = deepest_matching_checkout(&session_cwd, &checkout_roots) {
            new_links.push(checkout_association_link(session, worktree, root));
        }
    }

    snapshot.nodes.append(&mut process_nodes);
    snapshot.candidate_links.extend(new_links);

    suppress_subagent_mux_links(snapshot);

    // First-write-wins stamping covers links/nodes added above
    // without overriding entries earlier providers already tagged.
    crate::discovery::stamp_snapshot_mutations(
        snapshot,
        ADAPTER_NAME,
        crate::discovery::current_epoch(),
    );

    snapshot.canonicalize();
}

/// When a subagent session and its human parent both match the same
/// mux, the subagent's `LinkedToMux` candidate is overridden so the
/// parent session wins mux attachment in the TUI and resolver.  Orphan
/// subagents (no discovered parent) keep their candidates untouched.
fn suppress_subagent_mux_links(snapshot: &mut GraphSnapshot) {
    let mut parent_of: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    let mut is_subagent: HashSet<NodeId> = HashSet::new();

    for node in &snapshot.nodes {
        if let GraphNode::AgentSession(session) = node
            && session.session_kind == Some(SessionKind::Subagent)
        {
            is_subagent.insert(node.id());
        }
    }

    if is_subagent.is_empty() {
        return;
    }

    for link in &snapshot.candidate_links {
        if link.relation != RelationKind::ParentSession || !matches!(link.state, LinkState::Active)
        {
            continue;
        }
        if is_subagent.contains(&link.source)
            && let Some(parent) = link.target_node_id()
        {
            parent_of.insert(link.source.clone(), parent.clone());
        }
    }

    // Collect (subagent_id, parent_id, mux_id) triples for mux links
    // that should be overridden.
    let mut to_override: Vec<(usize, String)> = Vec::new();
    for (idx, link) in snapshot.candidate_links.iter().enumerate() {
        if link.relation != RelationKind::LinkedToMux || !matches!(link.state, LinkState::Active) {
            continue;
        }
        let Some(subagent_parent) = parent_of.get(&link.source) else {
            continue;
        };
        let Some(target_mux) = link.target_node_id() else {
            continue;
        };

        // Check if the parent also has a LinkedToMux to the same mux.
        let parent_has_match = snapshot.candidate_links.iter().any(|other| {
            other.relation == RelationKind::LinkedToMux
                && matches!(other.state, LinkState::Active)
                && &other.source == subagent_parent
                && other.target_node_id() == Some(target_mux)
        });

        if parent_has_match {
            let reason = format!(
                "overridden: subagent {} linked to mux {}; parent {} takes precedence",
                link.source, target_mux, subagent_parent,
            );
            to_override.push((idx, reason));
        }
    }

    for (idx, reason) in to_override {
        if let Some(link) = snapshot.candidate_links.get_mut(idx) {
            link.state = LinkState::Overridden {
                by: "cross_link".to_string(),
                reason: Some(reason),
            };
        }
    }
}

fn checkout_roots(snapshot: &GraphSnapshot) -> Vec<(CheckoutId, String)> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Checkout(worktree) => Some((worktree.id.clone(), worktree.root.clone())),
            _ => None,
        })
        .collect()
}

/// Index workspaces by every path inside their visible tree —
/// both the workspace `root` itself (so a session launched at
/// the multi-repo composite directory attributes correctly, the
/// agent-deck launch shape) and every member's `logical_path`
/// (so a session inside a specific member subdir attributes via
/// the deeper match). Used by [`matching_workspaces`] which
/// picks the deepest match per workspace, so a session in a
/// member subdir prefers the member; a session at the workspace
/// root attributes to the root.
///
/// **Crucially excludes `canonical_checkout_root`** even though
/// that field is also present on the membership link: when a
/// workspace member is a symlink pointing outside the workspace
/// tree, the canonical path is the *target* checkout's location
/// and a session running there is *not* doing workspace-context
/// work — it just happens to touch a repo that is also a
/// workspace member. Indexing the canonical root would collapse
/// the (A) workspace-rooted and (B) repo-shared classes that
/// `H-WS-001` / `docs/plans/workspace-view-redesign.md` are
/// explicitly trying to keep separate.
///
/// `canonical_checkout_root` remains on the membership link's
/// `source_metadata.fields` for downstream consumers that need
/// the canonical path; it just doesn't drive the
/// session→workspace association.
fn workspace_member_roots(snapshot: &GraphSnapshot) -> Vec<(NodeId, String)> {
    let mut roots = BTreeSet::new();

    for link in &snapshot.candidate_links {
        if link.relation != RelationKind::WorkspaceContainsRepo {
            continue;
        }

        // Workspace root itself — covers `cd <workspace> && agent`
        // (the typical agent-deck launch shape).
        if let NodeId::Workspace(ws) = &link.source {
            roots.insert((link.source.clone(), normalize_path(&ws.root)));
        }

        // Member `logical_path` — covers `cd <workspace>/<member> && agent`
        // (the typical atelier composite-tree shape). The deeper
        // match wins via [`matching_workspaces`] so the member
        // attribution preempts the root attribution when both fire.
        if let Some(path) = link
            .source_metadata
            .fields
            .get(crate::model::source_field::LOGICAL_PATH)
            .and_then(serde_json::Value::as_str)
        {
            roots.insert((link.source.clone(), normalize_path(path)));
        }
    }

    roots.into_iter().collect()
}

fn matching_workspaces<'a>(
    session_cwd: &str,
    workspace_roots: &'a [(NodeId, String)],
) -> Vec<(&'a NodeId, &'a str)> {
    let mut deepest_by_workspace: HashMap<&NodeId, &str> = HashMap::new();

    for (workspace, root) in workspace_roots {
        if !path_at_or_under(session_cwd, root) {
            continue;
        }

        let current = deepest_by_workspace
            .entry(workspace)
            .or_insert(root.as_str());
        if path_depth(root) > path_depth(current) {
            *current = root;
        }
    }

    deepest_by_workspace.into_iter().collect()
}

fn deepest_matching_checkout<'a>(
    session_cwd: &str,
    worktrees: &'a [(CheckoutId, String)],
) -> Option<(&'a CheckoutId, &'a str)> {
    worktrees
        .iter()
        .filter(|(_, root)| path_at_or_under(session_cwd, &normalize_path(root)))
        .max_by_key(|(_, root)| root.split('/').filter(|part| !part.is_empty()).count())
        .map(|(id, root)| (id, root.as_str()))
}

fn fork_roots(snapshot: &GraphSnapshot) -> Vec<(NodeId, String)> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::RootedAtPath)
        .filter_map(|link| match &link.target {
            LinkEndpoint::Unresolved { evidence } => evidence
                .path
                .as_ref()
                .map(|path| (link.source.clone(), path.clone())),
            LinkEndpoint::Node { .. } => None,
        })
        .fold(HashMap::new(), |mut acc, (fork, path)| {
            acc.entry(fork).or_insert(path);
            acc
        })
        .into_iter()
        .collect()
}

fn mux_match(
    session: &AgentSessionNode,
    mux: &MuxSessionNode,
    session_cwd: &str,
    mux_cwd: &str,
    active_sessions: Option<&ActiveMuxSessionMatches>,
) -> Option<GraphLink> {
    if let Some(active_sessions) = active_sessions {
        if let Some(identity_match) = active_sessions.identity_match_for(&session.id) {
            return Some(linked_to_mux(
                session,
                mux,
                identity_match.evidence,
                Provenance::StrongDiscovered,
                Confidence::High,
            ));
        }

        if let Some(process_match) = active_sessions.process_match_for(&session.id) {
            return Some(process_linked_to_mux(session, mux, process_match));
        }

        if active_sessions.has_current_evidence() {
            return None;
        }
    }

    if mux_has_non_harness_active_pane(mux) {
        return None;
    }

    if session_cwd == mux_cwd {
        return Some(linked_to_mux(
            session,
            mux,
            MatchKind::ExactCwdMatch,
            Provenance::StrongDiscovered,
            Confidence::High,
        ));
    }

    if path_at_or_under(session_cwd, mux_cwd) || path_at_or_under(mux_cwd, session_cwd) {
        return Some(linked_to_mux(
            session,
            mux,
            MatchKind::CwdPrefixMatch,
            Provenance::Discovered,
            Confidence::Medium,
        ));
    }

    None
}

fn process_linked_to_mux(
    session: &AgentSessionNode,
    mux: &MuxSessionNode,
    process_match: &ActiveMuxProcessMatch,
) -> GraphLink {
    let mut link = linked_to_mux(
        session,
        mux,
        MatchKind::ActivePaneProcessMatch,
        Provenance::StrongDiscovered,
        Confidence::High,
    );
    insert_process_fields(&mut link.source_metadata.fields, &process_match.evidence);
    link
}

fn insert_process_fields(fields: &mut crate::model::Metadata, evidence: &ProcessPaneEvidence) {
    fields.insert(
        "pane_root_pid".to_string(),
        serde_json::Value::Number(evidence.root_pid.into()),
    );
    fields.insert(
        "matched_pid".to_string(),
        serde_json::Value::Number(evidence.matched_pid.into()),
    );
    if let Some(parent_pid) = evidence.parent_pid {
        fields.insert(
            "parent_pid".to_string(),
            serde_json::Value::Number(parent_pid.into()),
        );
    }
    fields.insert(
        "process_depth".to_string(),
        serde_json::Value::Number((evidence.depth as u64).into()),
    );
    fields.insert(
        "process_command".to_string(),
        serde_json::Value::String(evidence.command.clone()),
    );
    if !evidence.session_keys.is_empty() {
        fields.insert(
            "process_session_keys".to_string(),
            serde_json::Value::Array(
                evidence
                    .session_keys
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(cwd) = &evidence.cwd {
        fields.insert(
            "process_cwd".to_string(),
            serde_json::Value::String(cwd.clone()),
        );
    }
}

fn process_unresolved_link(mux: &MuxSessionNode, evidence: &ProcessPaneEvidence) -> GraphLink {
    let source = NodeId::MuxSession(mux.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(MatchKind::ActivePaneProcessMatch.to_string()),
    );
    insert_process_fields(&mut fields, evidence);

    let mut endpoint_metadata = crate::model::Metadata::new();
    endpoint_metadata.insert(
        "pane_root_pid".to_string(),
        serde_json::Value::Number(evidence.root_pid.into()),
    );
    endpoint_metadata.insert(
        "matched_pid".to_string(),
        serde_json::Value::Number(evidence.matched_pid.into()),
    );
    if let Some(parent_pid) = evidence.parent_pid {
        endpoint_metadata.insert(
            "parent_pid".to_string(),
            serde_json::Value::Number(parent_pid.into()),
        );
    }
    endpoint_metadata.insert(
        "process_depth".to_string(),
        serde_json::Value::Number((evidence.depth as u64).into()),
    );
    endpoint_metadata.insert(
        "process_command".to_string(),
        serde_json::Value::String(evidence.command.clone()),
    );

    GraphLink {
        id: format!(
            "cross_link:{source}:linked_to_mux:unresolved_process:{}:{}",
            evidence.harness_key, evidence.matched_pid
        ),
        source,
        target: LinkEndpoint::Unresolved {
            evidence: crate::model::UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some(evidence.harness_key.clone()),
                native_id: None,
                state_scope: None,
                path: evidence.cwd.clone(),
                metadata: endpoint_metadata,
            },
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Low,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(MatchKind::ActivePaneProcessMatch.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn runtime_process_graph(
    sessions: &[&AgentSessionNode],
    muxes: &[&MuxSessionNode],
    process_evidence_by_mux: &HashMap<crate::model::MuxSessionId, Vec<ProcessPaneEvidence>>,
) -> (Vec<GraphNode>, Vec<GraphLink>) {
    let mut nodes_by_id = BTreeMap::new();
    let mut links_by_id = BTreeMap::new();

    for mux in muxes {
        let Some(process_evidence) = process_evidence_by_mux.get(&mux.id) else {
            continue;
        };
        for evidence in process_evidence {
            if evidence.role() == RuntimeProcessRole::Background {
                continue;
            }
            let process_node = runtime_process_node(mux, evidence);
            let process_id = NodeId::RuntimeProcess(process_node.id.clone());
            nodes_by_id.insert(
                process_node.id.clone(),
                GraphNode::RuntimeProcess(process_node),
            );

            let containment = runtime_process_link(
                format!(
                    "cross_link:{}:mux_contains_process:{}",
                    NodeId::MuxSession(mux.id.clone()),
                    process_id
                ),
                NodeId::MuxSession(mux.id.clone()),
                LinkEndpoint::Node {
                    id: process_id.clone(),
                },
                RelationKind::MuxContainsProcess,
                MatchKind::ActivePaneProcessObservation,
                Confidence::High,
                evidence,
            );
            links_by_id.insert(containment.id.clone(), containment);

            let matching_sessions: Vec<_> = sessions
                .iter()
                .copied()
                .filter(|session| evidence.matches_session(session))
                .collect();

            if matching_sessions.is_empty() {
                let unresolved = runtime_process_link(
                    format!(
                        "cross_link:{process_id}:process_candidates_session:unresolved:{}",
                        evidence.matched_pid
                    ),
                    process_id.clone(),
                    LinkEndpoint::Unresolved {
                        evidence: crate::model::UnresolvedEndpoint {
                            node_type: "agent_session".to_string(),
                            harness_key: Some(evidence.harness_key.clone()),
                            native_id: None,
                            state_scope: None,
                            path: evidence.cwd.clone(),
                            metadata: runtime_process_endpoint_metadata(evidence),
                        },
                    },
                    RelationKind::ProcessCandidatesSession,
                    MatchKind::ActivePaneProcessMatch,
                    Confidence::Low,
                    evidence,
                );
                links_by_id.insert(unresolved.id.clone(), unresolved);
                continue;
            }

            for session in matching_sessions {
                let relation = if evidence.session_keys.contains(&session.id.session_key) {
                    RelationKind::ProcessIdentifiesSession
                } else {
                    RelationKind::ProcessCandidatesSession
                };
                let confidence = if relation == RelationKind::ProcessIdentifiesSession {
                    Confidence::High
                } else {
                    Confidence::Medium
                };
                let target = NodeId::AgentSession(session.id.clone());
                let link = runtime_process_link(
                    format!(
                        "cross_link:{process_id}:{}:{target}",
                        process_relation_label(&relation)
                    ),
                    process_id.clone(),
                    LinkEndpoint::Node { id: target },
                    relation,
                    MatchKind::ActivePaneProcessMatch,
                    confidence,
                    evidence,
                );
                links_by_id.insert(link.id.clone(), link);
            }
        }
    }

    (
        nodes_by_id.into_values().collect(),
        links_by_id.into_values().collect(),
    )
}

fn runtime_process_node(
    mux: &MuxSessionNode,
    evidence: &ProcessPaneEvidence,
) -> RuntimeProcessNode {
    let observation_key = format!(
        "{}:root:{}:pid:{}",
        NodeId::MuxSession(mux.id.clone()),
        evidence.root_pid,
        evidence.matched_pid
    );
    RuntimeProcessNode {
        id: RuntimeProcessId::new(&observation_key),
        observation_key,
        pid: Some(evidence.matched_pid),
        parent_pid: evidence.parent_pid,
        root_pane_pid: Some(evidence.root_pid),
        command: Some(evidence.command.clone()),
        cwd: evidence.cwd.clone(),
        harness_key: Some(evidence.harness_key.clone()),
        role: Some(evidence.role()),
        depth: Some(evidence.depth as i64),
        observed_epoch: None,
    }
}

fn runtime_process_link(
    id: String,
    source: NodeId,
    target: LinkEndpoint,
    relation: RelationKind,
    evidence_label: MatchKind,
    confidence: Confidence,
    evidence: &ProcessPaneEvidence,
) -> GraphLink {
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(evidence_label.to_string()),
    );
    insert_process_fields(&mut fields, evidence);
    GraphLink {
        id,
        source,
        target,
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(evidence_label.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn runtime_process_endpoint_metadata(evidence: &ProcessPaneEvidence) -> crate::model::Metadata {
    let mut metadata = crate::model::Metadata::new();
    insert_process_fields(&mut metadata, evidence);
    metadata
}

fn process_relation_label(relation: &RelationKind) -> &'static str {
    match relation {
        RelationKind::ProcessIdentifiesSession => "process_identifies_session",
        RelationKind::ProcessCandidatesSession => "process_candidates_session",
        RelationKind::MuxContainsProcess => "mux_contains_process",
        _ => "process_evidence",
    }
}

fn fd_runtime_process_graph(
    sessions: &[&AgentSessionNode],
    muxes: &[&MuxSessionNode],
    fd_reader: &impl Fn(i64) -> Option<SessionKeyEvidence>,
) -> (Vec<GraphNode>, Vec<GraphLink>) {
    let mut nodes_by_id = BTreeMap::new();
    let mut links_by_id = BTreeMap::new();

    for mux in muxes {
        let Some(pid) = mux.active_pane_pid else {
            continue;
        };
        let Some(evidence) = active_pane_evidence(mux, fd_reader) else {
            continue;
        };
        if evidence.link_evidence == MatchKind::ActivePaneCommandSessionMatch {
            continue;
        }

        let process_node = fd_runtime_process_node(mux, pid, &evidence);
        let process_id = NodeId::RuntimeProcess(process_node.id.clone());
        nodes_by_id.insert(
            process_node.id.clone(),
            GraphNode::RuntimeProcess(process_node),
        );

        let containment = fd_runtime_process_link(
            format!(
                "cross_link:{}:mux_contains_process:{}",
                NodeId::MuxSession(mux.id.clone()),
                process_id
            ),
            NodeId::MuxSession(mux.id.clone()),
            LinkEndpoint::Node {
                id: process_id.clone(),
            },
            RelationKind::MuxContainsProcess,
            Confidence::High,
            pid,
            &evidence,
        );
        links_by_id.insert(containment.id.clone(), containment);

        let matching_sessions: Vec<_> = sessions
            .iter()
            .copied()
            .filter(|session| evidence.matches_session(session))
            .collect();

        if matching_sessions.is_empty() {
            let unresolved = fd_runtime_process_link(
                format!("cross_link:{process_id}:process_candidates_session:fd:{pid}"),
                process_id.clone(),
                LinkEndpoint::Unresolved {
                    evidence: crate::model::UnresolvedEndpoint {
                        node_type: "agent_session".to_string(),
                        harness_key: single_value(&evidence.harnesses),
                        native_id: None,
                        state_scope: None,
                        path: mux
                            .active_pane_current_path
                            .as_ref()
                            .or(mux.cwd.as_ref())
                            .cloned(),
                        metadata: fd_runtime_process_endpoint_metadata(pid, &evidence),
                    },
                },
                RelationKind::ProcessCandidatesSession,
                Confidence::Low,
                pid,
                &evidence,
            );
            links_by_id.insert(unresolved.id.clone(), unresolved);
            continue;
        }

        for session in matching_sessions {
            let target = NodeId::AgentSession(session.id.clone());
            let link = fd_runtime_process_link(
                format!("cross_link:{process_id}:process_identifies_session:{target}"),
                process_id.clone(),
                LinkEndpoint::Node { id: target },
                RelationKind::ProcessIdentifiesSession,
                Confidence::High,
                pid,
                &evidence,
            );
            links_by_id.insert(link.id.clone(), link);
        }
    }

    (
        nodes_by_id.into_values().collect(),
        links_by_id.into_values().collect(),
    )
}

fn fd_runtime_process_node(
    mux: &MuxSessionNode,
    pid: i64,
    evidence: &ActivePaneEvidence,
) -> RuntimeProcessNode {
    let observation_key = format!("{}:fd:pid:{}", NodeId::MuxSession(mux.id.clone()), pid);
    RuntimeProcessNode {
        id: RuntimeProcessId::new(&observation_key),
        observation_key,
        pid: Some(pid),
        parent_pid: None,
        root_pane_pid: Some(pid),
        command: mux
            .active_pane_start_command
            .clone()
            .or_else(|| mux.active_pane_command.clone()),
        cwd: mux
            .active_pane_current_path
            .clone()
            .or_else(|| mux.cwd.clone()),
        harness_key: single_value(&evidence.harnesses),
        role: Some(RuntimeProcessRole::HumanAgent),
        depth: Some(0),
        observed_epoch: mux.activity_epoch,
    }
}

fn fd_runtime_process_link(
    id: String,
    source: NodeId,
    target: LinkEndpoint,
    relation: RelationKind,
    confidence: Confidence,
    pid: i64,
    evidence: &ActivePaneEvidence,
) -> GraphLink {
    let fields = fd_runtime_process_endpoint_metadata(pid, evidence);
    GraphLink {
        id,
        source,
        target,
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(evidence.link_evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn fd_runtime_process_endpoint_metadata(
    pid: i64,
    evidence: &ActivePaneEvidence,
) -> crate::model::Metadata {
    let mut metadata = crate::model::Metadata::new();
    metadata.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(evidence.link_evidence.to_string()),
    );
    metadata.insert(
        "matched_pid".to_string(),
        serde_json::Value::Number(pid.into()),
    );
    metadata.insert(
        "session_keys".to_string(),
        serde_json::Value::Array(
            evidence
                .session_keys
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    metadata.insert(
        "harnesses".to_string(),
        serde_json::Value::Array(
            evidence
                .harnesses
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    metadata
}

fn single_value(values: &BTreeSet<String>) -> Option<String> {
    (values.len() == 1)
        .then(|| values.iter().next().cloned())
        .flatten()
}

fn mux_has_non_harness_active_pane(mux: &MuxSessionNode) -> bool {
    let has_active_pane = mux.active_pane_command.is_some()
        || mux.active_pane_start_command.is_some()
        || mux.active_pane_pid.is_some();
    has_active_pane && active_pane_harnesses(mux).is_empty()
}

fn active_mux_sessions(
    sessions: &[&AgentSessionNode],
    muxes: &[&MuxSessionNode],
    snapshot: &GraphSnapshot,
    fd_reader: &impl Fn(i64) -> Option<SessionKeyEvidence>,
    has_process_snapshot: bool,
    process_evidence_by_mux: &HashMap<crate::model::MuxSessionId, Vec<ProcessPaneEvidence>>,
) -> (
    HashMap<crate::model::MuxSessionId, ActiveMuxSessionMatches>,
    Vec<GraphLink>,
) {
    let parent_by_child = parent_session_keys_by_child(snapshot);
    let mut active = HashMap::new();
    let mut unresolved_links = Vec::new();

    for mux in muxes {
        let mut matches = ActiveMuxSessionMatches::default();
        let process_evidence = process_evidence_by_mux.get(&mux.id);
        let enforce_single_agent_session =
            has_process_snapshot && controlling_agent_process_count(process_evidence) <= 1;
        let activity_match = session_file_activity_match(mux, sessions, process_evidence);

        if let Some(evidence) = active_pane_evidence(mux, fd_reader) {
            let direct_matches: BTreeSet<_> = sessions
                .iter()
                .filter(|session| evidence.matches_session(session))
                .map(|session| session.id.clone())
                .collect();

            if direct_matches.is_empty()
                && evidence.link_evidence != MatchKind::ActivePaneCommandSessionMatch
            {
                matches.unresolved_identity = true;
            } else if !direct_matches.is_empty() {
                let child_matches: BTreeSet<_> = sessions
                    .iter()
                    .filter(|session| {
                        parent_by_child
                            .get(&session.id)
                            .is_some_and(|parents| !parents.is_disjoint(&direct_matches))
                    })
                    .map(|session| session.id.clone())
                    .collect();

                let matched_sessions = if child_matches.is_empty() {
                    direct_matches
                } else {
                    let direct_recent = most_recent_epoch(&direct_matches, sessions);
                    let child_recent = most_recent_epoch(&child_matches, sessions);
                    if direct_recent >= child_recent {
                        direct_matches
                    } else {
                        child_matches
                    }
                };

                let matched_sessions = if enforce_single_agent_session {
                    freshest_human_session(&matched_sessions, sessions)
                } else {
                    matched_sessions
                };

                matches.identity = Some(ActiveMuxSessionMatch {
                    sessions: matched_sessions,
                    evidence: evidence.link_evidence,
                });
                if evidence.link_evidence == MatchKind::ActivePaneCommandSessionMatch
                    && let Some(activity_match) = &activity_match
                    && most_recent_epoch(&activity_match.sessions, sessions)
                        > most_recent_epoch(
                            &matches
                                .identity
                                .as_ref()
                                .expect("identity just set")
                                .sessions,
                            sessions,
                        )
                {
                    matches.identity = Some(if enforce_single_agent_session {
                        activity_match
                            .clone()
                            .collapse_to_freshest_human_session(sessions)
                    } else {
                        activity_match.clone()
                    });
                }
            }
        }

        if !matches.has_identity_evidence()
            && let Some(process_evidence) = process_evidence
        {
            for evidence in process_evidence {
                if evidence.role() == RuntimeProcessRole::Background {
                    continue;
                }
                let process_matches: BTreeSet<_> = sessions
                    .iter()
                    .filter(|session| evidence.matches_session(session))
                    .map(|session| session.id.clone())
                    .collect();

                if process_matches.len() == 1 {
                    matches.process.push(ActiveMuxProcessMatch {
                        sessions: process_matches,
                        evidence: evidence.clone(),
                    });
                } else {
                    matches.unresolved_process = true;
                    unresolved_links.push(process_unresolved_link(mux, evidence));
                }
            }
        }

        if matches.identity.is_none()
            && let Some(activity_match) = activity_match
        {
            matches.identity = Some(if enforce_single_agent_session {
                activity_match.collapse_to_freshest_human_session(sessions)
            } else {
                activity_match
            });
        }

        if matches.has_current_evidence() {
            active.insert(mux.id.clone(), matches);
        }
    }

    (active, unresolved_links)
}

fn most_recent_epoch(
    matches: &BTreeSet<crate::model::AgentSessionId>,
    sessions: &[&AgentSessionNode],
) -> i64 {
    sessions
        .iter()
        .filter(|s| matches.contains(&s.id))
        .filter_map(|s| s.last_active_epoch)
        .max()
        .unwrap_or(0)
}

fn freshest_human_session(
    matches: &BTreeSet<crate::model::AgentSessionId>,
    sessions: &[&AgentSessionNode],
) -> BTreeSet<crate::model::AgentSessionId> {
    sessions
        .iter()
        .filter(|session| matches.contains(&session.id))
        .filter(|session| session.session_kind != Some(SessionKind::Subagent))
        .max_by_key(|session| (session.last_active_epoch.unwrap_or(0), session.id.clone()))
        .map_or_else(
            || matches.clone(),
            |session| BTreeSet::from([session.id.clone()]),
        )
}

fn session_file_activity_match(
    mux: &MuxSessionNode,
    sessions: &[&AgentSessionNode],
    process_evidence: Option<&Vec<ProcessPaneEvidence>>,
) -> Option<ActiveMuxSessionMatch> {
    let anchor_epoch = mux.created_epoch.or(mux.activity_epoch)?;
    let mux_cwd = mux
        .active_pane_current_path
        .as_deref()
        .or(mux.cwd.as_deref())
        .map(normalize_path)?;
    let harnesses = activity_harnesses(mux, process_evidence);
    if harnesses.is_empty() {
        return None;
    }

    let sessions: BTreeSet<_> = sessions
        .iter()
        .filter(|session| harnesses.contains(&session.harness_key))
        .filter(|session| {
            session
                .cwd
                .as_deref()
                .is_some_and(|cwd| normalize_path(cwd) == mux_cwd)
        })
        .filter(|session| {
            session
                .last_active_epoch
                .is_some_and(|epoch| epoch_close(epoch, anchor_epoch))
        })
        .map(|session| session.id.clone())
        .collect();

    (!sessions.is_empty()).then_some(ActiveMuxSessionMatch {
        sessions,
        evidence: MatchKind::SessionFileActivityMatch,
    })
}

fn activity_harnesses(
    mux: &MuxSessionNode,
    process_evidence: Option<&Vec<ProcessPaneEvidence>>,
) -> BTreeSet<String> {
    let mut harnesses = active_pane_harnesses(mux);
    if let Some(process_evidence) = process_evidence {
        harnesses.extend(
            process_evidence
                .iter()
                .filter(|evidence| evidence.role() != RuntimeProcessRole::Background)
                .map(|evidence| evidence.harness_key.clone()),
        );
    }
    harnesses
}

fn epoch_close(left: i64, right: i64) -> bool {
    left.abs_diff(right) <= SESSION_FILE_ACTIVITY_WINDOW_SECONDS as u64
}

fn controlling_agent_process_count(process_evidence: Option<&Vec<ProcessPaneEvidence>>) -> usize {
    process_evidence
        .into_iter()
        .flatten()
        .filter(|evidence| evidence.role() == RuntimeProcessRole::HumanAgent)
        .map(|evidence| evidence.matched_pid)
        .collect::<BTreeSet<_>>()
        .len()
}

#[derive(Default)]
struct ActiveMuxSessionMatches {
    identity: Option<ActiveMuxSessionMatch>,
    process: Vec<ActiveMuxProcessMatch>,
    unresolved_identity: bool,
    unresolved_process: bool,
}

impl ActiveMuxSessionMatches {
    fn has_current_evidence(&self) -> bool {
        self.has_identity_evidence() || !self.process.is_empty() || self.unresolved_process
    }

    fn has_identity_evidence(&self) -> bool {
        self.identity.is_some() || self.unresolved_identity
    }

    fn identity_match_for(
        &self,
        session: &crate::model::AgentSessionId,
    ) -> Option<&ActiveMuxSessionMatch> {
        self.identity
            .as_ref()
            .filter(|identity| identity.sessions.contains(session))
    }

    fn process_match_for(
        &self,
        session: &crate::model::AgentSessionId,
    ) -> Option<&ActiveMuxProcessMatch> {
        self.process
            .iter()
            .find(|process| process.sessions.contains(session))
    }
}

#[derive(Clone)]
struct ActiveMuxSessionMatch {
    sessions: BTreeSet<crate::model::AgentSessionId>,
    evidence: MatchKind,
}

impl ActiveMuxSessionMatch {
    fn collapse_to_freshest_human_session(self, sessions: &[&AgentSessionNode]) -> Self {
        Self {
            sessions: freshest_human_session(&self.sessions, sessions),
            evidence: self.evidence,
        }
    }
}

struct ActiveMuxProcessMatch {
    sessions: BTreeSet<crate::model::AgentSessionId>,
    evidence: ProcessPaneEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProcessPaneEvidence {
    harness_key: String,
    session_keys: BTreeSet<String>,
    root_pid: i64,
    matched_pid: i64,
    parent_pid: Option<i64>,
    depth: usize,
    command: String,
    cwd: Option<String>,
}

impl ProcessPaneEvidence {
    fn matches_session(&self, session: &AgentSessionNode) -> bool {
        if session.harness_key != self.harness_key {
            return false;
        }
        if !self.session_keys.is_empty() {
            return self.session_keys.contains(&session.id.session_key);
        }
        let (Some(process_cwd), Some(session_cwd)) = (&self.cwd, &session.cwd) else {
            return false;
        };
        normalize_path(process_cwd) == normalize_path(session_cwd)
    }

    fn is_opencode_subagent_process(&self) -> bool {
        // Delegate to the registered adapter's
        // signature. Preserves the pre-H-EXT-004 opencode-only
        // rule.
        self.signature_role_check(|sig| (sig.is_subagent_process)(&self.command))
    }

    fn is_claude_background_process(&self) -> bool {
        // Delegate to the registered adapter's
        // signature. Preserves the pre-H-EXT-004 claude-code-only
        // rule; adapters that don't ship helper daemons point
        // their `is_background_process` at
        // [`crate::discovery::harness::no_match`].
        self.signature_role_check(|sig| (sig.is_background_process)(&self.command))
    }

    fn signature_role_check<F>(&self, predicate: F) -> bool
    where
        F: Fn(&crate::discovery::harness::RuntimeSignature) -> bool,
    {
        crate::discovery::harness::registered_adapters()
            .find(|a| a.harness_key() == self.harness_key)
            .is_some_and(|a| predicate(a.runtime_signature()))
    }

    fn role(&self) -> RuntimeProcessRole {
        if self.is_claude_background_process() {
            RuntimeProcessRole::Background
        } else if self.is_opencode_subagent_process() {
            RuntimeProcessRole::Subagent
        } else {
            RuntimeProcessRole::HumanAgent
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRecord {
    pub pid: i64,
    pub parent_pid: Option<i64>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

pub trait ProcessSnapshot {
    fn process_records(&self) -> Vec<ProcessRecord>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LinuxProcSnapshot;

impl ProcessSnapshot for LinuxProcSnapshot {
    fn process_records(&self) -> Vec<ProcessRecord> {
        let Ok(entries) = fs::read_dir("/proc") else {
            return Vec::new();
        };

        entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let pid = entry.file_name().to_string_lossy().parse::<i64>().ok()?;
                process_record_from_proc(pid, &entry.path())
            })
            .collect()
    }
}

fn process_record_from_proc(pid: i64, proc_dir: &Path) -> Option<ProcessRecord> {
    let stat = fs::read_to_string(proc_dir.join("stat")).ok()?;
    let parent_pid = parse_proc_stat_parent_pid(&stat);
    let stat_command = parse_proc_stat_command(&stat);
    let command = fs::read(proc_dir.join("cmdline"))
        .ok()
        .and_then(|bytes| {
            let parts: Vec<_> = bytes
                .split(|byte| *byte == 0)
                .filter(|part| !part.is_empty())
                .filter_map(|part| String::from_utf8(part.to_vec()).ok())
                .collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        })
        .or(stat_command);
    let cwd = fs::read_link(proc_dir.join("cwd"))
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok());

    Some(ProcessRecord {
        pid,
        parent_pid,
        command,
        cwd,
    })
}

fn parse_proc_stat_parent_pid(stat: &str) -> Option<i64> {
    let after_command = stat.rsplit_once(") ")?.1;
    let mut fields = after_command.split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse().ok()
}

fn parse_proc_stat_command(stat: &str) -> Option<String> {
    let start = stat.find('(')? + 1;
    let end = stat.rfind(')')?;
    (end > start).then(|| stat[start..end].to_string())
}

/// Pid set per mux that exposes the same process-tree walk used internally
/// for `active_pane_process_match` candidates, but without the identity-
/// evidence gating that suppresses publication when fd/command evidence
/// already resolved the mux. Downstream linkers that *need* the pid even
/// when fd evidence won (e.g. ADR 0048 codex log attribution, which uses
/// the pid to query logs.process_uuid) can consume this directly.
pub fn active_harness_pids_per_mux(
    snapshot: &GraphSnapshot,
    process_snapshot: &dyn ProcessSnapshot,
) -> BTreeMap<crate::model::MuxSessionId, Vec<(String, i64)>> {
    let muxes: Vec<&MuxSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(mux),
            _ => None,
        })
        .collect();
    let evidence = active_pane_process_evidence_by_mux(&muxes, process_snapshot);
    evidence
        .into_iter()
        .map(|(mux_id, records)| {
            let mut pids: Vec<(String, i64)> = records
                .into_iter()
                .map(|r| (r.harness_key, r.matched_pid))
                .collect();
            pids.sort();
            pids.dedup();
            (mux_id, pids)
        })
        .collect()
}

fn active_pane_process_evidence_by_mux(
    muxes: &[&MuxSessionNode],
    process_snapshot: &dyn ProcessSnapshot,
) -> HashMap<crate::model::MuxSessionId, Vec<ProcessPaneEvidence>> {
    let records = process_snapshot.process_records();
    let mut by_pid: HashMap<i64, &ProcessRecord> = HashMap::new();
    let mut children_by_parent: HashMap<i64, Vec<i64>> = HashMap::new();

    for record in &records {
        by_pid.insert(record.pid, record);
        if let Some(parent) = record.parent_pid {
            children_by_parent
                .entry(parent)
                .or_default()
                .push(record.pid);
        }
    }

    let mut output = HashMap::new();
    for mux in muxes {
        let Some(root_pid) = mux.active_pane_pid else {
            continue;
        };
        let evidence = active_pane_process_evidence(root_pid, &by_pid, &children_by_parent);
        if !evidence.is_empty() {
            output.insert(mux.id.clone(), evidence);
        }
    }

    output
}

fn active_pane_process_evidence(
    root_pid: i64,
    by_pid: &HashMap<i64, &ProcessRecord>,
    children_by_parent: &HashMap<i64, Vec<i64>>,
) -> Vec<ProcessPaneEvidence> {
    let mut queue = VecDeque::from([(root_pid, 0usize)]);
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    let mut seen_harness_pid = HashSet::new();

    while let Some((pid, depth)) = queue.pop_front() {
        if !seen.insert(pid) {
            continue;
        }

        if let Some(record) = by_pid.get(&pid)
            && let Some(command) = record.command.as_deref()
        {
            for harness_key in process_command_harnesses(command) {
                if seen_harness_pid.insert((harness_key.clone(), pid)) {
                    output.push(ProcessPaneEvidence {
                        session_keys: command_session_keys_for_harness(command, &harness_key),
                        harness_key,
                        root_pid,
                        matched_pid: pid,
                        parent_pid: record.parent_pid,
                        depth,
                        command: command.to_string(),
                        cwd: record.cwd.clone(),
                    });
                }
            }
        }

        if depth >= PROCESS_TREE_MAX_DEPTH {
            continue;
        }
        if let Some(children) = children_by_parent.get(&pid) {
            for child in children {
                queue.push_back((*child, depth + 1));
            }
        }
    }

    output
}

fn process_command_harnesses(command: &str) -> BTreeSet<String> {
    // Iterate registered adapters and consult each
    // signature's `process_command_basenames`. The pre-H-EXT-004
    // hand-rolled match table for the four v1 harnesses now
    // lives on the per-adapter signatures.
    let Some(first) = command.split_whitespace().next() else {
        return BTreeSet::new();
    };
    let name = Path::new(first)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(first)
        .to_ascii_lowercase();
    let mut harnesses = BTreeSet::new();
    for adapter in crate::discovery::harness::registered_adapters() {
        let sig = adapter.runtime_signature();
        if sig
            .process_command_basenames
            .iter()
            .any(|b| b.eq_ignore_ascii_case(&name))
        {
            harnesses.insert(adapter.harness_key().to_string());
        }
    }
    harnesses
}

#[derive(Debug, PartialEq, Eq)]
struct ActivePaneEvidence {
    session_keys: BTreeSet<String>,
    harnesses: BTreeSet<String>,
    link_evidence: MatchKind,
}

impl ActivePaneEvidence {
    fn matches_session(&self, session: &AgentSessionNode) -> bool {
        self.session_keys.contains(&session.id.session_key)
            && (self.harnesses.is_empty() || self.harnesses.contains(&session.harness_key))
    }
}

fn active_pane_evidence(
    mux: &MuxSessionNode,
    fd_reader: &impl Fn(i64) -> Option<SessionKeyEvidence>,
) -> Option<ActivePaneEvidence> {
    let fd_evidence = mux.active_pane_pid.and_then(fd_reader).unwrap_or_default();
    let command_evidence = mux
        .active_pane_start_command
        .as_deref()
        .map(command_session_evidence)
        .unwrap_or_default();
    let command_harnesses = active_pane_harnesses(mux);

    active_pane_evidence_from_sources(fd_evidence, command_evidence, command_harnesses)
}

fn active_pane_evidence_from_sources(
    fd_evidence: SessionKeyEvidence,
    command_evidence: SessionKeyEvidence,
    command_harnesses: BTreeSet<String>,
) -> Option<ActivePaneEvidence> {
    let mut fd_harnesses = fd_evidence.harnesses;
    fd_harnesses.extend(command_harnesses.iter().cloned());

    if fd_evidence.session_keys.len() == 1 {
        return Some(ActivePaneEvidence {
            session_keys: fd_evidence.session_keys,
            harnesses: fd_harnesses,
            link_evidence: MatchKind::ActivePaneFdSessionMatch,
        });
    }

    let intersection: BTreeSet<_> = fd_evidence
        .session_keys
        .intersection(&command_evidence.session_keys)
        .cloned()
        .collect();
    if !intersection.is_empty() {
        let mut harnesses = fd_harnesses;
        harnesses.extend(command_evidence.harnesses);
        return Some(ActivePaneEvidence {
            session_keys: intersection,
            harnesses,
            link_evidence: MatchKind::ActivePaneFdCommandSessionMatch,
        });
    }

    if !command_evidence.session_keys.is_empty() {
        let mut harnesses = command_evidence.harnesses;
        harnesses.extend(command_harnesses);
        return Some(ActivePaneEvidence {
            session_keys: command_evidence.session_keys,
            harnesses,
            link_evidence: MatchKind::ActivePaneCommandSessionMatch,
        });
    }

    if !fd_evidence.session_keys.is_empty() {
        return Some(ActivePaneEvidence {
            session_keys: fd_evidence.session_keys,
            harnesses: fd_harnesses,
            link_evidence: MatchKind::ActivePaneFdSessionMatch,
        });
    }

    None
}

#[derive(Default)]
struct SessionKeyEvidence {
    session_keys: BTreeSet<String>,
    harnesses: BTreeSet<String>,
}

fn active_pane_fd_session_evidence(pid: i64) -> Option<SessionKeyEvidence> {
    let fd_dir = fs::read_dir(format!("/proc/{pid}/fd")).ok()?;
    let paths = fd_dir
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            fs::read_link(entry.path())
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok())
        });
    Some(session_key_evidence_from_fd_paths(paths))
}

fn session_key_evidence_from_fd_paths<I, S>(paths: I) -> SessionKeyEvidence
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut evidence = SessionKeyEvidence::default();

    for path in paths {
        let path = path.as_ref();
        // Match the path against every registered
        // adapter's `fd_path_patterns`. First match wins so
        // paths that contain multiple harness fragments
        // (rare) resolve to the first-registered adapter,
        // matching the pre-H-EXT-004 if-chain's short-circuit
        // behavior.
        let mut matched: Option<&'static dyn crate::discovery::harness::HarnessAdapter> = None;
        for adapter in crate::discovery::harness::registered_adapters() {
            let sig = adapter.runtime_signature();
            if sig.fd_path_patterns.iter().any(|p| path.contains(p)) {
                matched = Some(adapter);
                break;
            }
        }
        let Some(adapter) = matched else {
            continue;
        };
        let keys = (adapter.runtime_signature().extract_session_keys)(path);
        if keys.is_empty() {
            continue;
        }

        evidence.harnesses.insert(adapter.harness_key().to_string());
        evidence.session_keys.extend(keys);
    }

    evidence
}

fn command_session_evidence(command: &str) -> SessionKeyEvidence {
    let harnesses = command_harnesses(command);
    let mut session_keys = BTreeSet::new();
    if harnesses.is_empty() {
        session_keys.extend(crate::discovery::harness::generic_uuid_like_session_keys(
            command,
        ));
    } else {
        for harness in &harnesses {
            session_keys.extend(command_session_keys_for_harness(command, harness));
        }
    }

    SessionKeyEvidence {
        session_keys,
        harnesses,
    }
}

fn ordered_command_tokens(command: &str) -> Vec<String> {
    command
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn command_session_keys_for_harness(command: &str, harness: &str) -> BTreeSet<String> {
    let parts = ordered_command_tokens(command);
    let mut keys = BTreeSet::new();

    for (idx, part) in parts.iter().enumerate() {
        match part.as_str() {
            "resume" | "-s" | "--session" | "session" => {
                if let Some(next) = parts.get(idx + 1)
                    && !looks_like_command_flag(next)
                {
                    keys.insert(next.clone());
                }
            }
            _ => {}
        }
    }

    keys.extend(session_keys_for_harness_text(harness, command));
    keys
}

fn looks_like_command_flag(value: &str) -> bool {
    value.starts_with('-')
}

fn session_keys_for_harness_text(harness: &str, value: &str) -> BTreeSet<String> {
    // Dispatch to the registered adapter's
    // `extract_session_keys` callback. Unknown harnesses fall
    // back to the generic UUID grammar via
    // `crate::discovery::harness::generic_uuid_like_session_keys`
    // (matches pre-H-EXT-004 catch-all behavior).
    for adapter in crate::discovery::harness::registered_adapters() {
        if adapter.harness_key() == harness {
            return (adapter.runtime_signature().extract_session_keys)(value);
        }
    }
    crate::discovery::harness::generic_uuid_like_session_keys(value)
}

fn active_pane_harnesses(mux: &MuxSessionNode) -> BTreeSet<String> {
    let mut harnesses = BTreeSet::new();
    if let Some(command) = mux.active_pane_command.as_deref() {
        harnesses.extend(command_harnesses(command));
    }
    if let Some(command) = mux.active_pane_start_command.as_deref() {
        harnesses.extend(command_harnesses(command));
    }
    harnesses
}

fn command_harnesses(command: &str) -> BTreeSet<String> {
    // Iterate registered adapters and consult each
    // signature's `command_substrings` (loose case-insensitive
    // contains). The pre-H-EXT-004 hand-rolled if-chain for the
    // four v1 harnesses now lives on the per-adapter signatures.
    let command = command.to_ascii_lowercase();
    let mut harnesses = BTreeSet::new();
    for adapter in crate::discovery::harness::registered_adapters() {
        let sig = adapter.runtime_signature();
        if sig
            .command_substrings
            .iter()
            .any(|s| command.contains(&s.to_ascii_lowercase()))
        {
            harnesses.insert(adapter.harness_key().to_string());
        }
    }
    harnesses
}

fn parent_session_keys_by_child(
    snapshot: &GraphSnapshot,
) -> HashMap<crate::model::AgentSessionId, BTreeSet<crate::model::AgentSessionId>> {
    let mut parents: HashMap<crate::model::AgentSessionId, BTreeSet<crate::model::AgentSessionId>> =
        HashMap::new();

    for link in &snapshot.candidate_links {
        if link.relation != RelationKind::ParentSession {
            continue;
        }
        let NodeId::AgentSession(child) = &link.source else {
            continue;
        };
        let Some(NodeId::AgentSession(parent)) = link.target_node_id() else {
            continue;
        };
        parents
            .entry(child.clone())
            .or_default()
            .insert(parent.clone());
    }

    parents
}

fn linked_to_mux(
    session: &AgentSessionNode,
    mux: &MuxSessionNode,
    evidence: MatchKind,
    provenance: Provenance,
    confidence: Confidence,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let target = NodeId::MuxSession(mux.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(evidence.to_string()),
    );

    if let Some(activity) = mux.activity_epoch {
        fields.insert(
            crate::model::source_field::MUX_ACTIVITY_EPOCH.to_string(),
            serde_json::Value::Number(activity.into()),
        );
    }

    GraphLink {
        id: format!("cross_link:{source}:linked_to_mux:{target}:{evidence}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(evidence.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn fork_association_link(session: &AgentSessionNode, fork: &NodeId, root: &str) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        crate::model::source_field::FORK_ROOT.to_string(),
        serde_json::Value::String(root.to_string()),
    );
    GraphLink {
        id: format!("cross_link:{source}:associated_with:{fork}"),
        source,
        target: LinkEndpoint::Node { id: fork.clone() },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some("session cwd within fork root".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn workspace_association_link(
    session: &AgentSessionNode,
    workspace: &NodeId,
    member_root: &str,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "workspace_member_root".to_string(),
        serde_json::Value::String(member_root.to_string()),
    );
    GraphLink {
        id: format!("cross_link:{source}:associated_with:{workspace}:cwd_within_workspace"),
        source,
        target: LinkEndpoint::Node {
            id: workspace.clone(),
        },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some("session cwd within workspace member".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn checkout_association_link(
    session: &AgentSessionNode,
    worktree: &CheckoutId,
    root: &str,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let target = NodeId::Checkout(worktree.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "checkout_root".to_string(),
        serde_json::Value::String(root.to_string()),
    );
    GraphLink {
        id: format!("cross_link:{source}:associated_with:{target}:cwd_within_checkout"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some("session cwd within checkout root".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn path_at_or_under(child: &str, parent: &str) -> bool {
    if child == parent {
        return true;
    }
    if parent == "/" {
        return child.starts_with('/');
    }
    child.starts_with(&format!("{parent}/"))
}

fn path_depth(path: &str) -> usize {
    path.split('/').filter(|part| !part.is_empty()).count()
}

#[cfg(test)]
#[path = "cross_link_tests.rs"]
mod tests;
