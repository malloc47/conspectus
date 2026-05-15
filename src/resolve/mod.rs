//! Candidate-link resolution boundaries.

use std::collections::BTreeMap;

use crate::model::{Diagnostic, GraphLink, GraphSnapshot, LinkState, NodeId, ResolvedRelationship};

pub fn resolve_snapshot(mut snapshot: GraphSnapshot) -> GraphSnapshot {
    let output = resolve_links(&snapshot.candidate_links);
    snapshot.resolved_relationships = output.resolved_relationships;
    snapshot.diagnostics = output.diagnostics;
    snapshot.canonicalize();
    snapshot
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolveOutput {
    pub resolved_relationships: Vec<ResolvedRelationship>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn resolve_links(candidates: &[GraphLink]) -> ResolveOutput {
    let mut output = ResolveOutput::default();
    let mut concrete: BTreeMap<(NodeId, crate::model::RelationKind), Vec<&GraphLink>> =
        BTreeMap::new();

    for link in candidates {
        if link.state.is_ignored() {
            continue;
        }

        if link.target_node_id().is_none() {
            output.diagnostics.push(Diagnostic::UnresolvedEndpoint {
                link_id: link.id.clone(),
                relation: link.relation.clone(),
            });
            continue;
        }

        if matches!(link.state, LinkState::Overridden { .. }) {
            continue;
        }

        concrete
            .entry((link.source.clone(), link.relation.clone()))
            .or_default()
            .push(link);
    }

    for ((source, relation), mut links) in concrete {
        links.sort_by(|left, right| compare_candidates(left, right));
        let selected = links[0];
        let target = selected
            .target_node_id()
            .expect("concrete link groups have node targets")
            .clone();
        let competing_link_ids: Vec<String> =
            links.iter().skip(1).map(|link| link.id.clone()).collect();

        if !competing_link_ids.is_empty() {
            output.diagnostics.push(Diagnostic::Conflict {
                source: source.clone(),
                relation: relation.clone(),
                selected_link_id: selected.id.clone(),
                competing_link_ids: competing_link_ids.clone(),
            });
        }

        output.resolved_relationships.push(ResolvedRelationship {
            source,
            target,
            relation,
            selected_link_id: selected.id.clone(),
            competing_link_ids,
        });
    }

    output.resolved_relationships.sort();
    output.diagnostics.sort();
    output
}

fn compare_candidates(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    right
        .provenance
        .precedence()
        .cmp(&left.provenance.precedence())
        .then_with(|| right.confidence.cmp(&left.confidence))
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use crate::model::{
        AgentSessionId, Confidence, ForkId, GraphLink, LinkEndpoint, LinkState, MuxSessionId,
        NodeId, Provenance, RelationKind, UnresolvedEndpoint,
    };

    use super::*;

    fn session(id: &str) -> NodeId {
        NodeId::AgentSession(AgentSessionId::new("codex", "global", id))
    }

    fn mux(id: &str) -> NodeId {
        NodeId::MuxSession(MuxSessionId::new(id))
    }

    #[test]
    fn empty_candidates_resolve_to_empty_output() {
        assert_eq!(resolve_links(&[]), ResolveOutput::default());
    }

    #[test]
    fn unresolved_endpoint_becomes_diagnostic_not_relationship() {
        let link = GraphLink::new(
            "child-unresolved",
            NodeId::Fork(ForkId::new("atelier/fork-1")),
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some("codex".to_string()),
                    native_id: Some("child".to_string()),
                    state_scope: None,
                    path: None,
                    metadata: Default::default(),
                },
            },
            RelationKind::ChildSession,
            Provenance::StrongDiscovered,
        );

        let output = resolve_links(&[link]);

        assert!(output.resolved_relationships.is_empty());
        assert!(matches!(
            output.diagnostics.as_slice(),
            [Diagnostic::UnresolvedEndpoint { link_id, .. }] if link_id == "child-unresolved"
        ));
    }

    #[test]
    fn local_declared_wins_over_discovered_without_deleting_evidence() {
        let discovered = GraphLink::new(
            "discovered",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:1") },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );
        let declared = GraphLink::new(
            "declared",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:2") },
            RelationKind::LinkedToMux,
            Provenance::LocalDeclared,
        );
        let output = resolve_links(&[discovered, declared]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "declared"
        );
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["discovered"]
        );
    }

    #[test]
    fn strong_discovered_wins_over_cached() {
        let mut strong = GraphLink::new(
            "strong",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:1") },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );
        strong.confidence = Confidence::High;
        let cached = GraphLink::new(
            "cached",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:2") },
            RelationKind::LinkedToMux,
            Provenance::Cached,
        );

        let output = resolve_links(&[cached, strong]);

        assert_eq!(output.resolved_relationships[0].selected_link_id, "strong");
    }

    #[test]
    fn ignored_and_overridden_candidates_do_not_resolve() {
        let mut ignored = GraphLink::new(
            "ignored",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:1") },
            RelationKind::LinkedToMux,
            Provenance::LocalDeclared,
        );
        ignored.state = LinkState::Ignored { reason: None };
        let mut overridden = GraphLink::new(
            "overridden",
            session("a"),
            LinkEndpoint::Node { id: mux("tmux:2") },
            RelationKind::LinkedToMux,
            Provenance::GlobalDeclared,
        );
        overridden.state = LinkState::Overridden {
            by: "replacement".to_string(),
            reason: None,
        };

        let output = resolve_links(&[ignored, overridden]);

        assert!(output.resolved_relationships.is_empty());
        assert!(output.diagnostics.is_empty());
    }
}
