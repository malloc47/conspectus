//! Pin binding pass (ADR 0057 / H-PIN-004).
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
//! 3. Binds when exactly one harness session matches; emits
//!    `PinAmbiguous` for >1 (binding to the highest-ranked candidate),
//!    `PinStaleMux` for 0, and `PinUnbound` when the mux itself is
//!    missing.
//! 4. On a successful bind, synthesizes a `LinkedToMux` `GraphLink`
//!    carrying the pin's `LocalPin`/`GlobalPin` provenance so the
//!    rest of the resolver pipeline ranks pin evidence correctly,
//!    and registers the pin's `display_name` in the alias overlay
//!    (in-memory only — no TOML write, per ADR 0057).
//! 5. Emits `PinDrift` when the bound session's first-observed cwd
//!    diverges from the pin's declared cwd.
//!
//! The function mutates `snapshot.pins[i].binding`, appends to
//! `snapshot.candidate_links`, and registers alias overlay entries.
//! Diagnostics are *returned* rather than mutated in place because
//! [`crate::resolve::resolve_snapshot`] reassigns
//! `snapshot.diagnostics` after `resolve_links` runs — the caller is
//! responsible for merging.

use std::collections::BTreeMap;

use crate::model::{
    AgentSessionId, Confidence, Diagnostic, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, PinBinding,
    PinCandidate, PinId, RelationKind, SourceMetadata, UnresolvedEndpoint,
};
use serde_json::Value;

/// Run the pin binding pass over `snapshot`. Returns the diagnostics
/// the caller must merge into `snapshot.diagnostics` after
/// `resolve_links` has reassigned the vector.
pub fn apply_pin_bindings(snapshot: &mut GraphSnapshot) -> Vec<Diagnostic> {
    if snapshot.pins.is_empty() {
        return Vec::new();
    }

    let mux_by_native_id = mux_index(&snapshot.nodes);
    let agent_session_cwd_by_id = agent_session_cwd_index(&snapshot.nodes);
    let linked_to_mux_by_mux = active_linked_to_mux_by_mux(&snapshot.candidate_links);

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut synthesized_links: Vec<GraphLink> = Vec::new();
    let mut alias_inserts: Vec<(NodeId, String)> = Vec::new();
    let mut binding_updates: Vec<(usize, PinBinding)> = Vec::new();

    for (idx, pin) in snapshot.pins.iter().enumerate() {
        synthesized_links.push(synthesize_pin_target_link(pin, &mux_by_native_id));
        let target_native_id = pin.mux.native_id();
        let Some(mux) = mux_by_native_id.get(target_native_id.as_str()) else {
            diagnostics.push(Diagnostic::PinUnbound {
                pin_id: pin.id.clone(),
                expected_mux_native_id: target_native_id,
                // Populated by the post-resolve sidecar consumer
                // (ADR 0058 / H-PIN-RESUME-005); the bare resolver
                // pass stays evidence-only and never reads from
                // the cache directly.
                last_session: None,
            });
            binding_updates.push((idx, PinBinding::Unbound));
            continue;
        };

        let mux_node_id = NodeId::MuxSession(mux.id.clone());
        let candidates: &[&GraphLink] = linked_to_mux_by_mux
            .get(&mux_node_id)
            .map_or(&[][..], |v| v.as_slice());

        let mut harness_filtered: Vec<&GraphLink> = candidates
            .iter()
            .copied()
            .filter(|link| {
                matches!(&link.source, NodeId::AgentSession(id) if id.harness_key == pin.harness)
            })
            .collect();

        if harness_filtered.is_empty() {
            diagnostics.push(Diagnostic::PinStaleMux {
                pin_id: pin.id.clone(),
                mux: mux.id.clone(),
            });
            binding_updates.push((
                idx,
                PinBinding::StaleMux {
                    mux: mux.id.clone(),
                },
            ));
            continue;
        }

        // Deterministic ranking: provenance precedence (high first),
        // then freshness (Fresh < Stale < Unknown by enum order — Fresh
        // is most preferred), then source NodeId for a stable tiebreak.
        harness_filtered.sort_by(|a, b| {
            b.provenance
                .precedence()
                .cmp(&a.provenance.precedence())
                .then_with(|| freshness_rank(a.freshness).cmp(&freshness_rank(b.freshness)))
                .then_with(|| a.source.cmp(&b.source))
        });

        let chosen_link = harness_filtered[0];
        let NodeId::AgentSession(chosen_session_id) = chosen_link.source.clone() else {
            // Defensive: filter above guarantees this, but keep the
            // resolver total.
            binding_updates.push((idx, PinBinding::Unbound));
            continue;
        };

        if harness_filtered.len() > 1 {
            let competing: Vec<AgentSessionId> = harness_filtered[1..]
                .iter()
                .filter_map(|link| match &link.source {
                    NodeId::AgentSession(id) => Some(id.clone()),
                    _ => None,
                })
                .collect();
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

    for link in synthesized_links {
        // Idempotency: if the exact same synthesized link already
        // exists (re-resolve of an already-resolved snapshot), skip.
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
    }
    for (node_id, display_name) in alias_inserts {
        snapshot.aliases.insert_if_absent(node_id, display_name);
    }
    for (idx, binding) in binding_updates {
        snapshot.pins[idx].binding = Some(binding);
    }

    diagnostics
}

/// Key muxes by the prefixed encoding `pin.mux.native_id()` produces
/// (`tmux:<name>` for default sockets, `tmux:<socket>:<name>` for
/// non-default sockets per ADR 0057). `MuxSessionNode.native_id`
/// itself holds the *bare* tmux session name (per production
/// discovery at `discovery/tmux/mod.rs:1038`); we reconstruct the
/// prefixed form here so the lookup matches what the pin entries
/// declare. Non-default-socket muxes can't be reconstructed without
/// a socket field on the node (deferred per H-PIN-F-001), so they
/// never appear in the index and their pins resolve to
/// `PinUnbound` until the socket-aware discovery story lands.
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
            adapter: "pin".to_string(),
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
            adapter: "pin".to_string(),
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
            adapter: "pin".to_string(),
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
