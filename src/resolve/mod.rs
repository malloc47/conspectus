//! Candidate-link resolution boundaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    Confidence, Diagnostic, GraphLink, GraphSnapshot, LinkState, NodeId, Provenance, RelationKind,
    ResolvedRelationship,
};

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
    let mut concrete: BTreeMap<
        (NodeId, crate::model::RelationKind, Option<NodeId>),
        Vec<&GraphLink>,
    > = BTreeMap::new();

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

        let target = link
            .target_node_id()
            .expect("concrete links have node targets")
            .clone();
        let target_key = multi_target_relation(&link.relation).then_some(target);
        concrete
            .entry((link.source.clone(), link.relation.clone(), target_key))
            .or_default()
            .push(link);
    }

    for ((source, relation, _), mut links) in concrete {
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

    suppress_ambiguous_cwd_mux_links(candidates, &mut output);

    output.resolved_relationships.sort();
    output.diagnostics.sort();
    output
}

fn suppress_ambiguous_cwd_mux_links(candidates: &[GraphLink], output: &mut ResolveOutput) {
    let link_by_id: BTreeMap<&str, &GraphLink> = candidates
        .iter()
        .map(|link| (link.id.as_str(), link))
        .collect();

    let mut mux_all_keys: BTreeMap<&NodeId, BTreeSet<String>> = BTreeMap::new();

    for link in candidates {
        if link.state.is_ignored() || matches!(link.state, LinkState::Overridden { .. }) {
            continue;
        }
        if link.relation != RelationKind::LinkedToMux {
            continue;
        }
        if let Some(target) = link.target_node_id()
            && let Some(key) = session_logical_key(&link.source)
        {
            mux_all_keys.entry(target).or_default().insert(key);
        }
    }

    let mut suppressed: BTreeSet<String> = BTreeSet::new();

    for rel in &output.resolved_relationships {
        if rel.relation != RelationKind::LinkedToMux {
            continue;
        }
        let Some(link) = link_by_id.get(rel.selected_link_id.as_str()) else {
            continue;
        };
        if !is_cwd_evidence(link) {
            continue;
        }
        let distinct_session_count = mux_all_keys
            .get(&rel.target)
            .map(|keys| keys.len())
            .unwrap_or(0);
        if distinct_session_count > 1 {
            suppressed.insert(rel.selected_link_id.clone());
            output.diagnostics.push(Diagnostic::Conflict {
                source: rel.source.clone(),
                relation: rel.relation.clone(),
                selected_link_id: rel.selected_link_id.clone(),
                competing_link_ids: vec![],
            });
        }
    }

    output.resolved_relationships.retain(|rel| {
        rel.relation != RelationKind::LinkedToMux || !suppressed.contains(&rel.selected_link_id)
    });
}

fn session_logical_key(source: &NodeId) -> Option<String> {
    match source {
        NodeId::AgentSession(id) => Some(format!("{}:{}", id.harness_key, id.session_key)),
        _ => None,
    }
}

fn is_cwd_evidence(link: &GraphLink) -> bool {
    let match_kind = link
        .source_metadata
        .fields
        .get("match_kind")
        .and_then(|v| v.as_str())
        .or(link.source_metadata.evidence.as_deref());
    matches!(match_kind, Some("exact_cwd_match" | "cwd_prefix_match"))
}

fn multi_target_relation(relation: &RelationKind) -> bool {
    matches!(
        relation,
        RelationKind::AssociatedWith | RelationKind::WorkspaceContainsRepo
    )
}

fn compare_candidates(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    match left.relation {
        RelationKind::LinkedToMux => compare_session_mux(left, right),
        RelationKind::BranchHasForgePr => compare_branch_pr(left, right),
        _ => compare_generic(left, right),
    }
}

fn compare_generic(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    right
        .provenance
        .precedence()
        .cmp(&left.provenance.precedence())
        .then_with(|| right.confidence.cmp(&left.confidence))
        .then_with(|| left.id.cmp(&right.id))
}

/// Session ↔ mux ordering per ADR 0006: declared → strong evidence → exact
/// cwd/root match → naming convention → recency tie-breaker.
fn compare_session_mux(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    let l = mux_score(left);
    let r = mux_score(right);

    r.tier
        .cmp(&l.tier)
        .then_with(|| r.evidence_rank.cmp(&l.evidence_rank))
        .then_with(|| r.confidence.cmp(&l.confidence))
        .then_with(|| r.activity_epoch.cmp(&l.activity_epoch))
        .then_with(|| left.id.cmp(&right.id))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct MuxScore {
    tier: MuxTier,
    evidence_rank: u8,
    confidence: Confidence,
    activity_epoch: i64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum MuxTier {
    Cached = 0,
    Convention = 1,
    Discovered = 2,
    StrongDiscovered = 3,
    GlobalDeclared = 4,
    LocalDeclared = 5,
}

fn mux_score(link: &GraphLink) -> MuxScore {
    let match_kind = link
        .source_metadata
        .fields
        .get("match_kind")
        .and_then(serde_json::Value::as_str)
        .or(link.source_metadata.evidence.as_deref());

    MuxScore {
        tier: mux_tier(link.provenance),
        evidence_rank: mux_evidence_rank(match_kind),
        confidence: link.confidence,
        activity_epoch: link
            .source_metadata
            .fields
            .get("mux_activity_epoch")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i64::MIN),
    }
}

fn mux_evidence_rank(match_kind: Option<&str>) -> u8 {
    match match_kind {
        Some(
            "control_plane_current_session_match"
            | "hook_session_match"
            | "hook_session_path_match"
            | "active_pane_fd_session_match",
        ) => 50,
        Some("active_pane_fd_command_session_match") => 45,
        Some("session_file_activity_match" | "harness_state_current_session_match") => 40,
        Some("active_pane_process_match") => 35,
        Some("active_pane_command_session_match") => 30,
        Some("exact_cwd_match") => 20,
        Some("cwd_prefix_match") => 10,
        _ => 0,
    }
}

fn mux_tier(provenance: Provenance) -> MuxTier {
    match provenance {
        Provenance::LocalDeclared => MuxTier::LocalDeclared,
        Provenance::GlobalDeclared => MuxTier::GlobalDeclared,
        Provenance::StrongDiscovered => MuxTier::StrongDiscovered,
        Provenance::Discovered => MuxTier::Discovered,
        Provenance::Convention => MuxTier::Convention,
        Provenance::Cached => MuxTier::Cached,
    }
}

/// Branch ↔ pull-request ordering: declared links win first, then
/// open (non-draft) state, with closed/merged and draft demoted to
/// tie-breakers and finally `updated_epoch` recency.
///
/// Reads `state` / `is_draft` / `updated_epoch` from
/// [`GraphLink::source_metadata`] fields populated by the GitHub
/// forge adapter.
fn compare_branch_pr(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    let l = pr_score(left);
    let r = pr_score(right);

    r.provenance_tier
        .cmp(&l.provenance_tier)
        .then_with(|| r.state_rank.cmp(&l.state_rank))
        .then_with(|| l.is_draft.cmp(&r.is_draft))
        .then_with(|| r.updated_epoch.cmp(&l.updated_epoch))
        .then_with(|| r.confidence.cmp(&l.confidence))
        .then_with(|| left.id.cmp(&right.id))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PrScore {
    provenance_tier: PrProvenanceTier,
    state_rank: PrStateRank,
    is_draft: bool,
    updated_epoch: i64,
    confidence: Confidence,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum PrProvenanceTier {
    Cached = 0,
    Convention = 1,
    Discovered = 2,
    StrongDiscovered = 3,
    GlobalDeclared = 4,
    LocalDeclared = 5,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum PrStateRank {
    Other = 0,
    Closed = 1,
    Merged = 2,
    Open = 3,
}

fn pr_score(link: &GraphLink) -> PrScore {
    PrScore {
        provenance_tier: pr_provenance_tier(link.provenance),
        state_rank: pr_state_rank(
            link.source_metadata
                .fields
                .get("state")
                .and_then(serde_json::Value::as_str),
        ),
        is_draft: link
            .source_metadata
            .fields
            .get("is_draft")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        updated_epoch: link
            .source_metadata
            .fields
            .get("updated_epoch")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i64::MIN),
        confidence: link.confidence,
    }
}

fn pr_provenance_tier(provenance: Provenance) -> PrProvenanceTier {
    match provenance {
        Provenance::LocalDeclared => PrProvenanceTier::LocalDeclared,
        Provenance::GlobalDeclared => PrProvenanceTier::GlobalDeclared,
        Provenance::StrongDiscovered => PrProvenanceTier::StrongDiscovered,
        Provenance::Discovered => PrProvenanceTier::Discovered,
        Provenance::Convention => PrProvenanceTier::Convention,
        Provenance::Cached => PrProvenanceTier::Cached,
    }
}

fn pr_state_rank(raw: Option<&str>) -> PrStateRank {
    match raw.map(str::to_ascii_lowercase).as_deref() {
        Some("open") => PrStateRank::Open,
        Some("merged") => PrStateRank::Merged,
        Some("closed") => PrStateRank::Closed,
        _ => PrStateRank::Other,
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{
        AgentSessionId, BranchId, CheckoutId, Confidence, ForgePrId, ForkId, GraphLink,
        LinkEndpoint, LinkState, MuxSessionId, NodeId, Provenance, RelationKind, RepoId,
        UnresolvedEndpoint, WorkspaceId,
    };

    use super::*;

    fn session(id: &str) -> NodeId {
        NodeId::AgentSession(AgentSessionId::new("codex", "global", id))
    }

    fn mux(id: &str) -> NodeId {
        NodeId::MuxSession(MuxSessionId::new(id))
    }

    fn workspace(id: &str) -> NodeId {
        NodeId::Workspace(WorkspaceId::new(id))
    }

    fn checkout(root: &str) -> NodeId {
        NodeId::Checkout(CheckoutId::new(RepoId::new("/repo/.git"), root))
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
    fn multi_target_relations_resolve_each_distinct_target() {
        let session = session("a");
        let workspace_link = GraphLink::new(
            "workspace",
            session.clone(),
            LinkEndpoint::Node {
                id: workspace("/workspace"),
            },
            RelationKind::AssociatedWith,
            Provenance::Discovered,
        );
        let checkout_link = GraphLink::new(
            "checkout",
            session.clone(),
            LinkEndpoint::Node {
                id: checkout("/workspace/repo"),
            },
            RelationKind::AssociatedWith,
            Provenance::Discovered,
        );

        let output = resolve_links(&[workspace_link, checkout_link]);

        assert_eq!(output.resolved_relationships.len(), 2);
        assert!(output.diagnostics.is_empty());
        assert!(
            output
                .resolved_relationships
                .iter()
                .any(|relationship| relationship.target == workspace("/workspace"))
        );
        assert!(
            output
                .resolved_relationships
                .iter()
                .any(|relationship| relationship.target == checkout("/workspace/repo"))
        );
    }

    #[test]
    fn multi_target_relations_still_compete_for_same_target() {
        let session = session("a");
        let lower = GraphLink::new(
            "lower",
            session.clone(),
            LinkEndpoint::Node {
                id: workspace("/workspace"),
            },
            RelationKind::AssociatedWith,
            Provenance::Discovered,
        );
        let higher = GraphLink::new(
            "higher",
            session,
            LinkEndpoint::Node {
                id: workspace("/workspace"),
            },
            RelationKind::AssociatedWith,
            Provenance::LocalDeclared,
        );

        let output = resolve_links(&[lower, higher]);

        assert_eq!(output.resolved_relationships.len(), 1);
        assert_eq!(output.resolved_relationships[0].selected_link_id, "higher");
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["lower".to_string()]
        );
    }

    fn linked_to_mux_link(
        id: &str,
        source: NodeId,
        target: NodeId,
        provenance: Provenance,
        confidence: Confidence,
        activity_epoch: Option<i64>,
        match_kind: Option<&str>,
    ) -> GraphLink {
        let mut link = GraphLink::new(
            id,
            source,
            LinkEndpoint::Node { id: target },
            RelationKind::LinkedToMux,
            provenance,
        );
        link.confidence = confidence;

        if let Some(match_kind) = match_kind {
            link.source_metadata.evidence = Some(match_kind.to_string());
            link.source_metadata.fields.insert(
                "match_kind".to_string(),
                serde_json::Value::String(match_kind.to_string()),
            );
        }

        if let Some(epoch) = activity_epoch {
            link.source_metadata.fields.insert(
                "mux_activity_epoch".to_string(),
                serde_json::Value::Number(epoch.into()),
            );
        }

        link
    }

    #[test]
    fn session_mux_resolver_picks_local_declared_over_lower_tiers() {
        let strong = linked_to_mux_link(
            "strong",
            session("a"),
            mux("tmux:strong"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            None,
        );
        let declared = linked_to_mux_link(
            "declared",
            session("a"),
            mux("tmux:declared"),
            Provenance::LocalDeclared,
            Confidence::Low,
            None,
            None,
        );
        let convention = linked_to_mux_link(
            "convention",
            session("a"),
            mux("tmux:convention"),
            Provenance::Convention,
            Confidence::High,
            Some(9_999),
            None,
        );

        let output = resolve_links(&[convention, strong, declared]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "declared"
        );
    }

    #[test]
    fn session_mux_resolver_prefers_exact_cwd_over_naming_convention() {
        let discovered_exact = linked_to_mux_link(
            "exact",
            session("a"),
            mux("tmux:exact"),
            Provenance::Discovered,
            Confidence::Medium,
            None,
            Some("exact_cwd_match"),
        );
        let convention = linked_to_mux_link(
            "convention",
            session("a"),
            mux("tmux:convention"),
            Provenance::Convention,
            Confidence::High,
            Some(9_999),
            None,
        );

        let output = resolve_links(&[convention, discovered_exact]);

        assert_eq!(output.resolved_relationships[0].selected_link_id, "exact");
    }

    #[test]
    fn session_mux_resolver_breaks_ties_on_activity_recency() {
        let older = linked_to_mux_link(
            "older",
            session("a"),
            mux("tmux:older"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(1_000),
            None,
        );
        let newer = linked_to_mux_link(
            "newer",
            session("a"),
            mux("tmux:newer"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(5_000),
            None,
        );

        let output = resolve_links(&[older, newer]);

        assert_eq!(output.resolved_relationships[0].selected_link_id, "newer");
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["older".to_string()]
        );
    }

    #[test]
    fn session_mux_resolver_prefers_current_session_evidence_over_launch_argv() {
        let launch_argv = linked_to_mux_link(
            "launch-argv",
            session("a"),
            mux("tmux:stale"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(5_000),
            Some("active_pane_command_session_match"),
        );
        let current_session = linked_to_mux_link(
            "current-session",
            session("a"),
            mux("tmux:current"),
            Provenance::StrongDiscovered,
            Confidence::Medium,
            Some(1_000),
            Some("hook_session_match"),
        );

        let output = resolve_links(&[launch_argv, current_session]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "current-session"
        );
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["launch-argv".to_string()]
        );
    }

    #[test]
    fn session_mux_resolver_keeps_launch_argv_usable_without_current_evidence() {
        let launch_argv = linked_to_mux_link(
            "launch-argv",
            session("a"),
            mux("tmux:launch"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("active_pane_command_session_match"),
        );
        let cwd = linked_to_mux_link(
            "cwd",
            session("a"),
            mux("tmux:cwd"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(5_000),
            Some("exact_cwd_match"),
        );

        let output = resolve_links(&[cwd, launch_argv]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "launch-argv"
        );
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["cwd".to_string()]
        );
    }

    #[test]
    fn session_mux_resolver_prefers_process_match_over_launch_argv() {
        let launch_argv = linked_to_mux_link(
            "launch-argv",
            session("a"),
            mux("tmux:launch"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(5_000),
            Some("active_pane_command_session_match"),
        );
        let process = linked_to_mux_link(
            "process",
            session("a"),
            mux("tmux:process"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("active_pane_process_match"),
        );

        let output = resolve_links(&[launch_argv, process]);

        assert_eq!(output.resolved_relationships[0].selected_link_id, "process");
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["launch-argv".to_string()]
        );
    }

    #[test]
    fn session_mux_resolver_emits_ambiguity_diagnostic_for_multiple_candidates() {
        let one = linked_to_mux_link(
            "one",
            session("a"),
            mux("tmux:one"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(2_000),
            None,
        );
        let two = linked_to_mux_link(
            "two",
            session("a"),
            mux("tmux:two"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(3_000),
            None,
        );

        let output = resolve_links(&[one, two]);

        let conflict = output
            .diagnostics
            .iter()
            .find(|d| matches!(d, Diagnostic::Conflict { .. }))
            .expect("conflict diagnostic");
        match conflict {
            Diagnostic::Conflict {
                selected_link_id,
                competing_link_ids,
                ..
            } => {
                assert_eq!(selected_link_id, "two");
                assert_eq!(competing_link_ids, &vec!["one".to_string()]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn session_mux_resolver_skips_ignored_and_overridden_candidates() {
        let mut ignored = linked_to_mux_link(
            "ignored",
            session("a"),
            mux("tmux:ignored"),
            Provenance::LocalDeclared,
            Confidence::High,
            Some(9_999),
            None,
        );
        ignored.state = LinkState::Ignored { reason: None };
        let mut overridden = linked_to_mux_link(
            "overridden",
            session("a"),
            mux("tmux:overridden"),
            Provenance::GlobalDeclared,
            Confidence::High,
            Some(9_999),
            None,
        );
        overridden.state = LinkState::Overridden {
            by: "winner".to_string(),
            reason: None,
        };
        let active = linked_to_mux_link(
            "active",
            session("a"),
            mux("tmux:active"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(1_000),
            None,
        );

        let output = resolve_links(&[ignored, overridden, active]);

        assert_eq!(output.resolved_relationships.len(), 1);
        assert_eq!(output.resolved_relationships[0].selected_link_id, "active");
    }

    fn forge_pr(number: u64) -> NodeId {
        NodeId::ForgePr(ForgePrId::new(
            "github",
            "github.com",
            "octo",
            "repo",
            number,
        ))
    }

    fn branch_node(refname: &str) -> NodeId {
        NodeId::Branch(BranchId::new(
            RepoId::new("/workspace/repo/.git"),
            refname.to_string(),
        ))
    }

    fn branch_pr_link(
        id: &str,
        source: NodeId,
        target: NodeId,
        provenance: Provenance,
        state: &str,
        is_draft: bool,
        updated_epoch: Option<i64>,
    ) -> GraphLink {
        let mut link = GraphLink::new(
            id,
            source,
            LinkEndpoint::Node { id: target },
            RelationKind::BranchHasForgePr,
            provenance,
        );
        link.source_metadata.fields.insert(
            "state".to_string(),
            serde_json::Value::String(state.to_string()),
        );
        link.source_metadata
            .fields
            .insert("is_draft".to_string(), serde_json::Value::Bool(is_draft));
        if let Some(epoch) = updated_epoch {
            link.source_metadata.fields.insert(
                "updated_epoch".to_string(),
                serde_json::Value::Number(epoch.into()),
            );
        }
        link
    }

    #[test]
    fn branch_pr_resolver_picks_open_non_draft_over_merged() {
        let merged = branch_pr_link(
            "merged",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "merged",
            false,
            Some(5_000),
        );
        let open = branch_pr_link(
            "open",
            forge_pr(2),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(1_000),
        );

        let output = resolve_links(&[merged, open]);

        // Two distinct sources, so each resolves independently; the key
        // assertion is that when the same source has multiple candidates
        // (next test) the open one wins.
        assert_eq!(output.resolved_relationships.len(), 2);
    }

    #[test]
    fn branch_pr_resolver_demotes_draft_among_open_candidates() {
        let draft = branch_pr_link(
            "draft",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            true,
            Some(9_999),
        );
        let mut ready = branch_pr_link(
            "ready",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(1_000),
        );
        // Force a different ID-stable target so both share the same source
        // (a branch may technically only have one PR per number, but the
        // resolver groups by source so we use the same source).
        ready.target = LinkEndpoint::Node {
            id: branch_node("refs/heads/feature"),
        };

        let output = resolve_links(&[draft, ready]);

        let selected = output
            .resolved_relationships
            .iter()
            .find(|r| r.source == forge_pr(1))
            .expect("relationship");
        assert_eq!(selected.selected_link_id, "ready");
        assert_eq!(selected.competing_link_ids, vec!["draft".to_string()]);
    }

    #[test]
    fn branch_pr_resolver_prefers_most_recent_among_same_state() {
        let older = branch_pr_link(
            "older",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(1_000),
        );
        let newer = branch_pr_link(
            "newer",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(5_000),
        );

        let output = resolve_links(&[older, newer]);

        let selected = &output.resolved_relationships[0];
        assert_eq!(selected.selected_link_id, "newer");
        assert_eq!(selected.competing_link_ids, vec!["older".to_string()]);
    }

    #[test]
    fn branch_pr_resolver_open_beats_merged_for_same_source() {
        let merged_recent = branch_pr_link(
            "merged-recent",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "merged",
            false,
            Some(9_999),
        );
        let open_old = branch_pr_link(
            "open-old",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(100),
        );

        let output = resolve_links(&[merged_recent, open_old]);

        let selected = &output.resolved_relationships[0];
        assert_eq!(selected.selected_link_id, "open-old");
        assert!(
            selected
                .competing_link_ids
                .contains(&"merged-recent".to_string())
        );
    }

    #[test]
    fn branch_pr_resolver_open_beats_closed_for_same_source() {
        let closed_recent = branch_pr_link(
            "closed-recent",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "closed",
            false,
            Some(9_999),
        );
        let open_old = branch_pr_link(
            "open-old",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(100),
        );

        let output = resolve_links(&[closed_recent, open_old]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "open-old"
        );
    }

    #[test]
    fn branch_pr_resolver_declared_overrides_state_and_recency() {
        let recent_open = branch_pr_link(
            "recent-open",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(9_999),
        );
        let declared_closed = branch_pr_link(
            "declared-closed",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::LocalDeclared,
            "closed",
            false,
            Some(1),
        );

        let output = resolve_links(&[recent_open, declared_closed]);

        let selected = &output.resolved_relationships[0];
        assert_eq!(selected.selected_link_id, "declared-closed");
    }

    #[test]
    fn branch_pr_resolver_emits_conflict_diagnostic_for_multiple_candidates() {
        let one = branch_pr_link(
            "one",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(2_000),
        );
        let two = branch_pr_link(
            "two",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::StrongDiscovered,
            "open",
            false,
            Some(3_000),
        );

        let output = resolve_links(&[one, two]);

        let conflict = output
            .diagnostics
            .iter()
            .find_map(|d| match d {
                Diagnostic::Conflict {
                    selected_link_id,
                    competing_link_ids,
                    ..
                } => Some((selected_link_id.clone(), competing_link_ids.clone())),
                _ => None,
            })
            .expect("conflict diagnostic");
        assert_eq!(conflict.0, "two");
        assert_eq!(conflict.1, vec!["one".to_string()]);
    }

    #[test]
    fn branch_pr_resolver_skips_ignored_and_overridden_candidates() {
        let mut ignored = branch_pr_link(
            "ignored",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::LocalDeclared,
            "open",
            false,
            Some(9_999),
        );
        ignored.state = LinkState::Ignored { reason: None };
        let mut overridden = branch_pr_link(
            "overridden",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::GlobalDeclared,
            "open",
            false,
            Some(9_999),
        );
        overridden.state = LinkState::Overridden {
            by: "winner".to_string(),
            reason: None,
        };
        let active = branch_pr_link(
            "active",
            forge_pr(1),
            branch_node("refs/heads/feature"),
            Provenance::Discovered,
            "open",
            false,
            Some(1_000),
        );

        let output = resolve_links(&[ignored, overridden, active]);

        assert_eq!(output.resolved_relationships.len(), 1);
        assert_eq!(output.resolved_relationships[0].selected_link_id, "active");
    }

    #[test]
    fn branch_pr_resolver_handles_zero_candidates() {
        let output = resolve_links(&[]);
        assert!(output.resolved_relationships.is_empty());
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

    #[test]
    fn suppresses_cwd_link_when_multiple_sessions_resolve_to_same_mux() {
        let cwd_a = linked_to_mux_link(
            "cwd-a",
            session("a"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("exact_cwd_match"),
        );
        let cwd_b = linked_to_mux_link(
            "cwd-b",
            session("b"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            Some("exact_cwd_match"),
        );

        let output = resolve_links(&[cwd_a, cwd_b]);

        assert!(output.resolved_relationships.is_empty());
        assert_eq!(output.diagnostics.len(), 2);
    }

    #[test]
    fn allows_cwd_link_when_single_session_to_single_mux() {
        let cwd = linked_to_mux_link(
            "cwd",
            session("a"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("exact_cwd_match"),
        );

        let output = resolve_links(&[cwd]);

        assert_eq!(output.resolved_relationships.len(), 1);
        assert_eq!(output.resolved_relationships[0].selected_link_id, "cwd");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn suppresses_cwd_link_when_stronger_evidence_exists_for_same_mux() {
        let cwd_b = linked_to_mux_link(
            "cwd-b",
            session("b"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("exact_cwd_match"),
        );
        let fd_a = linked_to_mux_link(
            "fd-a",
            session("a"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            Some("active_pane_fd_session_match"),
        );

        let output = resolve_links(&[cwd_b, fd_a]);

        assert_eq!(output.resolved_relationships.len(), 1);
        assert_eq!(output.resolved_relationships[0].selected_link_id, "fd-a");
        assert_eq!(output.diagnostics.len(), 1);
    }

    #[test]
    fn allows_cwd_links_to_different_muxes() {
        let cwd_a = linked_to_mux_link(
            "cwd-a",
            session("a"),
            mux("tmux:one"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("exact_cwd_match"),
        );
        let cwd_b = linked_to_mux_link(
            "cwd-b",
            session("b"),
            mux("tmux:two"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            Some("exact_cwd_match"),
        );

        let output = resolve_links(&[cwd_a, cwd_b]);

        assert_eq!(output.resolved_relationships.len(), 2);
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn suppresses_cwd_prefix_match_like_exact_cwd_match() {
        let prefix_a = linked_to_mux_link(
            "prefix-a",
            session("a"),
            mux("tmux:one"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(1_000),
            Some("cwd_prefix_match"),
        );
        let prefix_b = linked_to_mux_link(
            "prefix-b",
            session("b"),
            mux("tmux:one"),
            Provenance::Discovered,
            Confidence::Medium,
            Some(2_000),
            Some("cwd_prefix_match"),
        );

        let output = resolve_links(&[prefix_a, prefix_b]);

        assert!(output.resolved_relationships.is_empty());
        assert_eq!(output.diagnostics.len(), 2);
    }
}
