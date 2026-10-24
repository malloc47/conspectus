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
pub mod project;
pub mod reap;
pub mod recorder;
mod store;

pub use merge::{
    CheckoutObservation, MergeOutcome, MuxObservation, Observations, PrObservation, RepoObservation,
};
pub use recorder::{HISTORY_ADAPTER, RecordContext, observe};
pub use store::{HISTORY_SCHEMA_VERSION, HistoryLoad, HistoryStore};

/// A mux a launch path just created.
#[derive(Clone, Debug)]
pub struct LaunchedMux<'a> {
    pub backend: &'a str,
    pub socket: Option<&'a str>,
    pub name: &'a str,
    pub cwd: &'a str,
    pub argv: &'a [String],
    pub harness: Option<&'a str>,
}

/// A history writer: the store, the `[history]` settings, and the
/// home directory display context abbreviates to `~`.
#[derive(Clone, Debug)]
pub struct HistoryWriter {
    pub store: HistoryStore,
    pub config: crate::config::HistoryConfig,
    pub home: Option<std::path::PathBuf>,
}

impl HistoryWriter {
    /// The writer for this process's state home, or `None` without one.
    pub fn from_env(config: crate::config::HistoryConfig) -> Option<Self> {
        Some(Self {
            store: HistoryStore::from_env()?,
            config,
            home: std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(std::path::PathBuf::from),
        })
    }

    /// Record a mux this process just created with `argv`, so it is
    /// rebuildable before the next probe sees it (ADR 0140). Returns
    /// the record's id, or `None` when an ignore glob skips it.
    pub fn record_launched_mux(&self, launch: &LaunchedMux<'_>) -> std::io::Result<Option<String>> {
        if self
            .config
            .ignores(crate::model::HistoryKind::Mux, launch.name)
        {
            return Ok(None);
        }
        let argv: Vec<String> = launch.argv.to_vec();
        let recipe = crate::model::MuxRecipe {
            backend: launch.backend.to_string(),
            socket: crate::model::effective_socket(launch.socket).map(str::to_string),
            name: launch.name.to_string(),
            cwd: Some(launch.cwd.to_string()),
            argv_source: (!argv.is_empty()).then_some(crate::model::ArgvSource::Conspectus),
            argv: (!argv.is_empty()).then_some(argv),
            harness: launch.harness.map(str::to_string),
            ..Default::default()
        };
        let key =
            crate::model::NodeId::MuxSession(crate::model::MuxSessionId::new(recipe.native_id()))
                .to_string();
        let context = recorder::mux_context(
            launch.harness,
            recipe.argv.as_deref(),
            recipe.cwd.as_deref(),
            self.home.as_deref(),
        );
        let observation = MuxObservation {
            seen_at: crate::discovery::current_epoch(),
            key,
            display: crate::model::RecordDisplay {
                label: launch.name.to_string(),
                context,
            },
            origin: crate::model::RecordOrigin::Launched,
            recipe,
        };
        let (_, outcome) = self.store.record(&Observations {
            muxes: vec![observation],
            ..Default::default()
        })?;
        Ok(outcome.mux_ids.into_iter().next())
    }

    /// Record a worktree a rebuild just brought back (ADR 0142), linked
    /// to the record it replaced when it landed somewhere else. Returns
    /// the record's id.
    pub fn record_rebuilt_checkout(
        &self,
        repo_key: &str,
        root: &str,
        branch: Option<&str>,
        head: Option<&str>,
        rebuilt_from: Option<&str>,
    ) -> std::io::Result<String> {
        use crate::model::{CheckoutId, NodeId, RepoId};
        let common_dir = repo_key.strip_prefix("repo:").unwrap_or(repo_key);
        let key = NodeId::Checkout(CheckoutId::new(RepoId::new(common_dir), root)).to_string();
        let label = std::path::Path::new(root)
            .file_name()
            .map_or_else(|| root.to_string(), |n| n.to_string_lossy().into_owned());
        let context = Some(match branch {
            Some(branch) => format!("{} · {branch}", recorder::repo_name(common_dir)),
            None => recorder::repo_name(common_dir),
        });
        let observation = CheckoutObservation {
            seen_at: crate::discovery::current_epoch(),
            key,
            recipe: crate::model::CheckoutRecipe {
                repo: repo_key.to_string(),
                root: root.to_string(),
                linked: true,
                branch: branch.map(str::to_string),
                head: head.map(str::to_string),
                upstream: None,
                extra: Default::default(),
            },
            display: crate::model::RecordDisplay { label, context },
            origin: crate::model::RecordOrigin::Launched,
            rebuilt_from: rebuilt_from.map(str::to_string),
        };
        let (_, outcome) = self.store.record(&Observations {
            checkouts: vec![observation],
            ..Default::default()
        })?;
        outcome
            .checkout_ids
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("the checkout wasn't recorded"))
    }

    /// Record what `snapshot`'s probes since `fresh_since` observed.
    pub fn record(
        &self,
        snapshot: &mut crate::model::GraphSnapshot,
        fresh_since: i64,
    ) -> std::io::Result<()> {
        record_snapshot(
            &self.store,
            snapshot,
            &RecordContext {
                fresh_since,
                config: &self.config,
                home: self.home.as_deref(),
            },
        )
    }
}

/// Record what `snapshot`'s fresh data observed and copy the merged
/// records into it, keeping each record's computed presence. Errors
/// are returned for the caller to report; history is best-effort and
/// never fails discovery.
pub fn record_snapshot(
    store: &HistoryStore,
    snapshot: &mut crate::model::GraphSnapshot,
    ctx: &RecordContext<'_>,
) -> std::io::Result<()> {
    let observations = observe(snapshot, ctx);
    if observations.is_empty() {
        return Ok(());
    }
    let (load, _) = store.record(&observations)?;
    let presence = std::mem::take(&mut snapshot.history.presence);
    snapshot.history = load.history;
    snapshot.history.presence = presence;
    Ok(())
}

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

#[cfg(test)]
#[path = "audit_tests.rs"]
mod audit_tests;
