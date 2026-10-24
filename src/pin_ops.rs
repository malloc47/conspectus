//! Pin slot gestures shared by the CLI and the TUI (ADR 0141): pin an
//! entity, edit a slot's overrides, retarget it, and refresh its
//! snapshot. Each works on the graph snapshot and the slot store, so
//! both surfaces behave the same.

use std::fmt;

use crate::history::{HistoryWriter, observe, recorder::RecordContext};
use crate::model::{
    FieldValue, GraphSnapshot, HistoryKind, NodeId, PinKind, PinSlot, SlotFields, SlotStatus,
};
use crate::pin_slots::{SlotError, SlotStore, validate_id};
use crate::resolve::slots::{ResolvedSlot, resolve_slot};

#[derive(Debug)]
pub enum PinError {
    /// The kind can't be pinned (runtime processes) or isn't a node.
    NotPinnable(String),
    /// Nothing in the graph or history matches.
    NotFound(String),
    /// The slot exists but is a different kind than the new target.
    KindMismatch {
        slot: PinKind,
        target: PinKind,
    },
    /// A mux, checkout, or repo needs a history record to point at, and
    /// there is no history store to make one in.
    NoHistory(String),
    Slot(SlotError),
    Invalid(String),
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPinnable(what) => write!(f, "{what} can't be pinned"),
            Self::NotFound(what) => write!(
                f,
                "nothing matches `{what}`: pass a node id, a history id (h_…), or a mux name"
            ),
            Self::KindMismatch { slot, target } => write!(
                f,
                "this pin is a {} pin; it can't point at a {}",
                slot.as_str(),
                target.as_str()
            ),
            Self::NoHistory(what) => write!(
                f,
                "{what} needs a history record to point at, and no history store is available"
            ),
            Self::Slot(err) => write!(f, "{err}"),
            Self::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PinError {}

impl From<SlotError> for PinError {
    fn from(err: SlotError) -> Self {
        Self::Slot(err)
    }
}

/// What to pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pinnable {
    Node(NodeId),
    /// A history record, by id.
    Record(String),
}

/// Find what `input` names: a history id, a node id, or a mux name.
pub fn find_pinnable(snapshot: &GraphSnapshot, input: &str) -> Result<Pinnable, PinError> {
    let input = input.trim();
    if snapshot.history.meta(input).is_some() {
        return Ok(Pinnable::Record(input.to_string()));
    }
    if let Some(id) = NodeId::parse(input)
        && snapshot.find_node(&id).is_some()
    {
        return Ok(Pinnable::Node(id));
    }
    let muxes: Vec<NodeId> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            crate::model::GraphNode::MuxSession(mux)
                if mux.session_name() == input || mux.qualified_name() == input =>
            {
                Some(NodeId::MuxSession(mux.id.clone()))
            }
            _ => None,
        })
        .collect();
    match muxes.as_slice() {
        [id] => Ok(Pinnable::Node(id.clone())),
        [] => Err(PinError::NotFound(input.to_string())),
        many => Err(PinError::Invalid(format!(
            "`{input}` matches {} muxes: {}",
            many.len(),
            many.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// A target resolved to a slot kind and the `target` text to store.
struct Resolved {
    kind: PinKind,
    target: String,
}

fn resolve_target(
    snapshot: &GraphSnapshot,
    history: Option<&HistoryWriter>,
    what: &Pinnable,
) -> Result<Resolved, PinError> {
    match what {
        Pinnable::Record(id) => {
            let (kind, _) = snapshot
                .history
                .meta(id)
                .ok_or_else(|| PinError::NotFound(id.clone()))?;
            let kind = match kind {
                HistoryKind::Mux => PinKind::Mux,
                HistoryKind::Checkout => PinKind::Checkout,
                HistoryKind::Repo => PinKind::Repo,
                // A PR slot points at the PR, not its snapshot.
                HistoryKind::Pr => {
                    let record = snapshot.history.pr(id).expect("record exists");
                    return Ok(Resolved {
                        kind: PinKind::Pr,
                        target: pr_node_id(record).to_string(),
                    });
                }
            };
            Ok(Resolved {
                kind,
                target: PinSlot::history_target(id),
            })
        }
        Pinnable::Node(id) => {
            let kind = PinKind::of_node(id)
                .ok_or_else(|| PinError::NotPinnable(format!("a {}", node_kind_name(id))))?;
            if !kind.targets_history() {
                return Ok(Resolved {
                    kind,
                    target: id.to_string(),
                });
            }
            let record = match snapshot.history.by_key(&id.to_string()) {
                Some((_, meta)) => meta.id.clone(),
                None => ensure_record(snapshot, history, id)?,
            };
            Ok(Resolved {
                kind,
                target: PinSlot::history_target(&record),
            })
        }
    }
}

fn pr_node_id(record: &crate::model::PrRecord) -> NodeId {
    let r = &record.recipe;
    NodeId::ForgePr(crate::model::ForgePrId::new(
        r.provider.clone(),
        r.host.clone(),
        r.owner.clone(),
        r.repo.clone(),
        r.number,
    ))
}

fn node_kind_name(id: &NodeId) -> &'static str {
    match id {
        NodeId::RuntimeProcess(_) => "runtime process (it is ephemeral)",
        NodeId::Pin(_) => "pin",
        _ => "node",
    }
}

/// Make sure a live mux, checkout, or repo has a history record, so a
/// slot has something stable to point at, and return its id.
fn ensure_record(
    snapshot: &GraphSnapshot,
    history: Option<&HistoryWriter>,
    id: &NodeId,
) -> Result<String, PinError> {
    let writer = history.ok_or_else(|| PinError::NoHistory(id.to_string()))?;
    let config = crate::config::HistoryConfig::default();
    let observations = observe(
        snapshot,
        &RecordContext {
            fresh_since: 0,
            config: &config,
            home: writer.home.as_deref(),
        },
    );
    let key = id.to_string();
    let mut only = crate::history::Observations {
        muxes: observations
            .muxes
            .into_iter()
            .filter(|o| o.key == key)
            .collect(),
        ..Default::default()
    };
    only.checkouts = observations
        .checkouts
        .into_iter()
        .filter(|o| o.key == key)
        .collect();
    only.repos = observations
        .repos
        .into_iter()
        .filter(|o| o.key == key)
        .collect();
    if only.is_empty() {
        return Err(PinError::NotFound(key));
    }
    let (load, outcome) = writer
        .store
        .record(&only)
        .map_err(|err| PinError::Invalid(format!("could not record `{key}` in history: {err}")))?;
    // A file this build must not rewrite (a newer schema, a corrupt
    // record) keeps the record out, so a slot would point at nothing.
    if let Some(kind) = outcome
        .changed
        .iter()
        .find(|kind| !load.is_writable(**kind))
    {
        let why = load.problems.join("; ");
        return Err(PinError::Invalid(format!(
            "could not record `{key}` in history: the {} file can't be written ({why})",
            kind.as_str()
        )));
    }
    outcome
        .mux_ids
        .into_iter()
        .chain(outcome.checkout_ids)
        .chain(outcome.repo_ids)
        .next()
        .ok_or(PinError::NotFound(key))
}

/// A readable default name for a slot: the entity's own name.
pub fn default_slot_name(kind: PinKind, resolved: &ResolvedSlot) -> String {
    let base = |path: &str| {
        std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
    };
    let name = match kind {
        PinKind::Mux => resolved.text("name").map(str::to_string),
        PinKind::Session => resolved
            .text("title")
            .filter(|title| !title.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                resolved
                    .text("session_id")
                    .map(|id| id.get(..8).unwrap_or(id).to_string())
            }),
        PinKind::Checkout => resolved.text("root").map(base),
        PinKind::Repo => resolved
            .text("common_dir")
            .map(crate::history::recorder::repo_name),
        PinKind::Pr => resolved.text("number").map(|n| format!("pr-{n}")),
        PinKind::Branch => resolved
            .text("refname")
            .map(|r| crate::history::recorder::short_ref(r).to_string()),
        PinKind::Workspace => resolved.text("root").map(base),
        PinKind::Fork => resolved.text("name").map(str::to_string),
    };
    sanitize(&name.unwrap_or_else(|| kind.as_str().to_string()))
}

/// A slot name is one trimmed line; runs of whitespace become `-`.
fn sanitize(name: &str) -> String {
    let joined = name.split_whitespace().collect::<Vec<_>>().join("-");
    if joined.is_empty() {
        "pin".to_string()
    } else {
        joined
    }
}

fn unique_name(base: &str, taken: &[PinSlot]) -> String {
    if !taken.iter().any(|slot| slot.id == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|name| !taken.iter().any(|slot| &slot.id == name))
        .expect("an unbounded range has a free name")
}

/// The snapshot to store for a resolved target: every effective field,
/// and a label.
fn snapshot_of(resolved: &ResolvedSlot, fallback_label: &str) -> SlotFields {
    let mut fields = SlotFields::new();
    for field in &resolved.effective {
        if field.name != "label" {
            fields.insert(field.name.clone(), field.value.clone());
        }
    }
    let label = resolved
        .effective
        .iter()
        .find(|f| f.name == "label")
        .filter(|f| f.source != crate::model::FieldSource::Default)
        .and_then(|f| f.value.as_text().map(str::to_string))
        .unwrap_or_else(|| fallback_label.to_string());
    fields.insert("label".into(), FieldValue::Text(label));
    fields
}

fn now_rfc3339() -> String {
    chrono::DateTime::from_timestamp(crate::discovery::current_epoch(), 0)
        .map_or_else(String::new, |t| {
            t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
}

/// Pin an entity: add a slot targeting it, with a snapshot of its
/// current recipe and no overrides. `id` defaults to the entity's name,
/// made unique. A live mux, checkout, or repo with no history record
/// gets one first.
pub fn pin(
    snapshot: &GraphSnapshot,
    store: &SlotStore,
    history: Option<&HistoryWriter>,
    what: &Pinnable,
    id: Option<&str>,
) -> Result<PinSlot, PinError> {
    let resolved = resolve_target(snapshot, history, what)?;
    // Resolve against the record just made, which the snapshot can't
    // see yet, by building the slot from its snapshot-independent
    // parts when the record is new.
    let transient = PinSlot {
        id: String::new(),
        kind: resolved.kind,
        target: Some(resolved.target.clone()),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    };
    let effective = resolve_new_target(snapshot, &transient, what);
    let existing = store.load().slots;
    let name = match id {
        Some(id) => {
            validate_id(id)?;
            id.to_string()
        }
        None => unique_name(&default_slot_name(resolved.kind, &effective), &existing),
    };
    let slot = PinSlot {
        id: name.clone(),
        kind: resolved.kind,
        target: Some(resolved.target),
        pinned_at: Some(now_rfc3339()),
        overrides: SlotFields::new(),
        snapshot: snapshot_of(&effective, &name),
        order: 0,
        legacy: false,
    };
    store.add(slot.clone())?;
    Ok(slot)
}

/// Resolve a slot whose target was just chosen. A record made a moment
/// ago isn't in `snapshot` yet, so a slot that comes out Missing is
/// resolved again from the live node it was made from.
fn resolve_new_target(snapshot: &GraphSnapshot, slot: &PinSlot, what: &Pinnable) -> ResolvedSlot {
    let resolved = resolve_slot(snapshot, slot);
    match what {
        Pinnable::Node(node) if resolved.status == SlotStatus::Missing => resolve_slot(
            snapshot,
            &PinSlot {
                target: Some(node.to_string()),
                ..slot.clone()
            },
        ),
        _ => resolved,
    }
}

/// A planned slot: no target yet, built by its first rebuild.
pub fn pin_planned(
    store: &SlotStore,
    id: &str,
    kind: PinKind,
    overrides: SlotFields,
) -> Result<PinSlot, PinError> {
    validate_id(id)?;
    let slot = PinSlot {
        id: id.to_string(),
        kind,
        target: None,
        pinned_at: Some(now_rfc3339()),
        snapshot: SlotFields::from([("label".to_string(), FieldValue::Text(id.to_string()))]),
        overrides,
        order: 0,
        legacy: false,
    };
    store.add(slot.clone())?;
    Ok(slot)
}

/// Pin a history record directly, with the recipe fields to snapshot.
/// Used by `mux new --pin` and `mux launch --pin`, where the record
/// was just made from a launch and the graph can't see it yet.
pub fn pin_record(
    store: &SlotStore,
    id: &str,
    kind: PinKind,
    record_id: &str,
    mut snapshot: SlotFields,
) -> Result<PinSlot, PinError> {
    validate_id(id)?;
    snapshot
        .entry("label".to_string())
        .or_insert_with(|| FieldValue::Text(id.to_string()));
    let slot = PinSlot {
        id: id.to_string(),
        kind,
        target: Some(PinSlot::history_target(record_id)),
        pinned_at: Some(now_rfc3339()),
        overrides: SlotFields::new(),
        snapshot,
        order: 0,
        legacy: false,
    };
    store.add(slot.clone())?;
    Ok(slot)
}

/// Parse a `field=value` override: lists are shell-split.
pub fn parse_override(kind: PinKind, assignment: &str) -> Result<(String, FieldValue), PinError> {
    let (field, value) = assignment
        .split_once('=')
        .ok_or_else(|| PinError::Invalid(format!("`{assignment}` isn't `field=value`")))?;
    let field = field.trim();
    if !kind.allows_override(field) {
        return Err(PinError::Invalid(format!(
            "a {} pin has no overridable field `{field}` (allowed: label, {})",
            kind.as_str(),
            kind.overridable_fields().join(", ")
        )));
    }
    let value = if PinKind::field_is_list(field) {
        FieldValue::List(crate::argv::split_command_line(value))
    } else {
        FieldValue::Text(value.to_string())
    };
    Ok((field.to_string(), value))
}

/// Set and clear overrides on a slot.
pub fn edit(
    store: &SlotStore,
    id: &str,
    set: &[(String, FieldValue)],
    unset: &[String],
) -> Result<bool, PinError> {
    let write = store.update(id, |slot| {
        for field in unset {
            slot.overrides.remove(field);
        }
        for (field, value) in set {
            slot.overrides.insert(field.clone(), value.clone());
        }
        Ok(())
    })?;
    Ok(write.changed)
}

/// Point a slot at another entity of the same kind, live or from
/// history. Its snapshot is refreshed to the new target.
pub fn retarget(
    snapshot: &GraphSnapshot,
    store: &SlotStore,
    history: Option<&HistoryWriter>,
    id: &str,
    what: &Pinnable,
) -> Result<PinSlot, PinError> {
    let slot = store
        .load()
        .slots
        .into_iter()
        .find(|slot| slot.id == id)
        .ok_or_else(|| PinError::Slot(SlotError::NotFound(id.to_string())))?;
    let resolved = resolve_target(snapshot, history, what)?;
    if resolved.kind != slot.kind {
        return Err(PinError::KindMismatch {
            slot: slot.kind,
            target: resolved.kind,
        });
    }
    let probe = PinSlot {
        target: Some(resolved.target.clone()),
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        ..slot.clone()
    };
    let effective = resolve_new_target(snapshot, &probe, what);
    let fresh = snapshot_of(&effective, &slot.label());
    store.update(id, |slot| {
        slot.target = Some(resolved.target.clone());
        slot.snapshot = fresh.clone();
        Ok(())
    })?;
    let mut updated = slot;
    updated.target = Some(resolved.target);
    updated.snapshot = fresh;
    Ok(updated)
}

/// Rename a slot. A label that was only the old name (planned slots and
/// unnamed pins start that way) follows it; a label the operator or the
/// entity chose stays.
pub fn rename(store: &SlotStore, id: &str, new_id: &str) -> Result<bool, PinError> {
    let write = store.rename(id, new_id)?;
    store.update(new_id, |slot| {
        if slot.snapshot.get("label").and_then(FieldValue::as_text) == Some(id) {
            slot.snapshot
                .insert("label".into(), FieldValue::Text(new_id.to_string()));
        }
        Ok(())
    })?;
    Ok(write.changed)
}

/// Copy a slot's current effective recipe into its snapshot, so it
/// still renders and rebuilds if history is lost.
pub fn refresh_snapshot(
    snapshot: &GraphSnapshot,
    store: &SlotStore,
    id: &str,
) -> Result<bool, PinError> {
    let slot = store
        .load()
        .slots
        .into_iter()
        .find(|slot| slot.id == id)
        .ok_or_else(|| PinError::Slot(SlotError::NotFound(id.to_string())))?;
    // Overrides stay in `overrides`; the snapshot records what the
    // entity itself looks like.
    let mut probe = slot.clone();
    probe.overrides.clear();
    let entity = resolve_slot(snapshot, &probe);
    let fresh = snapshot_of(&entity, &slot.label());
    Ok(store
        .update(id, |slot| {
            slot.snapshot = fresh.clone();
            Ok(())
        })?
        .changed)
}

#[cfg(test)]
#[path = "pin_ops_tests.rs"]
mod tests;
