//! Turning what the operator typed into something to rebuild.

use crate::model::{GraphNode, GraphSnapshot, HistoryKind, NodeId, PinKind, PinSlot, SlotFields};

use super::RebuildError;

/// Where a subject came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubjectSource {
    /// A pin slot, with its overrides.
    Slot,
    /// A history record (`h_…`).
    History,
    /// A node in the graph, live or gone.
    Node,
}

/// What a rebuild works from: a slot, or a transient one standing in
/// for a history record or node so every source resolves the same way.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    pub slot: PinSlot,
    pub source: SubjectSource,
}

fn transient(id: &str, kind: PinKind, target: String) -> PinSlot {
    PinSlot {
        id: id.to_string(),
        kind,
        target: Some(target),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: u32::MAX,
        legacy: false,
    }
}

fn pin_kind_of_history(kind: HistoryKind) -> PinKind {
    match kind {
        HistoryKind::Mux => PinKind::Mux,
        HistoryKind::Checkout => PinKind::Checkout,
        HistoryKind::Repo => PinKind::Repo,
        HistoryKind::Pr => PinKind::Pr,
    }
}

/// Resolve `input`: a slot name, a history id, a node id, or a mux
/// name (`work`, or `tmux:work`).
pub fn resolve_subject(
    snapshot: &GraphSnapshot,
    slots: &[PinSlot],
    input: &str,
) -> Result<Subject, RebuildError> {
    let input = input.trim();
    if let Some(slot) = slots.iter().find(|slot| slot.id == input) {
        return Ok(Subject {
            slot: slot.clone(),
            source: SubjectSource::Slot,
        });
    }

    if let Some((kind, meta)) = snapshot.history.meta(input) {
        return Ok(Subject {
            slot: transient(
                input,
                pin_kind_of_history(kind),
                PinSlot::history_target(&meta.id),
            ),
            source: SubjectSource::History,
        });
    }

    if let Some(id) = NodeId::parse(input) {
        return from_node(snapshot, input, &id).ok_or_else(|| RebuildError::NotFound(input.into()));
    }

    // A mux by name: `work`, or `tmux:work`.
    let matches: Vec<NodeId> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux)
                if mux.session_name() == input || mux.qualified_name() == input =>
            {
                Some(NodeId::MuxSession(mux.id.clone()))
            }
            _ => None,
        })
        .collect();
    match matches.as_slice() {
        [] => Err(RebuildError::NotFound(input.into())),
        [id] => from_node(snapshot, input, id).ok_or_else(|| RebuildError::NotFound(input.into())),
        _ => Err(RebuildError::Ambiguous {
            input: input.into(),
            candidates: matches
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

fn from_node(snapshot: &GraphSnapshot, input: &str, id: &NodeId) -> Option<Subject> {
    snapshot.find_node(id)?;
    let kind = PinKind::of_node(id).ok_or(()).ok()?;
    // A mux, worktree, or repo rebuilds from its history record when it
    // has one, since the record holds the recipe a gone node lost.
    let target = if kind.targets_history()
        && let Some((_, meta)) = snapshot.history.by_key(&id.to_string())
    {
        PinSlot::history_target(&meta.id)
    } else {
        id.to_string()
    };
    Some(Subject {
        slot: transient(input, kind, target),
        source: SubjectSource::Node,
    })
}
