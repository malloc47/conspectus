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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;

use crate::model::{
    AgentSessionNode, CheckoutId, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance, RelationKind, SessionKind,
    SourceMetadata,
};

const ADAPTER_NAME: &str = "cross_link";

pub fn infer(snapshot: &mut GraphSnapshot) {
    infer_with_fd_reader(snapshot, active_pane_fd_session_evidence);
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
    infer_with_fd_reader(snapshot, |pid| {
        fd_paths_by_pid
            .get(&pid)
            .map(session_key_evidence_from_fd_paths)
    });
}

fn infer_with_fd_reader(
    snapshot: &mut GraphSnapshot,
    fd_reader: impl Fn(i64) -> Option<SessionKeyEvidence>,
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
    let active_mux_sessions =
        active_mux_sessions(&agent_sessions, &mux_sessions, snapshot, &fd_reader);

    let mut new_links = Vec::new();

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
    active_sessions: Option<&ActiveMuxSessionMatch>,
) -> Option<GraphLink> {
    if let Some(active_sessions) = active_sessions {
        return active_sessions.contains(&session.id).then(|| {
            linked_to_mux(
                session,
                mux,
                active_sessions.evidence,
                Provenance::StrongDiscovered,
                Confidence::High,
            )
        });
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
) -> HashMap<crate::model::MuxSessionId, ActiveMuxSessionMatch> {
    let parent_by_child = parent_session_keys_by_child(snapshot);
    let mut active = HashMap::new();

    for mux in muxes {
        let Some(evidence) = active_pane_evidence(mux, fd_reader) else {
            continue;
        };
        let direct_matches: BTreeSet<_> = sessions
            .iter()
            .filter(|session| evidence.matches_session(session))
            .map(|session| session.id.clone())
            .collect();
        if direct_matches.is_empty() {
            continue;
        }

        let child_matches: BTreeSet<_> = sessions
            .iter()
            .filter(|session| {
                parent_by_child
                    .get(&session.id)
                    .is_some_and(|parents| !parents.is_disjoint(&direct_matches))
            })
            .map(|session| session.id.clone())
            .collect();

        if child_matches.is_empty() {
            active.insert(
                mux.id.clone(),
                ActiveMuxSessionMatch {
                    sessions: direct_matches,
                    evidence: evidence.link_evidence,
                },
            );
        } else {
            let direct_recent = most_recent_epoch(&direct_matches, sessions);
            let child_recent = most_recent_epoch(&child_matches, sessions);
            let sessions = if direct_recent >= child_recent {
                direct_matches
            } else {
                child_matches
            };
            active.insert(
                mux.id.clone(),
                ActiveMuxSessionMatch {
                    sessions,
                    evidence: evidence.link_evidence,
                },
            );
        }
    }

    active
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

struct ActiveMuxSessionMatch {
    sessions: BTreeSet<crate::model::AgentSessionId>,
    evidence: &'static str,
}

impl ActiveMuxSessionMatch {
    fn contains(&self, session: &crate::model::AgentSessionId) -> bool {
        self.sessions.contains(session)
    }
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
