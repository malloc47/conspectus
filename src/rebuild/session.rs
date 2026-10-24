//! The session executor (ADR 0142): resume a session into a mux.
//!
//! The natural place is the mux it last ran in, rebuilt if it's a
//! ghost. When that mux is busy running something else, or the session
//! never had a mux, the resume goes into a new mux named after it. The
//! session is resumed at its lineage head, so a session that compacted
//! or resumed since still comes back as the newest one (ADR 0018).

use std::collections::BTreeSet;

use crate::model::{
    AgentSessionId, FieldValue, GraphNode, GraphSnapshot, LinkState, MuxRecord, NodeId, PinKind,
    PinSlot, RelationKind, SlotFields, SlotStatus, SlotTarget,
};
use crate::resolve::slots::resolve_slot;

use super::{Env, Options, Plan, RebuildError, Subject, SubjectSource, mux};

pub fn plan(
    snapshot: &GraphSnapshot,
    subject: &Subject,
    options: &Options,
    env: &Env<'_>,
    now: i64,
) -> Result<Plan, RebuildError> {
    let id = session_id(&subject.slot)?;
    let resolved = resolve_slot(snapshot, &subject.slot);
    let cwd = session_cwd(snapshot, &id);
    let ids = lineage_ids(snapshot, &id);

    // Where to resume: the slot's own choice, else the mux that hosted
    // the session most recently.
    let hosting = resolved
        .text("resume_into")
        .and_then(|name| mux_by_name(snapshot, name))
        .or_else(|| last_hosting_mux(snapshot, &id.harness_key, &ids));

    let resume_options = Options {
        session: Some(id.session_key.clone()),
        attach: options.attach,
        assume_stopped: options.assume_stopped,
    };
    let mut plan = if let Some(record) = hosting {
        let subject = mux_subject(&record.meta.id, &record.recipe.name);
        let resolved_mux = resolve_slot(snapshot, &subject.slot);
        // Asked what would happen if its mux had stopped, it isn't busy.
        let busy = !options.assume_stopped
            && resolved_mux.status == SlotStatus::Live
            && resolved_mux
                .target_node
                .as_ref()
                .is_some_and(|node| mux_is_busy(snapshot, node, &id.harness_key));
        if busy {
            new_mux_plan(
                snapshot,
                &id,
                &cwd,
                &record.recipe.name,
                &resume_options,
                env,
                now,
            )?
        } else {
            mux::plan(snapshot, &subject, &resume_options, env, now)?
        }
    } else {
        let base = format!("{}-{}", id.harness_key, short(&id.session_key));
        new_mux_plan(snapshot, &id, &cwd, &base, &resume_options, env, now)?
    };
    plan.subject = format!(
        "session `{}` ({}) into {}",
        short(&id.session_key),
        resolved.status.label_for(crate::model::PinKind::Session),
        plan.subject
    );
    Ok(plan)
}

fn session_id(slot: &PinSlot) -> Result<AgentSessionId, RebuildError> {
    match slot.parsed_target() {
        Ok(SlotTarget::Node(NodeId::AgentSession(id))) => Ok(id),
        _ => Err(RebuildError::Cannot(format!(
            "pin `{}` doesn't point at a session",
            slot.id
        ))),
    }
}

fn short(key: &str) -> &str {
    key.get(..8).unwrap_or(key)
}

/// A transient mux slot for a history record.
fn mux_subject(record_id: &str, name: &str) -> Subject {
    Subject {
        slot: PinSlot {
            id: name.to_string(),
            kind: PinKind::Mux,
            target: Some(PinSlot::history_target(record_id)),
            pinned_at: None,
            overrides: SlotFields::new(),
            snapshot: SlotFields::new(),
            order: u32::MAX,
            legacy: false,
        },
        source: SubjectSource::History,
    }
}

fn mux_by_name(snapshot: &GraphSnapshot, name: &str) -> Option<MuxRecord> {
    snapshot
        .history
        .muxes
        .iter()
        .filter(|record| record.recipe.name == name)
        .max_by_key(|record| record.meta.last_seen)
        .cloned()
}

/// The mux record that hosted any session of this lineage most recently.
fn last_hosting_mux(
    snapshot: &GraphSnapshot,
    harness: &str,
    ids: &BTreeSet<String>,
) -> Option<MuxRecord> {
    snapshot
        .history
        .muxes
        .iter()
        .filter_map(|record| {
            let seen = record
                .recipe
                .hosted
                .iter()
                .filter(|hosted| hosted.harness == harness && ids.contains(&hosted.session_id))
                .map(|hosted| hosted.last_seen)
                .max()?;
            Some((seen, record))
        })
        .max_by_key(|(seen, _)| *seen)
        .map(|(_, record)| record.clone())
}

/// A live mux counts as busy when its harness is running a session, so
/// resuming another one into it would mean typing into a live agent.
fn mux_is_busy(snapshot: &GraphSnapshot, mux_id: &NodeId, harness: &str) -> bool {
    let Some(GraphNode::MuxSession(node)) = snapshot.find_node(mux_id) else {
        return false;
    };
    mux::mux_runs_harness(snapshot, mux_id, node, harness)
}

/// Every session id in the lineage of `id`: its ancestors and
/// descendants, so a mux that hosted an older session of the chain
/// still counts.
fn lineage_ids(snapshot: &GraphSnapshot, id: &AgentSessionId) -> BTreeSet<String> {
    let mut seen: BTreeSet<AgentSessionId> = BTreeSet::from([id.clone()]);
    let mut frontier = vec![id.clone()];
    while let Some(current) = frontier.pop() {
        let node = NodeId::AgentSession(current.clone());
        for link in &snapshot.candidate_links {
            if link.relation != RelationKind::ParentSession
                || !matches!(link.state, LinkState::Active)
            {
                continue;
            }
            let other = match (&link.source, link.target_node_id()) {
                (NodeId::AgentSession(source), Some(target)) if *target == node => Some(source),
                (source, Some(NodeId::AgentSession(target))) if *source == node => Some(target),
                _ => None,
            };
            if let Some(other) = other
                && seen.insert(other.clone())
            {
                frontier.push(other.clone());
            }
        }
    }
    seen.into_iter().map(|s| s.session_key).collect()
}

fn session_cwd(snapshot: &GraphSnapshot, id: &AgentSessionId) -> Option<String> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(s) if s.id == *id => s.cwd.clone(),
        _ => None,
    })
}

/// A new mux for the session: named after `base`, made unique, started
/// in the session's own directory.
fn new_mux_plan(
    snapshot: &GraphSnapshot,
    id: &AgentSessionId,
    cwd: &Option<String>,
    base: &str,
    options: &Options,
    env: &Env<'_>,
    now: i64,
) -> Result<Plan, RebuildError> {
    let name = unique_mux_name(snapshot, base);
    let cwd = cwd.clone().ok_or_else(|| {
        RebuildError::Cannot(format!(
            "session `{}` has no recorded working directory to start a mux in",
            short(&id.session_key)
        ))
    })?;
    let subject = Subject {
        slot: PinSlot {
            id: name.clone(),
            kind: PinKind::Mux,
            target: None,
            pinned_at: None,
            overrides: SlotFields::from([
                ("name".to_string(), FieldValue::Text(name)),
                ("cwd".to_string(), FieldValue::Text(cwd)),
                (
                    "harness".to_string(),
                    FieldValue::Text(id.harness_key.clone()),
                ),
            ]),
            snapshot: SlotFields::new(),
            order: u32::MAX,
            legacy: false,
        },
        source: SubjectSource::Node,
    };
    mux::plan(snapshot, &subject, options, env, now)
}

/// `base`, or `base-2`, `base-3`, … when a mux or record already has it.
fn unique_mux_name(snapshot: &GraphSnapshot, base: &str) -> String {
    let taken =
        |name: &str| {
            snapshot.nodes.iter().any(
                |node| matches!(node, GraphNode::MuxSession(mux) if mux.session_name() == name),
            ) || snapshot
                .history
                .muxes
                .iter()
                .any(|record| record.recipe.name == name)
        };
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|name| !taken(name))
        .expect("an unbounded range has a free name")
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
