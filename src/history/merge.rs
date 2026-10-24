//! The commutative merge of a writer's observations into history.
//!
//! Every rule here is order-independent, so writers can apply their
//! observations in any order and agree: `first_seen` takes the minimum
//! and `last_seen` the maximum, hosted-session lists are unioned and
//! then truncated, and mutable recipe fields come from the newest
//! sighting. A writer merges what it observed, never a copy of the
//! store, so a reap stays reaped until something sees the entity
//! again.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    ArgvSource, CheckoutRecipe, CheckoutRecord, HOSTED_SESSIONS_KEPT, HistoryKind, HistorySnapshot,
    HostedSession, MuxIncarnation, MuxRecipe, MuxRecord, PrRecipe, PrRecord, Record, RecordDisplay,
    RecordMeta, RecordOrigin, RepoRecipe, RepoRecord, effective_socket,
};

use super::{LAST_SEEN_CADENCE_SECS, new_record_id};

/// What one writer saw in one cycle. Each observation carries the
/// time its probe or listing ran, so data reused from an earlier cycle
/// can't move a record's `last_seen` past when it was really seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observations {
    pub muxes: Vec<MuxObservation>,
    pub checkouts: Vec<CheckoutObservation>,
    pub repos: Vec<RepoObservation>,
    pub prs: Vec<PrObservation>,
}

impl Observations {
    pub fn is_empty(&self) -> bool {
        self.muxes.is_empty()
            && self.checkouts.is_empty()
            && self.repos.is_empty()
            && self.prs.is_empty()
    }
}

/// A mux a probe listed (or a launch created). `recipe.hosted` holds
/// the sessions attributed to it this cycle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MuxObservation {
    pub seen_at: i64,
    pub key: String,
    pub recipe: MuxRecipe,
    pub display: RecordDisplay,
    pub origin: RecordOrigin,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckoutObservation {
    pub seen_at: i64,
    pub key: String,
    pub recipe: CheckoutRecipe,
    pub display: RecordDisplay,
    pub origin: RecordOrigin,
    /// The record a rebuild replaced, set by the rebuild path.
    pub rebuilt_from: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepoObservation {
    pub seen_at: i64,
    pub key: String,
    pub recipe: RepoRecipe,
    pub display: RecordDisplay,
    pub origin: RecordOrigin,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrObservation {
    pub seen_at: i64,
    pub key: String,
    pub recipe: PrRecipe,
    pub display: RecordDisplay,
}

/// What a merge changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeOutcome {
    /// Kinds whose records changed, so their files need writing.
    pub changed: BTreeSet<HistoryKind>,
    /// The record id each mux observation landed on, by observation
    /// index.
    pub mux_ids: Vec<String>,
    pub checkout_ids: Vec<String>,
    pub repo_ids: Vec<String>,
    pub pr_ids: Vec<String>,
}

impl MergeOutcome {
    pub fn changed(kind: HistoryKind) -> Self {
        let mut outcome = Self::default();
        outcome.changed.insert(kind);
        outcome
    }
}

/// Merge `observations` into `history`.
pub fn merge(history: &mut HistorySnapshot, observations: &Observations) -> MergeOutcome {
    let mut ids: BTreeSet<String> = history.metas().map(|(_, meta)| meta.id.clone()).collect();
    let mut outcome = MergeOutcome::default();

    let (mux_ids, changed) = merge_muxes(&mut history.muxes, &observations.muxes, &mut ids);
    outcome.mux_ids = mux_ids;
    if changed {
        outcome.changed.insert(HistoryKind::Mux);
    }

    let (checkout_ids, changed) = merge_keyed(
        &mut history.checkouts,
        &observations.checkouts,
        &mut ids,
        |obs| (&obs.key, obs.seen_at),
        |obs, id| CheckoutRecord {
            meta: new_meta(id, &obs.key, obs.seen_at, obs.origin, &obs.display),
            recipe: obs.recipe.clone(),
            extra: Default::default(),
        },
        |record, obs| {
            let recipe = &mut record.recipe;
            recipe.root.clone_from(&obs.recipe.root);
            recipe.repo.clone_from(&obs.recipe.repo);
            recipe.linked = obs.recipe.linked;
            set_if_some(&mut recipe.branch, &obs.recipe.branch);
            set_if_some(&mut recipe.head, &obs.recipe.head);
            set_if_some(&mut recipe.upstream, &obs.recipe.upstream);
            record.meta.display = obs.display.clone();
        },
        |record, obs| {
            if record.meta.rebuilt_from.is_none() {
                record.meta.rebuilt_from.clone_from(&obs.rebuilt_from);
            }
        },
    );
    outcome.checkout_ids = checkout_ids;
    if changed {
        outcome.changed.insert(HistoryKind::Checkout);
    }

    let (repo_ids, changed) = merge_keyed(
        &mut history.repos,
        &observations.repos,
        &mut ids,
        |obs| (&obs.key, obs.seen_at),
        |obs, id| RepoRecord {
            meta: new_meta(id, &obs.key, obs.seen_at, obs.origin, &obs.display),
            recipe: obs.recipe.clone(),
            extra: Default::default(),
        },
        |record, obs| {
            let recipe = &mut record.recipe;
            recipe.common_dir.clone_from(&obs.recipe.common_dir);
            if !obs.recipe.remotes.is_empty() {
                recipe.remotes.clone_from(&obs.recipe.remotes);
            }
            set_if_some(&mut recipe.primary_root, &obs.recipe.primary_root);
            set_if_some(&mut recipe.default_branch, &obs.recipe.default_branch);
            record.meta.display = obs.display.clone();
        },
        |_, _| {},
    );
    outcome.repo_ids = repo_ids;
    if changed {
        outcome.changed.insert(HistoryKind::Repo);
    }

    let (pr_ids, changed) = merge_keyed(
        &mut history.prs,
        &observations.prs,
        &mut ids,
        |obs| (&obs.key, obs.seen_at),
        |obs, id| PrRecord {
            meta: new_meta(
                id,
                &obs.key,
                obs.seen_at,
                RecordOrigin::Observed,
                &obs.display,
            ),
            recipe: obs.recipe.clone(),
            extra: Default::default(),
        },
        |record, obs| {
            let extra = std::mem::take(&mut record.recipe.extra);
            record.recipe = PrRecipe {
                extra,
                ..obs.recipe.clone()
            };
            record.meta.display = obs.display.clone();
        },
        |_, _| {},
    );
    outcome.pr_ids = pr_ids;
    if changed {
        outcome.changed.insert(HistoryKind::Pr);
    }

    outcome
}

fn new_meta(
    id: String,
    key: &str,
    seen_at: i64,
    origin: RecordOrigin,
    display: &RecordDisplay,
) -> RecordMeta {
    RecordMeta {
        id,
        key: key.to_string(),
        first_seen: seen_at,
        last_seen: seen_at,
        origin,
        display: display.clone(),
        rebuilt_from: None,
    }
}

fn set_if_some<T: Clone>(slot: &mut Option<T>, value: &Option<T>) {
    if value.is_some() {
        slot.clone_from(value);
    }
}

/// Record-level bookkeeping shared by every kind: the first and last
/// sightings and the 10-minute rewrite cadence. `apply` updates the
/// mutable fields; it runs only for a sighting at least as new as the
/// record's, so an older writer can't roll fields back.
fn touch<R: Record + Clone + PartialEq>(
    record: &mut R,
    key: &str,
    seen_at: i64,
    apply: impl FnOnce(&mut R),
) -> bool {
    let before = record.clone();
    let newer = seen_at >= record.meta().last_seen;
    {
        let meta = record.meta_mut();
        meta.first_seen = meta.first_seen.min(seen_at);
        if newer {
            meta.key = key.to_string();
        }
    }
    if newer {
        apply(record);
    }
    let fields_changed = *record != before;
    let meta = record.meta_mut();
    if seen_at > meta.last_seen
        && (fields_changed || seen_at - meta.last_seen >= LAST_SEEN_CADENCE_SECS)
    {
        meta.last_seen = seen_at;
    }
    *record != before
}

/// Merge kinds that match on their key: checkouts (repo and root),
/// repos (common dir), and PRs (PR id). `key_of` gives an
/// observation's key and when it was seen.
fn merge_keyed<R: Record + Clone + PartialEq, O>(
    records: &mut Vec<R>,
    observations: &[O],
    ids: &mut BTreeSet<String>,
    key_of: fn(&O) -> (&String, i64),
    create: impl Fn(&O, String) -> R,
    update: impl Fn(&mut R, &O),
    always: impl Fn(&mut R, &O),
) -> (Vec<String>, bool) {
    let mut changed = false;
    let mut out = Vec::with_capacity(observations.len());
    let mut index: BTreeMap<String, usize> = records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.meta().key.clone(), i))
        .collect();
    for obs in observations {
        let (key, seen_at) = key_of(obs);
        if let Some(&i) = index.get(key) {
            let record = &mut records[i];
            let before = record.clone();
            touch(record, key, seen_at, |r| update(r, obs));
            always(record, obs);
            changed |= *record != before;
            out.push(record.meta().id.clone());
        } else {
            let id = new_record_id(ids);
            ids.insert(id.clone());
            let mut record = create(obs, id.clone());
            always(&mut record, obs);
            records.push(record);
            index.insert(key.clone(), records.len() - 1);
            changed = true;
            out.push(id);
        }
    }
    (out, changed)
}

/// Muxes match by incarnation first, then by key, then become new
/// records (ADR 0140). Every incarnation match is made before any key
/// match, so a rename plus a new session under the old name land on
/// the right records.
fn merge_muxes(
    records: &mut Vec<MuxRecord>,
    observations: &[MuxObservation],
    ids: &mut BTreeSet<String>,
) -> (Vec<String>, bool) {
    let mut matched: Vec<Option<usize>> = vec![None; observations.len()];
    let mut taken: BTreeSet<usize> = BTreeSet::new();

    for (i, obs) in observations.iter().enumerate() {
        let Some(incarnation) = obs
            .recipe
            .incarnation
            .as_ref()
            .filter(|inc| inc.is_complete())
        else {
            continue;
        };
        if let Some(r) = records.iter().enumerate().position(|(r, record)| {
            !taken.contains(&r)
                && same_server(&record.recipe, &obs.recipe)
                && record.recipe.incarnation.as_ref() == Some(incarnation)
        }) {
            matched[i] = Some(r);
            taken.insert(r);
        }
    }
    for (i, obs) in observations.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        if let Some(r) = records.iter().enumerate().position(|(r, record)| {
            !taken.contains(&r)
                && same_server(&record.recipe, &obs.recipe)
                && record.recipe.name == obs.recipe.name
        }) {
            matched[i] = Some(r);
            taken.insert(r);
        }
    }

    let mut changed = false;
    let mut out = Vec::with_capacity(observations.len());
    for (i, obs) in observations.iter().enumerate() {
        if let Some(r) = matched[i] {
            let record = &mut records[r];
            changed |= touch(record, &obs.key, obs.seen_at, |record| {
                update_mux(record, obs);
            });
            // Hosted sessions union regardless of which sighting is
            // newer, so they don't depend on write order.
            let before = record.recipe.hosted.clone();
            union_hosted(&mut record.recipe.hosted, &obs.recipe.hosted);
            changed |= record.recipe.hosted != before;
            out.push(record.meta.id.clone());
        } else {
            let id = new_record_id(ids);
            ids.insert(id.clone());
            let mut recipe = obs.recipe.clone();
            recipe.hosted.clear();
            union_hosted(&mut recipe.hosted, &obs.recipe.hosted);
            records.push(MuxRecord {
                meta: new_meta(id.clone(), &obs.key, obs.seen_at, obs.origin, &obs.display),
                recipe,
                extra: Default::default(),
            });
            changed = true;
            out.push(id);
        }
    }
    (out, changed)
}

fn same_server(a: &MuxRecipe, b: &MuxRecipe) -> bool {
    a.backend == b.backend
        && effective_socket(a.socket.as_deref()) == effective_socket(b.socket.as_deref())
}

fn update_mux(record: &mut MuxRecord, obs: &MuxObservation) {
    let new = &obs.recipe;
    let recipe = &mut record.recipe;
    let reborn = match (&recipe.incarnation, &new.incarnation) {
        (Some(old), Some(new)) => old != new,
        _ => false,
    };
    if reborn {
        // The name came back as a new session: sessions it hosted
        // before don't carry over unless they were active after it
        // was created (ADR 0102's rule).
        if let Some(created) = new.incarnation.as_ref().and_then(|inc| inc.created) {
            recipe.hosted.retain(|hosted| hosted.last_seen >= created);
        }
        recipe.argv = None;
        recipe.argv_source = None;
    }
    recipe.name.clone_from(&new.name);
    set_if_some(&mut recipe.cwd, &new.cwd);
    if let Some(incarnation) = &new.incarnation {
        merge_incarnation(&mut recipe.incarnation, incarnation);
    }
    // An observed `pane_start_command` never overwrites the argv
    // Conspectus itself launched with: they name the same command, and
    // the recorded one keeps its exact tokens.
    let keep_launched = recipe.argv_source == Some(ArgvSource::Conspectus)
        && new.argv_source == Some(ArgvSource::Observed);
    if new.argv.is_some() && !keep_launched {
        recipe.argv.clone_from(&new.argv);
        recipe.argv_source = new.argv_source;
    }
    set_if_some(&mut recipe.harness, &new.harness);
    set_if_some(&mut recipe.orchestrator, &new.orchestrator);
    record.meta.display = obs.display.clone();
}

fn merge_incarnation(slot: &mut Option<MuxIncarnation>, new: &MuxIncarnation) {
    match slot {
        Some(old) if old.session_id == new.session_id || new.session_id.is_none() => {
            if old.created.is_none() {
                old.created = new.created;
            }
            if old.session_id.is_none() {
                old.session_id.clone_from(&new.session_id);
            }
        }
        _ => *slot = Some(new.clone()),
    }
}

/// Union by (harness, session id), keeping the earliest first sighting
/// and the latest last one, newest first, at most
/// [`HOSTED_SESSIONS_KEPT`].
pub fn union_hosted(into: &mut Vec<HostedSession>, from: &[HostedSession]) {
    for hosted in from {
        match into
            .iter_mut()
            .find(|h| h.harness == hosted.harness && h.session_id == hosted.session_id)
        {
            Some(existing) => {
                existing.first_seen = existing.first_seen.min(hosted.first_seen);
                existing.last_seen = existing.last_seen.max(hosted.last_seen);
            }
            None => into.push(hosted.clone()),
        }
    }
    into.sort_by(|a, b| {
        b.last_seen
            .cmp(&a.last_seen)
            .then_with(|| a.harness.cmp(&b.harness))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    into.truncate(HOSTED_SESSIONS_KEPT);
}
