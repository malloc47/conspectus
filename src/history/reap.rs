//! Removing records from history (ADR 0140): `history forget`,
//! `history reap`, and `worktree close`. Only records go; nothing here
//! touches tmux, git, or the filesystem.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::config::HistoryConfig;
use crate::model::{
    GraphSnapshot, HistoryKind, HistorySnapshot, NodeId, NodePresence, PinSlot, Record,
    path_is_ancestor_of,
};

use super::merge::MergeOutcome;
use super::store::HistoryStore;

/// Remove every record `doomed` names, under the store's lock.
/// Returns how many were removed.
pub fn forget_where(
    store: &HistoryStore,
    mut doomed: impl FnMut(HistoryKind, &HistorySnapshot, &dyn RecordRef) -> bool,
) -> io::Result<usize> {
    let mut removed = 0usize;
    store.update(|history| {
        let mut outcome = MergeOutcome::default();
        macro_rules! sweep {
            ($field:ident, $kind:expr) => {{
                let snapshot = history.clone();
                let before = history.$field.len();
                history
                    .$field
                    .retain(|record| !doomed($kind, &snapshot, record));
                let gone = before - history.$field.len();
                if gone > 0 {
                    removed += gone;
                    outcome.changed.insert($kind);
                }
            }};
        }
        sweep!(muxes, HistoryKind::Mux);
        sweep!(checkouts, HistoryKind::Checkout);
        sweep!(repos, HistoryKind::Repo);
        sweep!(prs, HistoryKind::Pr);
        outcome
    })?;
    Ok(removed)
}

/// What a sweep predicate can read of any record.
pub trait RecordRef {
    fn id(&self) -> &str;
    /// The directory the record belongs to, if it has one: a mux's
    /// working directory, a checkout's root, a repo's primary root.
    fn path(&self) -> Option<&str>;
}

macro_rules! impl_record_ref {
    ($ty:ty, |$r:ident| $path:expr) => {
        impl RecordRef for $ty {
            fn id(&self) -> &str {
                &Record::meta(self).id
            }
            fn path(&self) -> Option<&str> {
                let $r = self;
                $path
            }
        }
    };
}

impl_record_ref!(crate::model::MuxRecord, |r| r.recipe.cwd.as_deref());
impl_record_ref!(crate::model::CheckoutRecord, |r| Some(
    r.recipe.root.as_str()
));
impl_record_ref!(crate::model::RepoRecord, |r| r
    .recipe
    .primary_root
    .as_deref());
impl_record_ref!(crate::model::PrRecord, |_r| None);

/// Forget the muxes and checkouts rooted at or under `root`, except
/// records in `protected` (the ones pin slots target). Used when
/// `worktree close` removes a worktree on purpose: its ghosts would
/// otherwise litter History.
pub fn forget_rooted_in(
    store: &HistoryStore,
    root: &Path,
    protected: &BTreeSet<String>,
) -> io::Result<usize> {
    forget_where(store, |kind, _, record| {
        matches!(kind, HistoryKind::Mux | HistoryKind::Checkout)
            && !protected.contains(record.id())
            && record
                .path()
                .is_some_and(|path| path_is_ancestor_of(root, Path::new(path)))
    })
}

/// A record `history reap` would remove, and why it qualifies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub kind: HistoryKind,
    pub id: String,
    pub label: String,
    /// When the entity was last seen.
    pub last_seen: i64,
}

/// Which gone records `history reap` would remove at `now`.
///
/// A mux, checkout, or repo qualifies when a targeted check found it
/// gone and it was last seen longer ago than its kind's threshold; a PR
/// qualifies when it is no longer in the graph and was last listed that
/// long ago. `older_than` replaces every kind's configured threshold,
/// and a kind with no threshold (repos by default) is never a candidate
/// unless `older_than` is given. A record a pin slot targets and a record
/// whose presence is Unknown are never candidates.
pub fn candidates(
    snapshot: &GraphSnapshot,
    config: &HistoryConfig,
    now: i64,
    kind: Option<HistoryKind>,
    older_than: Option<Duration>,
) -> Vec<Candidate> {
    let protected = pinned_records(snapshot);
    let mut out = Vec::new();
    for (record_kind, meta) in snapshot.history.metas() {
        if kind.is_some_and(|wanted| wanted != record_kind) || protected.contains(&meta.id) {
            continue;
        }
        let Some(threshold) = older_than.or_else(|| config.reap_after(record_kind)) else {
            continue;
        };
        let last_seen = match (record_kind, snapshot.history.presence_of(&meta.id)) {
            (_, Some(NodePresence::Unknown { .. })) => continue,
            (HistoryKind::Pr, _) => {
                let listed =
                    NodeId::parse(&meta.key).is_some_and(|id| snapshot.find_node(&id).is_some());
                if listed {
                    continue;
                }
                meta.last_seen
            }
            (_, Some(gone @ NodePresence::Gone { .. })) => gone.last_seen(),
            (_, None) => continue,
        };
        let age = now.saturating_sub(last_seen);
        if u64::try_from(age).is_ok_and(|age| age >= threshold.as_secs()) {
            out.push(Candidate {
                kind: record_kind,
                id: meta.id.clone(),
                label: snapshot
                    .history
                    .label_of(&meta.id)
                    .unwrap_or_else(|| meta.display.label.clone()),
                last_seen,
            });
        }
    }
    out.sort_by(|a, b| a.last_seen.cmp(&b.last_seen).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Ids of the records pin slots target, by `history:<id>` or by the key
/// of the node a slot points at.
fn pinned_records(snapshot: &GraphSnapshot) -> BTreeSet<String> {
    let targets: BTreeSet<&str> = snapshot
        .pins
        .iter()
        .filter_map(|slot| slot.target.as_deref())
        .collect();
    snapshot
        .history
        .metas()
        .filter(|(_, meta)| {
            targets.contains(PinSlot::history_target(&meta.id).as_str())
                || targets.contains(meta.key.as_str())
        })
        .map(|(_, meta)| meta.id.clone())
        .collect()
}

/// Remove exactly the records with these ids. Returns how many went.
pub fn reap(store: &HistoryStore, ids: &BTreeSet<String>) -> io::Result<usize> {
    forget_where(store, |_, _, record| ids.contains(record.id()))
}

#[cfg(test)]
#[path = "reap_tests.rs"]
mod tests;
