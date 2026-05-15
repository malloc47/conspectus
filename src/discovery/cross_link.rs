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
//!
//! No nodes are created here, and any `Unresolved` lineage endpoints already
//! present in `candidate_links` are left untouched.

use std::collections::HashMap;

use crate::model::{
    AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, MuxSessionNode, NodeId, Provenance, RelationKind, SourceMetadata,
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
    }

    snapshot.candidate_links.extend(new_links);
    snapshot.canonicalize();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, ForkId, ForkNode, GraphSnapshot, MuxSessionId,
        MuxSessionNode, UnresolvedEndpoint,
    };

    fn session(id: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", id),
            harness_key: "codex".to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
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
