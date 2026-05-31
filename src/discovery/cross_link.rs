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
//! No nodes are created here, and any `Unresolved` lineage endpoints already
//! present in `candidate_links` are left untouched.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

use crate::model::{
    AgentSessionNode, CheckoutId, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance, RelationKind, SessionKind,
    SourceMetadata,
};

const ADAPTER_NAME: &str = "cross_link";
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
    let (active_mux_sessions, mut process_unresolved_links) = active_mux_sessions(
        &agent_sessions,
        &mux_sessions,
        snapshot,
        &fd_reader,
        process_snapshot,
    );

    let mut new_links = Vec::new();
    new_links.append(&mut process_unresolved_links);

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

    snapshot.candidate_links.extend(new_links);

    suppress_subagent_mux_links(snapshot);

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

fn workspace_member_roots(snapshot: &GraphSnapshot) -> Vec<(NodeId, String)> {
    let mut roots = HashMap::new();

    for link in &snapshot.candidate_links {
        if link.relation != RelationKind::WorkspaceContainsRepo {
            continue;
        }

        for key in ["logical_path", "canonical_checkout_root"] {
            if let Some(path) = link
                .source_metadata
                .fields
                .get(key)
                .and_then(serde_json::Value::as_str)
            {
                roots
                    .entry((link.source.clone(), normalize_path(path)))
                    .or_insert(());
            }
        }
    }

    roots.into_keys().collect()
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
            "exact_cwd_match",
            Provenance::StrongDiscovered,
            Confidence::High,
        ));
    }

    if path_at_or_under(session_cwd, mux_cwd) || path_at_or_under(mux_cwd, session_cwd) {
        return Some(linked_to_mux(
            session,
            mux,
            "cwd_prefix_match",
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
        "active_pane_process_match",
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
        "match_kind".to_string(),
        serde_json::Value::String("active_pane_process_match".to_string()),
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
            evidence: Some("active_pane_process_match".to_string()),
            fields,
        },
        state: LinkState::Active,
    }
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
    process_snapshot: Option<&dyn ProcessSnapshot>,
) -> (
    HashMap<crate::model::MuxSessionId, ActiveMuxSessionMatches>,
    Vec<GraphLink>,
) {
    let parent_by_child = parent_session_keys_by_child(snapshot);
    let mut active = HashMap::new();
    let process_evidence_by_mux = process_snapshot
        .map(|snapshot| active_pane_process_evidence_by_mux(muxes, snapshot))
        .unwrap_or_default();
    let mut unresolved_links = Vec::new();

    for mux in muxes {
        let mut matches = ActiveMuxSessionMatches::default();
        let process_evidence = process_evidence_by_mux.get(&mux.id);
        let enforce_single_agent_session =
            process_snapshot.is_some() && controlling_agent_process_count(process_evidence) <= 1;

        if let Some(evidence) = active_pane_evidence(mux, fd_reader) {
            let direct_matches: BTreeSet<_> = sessions
                .iter()
                .filter(|session| evidence.matches_session(session))
                .map(|session| session.id.clone())
                .collect();

            if direct_matches.is_empty()
                && evidence.link_evidence != "active_pane_command_session_match"
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
            }
        }

        if !matches.has_identity_evidence()
            && let Some(process_evidence) = process_evidence
        {
            for evidence in process_evidence {
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
            && let Some(activity_match) =
                session_file_activity_match(mux, sessions, process_evidence)
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
        .map(|session| BTreeSet::from([session.id.clone()]))
        .unwrap_or_else(|| matches.clone())
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
        evidence: "session_file_activity_match",
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
        .filter(|evidence| !evidence.is_opencode_subagent_process())
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

struct ActiveMuxSessionMatch {
    sessions: BTreeSet<crate::model::AgentSessionId>,
    evidence: &'static str,
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
        self.harness_key == "opencode" && self.command.to_ascii_lowercase().contains(" subagent")
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
            .filter_map(|entry| entry.ok())
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
                        harness_key,
                        session_keys: uuid_like_values(command),
                        root_pid,
                        matched_pid: pid,
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
    let Some(first) = command.split_whitespace().next() else {
        return BTreeSet::new();
    };
    let name = Path::new(first)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(first)
        .to_ascii_lowercase();
    let mut harnesses = BTreeSet::new();
    match name.as_str() {
        "claude" | "claude-code" => {
            harnesses.insert("claude-code".to_string());
        }
        "codex" => {
            harnesses.insert("codex".to_string());
        }
        "opencode" => {
            harnesses.insert("opencode".to_string());
        }
        "aider" => {
            harnesses.insert("aider".to_string());
        }
        _ => {}
    }
    harnesses
}

#[derive(Debug, PartialEq, Eq)]
struct ActivePaneEvidence {
    session_keys: BTreeSet<String>,
    harnesses: BTreeSet<String>,
    link_evidence: &'static str,
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
            link_evidence: "active_pane_fd_session_match",
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
            link_evidence: "active_pane_fd_command_session_match",
        });
    }

    if !command_evidence.session_keys.is_empty() {
        let mut harnesses = command_evidence.harnesses;
        harnesses.extend(command_harnesses);
        return Some(ActivePaneEvidence {
            session_keys: command_evidence.session_keys,
            harnesses,
            link_evidence: "active_pane_command_session_match",
        });
    }

    if !fd_evidence.session_keys.is_empty() {
        return Some(ActivePaneEvidence {
            session_keys: fd_evidence.session_keys,
            harnesses: fd_harnesses,
            link_evidence: "active_pane_fd_session_match",
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
    let paths = fd_dir.filter_map(|entry| entry.ok()).filter_map(|entry| {
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
        let harness = if path.contains("/.codex/sessions/") || path.contains("/.codex/tmp/") {
            Some("codex")
        } else if path.contains("/.claude/tasks/") || path.contains("/.claude/projects/") {
            Some("claude-code")
        } else if path.contains("/.local/share/opencode/")
            || path.contains("/.config/opencode/")
            || path.contains("/.opencode/")
        {
            Some("opencode")
        } else {
            None
        };
        let Some(harness) = harness else {
            continue;
        };

        let keys = uuid_like_values(path);
        if keys.is_empty() {
            continue;
        }

        evidence.harnesses.insert(harness.to_string());
        evidence.session_keys.extend(keys);
    }

    evidence
}

fn command_session_evidence(command: &str) -> SessionKeyEvidence {
    SessionKeyEvidence {
        session_keys: command_session_keys(command),
        harnesses: command_harnesses(command),
    }
}

fn command_session_keys(command: &str) -> BTreeSet<String> {
    command
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn uuid_like_values(value: &str) -> BTreeSet<String> {
    const UUID_LEN: usize = 36;

    if value.len() < UUID_LEN {
        return BTreeSet::new();
    }

    let bytes = value.as_bytes();
    (0..=bytes.len() - UUID_LEN)
        .filter(|start| {
            is_uuid_like_bytes(&bytes[*start..*start + UUID_LEN])
                && uuid_boundary(bytes.get(start.wrapping_sub(1)).copied())
                && uuid_boundary(bytes.get(*start + UUID_LEN).copied())
        })
        .filter_map(|start| value.get(start..start + UUID_LEN).map(str::to_string))
        .collect()
}

fn is_uuid_like_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(idx, byte)| match idx {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn uuid_boundary(byte: Option<u8>) -> bool {
    !byte.is_some_and(|byte| byte.is_ascii_hexdigit())
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
    let command = command.to_ascii_lowercase();
    let mut harnesses = BTreeSet::new();
    if command.contains("claude") {
        harnesses.insert("claude-code".to_string());
    }
    if command.contains("codex") {
        harnesses.insert("codex".to_string());
    }
    if command.contains("opencode") {
        harnesses.insert("opencode".to_string());
    }
    if command.contains("aider") {
        harnesses.insert("aider".to_string());
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
    evidence: &str,
    provenance: Provenance,
    confidence: Confidence,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let target = NodeId::MuxSession(mux.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "match_kind".to_string(),
        serde_json::Value::String(evidence.to_string()),
    );

    if let Some(activity) = mux.activity_epoch {
        fields.insert(
            "mux_activity_epoch".to_string(),
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
        },
        state: LinkState::Active,
    }
}

fn fork_association_link(session: &AgentSessionNode, fork: &NodeId, root: &str) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "fork_root".to_string(),
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
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutNode, ForkId, ForkNode, GraphSnapshot,
        MuxSessionId, MuxSessionNode, RepoId, RepoNode, UnresolvedEndpoint, WorkspaceId,
        WorkspaceNode,
    };

    fn session(id: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", id),
            harness_key: "codex".to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn session_with_activity(id: &str, cwd: Option<&str>, last_active_epoch: i64) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", id),
            harness_key: "codex".to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(last_active_epoch),
            session_kind: None,
        })
    }

    fn mux(native: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: cwd.map(str::to_string),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn mux_with_active_command(native: &str, cwd: Option<&str>, command: &str) -> GraphNode {
        let active_pane_command = command.split_whitespace().next().map(str::to_string);
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: cwd.map(str::to_string),
            active_pane_command,
            active_pane_pid: None,
            active_pane_current_path: cwd.map(str::to_string),
            active_pane_start_command: Some(command.to_string()),
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn mux_with_active_process(
        native: &str,
        cwd: Option<&str>,
        command: &str,
        pid: i64,
    ) -> GraphNode {
        let active_pane_command = command.split_whitespace().next().map(str::to_string);
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: cwd.map(str::to_string),
            active_pane_command,
            active_pane_pid: Some(pid),
            active_pane_current_path: cwd.map(str::to_string),
            active_pane_start_command: Some(command.to_string()),
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    #[derive(Clone, Debug, Default)]
    struct FakeProcessSnapshot {
        records: Vec<ProcessRecord>,
    }

    impl FakeProcessSnapshot {
        fn new(records: impl IntoIterator<Item = ProcessRecord>) -> Self {
            Self {
                records: records.into_iter().collect(),
            }
        }
    }

    impl ProcessSnapshot for FakeProcessSnapshot {
        fn process_records(&self) -> Vec<ProcessRecord> {
            self.records.clone()
        }
    }

    fn process(
        pid: i64,
        parent_pid: Option<i64>,
        command: &str,
        cwd: Option<&str>,
    ) -> ProcessRecord {
        ProcessRecord {
            pid,
            parent_pid,
            command: Some(command.to_string()),
            cwd: cwd.map(str::to_string),
        }
    }

    fn fork_node(key: &str) -> GraphNode {
        GraphNode::Fork(ForkNode {
            id: ForkId::new(key),
            provider: "atelier".to_string(),
            provider_source_key: key.to_string(),
            name: Some(key.to_string()),
            scope: Some("workspace".to_string()),
            capabilities: Vec::new(),
        })
    }

    fn worktree(repo_common_dir: &str, root: &str) -> GraphNode {
        let repo = RepoId::new(repo_common_dir);
        GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo, root.to_string()),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
        })
    }

    fn repo(common_dir: &str) -> GraphNode {
        GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
    }

    fn workspace(root: &str) -> GraphNode {
        GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: None,
            name: None,
        })
    }

    fn workspace_contains_repo(
        workspace_root: &str,
        common_dir: &str,
        logical_path: &str,
    ) -> GraphLink {
        let source = NodeId::Workspace(WorkspaceId::new(workspace_root));
        let target = NodeId::Repo(RepoId::new(common_dir));
        let mut fields = crate::model::Metadata::new();
        fields.insert(
            "logical_path".to_string(),
            serde_json::Value::String(logical_path.to_string()),
        );
        fields.insert(
            "canonical_checkout_root".to_string(),
            serde_json::Value::String(logical_path.to_string()),
        );
        GraphLink {
            id: format!("test:{source}:workspace_contains_repo:{target}"),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "test".to_string(),
                evidence: Some("test workspace member".to_string()),
                fields,
            },
            state: LinkState::Active,
        }
    }

    fn rooted_at_path(fork_key: &str, path: &str) -> GraphLink {
        let source = NodeId::Fork(ForkId::new(fork_key));
        GraphLink {
            id: format!("test:{source}:rooted_at_path:{path}"),
            source,
            target: LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "path".to_string(),
                    harness_key: None,
                    native_id: None,
                    state_scope: None,
                    path: Some(path.to_string()),
                    metadata: Default::default(),
                },
            },
            relation: RelationKind::RootedAtPath,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "atelier".to_string(),
                evidence: Some("test rooted_at_path".to_string()),
                fields: Default::default(),
            },
            state: LinkState::Active,
        }
    }

    fn parent_session_link(child: &str, parent: &str) -> GraphLink {
        let source = NodeId::AgentSession(AgentSessionId::new("codex", "/state", child));
        let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", parent));
        GraphLink {
            id: format!("test:{source}:parent_session:{target}"),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "test".to_string(),
                evidence: Some("test parent".to_string()),
                fields: Default::default(),
            },
            state: LinkState::Active,
        }
    }

    #[test]
    fn orphan_sessions_get_no_links() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("orphan", Some("/work/orphan"))],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert!(snapshot.candidate_links.is_empty());
    }

    #[test]
    fn mux_only_snapshot_gets_no_links() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![mux("only", Some("/work/repo"))],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert!(snapshot.candidate_links.is_empty());
    }

    #[test]
    fn exact_cwd_match_yields_strong_linked_to_mux() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/work/repo")),
                mux("one", Some("/work/repo")),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert_eq!(snapshot.candidate_links.len(), 1);
        let link = &snapshot.candidate_links[0];
        assert_eq!(link.relation, RelationKind::LinkedToMux);
        assert_eq!(link.provenance, Provenance::StrongDiscovered);
        assert_eq!(link.confidence, Confidence::High);
    }

    #[test]
    fn plain_shell_active_pane_suppresses_cwd_only_mux_links() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("stale", Some("/work/repo")),
                mux_with_active_command("shell", Some("/work/repo"), "zsh"),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert!(
            snapshot
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::LinkedToMux),
            "plain shell mux should not claim stale sessions by cwd: {:#?}",
            snapshot.candidate_links
        );
    }

    #[test]
    fn cwd_match_generated_across_harnesses_when_no_pid_match() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("opencode", "/state", "a"),
                    harness_key: "opencode".to_string(),
                    cwd: Some("/work/repo".to_string()),
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                }),
                GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "/state", "b"),
                    harness_key: "codex".to_string(),
                    cwd: Some("/work/repo".to_string()),
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                }),
                mux_with_active_command("one", Some("/work/repo"), "opencode"),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref() == Some("exact_cwd_match")
            })
            .collect();
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn session_matches_multiple_mux_sessions_with_preserved_candidates() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/work/repo")),
                mux("one", Some("/work/repo")),
                mux("two", Some("/work/repo")),
                mux("three", Some("/work/repo/sub")),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(
            mux_links.len(),
            3,
            "every plausible mux candidate is preserved"
        );
        let strong: Vec<_> = mux_links
            .iter()
            .filter(|link| link.provenance == Provenance::StrongDiscovered)
            .collect();
        assert_eq!(strong.len(), 2, "two exact-cwd matches");
    }

    #[test]
    fn active_pane_command_session_match_suppresses_cwd_only_mux_links() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("current", Some("/work/repo")),
                session("stale", Some("/work/repo")),
                mux_with_active_command("one", Some("/work/repo"), "codex resume current"),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(mux_links.len(), 1);
        let link = mux_links[0];
        assert_eq!(
            link.source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "current"))
        );
        assert_eq!(
            link.source_metadata.evidence.as_deref(),
            Some("active_pane_command_session_match")
        );
    }

    #[test]
    fn active_pane_process_match_links_direct_harness_process() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("target-session", Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let link = snapshot
            .candidate_links
            .iter()
            .find(|link| link.relation == RelationKind::LinkedToMux)
            .expect("process link");
        assert_eq!(
            link.source_metadata.evidence.as_deref(),
            Some("active_pane_process_match")
        );
        assert_eq!(
            link.source_metadata
                .fields
                .get("matched_pid")
                .and_then(serde_json::Value::as_i64),
            Some(100)
        );
    }

    #[test]
    fn active_pane_process_match_walks_nested_shell_children() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("target-session", Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([
            process(100, None, "bash", Some("/work/repo")),
            process(101, Some(100), "zsh", Some("/work/repo")),
            process(102, Some(101), "/usr/bin/codex exec", Some("/work/repo")),
        ]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let link = snapshot
            .candidate_links
            .iter()
            .find(|link| link.relation == RelationKind::LinkedToMux)
            .expect("process link");
        assert_eq!(
            link.source_metadata.evidence.as_deref(),
            Some("active_pane_process_match")
        );
        assert_eq!(
            link.source_metadata
                .fields
                .get("process_depth")
                .and_then(serde_json::Value::as_u64),
            Some(2)
        );
    }

    #[test]
    fn active_pane_process_match_does_not_fan_out_across_same_cwd_sessions() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("old-session", Some("/work/repo")),
                session("new-session", Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let concrete_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.source, NodeId::AgentSession(_))
            })
            .collect();
        assert!(
            concrete_links.is_empty(),
            "ambiguous cwd process evidence should not fan out: {concrete_links:#?}"
        );

        let unresolved = snapshot
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.source, NodeId::MuxSession(_))
            })
            .expect("unresolved process evidence");
        assert_eq!(
            unresolved.source_metadata.evidence.as_deref(),
            Some("active_pane_process_match")
        );
    }

    #[test]
    fn active_pane_process_match_uses_process_command_session_key() {
        let target = "019e434b-9eff-7110-b2af-7c963aa8085e";
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("019e4354-26b9-7ad2-9521-4ad921cc312b", Some("/work/repo")),
                session(target, Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([process(
            100,
            None,
            &format!("codex resume {target}"),
            Some("/work/repo"),
        )]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let link = snapshot
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.source, NodeId::AgentSession(_))
            })
            .expect("exact process command link");
        assert_eq!(
            link.source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", target))
        );
        assert_eq!(
            link.source_metadata
                .fields
                .get("process_session_keys")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn active_pane_fd_session_suppresses_conflicting_process_resume_key() {
        let fd_target = "019e7733-0be9-7720-b828-e185f9029793";
        let process_target = "019e434b-9eff-7110-b2af-7c963aa8085e";
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session(process_target, Some("/work/repo")),
                session(fd_target, Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "codex", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([process(
            100,
            None,
            &format!("codex resume {process_target}"),
            Some("/work/repo"),
        )]);
        let fd_path = format!("/home/me/.codex/sessions/rollout-{fd_target}.jsonl");

        infer_with_readers(
            &mut snapshot,
            |pid| (pid == 100).then(|| session_key_evidence_from_fd_paths([fd_path.as_str()])),
            Some(&processes),
        );

        let concrete_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.source, NodeId::AgentSession(_))
            })
            .collect();
        assert_eq!(concrete_links.len(), 1);
        assert_eq!(
            concrete_links[0].source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", fd_target))
        );
        assert_eq!(
            concrete_links[0].source_metadata.evidence.as_deref(),
            Some("active_pane_fd_session_match")
        );
    }

    #[test]
    fn single_agent_process_collapses_activity_matches_to_one_session() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session_with_activity("older", Some("/work/repo"), 1_700_000_010),
                session_with_activity("newer", Some("/work/repo"), 1_700_000_020),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
            mux.created_epoch = Some(1_700_000_000);
        }
        let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let activity_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref()
                        == Some("session_file_activity_match")
            })
            .collect();
        assert_eq!(activity_links.len(), 1);
        assert_eq!(
            activity_links[0].source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "newer"))
        );
    }

    #[test]
    fn multiple_agent_processes_allow_multiple_session_attribution() {
        let codex_id = "019e7733-0be9-7720-b828-e185f9029793";
        let claude_id = "c1901a9e-f3db-48e3-a46c-3f92c7c0f2d3";
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session(codex_id, Some("/work/repo")),
                GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("claude-code", "/state", claude_id),
                    harness_key: "claude-code".to_string(),
                    cwd: Some("/work/repo".to_string()),
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                }),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([
            process(100, None, "bash", Some("/work/repo")),
            process(
                101,
                Some(100),
                &format!("codex resume {codex_id}"),
                Some("/work/repo"),
            ),
            process(
                102,
                Some(100),
                &format!("claude --resume {claude_id}"),
                Some("/work/repo"),
            ),
        ]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let process_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref() == Some("active_pane_process_match")
            })
            .collect();
        assert_eq!(process_links.len(), 2);
    }

    #[test]
    fn active_pane_process_unknown_binary_degrades_without_link() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("target-session", Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::new([process(100, None, "vim", Some("/work/repo"))]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        assert!(
            snapshot
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::LinkedToMux)
        );
    }

    #[test]
    fn active_pane_process_missing_pid_degrades_without_link() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("target-session", Some("/work/repo")),
                mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
            ],
            ..GraphSnapshot::empty()
        };
        let processes = FakeProcessSnapshot::default();

        infer_with_process_snapshot(&mut snapshot, &processes);

        assert!(
            snapshot
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::LinkedToMux)
        );
    }

    #[test]
    fn active_pane_process_preserves_unresolved_agent_evidence() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![mux_with_active_process(
                "editor",
                Some("/work/repo"),
                "bash",
                100,
            )],
            ..GraphSnapshot::empty()
        };
        let processes =
            FakeProcessSnapshot::new([process(101, Some(100), "codex", Some("/work/repo"))]);

        infer_with_process_snapshot(&mut snapshot, &processes);

        let link = snapshot
            .candidate_links
            .iter()
            .find(|link| link.relation == RelationKind::LinkedToMux)
            .expect("unresolved process evidence");
        assert!(matches!(link.source, NodeId::MuxSession(_)));
        let LinkEndpoint::Unresolved { evidence } = &link.target else {
            panic!("expected unresolved target");
        };
        assert_eq!(evidence.node_type, "agent_session");
        assert_eq!(evidence.harness_key.as_deref(), Some("codex"));
        assert_eq!(evidence.path.as_deref(), Some("/work/repo"));
    }

    #[test]
    fn session_file_activity_match_links_recent_same_harness_session() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session_with_activity("recent", Some("/work/repo"), 1_700_000_050),
                session_with_activity("stale", Some("/work/repo"), 1_699_000_000),
                mux_with_active_command("one", Some("/work/repo"), "codex"),
            ],
            ..GraphSnapshot::empty()
        };
        if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
            mux.created_epoch = Some(1_700_000_000);
        }

        infer_without_process_tree(&mut snapshot);

        let mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(mux_links.len(), 1);
        assert_eq!(
            mux_links[0].source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "recent"))
        );
        assert_eq!(
            mux_links[0].source_metadata.evidence.as_deref(),
            Some("session_file_activity_match")
        );
    }

    #[test]
    fn stale_session_file_activity_does_not_emit_activity_match() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session_with_activity("stale", Some("/work/repo"), 1_699_000_000),
                mux_with_active_command("one", Some("/work/repo"), "codex"),
            ],
            ..GraphSnapshot::empty()
        };
        if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[1] {
            mux.created_epoch = Some(1_700_000_000);
        }

        infer_without_process_tree(&mut snapshot);

        assert!(
            snapshot.candidate_links.iter().all(|link| {
                link.source_metadata.evidence.as_deref() != Some("session_file_activity_match")
            }),
            "stale session activity should not become activity evidence: {:#?}",
            snapshot.candidate_links
        );
    }

    #[test]
    fn ambiguous_same_cwd_activity_matches_remain_candidates() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session_with_activity("one", Some("/work/repo"), 1_700_000_010),
                session_with_activity("two", Some("/work/repo"), 1_700_000_020),
                mux_with_active_command("one", Some("/work/repo"), "codex"),
            ],
            ..GraphSnapshot::empty()
        };
        if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
            mux.created_epoch = Some(1_700_000_000);
        }

        infer_without_process_tree(&mut snapshot);

        let activity_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref()
                        == Some("session_file_activity_match")
            })
            .collect();
        assert_eq!(activity_links.len(), 2);
    }

    #[test]
    fn active_pane_resume_target_prefers_lineage_child_when_more_recent() {
        let parent = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "parent"),
            harness_key: "codex".to_string(),
            cwd: Some("/work/repo".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_000),
            session_kind: None,
        });
        let child = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "child"),
            harness_key: "codex".to_string(),
            cwd: Some("/work/repo".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(2_000),
            session_kind: None,
        });
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                parent,
                child,
                mux_with_active_command("one", Some("/work/repo"), "codex resume parent"),
            ],
            candidate_links: vec![parent_session_link("child", "parent")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(mux_links.len(), 1);
        assert_eq!(
            mux_links[0].source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "child"))
        );
    }

    #[test]
    fn active_pane_argv_match_prefers_parent_when_more_recent_than_child() {
        let parent = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "parent"),
            harness_key: "codex".to_string(),
            cwd: Some("/work/repo".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(2_000),
            session_kind: None,
        });
        let child = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "child"),
            harness_key: "codex".to_string(),
            cwd: Some("/work/repo".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_000),
            session_kind: None,
        });
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                parent,
                child,
                mux_with_active_command("one", Some("/work/repo"), "codex -s parent"),
            ],
            candidate_links: vec![parent_session_link("child", "parent")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(mux_links.len(), 1);
        assert_eq!(
            mux_links[0].source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "parent"))
        );
    }

    #[test]
    fn uuid_like_values_extracts_uuid_shaped_tokens() {
        let values = uuid_like_values(
            "/home/me/.codex/sessions/2026/05/19/rollout-2026-05-19T23-00-48-019e4354-26b9-7ad2-9521-4ad921cc312b.jsonl",
        );

        assert_eq!(
            values,
            BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()])
        );
    }

    #[test]
    fn fd_paths_extract_codex_and_claude_session_keys() {
        let evidence = session_key_evidence_from_fd_paths([
            "/home/me/.codex/sessions/2026/05/19/rollout-2026-05-19T23-00-48-019e4354-26b9-7ad2-9521-4ad921cc312b.jsonl",
            "/home/me/.claude/tasks/e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6/.lock",
            "/home/me/.config/other/11111111-2222-3333-4444-555555555555",
        ]);

        assert_eq!(
            evidence.session_keys,
            BTreeSet::from([
                "019e4354-26b9-7ad2-9521-4ad921cc312b".to_string(),
                "e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string(),
            ])
        );
        assert_eq!(
            evidence.harnesses,
            BTreeSet::from(["claude-code".to_string(), "codex".to_string()])
        );
    }

    #[test]
    fn active_pane_evidence_prefers_single_fd_session_over_command() {
        let mux = match mux_with_active_command(
            "one",
            Some("/work/repo"),
            "codex resume 019e3b8b-e512-7532-a1f2-7e88fcace046",
        ) {
            GraphNode::MuxSession(mut mux) => {
                mux.active_pane_pid = None;
                mux
            }
            _ => unreachable!(),
        };
        let fd = SessionKeyEvidence {
            session_keys: BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()]),
            harnesses: BTreeSet::from(["codex".to_string()]),
        };
        let command = command_session_evidence(mux.active_pane_start_command.as_deref().unwrap());
        let evidence =
            active_pane_evidence_from_sources(fd, command, active_pane_harnesses(&mux)).unwrap();

        assert_eq!(evidence.link_evidence, "active_pane_fd_session_match");
        assert_eq!(
            evidence.session_keys,
            BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()])
        );
    }

    #[test]
    fn active_pane_evidence_uses_fd_command_intersection() {
        let fd = SessionKeyEvidence {
            session_keys: BTreeSet::from([
                "e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string(),
                "c1901a9e-f3db-48e3-a46c-3f92c7c0f2d3".to_string(),
            ]),
            harnesses: BTreeSet::from(["claude-code".to_string()]),
        };
        let command =
            command_session_evidence("claude --resume e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6");
        let evidence = active_pane_evidence_from_sources(fd, command, BTreeSet::new()).unwrap();

        assert_eq!(
            evidence.link_evidence,
            "active_pane_fd_command_session_match"
        );
        assert_eq!(
            evidence.session_keys,
            BTreeSet::from(["e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string()])
        );
    }

    #[test]
    fn fork_root_match_emits_associated_with_candidate() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/work/fork-alpha/inside")),
                fork_node("atelier:alpha"),
            ],
            candidate_links: vec![rooted_at_path("atelier:alpha", "/work/fork-alpha")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let associated: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::AssociatedWith)
            .collect();
        assert_eq!(associated.len(), 1);
        let link = associated[0];
        assert_eq!(
            link.source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"))
        );
        assert_eq!(
            link.target_node_id(),
            Some(&NodeId::Fork(ForkId::new("atelier:alpha")))
        );
        assert_eq!(
            link.source_metadata
                .fields
                .get("fork_root")
                .and_then(serde_json::Value::as_str),
            Some("/work/fork-alpha")
        );
    }

    #[test]
    fn session_inside_worktree_emits_checkout_association_candidate() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/work/repo/crates/core")),
                worktree("/work/repo/.git", "/work/repo"),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let associated: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::AssociatedWith)
            .collect();
        assert_eq!(associated.len(), 1);
        let link = associated[0];
        assert_eq!(
            link.source,
            NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"))
        );
        assert_eq!(
            link.target_node_id(),
            Some(&NodeId::Checkout(CheckoutId::new(
                RepoId::new("/work/repo/.git"),
                "/work/repo"
            )))
        );
        assert_eq!(
            link.source_metadata
                .fields
                .get("checkout_root")
                .and_then(serde_json::Value::as_str),
            Some("/work/repo")
        );
    }

    #[test]
    fn session_inside_workspace_member_emits_workspace_and_checkout_candidates() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/workspace/repo/crates/core")),
                workspace("/workspace"),
                repo("/workspace/repo/.git"),
                worktree("/workspace/repo/.git", "/workspace/repo"),
            ],
            candidate_links: vec![workspace_contains_repo(
                "/workspace",
                "/workspace/repo/.git",
                "/workspace/repo",
            )],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let associated: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::AssociatedWith)
            .collect();
        assert_eq!(associated.len(), 2);
        assert!(associated.iter().any(|link| {
            link.target_node_id() == Some(&NodeId::Workspace(WorkspaceId::new("/workspace")))
        }));
        assert!(associated.iter().any(|link| {
            link.target_node_id()
                == Some(&NodeId::Checkout(CheckoutId::new(
                    RepoId::new("/workspace/repo/.git"),
                    "/workspace/repo",
                )))
        }));
    }

    #[test]
    fn nested_worktree_association_chooses_deepest_checkout() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                session("a", Some("/work/repo/nested/src")),
                worktree("/work/repo/.git", "/work/repo"),
                worktree("/work/repo/nested/.git", "/work/repo/nested"),
            ],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let associated: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::AssociatedWith)
            .collect();
        assert_eq!(associated.len(), 1);
        assert_eq!(
            associated[0].target_node_id(),
            Some(&NodeId::Checkout(CheckoutId::new(
                RepoId::new("/work/repo/nested/.git"),
                "/work/repo/nested"
            )))
        );
    }

    #[test]
    fn session_cwd_outside_fork_root_does_not_associate() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("a", Some("/elsewhere")), fork_node("atelier:alpha")],
            candidate_links: vec![rooted_at_path("atelier:alpha", "/work/fork-alpha")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert!(
            snapshot
                .candidate_links
                .iter()
                .all(|link| link.relation != RelationKind::AssociatedWith)
        );
    }

    #[test]
    fn unresolved_lineage_links_are_preserved() {
        let lineage = GraphLink {
            id: "atelier:lineage".to_string(),
            source: NodeId::Fork(ForkId::new("atelier:alpha")),
            target: LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some("codex".to_string()),
                    native_id: Some("missing".to_string()),
                    state_scope: None,
                    path: Some("/work/fork-alpha".to_string()),
                    metadata: Default::default(),
                },
            },
            relation: RelationKind::ChildSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "atelier".to_string(),
                evidence: Some("test lineage".to_string()),
                fields: Default::default(),
            },
            state: LinkState::Active,
        };
        let mut snapshot = GraphSnapshot {
            nodes: vec![fork_node("atelier:alpha")],
            candidate_links: vec![lineage.clone()],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        assert!(snapshot.candidate_links.contains(&lineage));
    }

    // --- Subagent mux suppression tests ---

    fn opencode_session(id: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("opencode", "/state", id),
            harness_key: "opencode".to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn subagent_session(id: &str, cwd: Option<&str>, parent_id: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("opencode", "/state", id),
            harness_key: "opencode".to_string(),
            cwd: cwd.map(str::to_string),
            title: Some(format!("(@general subagent) task from {parent_id}")),
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: Some(SessionKind::Subagent),
        })
    }

    fn subagent_parent_link(child_key: &str, parent_key: &str) -> GraphLink {
        let child = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", child_key));
        let parent = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", parent_key));
        GraphLink {
            id: format!("opencode:lineage:{child_key}:parent_session:{parent_key}"),
            source: child,
            target: LinkEndpoint::Node { id: parent },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "opencode".to_string(),
                evidence: Some("opencode session.parent_id unknown".to_string()),
                fields: Default::default(),
            },
            state: LinkState::Active,
        }
    }

    #[test]
    fn subagent_mux_link_overridden_when_parent_matches_same_mux() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                opencode_session("parent", Some("/work/repo")),
                subagent_session("child", Some("/work/repo"), "parent"),
                mux("one", Some("/work/repo")),
            ],
            candidate_links: vec![subagent_parent_link("child", "parent")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let child_mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source
                        == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "child"))
            })
            .collect();

        assert_eq!(child_mux_links.len(), 1);
        assert!(
            matches!(child_mux_links[0].state, LinkState::Overridden { .. }),
            "subagent mux link should be overridden when parent matches same mux"
        );
    }

    #[test]
    fn subagent_mux_link_stays_active_when_parent_does_not_match_mux() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                opencode_session("parent", Some("/other")),
                subagent_session("child", Some("/work/repo"), "parent"),
                mux("one", Some("/work/repo")),
            ],
            // ParentSession link exists, but parent has no LinkedToMux — so
            // the subagent's link should remain active.
            candidate_links: vec![subagent_parent_link("child", "parent")],
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let child_mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source
                        == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "child"))
            })
            .collect();

        assert_eq!(child_mux_links.len(), 1);
        assert!(
            matches!(child_mux_links[0].state, LinkState::Active),
            "subagent mux link should stay active when parent doesn't match the same mux"
        );
    }

    #[test]
    fn orphan_subagent_mux_link_stays_active() {
        // Subagent with no discovered parent should have normal mux linking.
        let mut snapshot = GraphSnapshot {
            nodes: vec![
                subagent_session("orphan", Some("/work/repo"), "missing-parent"),
                mux("one", Some("/work/repo")),
            ],
            // No ParentSession link (parent not discovered).
            ..GraphSnapshot::empty()
        };

        infer(&mut snapshot);

        let orphan_mux_links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source
                        == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "orphan"))
            })
            .collect();

        assert_eq!(orphan_mux_links.len(), 1);
        assert!(
            matches!(orphan_mux_links[0].state, LinkState::Active),
            "orphan subagent mux link should stay active"
        );
    }
}
