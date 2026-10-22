//! The history store (ADR 0140): every rebuildable entity Conspectus
//! has observed, gone ones included, under
//! `$XDG_STATE_HOME/conspectus/history/`.
//!
//! One JSON file per kind (`muxes.json`, `checkouts.json`,
//! `repos.json`, `prs.json`) holds `{ "schema_version": 1, "records":
//! [...] }`, and `history.lock` serializes writers. Writers never
//! replace history with a copy: each takes the lock, re-reads the
//! files, merges in the observations it made itself
//! ([`merge::merge`]), and writes the files that changed atomically.
//! The merge is commutative, so the daemon, a daemonless TUI, and
//! one-shot CLI runs can all write in any order without losing each
//! other's observations, and a writer holding an old copy can't undo
//! a reap.
//!
//! History is a category 2 observation under the mutation envelope
//! (ADR 0143), and the one Tier 3 store that may carry a launch argv
//! verbatim (ADR 0086 as amended by ADR 0140).

pub mod merge;
mod store;

pub use merge::{
    CheckoutObservation, MergeOutcome, MuxObservation, Observations, PrObservation, RepoObservation,
};
pub use store::{HISTORY_SCHEMA_VERSION, HistoryLoad, HistoryStore};

use std::collections::BTreeSet;

/// Rewrite an unchanged live record's `last_seen` at most this often
/// (seconds). Reap thresholds are in days, so ten minutes is precise
/// enough, and quiet cycles leave the files alone.
pub const LAST_SEEN_CADENCE_SECS: i64 = 600;

/// A fresh random record id: `h_` and 12 hex digits, unique among
/// `existing`. Drawn from the standard library's per-process random
/// hasher keys, so no randomness crate is needed.
pub fn new_record_id(existing: &BTreeSet<String>) -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        hasher.write_u32(std::process::id());
        hasher.write_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        let id = format!("h_{:012x}", hasher.finish() & 0xffff_ffff_ffff);
        if !existing.contains(&id) {
            return id;
        }
    }
}

/// Match `text` against a glob of `*` (any run) and `?` (any one
/// character). The `[history.ignore]` globs are this small on purpose.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
