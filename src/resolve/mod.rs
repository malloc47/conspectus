//! Candidate-link resolution boundaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    CandidateScore, Confidence, Diagnostic, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, NodeId, Provenance, RelationKind, ResolutionExplanation,
    ResolvedRelationship, RuntimeProcessRole, ScoreAxis, SourceMetadata,
};

pub mod evidence;
pub mod pins;

pub fn resolve_snapshot(mut snapshot: GraphSnapshot) -> GraphSnapshot {
    let process_links = derive_process_mux_links(&snapshot);
    append_unique_links(&mut snapshot.candidate_links, process_links);
    // Pin binding runs before link resolution so the synthesized
    // pin-derived `LinkedToMux` candidates participate in resolver
    // ranking. The pin pass emits its own diagnostics, which we
    // merge in after `resolve_links` reassigns `snapshot.diagnostics`.
    let pin_diagnostics = pins::apply_pin_bindings(&mut snapshot);
    // H-MUXPROC-021: demote `LinkedToMux` candidates whose source
    // session is materially stale compared to a fresher candidate
    // for the same mux, before bucketing kicks in.
    demote_stale_source_mux_candidates(&mut snapshot);
    let output = resolve_links(&snapshot.candidate_links);
    snapshot.resolved_relationships = output.resolved_relationships;
    snapshot.diagnostics = output.diagnostics;
    snapshot.diagnostics.extend(pin_diagnostics);
    snapshot.sync_pin_nodes();
    snapshot.canonicalize();
    snapshot
}

/// Populate resolver score explanations for every resolved relationship.
///
/// The default resolver keeps snapshots compact and leaves this field empty.
/// CLI/TUI surfaces that need an explainer can call this after
/// [`resolve_snapshot`] has produced the selected relationships.
pub fn explain_resolved_relationships(snapshot: &mut GraphSnapshot) {
    let links_by_id: BTreeMap<&str, &GraphLink> = snapshot
        .candidate_links
        .iter()
        .map(|link| (link.id.as_str(), link))
        .collect();

    for relationship in &mut snapshot.resolved_relationships {
        let selected = relationship
            .selected_link_id
            .as_deref()
            .and_then(|id| links_by_id.get(id).copied())
            .map(candidate_score);

        let competing: Vec<CandidateScore> = relationship
            .competing_link_ids
            .iter()
            .filter_map(|id| links_by_id.get(id.as_str()).copied())
            .map(candidate_score)
            .collect();

        let decisive_axis = selected.as_ref().and_then(|score| {
            competing
                .first()
                .and_then(|competitor| first_different_axis(score, competitor))
        });

        relationship.explanation = Some(ResolutionExplanation {
            selected,
            competing,
            decisive_axis,
        });
    }
}

fn candidate_score(link: &GraphLink) -> CandidateScore {
    let axes = match link.relation {
        RelationKind::LinkedToMux => mux_score_axes(link),
        RelationKind::BranchHasForgePr => pr_score_axes(link),
        _ => generic_score_axes(link),
    };
    CandidateScore {
        link_id: link.id.clone(),
        axes,
    }
}

fn first_different_axis(selected: &CandidateScore, competitor: &CandidateScore) -> Option<String> {
    selected
        .axes
        .iter()
        .zip(&competitor.axes)
        .find(|(left, right)| left.name == right.name && left.value != right.value)
        .map(|(axis, _)| axis.name.clone())
}

fn score_axis(name: &str, value: impl ToString) -> ScoreAxis {
    ScoreAxis {
        name: name.to_string(),
        value: value.to_string(),
    }
}

/// Window (in seconds) within which a candidate's source session must
/// agree with the mux's `activity_epoch` to be eligible as the "fresh
/// winner" that demotes other candidates for the same mux. Set wide
/// enough to tolerate clock skew, snapshot-vs-write timing, and short
/// idle periods between user turns; tight enough that a multi-day
/// stale source can never qualify.
const FRESH_MUX_BIND_WINDOW_SECONDS: i64 = 6 * 3600;

/// Minimum gap (in seconds) between the freshest candidate's source
/// `last_active_epoch` and a competing candidate's source
/// `last_active_epoch` before the older candidate is demoted. Anything
/// closer than this stays Active so genuinely-shared muxes (an agent
/// hand-off between two sessions that were both touched within a
/// reasonable window) keep both attachments visible.
const STALE_SOURCE_GAP_SECONDS: i64 = 24 * 3600;

/// Mark `LinkedToMux` candidates whose source `AgentSession` is far
/// older than a competing candidate's source for the same mux as
/// `LinkState::Overridden`. The freshest candidate must itself be
/// within `FRESH_MUX_BIND_WINDOW_SECONDS` of the mux's
/// `activity_epoch`; otherwise no demotion runs (we don't penalize
/// other candidates on a weak signal). Declared and Pin provenance
/// candidates are exempt — explicit user intent always wins.
///
/// The resolver buckets `LinkedToMux` candidates per source rather
/// than per target, so without this pass two sessions both linking
/// to the same mux each win their own bucket and surface as parallel
/// attachments. In practice that produces the "stale `--resume <uuid>`
/// keeps showing as attached" behavior described in `H-MUXPROC-015`,
/// `H-MUXPROC-020`, and the live caveat-mux case that motivated
/// `H-MUXPROC-021`.
fn demote_stale_source_mux_candidates(snapshot: &mut GraphSnapshot) {
    let last_active_by_session: BTreeMap<NodeId, i64> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => session
                .last_active_epoch
                .map(|epoch| (NodeId::AgentSession(session.id.clone()), epoch)),
            _ => None,
        })
        .collect();
    let mux_activity: BTreeMap<NodeId, i64> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => mux
                .activity_epoch
                .map(|epoch| (NodeId::MuxSession(mux.id.clone()), epoch)),
            _ => None,
        })
        .collect();

    // Group eligible Active candidates by target mux. Each entry holds
    // the index into `candidate_links` and the source's last-active
    // epoch.
    let mut by_mux: BTreeMap<NodeId, Vec<(usize, i64)>> = BTreeMap::new();
    for (idx, link) in snapshot.candidate_links.iter().enumerate() {
        if link.relation != RelationKind::LinkedToMux
            || !matches!(link.state, LinkState::Active)
            || is_declared_or_pin(link.provenance)
        {
            continue;
        }
        let Some(target) = link.target_node_id().cloned() else {
            continue;
        };
        let Some(&source_epoch) = last_active_by_session.get(&link.source) else {
            continue;
        };
        by_mux.entry(target).or_default().push((idx, source_epoch));
    }

    // Plan demotions out of the borrow on `snapshot.candidate_links`
    // so we can mutate state in a second pass.
    let mut to_demote: Vec<(usize, String)> = Vec::new();
    for (target, entries) in by_mux {
        if entries.len() < 2 {
            continue;
        }
        let Some(&mux_epoch) = mux_activity.get(&target) else {
            continue;
        };
        let (winner_idx, winner_epoch) = entries
            .iter()
            .copied()
            .max_by_key(|(_, epoch)| *epoch)
            .expect("non-empty");
        if (mux_epoch - winner_epoch).abs() > FRESH_MUX_BIND_WINDOW_SECONDS {
            continue;
        }
        let winner_id = snapshot.candidate_links[winner_idx].id.clone();
        for (idx, epoch) in entries {
            if idx == winner_idx {
                continue;
            }
            if winner_epoch - epoch >= STALE_SOURCE_GAP_SECONDS {
                to_demote.push((idx, winner_id.clone()));
            }
        }
    }

    for (idx, winner_id) in to_demote {
        snapshot.candidate_links[idx].state = LinkState::Overridden {
            by: winner_id,
            reason: Some(
                "source session stale compared to fresher LinkedToMux candidate for same mux"
                    .to_string(),
            ),
        };
    }
}

fn is_declared_or_pin(provenance: Provenance) -> bool {
    matches!(
        provenance,
        Provenance::LocalDeclared
            | Provenance::GlobalDeclared
            | Provenance::LocalPin
            | Provenance::GlobalPin
    )
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
                        .map_or(0, BTreeSet::len),
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
            freshness_epoch: None,
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
            selected_link_id: Some(selected.id.clone()),
            competing_link_ids,
            explanation: None,
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

    // ADR 0077: rather than removing the matched `ResolvedRelationship`
    // (which left downstream consumers inferring ambiguity from the
    // raw candidate set, hence the H-UI-007 + H-UI-008 fallbacks),
    // mutate the slot in place. `selected_link_id` becomes `None`
    // — the resolver's honest "I cannot pick" — and the original
    // winner id joins `competing_link_ids` so the candidate
    // accounting still totals every link that was considered.
    let mut indices_to_suppress: Vec<usize> = Vec::new();
    for (idx, rel) in output.resolved_relationships.iter().enumerate() {
        if rel.relation != RelationKind::LinkedToMux {
            continue;
        }
        let Some(selected_id) = rel.selected_link_id.as_deref() else {
            continue;
        };
        let Some(link) = link_by_id.get(selected_id) else {
            continue;
        };
        if !is_cwd_evidence(link) {
            continue;
        }
        let distinct_session_count = mux_all_keys.get(&rel.target).map_or(0, |keys| keys.len());
        if distinct_session_count > 1 {
            indices_to_suppress.push(idx);
        }
    }

    for idx in indices_to_suppress {
        let rel = &mut output.resolved_relationships[idx];
        if let Some(prior_winner) = rel.selected_link_id.take() {
            output.diagnostics.push(Diagnostic::Conflict {
                source: rel.source.clone(),
                relation: rel.relation.clone(),
                selected_link_id: prior_winner.clone(),
                competing_link_ids: vec![],
            });
            // Add the prior winner to the competing set so the
            // candidate accounting stays complete: when consumers
            // walk `competing_link_ids` for an ambiguous slot, the
            // tiebreak winner is one of the competitors.
            rel.competing_link_ids.push(prior_winner);
            rel.competing_link_ids.sort();
            rel.competing_link_ids.dedup();
        }
    }
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

fn generic_score_axes(link: &GraphLink) -> Vec<ScoreAxis> {
    vec![
        score_axis("provenance", link.provenance.snake_case()),
        score_axis("provenance_rank", link.provenance.precedence()),
        score_axis("confidence", link.confidence.snake_case()),
        score_axis("link_id", &link.id),
    ]
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
    // H-EXT-004: match against the shared evidence-string
    // constants so a rename anywhere in the pipeline is caught
    // at compile time instead of silently losing rank.
    use evidence::*;
    match match_kind {
        Some(s) if s == CODEX_LOG_PROCESS_THREAD_MATCH || s == HOOK_PROCESS_SESSION_MATCH => 60,
        Some(s)
            if s == ACTIVE_PANE_FD_SESSION_MATCH || s == ACTIVE_PANE_FD_COMMAND_SESSION_MATCH =>
        {
            50
        }
        Some(s) if s == ACTIVE_PANE_PROCESS_MATCH => 35,
        Some(s) if s == ACTIVE_PANE_COMMAND_SESSION_MATCH => 30,
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
    tier: ProvenanceTier,
    evidence_rank: u8,
    confidence: Confidence,
    activity_epoch: i64,
}

/// H-REF-003: shared `Provenance` → tier mapping used by every
/// relation comparator's provenance axis. Pre-H-REF-003 the
/// resolver had two parallel enums (`MuxTier` +
/// `PrProvenanceTier`) with identical variants, identical
/// ordering, and identical labels. A new `Provenance` variant
/// meant three coordinated changes; now it means one.
///
/// Higher-tier discriminants win the comparator, which matches
/// [`crate::model::Provenance::precedence`] but is spelled as a
/// separate scoring enum so the comparators' arithmetic
/// stays local to the resolver.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ProvenanceTier {
    Cached = 0,
    Convention = 1,
    Discovered = 2,
    StrongDiscovered = 3,
    GlobalPin = 4,
    GlobalDeclared = 5,
    LocalPin = 6,
    LocalDeclared = 7,
}

impl ProvenanceTier {
    fn label(self) -> &'static str {
        match self {
            Self::Cached => "cached",
            Self::Convention => "convention",
            Self::Discovered => "discovered",
            Self::StrongDiscovered => "strong_discovered",
            Self::GlobalPin => "global_pin",
            Self::GlobalDeclared => "global_declared",
            Self::LocalPin => "local_pin",
            Self::LocalDeclared => "local_declared",
        }
    }

    fn from_provenance(provenance: Provenance) -> Self {
        match provenance {
            Provenance::LocalDeclared => Self::LocalDeclared,
            Provenance::LocalPin => Self::LocalPin,
            Provenance::GlobalDeclared => Self::GlobalDeclared,
            Provenance::GlobalPin => Self::GlobalPin,
            Provenance::StrongDiscovered => Self::StrongDiscovered,
            Provenance::Discovered => Self::Discovered,
            Provenance::Convention => Self::Convention,
            Provenance::Cached => Self::Cached,
        }
    }
}

fn mux_score(link: &GraphLink) -> MuxScore {
    let match_kind = link
        .source_metadata
        .fields
        .get("match_kind")
        .and_then(serde_json::Value::as_str)
        .or(link.source_metadata.evidence.as_deref());

    MuxScore {
        tier: ProvenanceTier::from_provenance(link.provenance),
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

fn mux_score_axes(link: &GraphLink) -> Vec<ScoreAxis> {
    let score = mux_score(link);
    vec![
        score_axis("tier", score.tier.label()),
        score_axis("tier_rank", score.tier as u8),
        score_axis("evidence_rank", score.evidence_rank),
        score_axis("confidence", score.confidence.snake_case()),
        score_axis("activity_epoch", score.activity_epoch),
        score_axis("link_id", &link.id),
    ]
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

// H-REF-003: `mux_tier` retired — replaced by
// `ProvenanceTier::from_provenance` shared with `pr_score`.

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
    provenance_tier: ProvenanceTier,
    state_rank: PrStateRank,
    is_draft: bool,
    updated_epoch: i64,
    confidence: Confidence,
}

// H-REF-003: `PrProvenanceTier` retired — replaced by the
// shared `ProvenanceTier` above (identical variants + labels).

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum PrStateRank {
    Other = 0,
    Closed = 1,
    Merged = 2,
    Open = 3,
}

impl PrStateRank {
    fn label(self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::Closed => "closed",
            Self::Merged => "merged",
            Self::Open => "open",
        }
    }
}

fn pr_score(link: &GraphLink) -> PrScore {
    PrScore {
        provenance_tier: ProvenanceTier::from_provenance(link.provenance),
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

fn pr_score_axes(link: &GraphLink) -> Vec<ScoreAxis> {
    let score = pr_score(link);
    vec![
        score_axis("provenance", score.provenance_tier.label()),
        score_axis("provenance_rank", score.provenance_tier as u8),
        score_axis("state_rank", score.state_rank.label()),
        score_axis("state_rank_value", score.state_rank as u8),
        score_axis("is_draft", score.is_draft),
        score_axis("updated_epoch", score.updated_epoch),
        score_axis("confidence", score.confidence.snake_case()),
        score_axis("link_id", &link.id),
    ]
}

// H-REF-003: `pr_provenance_tier` retired — replaced by
// `ProvenanceTier::from_provenance` shared with `mux_score`.

fn pr_state_rank(raw: Option<&str>) -> PrStateRank {
    match raw.map(str::to_ascii_lowercase).as_deref() {
        Some("open") => PrStateRank::Open,
        Some("merged") => PrStateRank::Merged,
        Some("closed") => PrStateRank::Closed,
        _ => PrStateRank::Other,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
