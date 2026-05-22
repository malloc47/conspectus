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

use std::collections::HashMap;

use crate::model::{
    AgentSessionNode, CheckoutId, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance, RelationKind, SourceMetadata,
};

const ADAPTER_NAME: &str = "cross_link";

pub fn infer(snapshot: &mut GraphSnapshot) {
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

    let mut new_links = Vec::new();

    for session in &agent_sessions {
        let Some(session_cwd) = session.cwd.as_deref().map(normalize_path) else {
            continue;
        };

        for mux in &mux_sessions {
            let Some(mux_cwd) = mux.cwd.as_deref().map(normalize_path) else {
                continue;
            };
            if let Some(link) = mux_match(session, mux, &session_cwd, &mux_cwd) {
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
    snapshot.canonicalize();
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
) -> Option<GraphLink> {
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
        })
    }

    fn mux(native: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: cwd.map(str::to_string),
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
}
