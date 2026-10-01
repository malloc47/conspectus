//! Pin binding pass (ADR 0057).
//!
//! Reads `snapshot.pins` (populated by [`crate::discovery::pins`]) and
//! per pin:
//!
//! 1. Looks up the live `MuxSession` by exact `native_id` match
//!    (`tmux:<name>` for default socket; `tmux:<socket>:<name>` for
//!    non-default).
//! 2. Restricts the existing mux-to-agent-session attribution
//!    pipeline (i.e. active `LinkedToMux` candidates landing at the
//!    bound mux) to the pin's harness.
//! 3. Assigns sessions to pins one-to-one (ADR 0102): the strongest
//!    remaining (pin, session) claim wins, so two pins whose muxes
//!    both carry evidence for one session can't both bind it. Emits
//!    `PinAmbiguous` when other free sessions also matched (binding
//!    to the highest-ranked candidate), `PinStaleMux` when none is
//!    left, and `PinUnbound` when the mux itself is missing.
//! 4. On a successful bind, synthesizes a `LinkedToMux` `GraphLink`
//!    carrying the pin's `LocalPin`/`GlobalPin` provenance so the
//!    rest of the resolver pipeline ranks pin evidence correctly,
//!    and registers the pin's `display_name` in the alias overlay
//!    (in-memory only — no TOML write, per ADR 0057).
//! 5. Emits `PinDrift` when the bound session's first-observed cwd
//!    diverges from the pin's declared cwd.
//!
//! The function mutates `snapshot.pins[i].binding`, replaces the
//! previous pass's synthesized links in `snapshot.candidate_links`,
//! and registers alias overlay entries.
//! Diagnostics are *returned* rather than mutated in place because
//! [`crate::resolve::resolve_snapshot`] reassigns
//! `snapshot.diagnostics` after `resolve_links` runs — the caller is
//! responsible for merging.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    AgentSessionId, Confidence, Diagnostic, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, PinBinding,
    PinCandidate, PinId, PinSessionClaim, Provenance, RelationKind, SourceMetadata,
    UnresolvedEndpoint,
};
use serde_json::Value;

/// Adapter label on every link this pass synthesizes. Nothing else
/// emits it, which lets each pass drop the previous pass's output.
const PIN_ADAPTER: &str = "pin";

/// Link-id prefix of a previous pass's binding re-issued as a
/// fallback candidate (see [`prior_binding_candidates`]).
const PRIOR_BINDING_PREFIX: &str = "pin-prior:";

/// Where a pin's mux search landed, before sessions are assigned.
enum PinTarget<'a> {
    /// No live mux matches the pin's `mux.native_id()`.
    MissingMux(String),
    /// The mux is live. `ranked` holds one link per candidate
    /// session of the pin's harness, best evidence first.
    Mux {
        mux: &'a MuxSessionNode,
        ranked: Vec<&'a GraphLink>,
    },
}

/// Run the pin binding pass over `snapshot`. Returns the diagnostics
/// the caller must merge into `snapshot.diagnostics` after
/// `resolve_links` has reassigned the vector.
pub fn apply_pin_bindings(snapshot: &mut GraphSnapshot) -> Vec<Diagnostic> {
    if snapshot.pins.is_empty() {
        return Vec::new();
    }

    // A re-resolve (daemon hook ingest, per-class refresh) runs over a
    // snapshot that still carries the previous pass's synthesized
    // links. Left in place they would compete as `LocalPin`
    // candidates and re-confirm last cycle's binding even after the
    // underlying evidence moved, so a wrong binding could never heal.
    // The previous binding still counts, but only as the weakest
    // candidate: it keeps a pin bound while time-based evidence
    // (session-file activity) wanders to a neighbouring session, and
    // loses to any current evidence.
    let prior_bindings = prior_binding_candidates(snapshot);
    snapshot
        .candidate_links
        .retain(|link| link.source_metadata.adapter != PIN_ADAPTER);

    let mux_by_native_id = mux_index(&snapshot.nodes);
    let agent_session_cwd_by_id = agent_session_cwd_index(&snapshot.nodes);
    let mut linked_to_mux_by_mux = active_linked_to_mux_by_mux(&snapshot.candidate_links);
    for link in &prior_bindings {
        if let Some(target) = link.target_node_id() {
            linked_to_mux_by_mux
                .entry(target.clone())
                .or_default()
                .push(link);
        }
    }

    let targets: Vec<PinTarget<'_>> = snapshot
        .pins
        .iter()
        .map(|pin| pin_target(pin, &mux_by_native_id, &linked_to_mux_by_mux))
        .collect();
    let (assignments, claimed_by) = assign_sessions(&targets);

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut synthesized_links: Vec<GraphLink> = Vec::new();
    let mut alias_inserts: Vec<(NodeId, String)> = Vec::new();
    let mut binding_updates: Vec<(usize, PinBinding)> = Vec::new();

    for (idx, (pin, target)) in snapshot.pins.iter().zip(&targets).enumerate() {
        synthesized_links.push(synthesize_pin_target_link(pin, &mux_by_native_id));
        let (mux, ranked) = match target {
            PinTarget::MissingMux(expected_mux_native_id) => {
                diagnostics.push(Diagnostic::PinUnbound {
                    pin_id: pin.id.clone(),
                    expected_mux_native_id: expected_mux_native_id.clone(),
                    // Populated by the post-resolve sidecar consumer
                    // (ADR 0058); the bare resolver pass stays
                    // evidence-only and never reads from the cache
                    // directly.
                    last_session: None,
                });
                binding_updates.push((idx, PinBinding::Unbound));
                continue;
            }
            PinTarget::Mux { mux, ranked } => (*mux, ranked),
        };

        // Sessions this pin's mux had evidence for that a different
        // pin bound first.
        let claimed_elsewhere: Vec<PinSessionClaim> = ranked
            .iter()
            .filter_map(|link| {
                let session = link_session(link)?;
                let owner = *claimed_by.get(session)?;
                (owner != idx).then(|| PinSessionClaim {
                    session: session.clone(),
                    claimed_by_pin: snapshot.pins[owner].id.clone(),
                })
            })
            .collect();

        let Some(&chosen_pos) = assignments.get(&idx) else {
            diagnostics.push(Diagnostic::PinStaleMux {
                pin_id: pin.id.clone(),
                mux: mux.id.clone(),
                claimed_elsewhere,
            });
            binding_updates.push((
                idx,
                PinBinding::StaleMux {
                    mux: mux.id.clone(),
                },
            ));
            continue;
        };
        let Some(chosen_session_id) = link_session(ranked[chosen_pos]).cloned() else {
            // Defensive: `pin_target` only keeps agent-session
            // sources, but keep the resolver total.
            binding_updates.push((idx, PinBinding::Unbound));
            continue;
        };

        // Runners-up are the other sessions still free for this pin;
        // sessions bound to other pins are not real competition.
        let competing: Vec<AgentSessionId> = ranked
            .iter()
            .enumerate()
            .filter(|(pos, link)| *pos != chosen_pos && !is_prior_binding(link))
            .filter_map(|(_, link)| link_session(link))
            .filter(|session| !claimed_by.contains_key(*session))
            .cloned()
            .collect();
        if !competing.is_empty() {
            diagnostics.push(Diagnostic::PinAmbiguous {
                pin_id: pin.id.clone(),
                chosen: chosen_session_id.clone(),
                competing,
            });
        }

        // Drift advisory: bound session's observed cwd vs pin's
        // declared cwd. Binding still holds either way.
        if let Some(observed_cwd) = agent_session_cwd_by_id
            .get(&NodeId::AgentSession(chosen_session_id.clone()))
            .and_then(|cwd| cwd.as_deref())
            && observed_cwd != pin.cwd
        {
            diagnostics.push(Diagnostic::PinDrift {
                pin_id: pin.id.clone(),
                declared_cwd: pin.cwd.clone(),
                observed_cwd: observed_cwd.to_string(),
            });
        }

        synthesized_links.push(synthesize_pin_link(pin, &chosen_session_id, &mux.id));
        synthesized_links.push(synthesize_pin_realized_by_link(pin, &chosen_session_id));
        alias_inserts.push((
            NodeId::AgentSession(chosen_session_id.clone()),
            pin.display_name.clone(),
        ));
        binding_updates.push((
            idx,
            PinBinding::Bound {
                mux: mux.id.clone(),
                session: chosen_session_id,
            },
        ));
    }

    snapshot.candidate_links.extend(synthesized_links);
    for (node_id, display_name) in alias_inserts {
        snapshot.aliases.insert_if_absent(node_id, display_name);
    }
    for (idx, binding) in binding_updates {
        snapshot.pins[idx].binding = Some(binding);
    }

    diagnostics
}

/// The previous pass's pin → session bindings, re-issued as
/// `Cached`-provenance candidates for this pass. A binding is dropped
/// when its session is gone, or when the session shows no activity
/// since the mux was (re)created: a mux recreated under the same name
/// must not inherit the old incarnation's session.
fn prior_binding_candidates(snapshot: &GraphSnapshot) -> Vec<GraphLink> {
    let session_activity: BTreeMap<&AgentSessionId, Option<i64>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some((&session.id, session.last_active_epoch)),
            _ => None,
        })
        .collect();
    let mux_created: BTreeMap<&MuxSessionId, Option<i64>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some((&mux.id, mux.created_epoch)),
            _ => None,
        })
        .collect();
    snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.source_metadata.adapter == PIN_ADAPTER
                && link.relation == RelationKind::LinkedToMux
                && matches!(link.state, LinkState::Active)
        })
        .filter(|link| {
            let (Some(session), Some(NodeId::MuxSession(mux))) =
                (link_session(link), link.target_node_id())
            else {
                return false;
            };
            let Some(&last_active) = session_activity.get(session) else {
                return false;
            };
            match (last_active, mux_created.get(mux).copied().flatten()) {
                (Some(active), Some(created)) => active >= created,
                _ => true,
            }
        })
        .map(|link| {
            let mut prior = link.clone();
            prior.id = format!("{PRIOR_BINDING_PREFIX}{}", link.id);
            prior.provenance = Provenance::Cached;
            prior.freshness = Freshness::Stale;
            prior.source_metadata.evidence = Some("previous pin binding".to_string());
            prior
        })
        .collect()
}

/// Find the pin's live mux and rank the candidate sessions of the
/// pin's harness attributed to it. Ranking is provenance first, then
/// freshness, then the resolver's own session ↔ mux evidence order,
/// so a hook-reported binding beats an activity-time heuristic the
/// same way it does in `resolve_links`. Only the best link per
/// session is kept.
fn pin_target<'a>(
    pin: &PinCandidate,
    mux_by_native_id: &BTreeMap<String, &'a MuxSessionNode>,
    linked_to_mux_by_mux: &BTreeMap<NodeId, Vec<&'a GraphLink>>,
) -> PinTarget<'a> {
    let target_native_id = pin.mux.native_id();
    let Some(mux) = mux_by_native_id.get(target_native_id.as_str()).copied() else {
        return PinTarget::MissingMux(target_native_id);
    };
    let mut ranked: Vec<&GraphLink> = linked_to_mux_by_mux
        .get(&NodeId::MuxSession(mux.id.clone()))
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .copied()
        .filter(|link| link_session(link).is_some_and(|id| id.harness_key == pin.harness))
        .collect();
    ranked.sort_by(|a, b| compare_pin_candidates(a, b));
    let mut seen: BTreeSet<&AgentSessionId> = BTreeSet::new();
    ranked.retain(|link| link_session(link).is_some_and(|session| seen.insert(session)));
    PinTarget::Mux { mux, ranked }
}

fn compare_pin_candidates(a: &GraphLink, b: &GraphLink) -> Ordering {
    b.provenance
        .precedence()
        .cmp(&a.provenance.precedence())
        .then_with(|| freshness_rank(a.freshness).cmp(&freshness_rank(b.freshness)))
        .then_with(|| super::compare_session_mux(a, b))
}

/// ADR 0102: a session realizes at most one pin. Assign greedily
/// across all pins: repeatedly take the strongest remaining
/// (pin, free session) claim, so when two pins' muxes both have
/// evidence for one session, the pin with the better evidence keeps
/// it and the other falls back to its next candidate, if any.
///
/// Returns each assigned pin's position in its `ranked` list, and the
/// owning pin index for every claimed session.
fn assign_sessions<'a>(
    targets: &[PinTarget<'a>],
) -> (BTreeMap<usize, usize>, BTreeMap<&'a AgentSessionId, usize>) {
    let mut assignments: BTreeMap<usize, usize> = BTreeMap::new();
    let mut claimed_by: BTreeMap<&'a AgentSessionId, usize> = BTreeMap::new();
    loop {
        let mut best: Option<(usize, usize, &'a GraphLink)> = None;
        for (idx, target) in targets.iter().enumerate() {
            if assignments.contains_key(&idx) {
                continue;
            }
            let PinTarget::Mux { ranked, .. } = target else {
                continue;
            };
            let free = ranked.iter().enumerate().find(|(_, link)| {
                link_session(link).is_some_and(|session| !claimed_by.contains_key(session))
            });
            let Some((pos, link)) = free else {
                continue;
            };
            // Strict `Less` keeps the earlier pin on a full tie.
            let better = best.is_none_or(|(_, _, current)| {
                compare_pin_candidates(link, current) == Ordering::Less
            });
            if better {
                best = Some((idx, pos, link));
            }
        }
        let Some((idx, pos, link)) = best else {
            break;
        };
        if let Some(session) = link_session(link) {
            claimed_by.insert(session, idx);
        }
        assignments.insert(idx, pos);
    }
    (assignments, claimed_by)
}

fn is_prior_binding(link: &GraphLink) -> bool {
    link.id.starts_with(PRIOR_BINDING_PREFIX)
}

fn link_session(link: &GraphLink) -> Option<&AgentSessionId> {
    match &link.source {
        NodeId::AgentSession(id) => Some(id),
        _ => None,
    }
}

/// Whether `mux` hosts a live process of `harness` according to the
/// pane-process evidence (`MuxContainsProcess` → `RuntimeProcess`).
///
/// A `StaleMux` pin can still have its harness running: the pane
/// process is visible but no transcript could be tied to it, or the
/// only matching session went to another pin (ADR 0102). Pin launch
/// uses this to attach instead of typing the launch argv into a live
/// agent pane (ADR 0028).
pub fn mux_hosts_harness(snapshot: &GraphSnapshot, mux: &MuxSessionId, harness: &str) -> bool {
    let mux_node = NodeId::MuxSession(mux.clone());
    let processes: BTreeSet<&NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::MuxContainsProcess
                && matches!(link.state, LinkState::Active)
                && link.source == mux_node
        })
        .filter_map(GraphLink::target_node_id)
        .collect();
    snapshot.nodes.iter().any(|node| match node {
        GraphNode::RuntimeProcess(process) => {
            process.harness_key.as_deref() == Some(harness)
                && processes.contains(&NodeId::RuntimeProcess(process.id.clone()))
        }
        _ => false,
    })
}

/// Key muxes by the prefixed encoding `pin.mux.native_id()` produces
/// (`tmux:<name>` for default sockets, `tmux:<socket>:<name>` for
/// non-default sockets per ADR 0057). `MuxSessionNode.native_id`
/// itself holds the *bare* tmux session name (as tmux discovery
/// emits it); we reconstruct the prefixed form here so the lookup
/// matches what the pin entries declare. Non-default-socket muxes
/// can't be reconstructed without a socket field on the node, so
/// they never appear in the index and their pins resolve to
/// `PinUnbound` until discovery covers non-default sockets.
fn mux_index(nodes: &[GraphNode]) -> BTreeMap<String, &MuxSessionNode> {
    nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => {
                let key = format!("{}:{}", mux.backend, mux.native_id);
                Some((key, mux))
            }
            _ => None,
        })
        .collect()
}

fn agent_session_cwd_index(nodes: &[GraphNode]) -> BTreeMap<NodeId, Option<String>> {
    nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some((
                NodeId::AgentSession(session.id.clone()),
                session.cwd.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn active_linked_to_mux_by_mux(links: &[GraphLink]) -> BTreeMap<NodeId, Vec<&GraphLink>> {
    let mut by_mux: BTreeMap<NodeId, Vec<&GraphLink>> = BTreeMap::new();
    for link in links {
        if link.relation != RelationKind::LinkedToMux {
            continue;
        }
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        let Some(target) = link.target_node_id() else {
            continue;
        };
        if !matches!(target, NodeId::MuxSession(_)) {
            continue;
        }
        by_mux.entry(target.clone()).or_default().push(link);
    }
    by_mux
}

fn synthesize_pin_link(
    pin: &PinCandidate,
    session: &AgentSessionId,
    mux: &MuxSessionId,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert("pin_id".to_string(), Value::String(pin.id.clone()));
    fields.insert(
        "pin_display_name".to_string(),
        Value::String(pin.display_name.clone()),
    );
    fields.insert("pin_cwd".to_string(), Value::String(pin.cwd.clone()));
    fields.insert(
        "pin_store_path".to_string(),
        Value::String(pin.store_path.clone()),
    );

    GraphLink {
        id: format!("pin:{}:{}", pin.provenance.snake_case(), pin.id),
        source: NodeId::AgentSession(session.clone()),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux.clone()),
        },
        relation: RelationKind::LinkedToMux,
        provenance: pin.provenance,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: PIN_ADAPTER.to_string(),
            evidence: Some(format!("synthesized from pin `{}`", pin.id)),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn synthesize_pin_target_link(
    pin: &PinCandidate,
    mux_by_native_id: &BTreeMap<String, &MuxSessionNode>,
) -> GraphLink {
    let target_native_id = pin.mux.native_id();
    let target = match mux_by_native_id.get(&target_native_id) {
        Some(mux) => LinkEndpoint::Node {
            id: NodeId::MuxSession(mux.id.clone()),
        },
        None => LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: "mux_session".to_string(),
                harness_key: None,
                native_id: Some(target_native_id.clone()),
                state_scope: None,
                path: None,
                metadata: Metadata::new(),
            },
        },
    };
    let mut fields = pin_link_metadata(pin);
    fields.insert(
        "target_mux_native_id".to_string(),
        Value::String(target_native_id),
    );
    GraphLink {
        id: format!(
            "pin-node:{}:{}:targets-mux",
            pin.provenance.snake_case(),
            pin.id
        ),
        source: NodeId::Pin(PinId::new(pin.id.clone())),
        target,
        relation: RelationKind::PinTargetsMux,
        provenance: pin.provenance,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: PIN_ADAPTER.to_string(),
            evidence: Some(format!("pin `{}` targets mux `{}`", pin.id, pin.mux.name)),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn synthesize_pin_realized_by_link(pin: &PinCandidate, session: &AgentSessionId) -> GraphLink {
    GraphLink {
        id: format!(
            "pin-node:{}:{}:realized-by-session",
            pin.provenance.snake_case(),
            pin.id
        ),
        source: NodeId::Pin(PinId::new(pin.id.clone())),
        target: LinkEndpoint::Node {
            id: NodeId::AgentSession(session.clone()),
        },
        relation: RelationKind::PinRealizedBySession,
        provenance: pin.provenance,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: PIN_ADAPTER.to_string(),
            evidence: Some(format!("pin `{}` is realized by a live session", pin.id)),
            fields: pin_link_metadata(pin),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn pin_link_metadata(pin: &PinCandidate) -> Metadata {
    let mut fields = Metadata::new();
    fields.insert("pin_id".to_string(), Value::String(pin.id.clone()));
    fields.insert(
        "pin_display_name".to_string(),
        Value::String(pin.display_name.clone()),
    );
    fields.insert("pin_cwd".to_string(), Value::String(pin.cwd.clone()));
    fields.insert(
        "pin_store_path".to_string(),
        Value::String(pin.store_path.clone()),
    );
    fields
}

/// Map [`Freshness`] to a rank used by the pin tiebreaker. Lower wins
/// (`Fresh` < `Stale` < `Unknown`). The enum's derived `Ord` orders
/// Fresh first by source order — we re-encode here to make the
/// resolver intent explicit and stable against any future enum
/// reordering.
fn freshness_rank(freshness: Freshness) -> u8 {
    match freshness {
        Freshness::Fresh => 0,
        Freshness::Stale => 1,
        Freshness::Unknown => 2,
    }
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
