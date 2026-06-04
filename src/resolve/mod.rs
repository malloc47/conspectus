//! Candidate-link resolution boundaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    Confidence, Diagnostic, Freshness, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, NodeId, Provenance, RelationKind, ResolvedRelationship, RuntimeProcessRole,
    SourceMetadata,
};

pub fn resolve_snapshot(mut snapshot: GraphSnapshot) -> GraphSnapshot {
    let process_links = derive_process_mux_links(&snapshot);
    append_unique_links(&mut snapshot.candidate_links, process_links);
    let output = resolve_links(&snapshot.candidate_links);
    snapshot.resolved_relationships = output.resolved_relationships;
    snapshot.diagnostics = output.diagnostics;
    snapshot.canonicalize();
    snapshot
}

fn append_unique_links(links: &mut Vec<GraphLink>, additions: Vec<GraphLink>) {
    for link in additions {
        if !links.iter().any(|existing| existing.id == link.id) {
            links.push(link);
        }
    }
}

fn derive_process_mux_links(snapshot: &GraphSnapshot) -> Vec<GraphLink> {
    let mut role_by_process = BTreeMap::new();
    for node in &snapshot.nodes {
        if let GraphNode::RuntimeProcess(process) = node {
            role_by_process.insert(
                NodeId::RuntimeProcess(process.id.clone()),
                process.role.unwrap_or(RuntimeProcessRole::Unknown),
            );
        }
    }

    let mut mux_links_by_process: BTreeMap<NodeId, Vec<&GraphLink>> = BTreeMap::new();
    let mut session_links_by_process: BTreeMap<NodeId, Vec<&GraphLink>> = BTreeMap::new();
    for link in &snapshot.candidate_links {
        if link.state.is_ignored() || matches!(link.state, LinkState::Overridden { .. }) {
            continue;
        }
        match link.relation {
            RelationKind::MuxContainsProcess => {
                if let Some(process) = link.target_node_id() {
                    mux_links_by_process
                        .entry(process.clone())
                        .or_default()
                        .push(link);
                }
            }
            RelationKind::ProcessIdentifiesSession | RelationKind::ProcessCandidatesSession => {
                session_links_by_process
                    .entry(link.source.clone())
                    .or_default()
                    .push(link);
            }
            _ => {}
        }
    }

    let candidate_counts_by_process: BTreeMap<NodeId, usize> = session_links_by_process
        .iter()
        .map(|(process, links)| {
            let count = links
                .iter()
                .filter(|link| link.relation == RelationKind::ProcessCandidatesSession)
                .filter(|link| matches!(link.target_node_id(), Some(NodeId::AgentSession(_))))
                .count();
            (process.clone(), count)
        })
        .collect();

    let preferred_identity_by_process = preferred_process_identity_by_process(
        session_links_by_process
            .iter()
            .map(|(process, links)| (process, links.as_slice())),
    );

    let mut human_processes_by_mux: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    for (process, mux_links) in &mux_links_by_process {
        if role_by_process.get(process) != Some(&RuntimeProcessRole::HumanAgent) {
            continue;
        }
        for mux_link in mux_links {
            human_processes_by_mux
                .entry(mux_link.source.clone())
                .or_default()
                .insert(process.clone());
        }
    }

    let mut derived = BTreeMap::new();
    for (process, session_links) in &session_links_by_process {
        let Some(mux_links) = mux_links_by_process.get(process) else {
            continue;
        };
        for session_link in session_links {
            if session_link.relation == RelationKind::ProcessIdentifiesSession
                && let Some(preferred) = preferred_identity_by_process.get(process)
                && session_link.target_node_id() != Some(preferred)
            {
                continue;
            }
            let Some(NodeId::AgentSession(_)) = session_link.target_node_id() else {
                continue;
            };
            if session_link.relation == RelationKind::ProcessCandidatesSession
                && candidate_counts_by_process
                    .get(process)
                    .copied()
                    .unwrap_or(0)
                    != 1
            {
                continue;
            }

            for mux_link in mux_links {
                let source = session_link
                    .target_node_id()
                    .expect("checked above")
                    .clone();
                let target = mux_link.source.clone();
                if !matches!(target, NodeId::MuxSession(_)) {
                    continue;
                }
                if has_compatible_session_mux_link(
                    &snapshot.candidate_links,
                    &source,
                    &target,
                    process,
                ) {
                    continue;
                }
                let link = process_mux_link(
                    source,
                    target,
                    process.clone(),
                    mux_link,
                    session_link,
                    human_processes_by_mux
                        .get(&mux_link.source)
                        .map(BTreeSet::len)
                        .unwrap_or(0),
                );
                if has_better_session_mux_link_for_target(&snapshot.candidate_links, &link) {
                    continue;
                }
                derived.insert(link.id.clone(), link);
            }
        }
    }

    derived.into_values().collect()
}

fn has_better_session_mux_link_for_target(links: &[GraphLink], derived: &GraphLink) -> bool {
    links.iter().any(|existing| {
        existing.relation == RelationKind::LinkedToMux
            && matches!(existing.state, LinkState::Active)
            && existing.target_node_id() == derived.target_node_id()
            && compare_session_mux(existing, derived).is_lt()
    })
}

fn preferred_process_identity_by_process<'a>(
    links_by_process: impl Iterator<Item = (&'a NodeId, &'a [&'a GraphLink])>,
) -> BTreeMap<NodeId, NodeId> {
    let mut preferred = BTreeMap::new();
    for (process, links) in links_by_process {
        let mut identifies: Vec<&GraphLink> = links
            .iter()
            .copied()
            .filter(|link| link.relation == RelationKind::ProcessIdentifiesSession)
            .filter(|link| matches!(link.target_node_id(), Some(NodeId::AgentSession(_))))
            .collect();
        if identifies.is_empty() {
            continue;
        }
        identifies.sort_by(|left, right| compare_process_identity_links(left, right));
        if let Some(target) = identifies[0].target_node_id() {
            preferred.insert(process.clone(), target.clone());
        }
    }
    preferred
}

fn has_compatible_session_mux_link(
    links: &[GraphLink],
    source: &NodeId,
    target: &NodeId,
    process: &NodeId,
) -> bool {
    links.iter().any(|link| {
        if link.relation != RelationKind::LinkedToMux
            || &link.source != source
            || link.target_node_id() != Some(target)
        {
            return false;
        }
        let match_kind = link
            .source_metadata
            .fields
            .get("match_kind")
            .and_then(serde_json::Value::as_str)
            .or(link.source_metadata.evidence.as_deref());
        let process_id = link
            .source_metadata
            .fields
            .get("runtime_process")
            .and_then(serde_json::Value::as_str);
        let process_id_matches = process_id.is_some_and(|value| value == process.to_string());
        matches!(
            match_kind,
            Some(
                "active_pane_process_match"
                    | "active_pane_fd_session_match"
                    | "active_pane_fd_command_session_match"
                    | "hook_session_match"
                    | "hook_session_path_match"
                    | "codex_log_thread_match"
                    | "runtime_process_identifies_session"
                    | "runtime_process_candidates_session"
            )
        ) || process_id_matches
    })
}

fn process_mux_link(
    source: NodeId,
    target: NodeId,
    process: NodeId,
    mux_link: &GraphLink,
    session_link: &GraphLink,
    human_process_count: usize,
) -> GraphLink {
    let identifies = session_link.relation == RelationKind::ProcessIdentifiesSession;
    let match_kind = if identifies {
        "runtime_process_identifies_session"
    } else {
        "runtime_process_candidates_session"
    };
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "match_kind".to_string(),
        serde_json::Value::String(match_kind.to_string()),
    );
    fields.insert(
        "runtime_process".to_string(),
        serde_json::Value::String(process.to_string()),
    );
    fields.insert(
        "mux_process_link_id".to_string(),
        serde_json::Value::String(mux_link.id.clone()),
    );
    fields.insert(
        "process_session_link_id".to_string(),
        serde_json::Value::String(session_link.id.clone()),
    );
    fields.insert(
        "human_process_count".to_string(),
        serde_json::Value::Number((human_process_count as u64).into()),
    );

    GraphLink {
        id: format!("resolve:{source}:linked_to_mux:{target}:via:{process}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: if identifies {
            Confidence::High
        } else {
            Confidence::Medium
        },
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "resolver".to_string(),
            evidence: Some(match_kind.to_string()),
            fields,
        },
        state: LinkState::Active,
    }
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
        RelationKind::AssociatedWith
            | RelationKind::WorkspaceContainsRepo
            | RelationKind::MuxContainsProcess
            | RelationKind::ProcessIdentifiesSession
            | RelationKind::ProcessCandidatesSession
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

fn compare_process_identity_links(left: &GraphLink, right: &GraphLink) -> std::cmp::Ordering {
    let l = process_identity_score(left);
    let r = process_identity_score(right);

    r.evidence_rank
        .cmp(&l.evidence_rank)
        .then_with(|| r.confidence.cmp(&l.confidence))
        .then_with(|| r.observed_epoch.cmp(&l.observed_epoch))
        .then_with(|| left.id.cmp(&right.id))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ProcessIdentityScore {
    evidence_rank: u8,
    confidence: Confidence,
    observed_epoch: i64,
}

fn process_identity_score(link: &GraphLink) -> ProcessIdentityScore {
    let match_kind = link
        .source_metadata
        .fields
        .get("match_kind")
        .and_then(serde_json::Value::as_str)
        .or(link.source_metadata.evidence.as_deref());

    ProcessIdentityScore {
        evidence_rank: process_identity_evidence_rank(match_kind),
        confidence: link.confidence,
        observed_epoch: link
            .source_metadata
            .fields
            .get("observed_epoch")
            .and_then(serde_json::Value::as_i64)
            .or_else(|| {
                link.source_metadata
                    .fields
                    .get("mux_activity_epoch")
                    .and_then(serde_json::Value::as_i64)
            })
            .unwrap_or(i64::MIN),
    }
}

fn process_identity_evidence_rank(match_kind: Option<&str>) -> u8 {
    match match_kind {
        Some("codex_log_process_thread_match" | "hook_process_session_match") => 60,
        Some("active_pane_fd_session_match" | "active_pane_fd_command_session_match") => 50,
        Some("active_pane_process_match") => 35,
        Some("active_pane_command_session_match") => 30,
        _ => 0,
    }
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
        Some(
            "active_pane_process_match"
            | "runtime_process_identifies_session"
            | "runtime_process_candidates_session",
        ) => 35,
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
        AgentSessionId, BranchId, CheckoutId, Confidence, ForgePrId, ForkId, GraphLink, GraphNode,
        LinkEndpoint, LinkState, MuxSessionId, NodeId, Provenance, RelationKind, RepoId,
        RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, UnresolvedEndpoint, WorkspaceId,
    };

    use super::*;

    fn session(id: &str) -> NodeId {
        NodeId::AgentSession(AgentSessionId::new("codex", "global", id))
    }

    fn mux(id: &str) -> NodeId {
        NodeId::MuxSession(MuxSessionId::new(id))
    }

    fn process(id: &str) -> NodeId {
        NodeId::RuntimeProcess(RuntimeProcessId::new(id))
    }

    fn process_node(id: &str, role: RuntimeProcessRole) -> GraphNode {
        GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: RuntimeProcessId::new(id),
            observation_key: id.to_string(),
            pid: None,
            parent_pid: None,
            root_pane_pid: None,
            command: None,
            cwd: None,
            harness_key: None,
            role: Some(role),
            depth: None,
            observed_epoch: None,
        })
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

    fn mux_contains_process_link(id: &str, mux: NodeId, process: NodeId) -> GraphLink {
        let mut link = GraphLink::new(
            id,
            mux,
            LinkEndpoint::Node { id: process },
            RelationKind::MuxContainsProcess,
            Provenance::StrongDiscovered,
        );
        link.confidence = Confidence::High;
        link
    }

    fn process_session_link(
        id: &str,
        process: NodeId,
        session: NodeId,
        relation: RelationKind,
    ) -> GraphLink {
        process_session_link_with_match_kind(id, process, session, relation, None, None)
    }

    fn process_session_link_with_match_kind(
        id: &str,
        process: NodeId,
        session: NodeId,
        relation: RelationKind,
        match_kind: Option<&str>,
        observed_epoch: Option<i64>,
    ) -> GraphLink {
        let mut link = GraphLink::new(
            id,
            process,
            LinkEndpoint::Node { id: session },
            relation,
            Provenance::StrongDiscovered,
        );
        link.confidence = Confidence::High;
        if let Some(match_kind) = match_kind {
            link.source_metadata.evidence = Some(match_kind.to_string());
            link.source_metadata.fields.insert(
                "match_kind".to_string(),
                serde_json::Value::String(match_kind.to_string()),
            );
        }
        if let Some(epoch) = observed_epoch {
            link.source_metadata.fields.insert(
                "observed_epoch".to_string(),
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
    fn resolve_snapshot_derives_session_mux_from_process_identity() {
        let session = session("a");
        let mux = mux("tmux:process");
        let process = process("proc:1");
        let snapshot = GraphSnapshot {
            nodes: vec![process_node("proc:1", RuntimeProcessRole::HumanAgent)],
            candidate_links: vec![
                mux_contains_process_link("mux-process", mux.clone(), process.clone()),
                process_session_link(
                    "process-session",
                    process,
                    session.clone(),
                    RelationKind::ProcessIdentifiesSession,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        let relation = resolved
            .resolved_relationships
            .iter()
            .find(|rel| rel.relation == RelationKind::LinkedToMux)
            .expect("derived session mux relationship");
        assert_eq!(relation.source, session);
        assert_eq!(relation.target, mux);
        assert!(
            relation
                .selected_link_id
                .starts_with("resolve:agent_session:codex:global:a:linked_to_mux:")
        );
        assert!(resolved.candidate_links.iter().any(|link| {
            link.id == relation.selected_link_id
                && link.source_metadata.evidence.as_deref()
                    == Some("runtime_process_identifies_session")
                && link
                    .source_metadata
                    .fields
                    .get("human_process_count")
                    .and_then(serde_json::Value::as_u64)
                    == Some(1)
        }));
    }

    #[test]
    fn resolve_snapshot_does_not_fan_out_ambiguous_process_candidates() {
        let process = process("proc:ambiguous");
        let snapshot = GraphSnapshot {
            nodes: vec![process_node(
                "proc:ambiguous",
                RuntimeProcessRole::HumanAgent,
            )],
            candidate_links: vec![
                mux_contains_process_link("mux-process", mux("tmux:process"), process.clone()),
                process_session_link(
                    "process-session-a",
                    process.clone(),
                    session("a"),
                    RelationKind::ProcessCandidatesSession,
                ),
                process_session_link(
                    "process-session-b",
                    process,
                    session("b"),
                    RelationKind::ProcessCandidatesSession,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        assert!(
            !resolved
                .resolved_relationships
                .iter()
                .any(|rel| rel.relation == RelationKind::LinkedToMux)
        );
    }

    #[test]
    fn process_unresolved_candidate_remains_a_diagnostic() {
        let link = GraphLink::new(
            "process-unresolved",
            process("proc:unresolved"),
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some("codex".to_string()),
                    native_id: None,
                    state_scope: None,
                    path: Some("/work/repo".to_string()),
                    metadata: Default::default(),
                },
            },
            RelationKind::ProcessCandidatesSession,
            Provenance::StrongDiscovered,
        );

        let output = resolve_links(&[link]);

        assert!(output.resolved_relationships.is_empty());
        assert!(matches!(
            output.diagnostics.as_slice(),
            [Diagnostic::UnresolvedEndpoint { link_id, relation }]
                if link_id == "process-unresolved"
                    && relation == &RelationKind::ProcessCandidatesSession
        ));
    }

    #[test]
    fn resolve_snapshot_counts_only_human_agent_runtime_processes() {
        let human = process("proc:human");
        let subagent = process("proc:subagent");
        let background = process("proc:background");
        let mux = mux("tmux:process");
        let snapshot = GraphSnapshot {
            nodes: vec![
                process_node("proc:human", RuntimeProcessRole::HumanAgent),
                process_node("proc:subagent", RuntimeProcessRole::Subagent),
                process_node("proc:background", RuntimeProcessRole::Background),
            ],
            candidate_links: vec![
                mux_contains_process_link("mux-human", mux.clone(), human.clone()),
                mux_contains_process_link("mux-subagent", mux.clone(), subagent),
                mux_contains_process_link("mux-background", mux, background),
                process_session_link(
                    "human-session",
                    human,
                    session("a"),
                    RelationKind::ProcessIdentifiesSession,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        let link = resolved
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref()
                        == Some("runtime_process_identifies_session")
            })
            .expect("derived process link");
        assert_eq!(
            link.source_metadata
                .fields
                .get("human_process_count")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn resolve_snapshot_process_identity_beats_stale_launch_argv() {
        let session = session("a");
        let process = process("proc:current");
        let stale = linked_to_mux_link(
            "launch-argv",
            session.clone(),
            mux("tmux:stale"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(1_000),
            Some("active_pane_command_session_match"),
        );
        let snapshot = GraphSnapshot {
            nodes: vec![process_node("proc:current", RuntimeProcessRole::HumanAgent)],
            candidate_links: vec![
                stale,
                mux_contains_process_link("mux-process", mux("tmux:current"), process.clone()),
                process_session_link(
                    "process-session",
                    process,
                    session,
                    RelationKind::ProcessIdentifiesSession,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        let relation = resolved
            .resolved_relationships
            .iter()
            .find(|rel| rel.relation == RelationKind::LinkedToMux)
            .expect("session mux relationship");
        assert_eq!(relation.target, mux("tmux:current"));
        assert_eq!(relation.competing_link_ids, vec!["launch-argv".to_string()]);
    }

    #[test]
    fn resolve_snapshot_does_not_derive_mux_link_from_stale_process_identity() {
        let stale_session = session("stale");
        let current_session = session("current");
        let mux = mux("tmux:agentdeck");
        let process = process("proc:codex");
        let current_mux_link = linked_to_mux_link(
            "current-fd-link",
            current_session.clone(),
            mux.clone(),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            Some("active_pane_fd_session_match"),
        );
        let snapshot = GraphSnapshot {
            nodes: vec![process_node("proc:codex", RuntimeProcessRole::HumanAgent)],
            candidate_links: vec![
                current_mux_link,
                mux_contains_process_link("mux-process", mux.clone(), process.clone()),
                process_session_link_with_match_kind(
                    "stale-argv-process",
                    process.clone(),
                    stale_session.clone(),
                    RelationKind::ProcessIdentifiesSession,
                    Some("active_pane_process_match"),
                    Some(1_000),
                ),
                process_session_link_with_match_kind(
                    "current-log-process",
                    process,
                    current_session.clone(),
                    RelationKind::ProcessIdentifiesSession,
                    Some("codex_log_process_thread_match"),
                    Some(2_000),
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        assert!(
            !resolved.candidate_links.iter().any(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source == stale_session
                    && link.source_metadata.evidence.as_deref()
                        == Some("runtime_process_identifies_session")
            }),
            "stale process argv identity should not derive a mux link"
        );
        let linked: Vec<_> = resolved
            .resolved_relationships
            .iter()
            .filter(|rel| rel.relation == RelationKind::LinkedToMux && rel.target == mux)
            .collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].source, current_session);
        assert_eq!(linked[0].selected_link_id, "current-fd-link");
    }

    #[test]
    fn resolve_snapshot_does_not_derive_weaker_process_link_when_mux_has_current_activity() {
        let stale_session = session("stale");
        let current_session = session("current");
        let mux = mux("tmux:agentdeck");
        let process = process("proc:opencode");
        let current_mux_link = linked_to_mux_link(
            "current-activity-link",
            current_session.clone(),
            mux.clone(),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(2_000),
            Some("session_file_activity_match"),
        );
        let snapshot = GraphSnapshot {
            nodes: vec![process_node(
                "proc:opencode",
                RuntimeProcessRole::HumanAgent,
            )],
            candidate_links: vec![
                current_mux_link,
                mux_contains_process_link("mux-process", mux.clone(), process.clone()),
                process_session_link_with_match_kind(
                    "stale-argv-process",
                    process,
                    stale_session.clone(),
                    RelationKind::ProcessIdentifiesSession,
                    Some("active_pane_process_match"),
                    Some(1_000),
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let resolved = resolve_snapshot(snapshot);

        assert!(
            !resolved.candidate_links.iter().any(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source == stale_session
                    && link.source_metadata.evidence.as_deref()
                        == Some("runtime_process_identifies_session")
            }),
            "weaker runtime process identity should not derive a competing mux link"
        );
        let linked: Vec<_> = resolved
            .resolved_relationships
            .iter()
            .filter(|rel| rel.relation == RelationKind::LinkedToMux && rel.target == mux)
            .collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].source, current_session);
        assert_eq!(linked[0].selected_link_id, "current-activity-link");
    }

    #[test]
    fn session_mux_resolver_prefers_file_activity_over_process_match() {
        let process = linked_to_mux_link(
            "process",
            session("a"),
            mux("tmux:process"),
            Provenance::StrongDiscovered,
            Confidence::High,
            Some(5_000),
            Some("active_pane_process_match"),
        );
        let activity = linked_to_mux_link(
            "activity",
            session("a"),
            mux("tmux:activity"),
            Provenance::StrongDiscovered,
            Confidence::Medium,
            Some(1_000),
            Some("session_file_activity_match"),
        );

        let output = resolve_links(&[process, activity]);

        assert_eq!(
            output.resolved_relationships[0].selected_link_id,
            "activity"
        );
        assert_eq!(
            output.resolved_relationships[0].competing_link_ids,
            vec!["process".to_string()]
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
