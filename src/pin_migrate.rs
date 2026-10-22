//! `conspectus pin migrate` (ADR 0141): convert legacy
//! `[[pins.entries]]` into pin slots.
//!
//! Each entry becomes one mux slot that keeps its id. Its launch
//! settings seed a history record (with the session the old binding
//! sidecar last saw), the slot targets that record, and the slot
//! carries overrides only for what the record doesn't already say. A
//! display name that no longer shows anywhere (it differed from the mux
//! name and the session's title) is kept as an alias on the session.
//! Converted entries are then removed from their config files,
//! comments and other tables intact. Running it again finds nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::aliases::{AliasEntry, alias_node_from_node_id, upsert_alias_entry};
use crate::discovery::pins::LegacyEntry;
use crate::history::HistoryWriter;
use crate::history::merge::{MuxObservation, Observations};
use crate::legacy_bindings::{PinBindingsCache, read as read_sidecar};
use crate::legacy_pins::{PinEntry, remove_entries};
use crate::model::{
    AgentSessionId, ArgvSource, FieldValue, GraphNode, GraphSnapshot, HostedSession, MuxRecipe,
    NodeId, PinKind, PinSlot, RecordDisplay, RecordOrigin, SlotFields,
};
use crate::pin_slots::SlotStore;

/// One entry, planned.
#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub entry: PinEntry,
    /// The session the old sidecar last bound, if any.
    pub last_session: Option<LastSession>,
    /// The alias to keep, as `(session, name)`.
    pub alias: Option<(AgentSessionId, String)>,
    /// A slot this migration already made for the entry exists, so only
    /// the leftover entry needs removing.
    pub already_migrated: bool,
}

#[derive(Clone, Debug)]
pub struct LastSession {
    pub harness: String,
    pub session_id: String,
    pub observed_epoch: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub items: Vec<Item>,
    /// `(id, why)` for entries left alone.
    pub skipped: Vec<(String, String)>,
    /// Things worth saying that don't stop the migration, such as a
    /// sidecar that couldn't be read.
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.skipped.is_empty()
    }

    /// What `--dry-run` prints.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            let e = &item.entry;
            if item.already_migrated {
                out.push_str(&format!(
                    "pin `{}`: already migrated, removes the leftover entry from {}\n",
                    e.id,
                    item.path.display()
                ));
                continue;
            }
            out.push_str(&format!(
                "pin `{}` -> mux slot (mux `{}`, {}, {})\n",
                e.id, e.mux.name, e.harness, e.cwd
            ));
            if let Some(last) = &item.last_session {
                out.push_str(&format!("    keeps its last session {}\n", last.session_id));
            }
            if let Some((_, name)) = &item.alias {
                out.push_str(&format!("    keeps the name `{name}` as an alias\n"));
            }
            out.push_str(&format!(
                "    removes the entry from {}\n",
                item.path.display()
            ));
        }
        for (id, why) in &self.skipped {
            out.push_str(&format!("pin `{id}`: skipped, {why}\n"));
        }
        out
    }
}

/// Plan the migration of `entries` into a store that already holds
/// `existing`.
pub fn plan(
    snapshot: &GraphSnapshot,
    entries: Vec<LegacyEntry>,
    existing: &[PinSlot],
    sidecars: &PinBindingsCache,
) -> Plan {
    let mut plan = Plan::default();
    let mut taken: BTreeSet<&str> = existing.iter().map(|slot| slot.id.as_str()).collect();
    let owned: Vec<String> = entries.iter().map(|e| e.entry.id.clone()).collect();
    for (legacy, id) in entries.into_iter().zip(&owned) {
        let migrated_before = existing
            .iter()
            .find(|slot| slot.id == *id)
            .is_some_and(|slot| is_migration_of(snapshot, slot, &legacy.entry));
        if migrated_before {
            plan.items.push(Item {
                path: legacy.path,
                entry: legacy.entry,
                last_session: None,
                alias: None,
                already_migrated: true,
            });
            continue;
        }
        if taken.contains(id.as_str()) {
            plan.skipped.push((
                id.clone(),
                "a slot with this name already exists; rename one of them".to_string(),
            ));
            continue;
        }
        taken.insert(id.as_str());
        let record = match read_sidecar(sidecars, id) {
            Ok(record) => record,
            Err(err) => {
                plan.warnings.push(format!(
                    "pin `{id}`: its last-session file can't be used ({err}); the slot starts without a remembered session"
                ));
                None
            }
        };
        let last_session = record.map(|record| LastSession {
            harness: record.harness,
            session_id: record.session_id,
            observed_epoch: record.observed_epoch,
        });
        let alias = alias_to_keep(snapshot, &legacy.entry, last_session.as_ref());
        plan.items.push(Item {
            path: legacy.path,
            entry: legacy.entry,
            last_session,
            alias,
            already_migrated: false,
        });
    }
    plan
}

/// Whether `slot` is what an earlier run of this migration made for
/// `entry`: it targets a record seeded by migration for the same mux.
fn is_migration_of(snapshot: &GraphSnapshot, slot: &PinSlot, entry: &PinEntry) -> bool {
    let Some(record) = slot
        .target
        .as_deref()
        .and_then(|target| target.strip_prefix(PinSlot::HISTORY_PREFIX))
        .and_then(|id| snapshot.history.mux(id))
    else {
        return false;
    };
    record.meta.origin == RecordOrigin::Migrated
        && record.recipe.name == entry.mux.name
        && record.recipe.cwd.as_deref() == Some(entry.cwd.as_str())
}

/// A pin's display name used to name its bound session. Keep it when it
/// would otherwise be lost: it differs from the mux name and from the
/// session's own title.
fn alias_to_keep(
    snapshot: &GraphSnapshot,
    entry: &PinEntry,
    last: Option<&LastSession>,
) -> Option<(AgentSessionId, String)> {
    if entry.display_name == entry.mux.name || entry.display_name == entry.id {
        return None;
    }
    let last = last?;
    let session = snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(s)
            if s.id.harness_key == last.harness && s.id.session_key == last.session_id =>
        {
            Some(s)
        }
        _ => None,
    })?;
    if session.title.as_deref() == Some(entry.display_name.as_str()) {
        return None;
    }
    Some((session.id.clone(), entry.display_name.clone()))
}

/// What a run did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub migrated: Vec<String>,
    pub aliases: Vec<String>,
    pub removed_entries: usize,
    /// `(id, why)` for entries that couldn't be migrated and stay in
    /// their config files.
    pub failed: Vec<(String, String)>,
    pub notes: Vec<String>,
}

/// Where the migration writes.
pub struct Targets<'a> {
    pub slots: &'a SlotStore,
    pub history: &'a HistoryWriter,
    /// The user config, where aliases go.
    pub user_config: Option<&'a Path>,
    /// Used when an entry has no sidecar to date its record.
    pub fallback_epoch: i64,
}

/// Run `plan`.
pub fn run(plan: &Plan, targets: &Targets<'_>) -> Report {
    let mut report = Report::default();
    let mut removable: Vec<(PathBuf, String)> = Vec::new();
    for item in &plan.items {
        if item.already_migrated {
            removable.push((item.path.clone(), item.entry.id.clone()));
            continue;
        }
        match migrate_one(item, targets, &mut report) {
            Ok(()) => {
                report.migrated.push(item.entry.id.clone());
                removable.push((item.path.clone(), item.entry.id.clone()));
            }
            Err(why) => report.failed.push((item.entry.id.clone(), why)),
        }
    }

    // Remove the converted entries file by file, only once their slots
    // are written, so a failure never loses a pin.
    let mut by_file: Vec<(PathBuf, BTreeSet<String>)> = Vec::new();
    for (path, id) in removable {
        match by_file.iter_mut().find(|(p, _)| *p == path) {
            Some((_, ids)) => {
                ids.insert(id);
            }
            None => by_file.push((path, BTreeSet::from([id]))),
        }
    }
    for (path, ids) in by_file {
        match remove_entries(&path, &ids) {
            Ok(count) => report.removed_entries += count,
            Err(err) => report.notes.push(format!(
                "could not remove the migrated entries from {}: {err}; they remain there",
                path.display()
            )),
        }
    }
    report
}

fn migrate_one(item: &Item, targets: &Targets<'_>, report: &mut Report) -> Result<(), String> {
    let entry = &item.entry;
    let socket = entry.mux.effective_socket().map(str::to_string);
    let argv = entry
        .launch
        .as_ref()
        .map(|launch| launch.argv.clone())
        .filter(|argv| !argv.is_empty());
    let seen_at = item
        .last_session
        .as_ref()
        .map_or(targets.fallback_epoch, |last| last.observed_epoch);

    // Seed history. Dated at the sidecar's time, which is older than
    // any real sighting, so a record discovery already made keeps its
    // observed fields; a new one is dated when the pin last ran.
    let recipe = MuxRecipe {
        backend: entry.mux.backend.clone(),
        socket: socket.clone(),
        name: entry.mux.name.clone(),
        cwd: Some(entry.cwd.clone()),
        argv: argv.clone(),
        argv_source: argv.as_ref().map(|_| ArgvSource::Conspectus),
        harness: Some(entry.harness.clone()),
        hosted: item
            .last_session
            .iter()
            .map(|last| HostedSession {
                harness: last.harness.clone(),
                session_id: last.session_id.clone(),
                first_seen: last.observed_epoch,
                last_seen: last.observed_epoch,
            })
            .collect(),
        ..Default::default()
    };
    let key = NodeId::MuxSession(crate::model::MuxSessionId::new(recipe.native_id())).to_string();
    let (load, outcome) = targets
        .history
        .store
        .record(&Observations {
            muxes: vec![MuxObservation {
                seen_at,
                key,
                recipe,
                display: RecordDisplay {
                    label: entry.mux.name.clone(),
                    context: Some(format!("{} · {}", entry.harness, entry.cwd)),
                },
                origin: RecordOrigin::Migrated,
            }],
            ..Default::default()
        })
        .map_err(|err| format!("could not seed history: {err}"))?;
    let record_id = outcome
        .mux_ids
        .into_iter()
        .next()
        .ok_or("history didn't keep the record")?;
    let record = load
        .history
        .mux(&record_id)
        .ok_or("the seeded record is missing")?;

    // Overrides: what the pin said that the record doesn't.
    let text = |value: &str| FieldValue::Text(value.to_string());
    let mut overrides = SlotFields::new();
    let r = &record.recipe;
    if r.name != entry.mux.name {
        overrides.insert("name".into(), text(&entry.mux.name));
    }
    if crate::model::effective_socket(r.socket.as_deref()) != socket.as_deref()
        && let Some(socket) = &socket
    {
        overrides.insert("socket".into(), text(socket));
    }
    if r.cwd.as_deref() != Some(entry.cwd.as_str()) {
        overrides.insert("cwd".into(), text(&entry.cwd));
    }
    if r.harness.as_deref() != Some(entry.harness.as_str()) {
        overrides.insert("harness".into(), text(&entry.harness));
    }
    if let Some(argv) = &argv
        && r.argv.as_ref() != Some(argv)
    {
        overrides.insert("argv".into(), FieldValue::List(argv.clone()));
    }
    if let Some(worktree) = &entry.worktree {
        overrides.insert("worktree_branch".into(), text(&worktree.branch));
    }

    let mut snapshot = SlotFields::new();
    snapshot.insert("name".into(), text(&entry.mux.name));
    if let Some(socket) = &socket {
        snapshot.insert("socket".into(), text(socket));
    }
    snapshot.insert("cwd".into(), text(&entry.cwd));
    snapshot.insert("harness".into(), text(&entry.harness));
    if let Some(argv) = &argv {
        snapshot.insert("argv".into(), FieldValue::List(argv.clone()));
    }
    snapshot.insert("label".into(), text(&entry.display_name));

    targets
        .slots
        .add(PinSlot {
            id: entry.id.clone(),
            kind: PinKind::Mux,
            target: Some(PinSlot::history_target(&record_id)),
            pinned_at: None,
            overrides,
            snapshot,
            order: 0,
            legacy: false,
        })
        .map_err(|err| format!("could not write the slot: {err}"))?;

    if let Some((session, name)) = &item.alias
        && let Some(path) = targets.user_config
    {
        let entry = AliasEntry {
            node: alias_node_from_node_id(&NodeId::AgentSession(session.clone())),
            display_name: name.clone(),
            reason: Some(format!(
                "kept by `pin migrate` from pin `{}`",
                item.entry.id
            )),
        };
        match upsert_alias_entry(path, entry) {
            Ok(_) => report.aliases.push(name.clone()),
            Err(err) => report.notes.push(format!(
                "could not keep the name `{name}` as an alias: {err}"
            )),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "pin_migrate_tests.rs"]
mod tests;
