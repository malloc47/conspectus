//! The store file and the pure rules that change it: folding events,
//! replacing entries with snapshots, ending a superseded session, and
//! collecting garbage. [`super::LiveStore`] runs them under the lock.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::live::{LiveDetail, LiveObservation, LiveSource, LiveState, Liveness};

use super::event::{LiveContext, LiveEvent, LiveSnapshot};
use super::reducer::{Reducer, ReducerBlock, tolerant_block};

/// The schema version of the file this binary writes. A reader that
/// meets a higher one reads no live state, and a writer leaves the file
/// alone.
pub const LIVE_STATE_SCHEMA_VERSION: u32 = 1;

/// An entry that has been `exited` for longer than this is dropped.
pub const EXITED_RETENTION_MS: i64 = 60 * 60 * 1000;

/// An entry whose process is dead and that has not changed for longer
/// than this is dropped.
pub const DEAD_RETENTION_MS: i64 = 24 * 60 * 60 * 1000;

/// The most entries the file keeps; the least recently changed go first.
pub const MAX_ENTRIES: usize = 512;

/// The live-state file: `{ "schema_version": 1, "rev": N, "entries":
/// { "<harness>:<session_id>": entry } }`. Top-level fields this version
/// does not know are written back as they were.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LiveFile {
    pub schema_version: u32,
    /// The revision of the last write. Every write takes the next one.
    pub rev: u64,
    pub entries: BTreeMap<String, LiveEntry>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl Default for LiveFile {
    fn default() -> Self {
        Self {
            schema_version: LIVE_STATE_SCHEMA_VERSION,
            rev: 0,
            entries: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// The tmux pane an agent runs in, as far as its writers know.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveTmux {
    /// The server's socket path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// The session name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The pane id (`%3`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

impl LiveTmux {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// One session's live state, keyed by harness and session id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveEntry {
    pub harness: String,
    pub session_id: String,
    /// What readers bind to the graph.
    pub observation: LiveObservation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux: Option<LiveTmux>,
    /// The plugin instance whose snapshot wrote the entry; `None` when
    /// events wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// That snapshot's sequence number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// The file revision that last changed the entry.
    #[serde(default)]
    pub rev: u64,
    /// When the entry last changed, by the store's clock, Unix epoch
    /// milliseconds.
    #[serde(default)]
    pub changed_ms: i64,
    #[serde(
        default,
        skip_serializing_if = "ReducerBlock::is_empty",
        deserialize_with = "tolerant_block"
    )]
    pub(crate) reducer: ReducerBlock,
}

impl LiveEntry {
    /// A new entry for a session first heard of at `observed_ms`: no
    /// evidence of a state yet.
    pub(crate) fn new(harness: &str, session_id: &str, observed_ms: i64) -> Self {
        Self {
            harness: harness.to_string(),
            session_id: session_id.to_string(),
            observation: LiveObservation::new(LiveState::Unknown, LiveSource::Hook, observed_ms),
            cwd: None,
            transcript_path: None,
            tmux: None,
            instance: None,
            seq: None,
            rev: 0,
            changed_ms: 0,
            reducer: ReducerBlock::default(),
        }
    }

    /// The entry's key in the file.
    pub fn key(&self) -> String {
        entry_key(&self.harness, &self.session_id)
    }

    /// Record what `context` knows. A known field replaces the stored
    /// one; a tmux pane other than the stored one replaces every tmux
    /// field, since the stored ones described another pane.
    pub(crate) fn merge_context(&mut self, context: &LiveContext) {
        if let Some(cwd) = &context.cwd {
            self.cwd = Some(cwd.clone());
        }
        if let Some(path) = &context.transcript_path {
            self.transcript_path = Some(path.clone());
        }
        if let Some(tmux) = &context.tmux {
            let merged = match self.tmux.take() {
                Some(old) if tmux.pane.is_none() || old.pane.is_none() || old.pane == tmux.pane => {
                    LiveTmux {
                        socket: tmux.socket.clone().or(old.socket),
                        session: tmux.session.clone().or(old.session),
                        pane: tmux.pane.clone().or(old.pane),
                    }
                }
                _ => tmux.clone(),
            };
            self.tmux = Some(merged).filter(|tmux| !tmux.is_empty());
        }
        if let Some(pid) = context.pid {
            if self.observation.pid != Some(pid) {
                self.observation.pid_start = None;
            }
            self.observation.pid = Some(pid);
            if context.pid_start.is_some() {
                self.observation.pid_start = context.pid_start;
            }
        }
    }

    /// End the entry with `exited/<detail>` at `at_ms`: nothing pending,
    /// no child agents.
    fn end(&mut self, detail: LiveDetail, at_ms: i64) {
        self.reducer = ReducerBlock::ended(detail, at_ms);
        self.observation = self.reducer.observe(&self.observation, at_ms);
    }
}

/// The key of a session's entry: `<harness>:<session_id>`.
pub fn entry_key(harness: &str, session_id: &str) -> String {
    format!("{harness}:{session_id}")
}

/// What a read of the file found.
#[derive(Debug)]
pub(crate) enum Parsed {
    File(LiveFile),
    /// Not a live-state file this version can read.
    Corrupt,
    /// Written by a newer version.
    Newer,
}

/// The file's shape before its entries are read one by one.
#[derive(Deserialize)]
struct RawFile {
    schema_version: u32,
    #[serde(default)]
    rev: u64,
    #[serde(default)]
    entries: BTreeMap<String, Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl LiveFile {
    /// The entry for `harness` and `session_id`, if the file has one.
    pub fn get(&self, harness: &str, session_id: &str) -> Option<&LiveEntry> {
        self.entries.get(&entry_key(harness, session_id))
    }

    /// Read `text`. An entry this version cannot read is left out, so one
    /// bad entry costs only itself; it is gone after the next write.
    pub(crate) fn parse(text: &str) -> Parsed {
        let Ok(raw) = serde_json::from_str::<RawFile>(text) else {
            return Parsed::Corrupt;
        };
        if raw.schema_version > LIVE_STATE_SCHEMA_VERSION {
            return Parsed::Newer;
        }
        let entries = raw
            .entries
            .into_values()
            .filter_map(|value| serde_json::from_value::<LiveEntry>(value).ok())
            .map(|entry| (entry.key(), entry))
            .collect();
        Parsed::File(Self {
            schema_version: LIVE_STATE_SCHEMA_VERSION,
            rev: raw.rev,
            entries,
            extra: raw.extra,
        })
    }

    /// What a reader takes `text` for: the file it holds, or no live
    /// state when it is corrupt or a newer version wrote it.
    pub(crate) fn read_text(text: &str) -> Self {
        match Self::parse(text) {
            Parsed::File(file) => file,
            Parsed::Corrupt | Parsed::Newer => Self::default(),
        }
    }

    /// The file as it is written: pretty JSON with a trailing newline.
    pub(crate) fn render(&self) -> serde_json::Result<String> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Fold `events` in order with `reducer`, as one write. Returns
    /// whether anything changed; when something did, the file has the
    /// next revision, every entry that changed carries it, and garbage
    /// has been collected at `now_ms`.
    pub(crate) fn apply_events(
        &mut self,
        reducer: &dyn Reducer,
        events: &[LiveEvent],
        now_ms: i64,
        liveness: &dyn Fn(&LiveObservation) -> Liveness,
    ) -> bool {
        let rev = self.rev + 1;
        let mut changed = false;
        for event in events {
            let key = entry_key(&event.harness, &event.session_id);
            let previous = self.entries.get(&key);
            let mut next = reducer.fold(previous, event);
            if previous == Some(&next) {
                continue;
            }
            next.rev = rev;
            next.changed_ms = now_ms;
            self.entries.insert(key.clone(), next);
            self.supersede(&key, event, rev, now_ms);
            changed = true;
        }
        if changed {
            self.rev = rev;
            self.collect_garbage(now_ms, liveness);
        }
        changed
    }

    /// Replace the snapshot's entry when the snapshot is newer than it.
    /// Returns whether anything changed, as [`Self::apply_events`] does.
    pub(crate) fn apply_snapshot(
        &mut self,
        snapshot: &LiveSnapshot,
        now_ms: i64,
        liveness: &dyn Fn(&LiveObservation) -> Liveness,
    ) -> bool {
        let key = entry_key(&snapshot.harness, &snapshot.session_id);
        let previous = self.entries.get(&key);
        let Some(mut next) = replace(previous, snapshot) else {
            return false;
        };
        if previous == Some(&next) {
            return false;
        }
        self.rev += 1;
        next.rev = self.rev;
        next.changed_ms = now_ms;
        self.entries.insert(key, next);
        self.collect_garbage(now_ms, liveness);
        true
    }

    /// One live entry per harness and process: when `event` changed the
    /// entry at `key` and names a pid, any other live entry of the same
    /// harness with that pid belongs to a session the process left. It
    /// ends `exited/ended`, or `exited/killed` when its recorded start
    /// time shows the pid was reused.
    fn supersede(&mut self, key: &str, event: &LiveEvent, rev: u64, now_ms: i64) {
        let Some(pid) = event.context.pid else {
            return;
        };
        for (other_key, other) in &mut self.entries {
            if other_key == key
                || other.harness != event.harness
                || other.observation.pid != Some(pid)
                || other.observation.state == LiveState::Exited
            {
                continue;
            }
            let reused = matches!(
                (other.observation.pid_start, event.context.pid_start),
                (Some(recorded), Some(current)) if recorded != current
            );
            let detail = if reused {
                LiveDetail::Killed
            } else {
                LiveDetail::Ended
            };
            other.end(detail, event.observed_ms);
            other.rev = rev;
            other.changed_ms = now_ms;
        }
    }

    /// Drop entries `exited` for longer than [`EXITED_RETENTION_MS`],
    /// entries whose process `liveness` finds dead that have not changed
    /// for longer than [`DEAD_RETENTION_MS`], and then the least recently
    /// changed entries past [`MAX_ENTRIES`]. An exited entry takes no
    /// more events from its process, so its change time is when it
    /// exited.
    pub(crate) fn collect_garbage(
        &mut self,
        now_ms: i64,
        liveness: &dyn Fn(&LiveObservation) -> Liveness,
    ) {
        self.entries.retain(|_, entry| {
            let idle_ms = now_ms.saturating_sub(entry.changed_ms);
            if entry.observation.state == LiveState::Exited && idle_ms > EXITED_RETENTION_MS {
                return false;
            }
            !(idle_ms > DEAD_RETENTION_MS && liveness(&entry.observation) == Liveness::Dead)
        });
        if self.entries.len() > MAX_ENTRIES {
            let mut by_age: Vec<(i64, u64, String)> = self
                .entries
                .iter()
                .map(|(key, entry)| (entry.changed_ms, entry.rev, key.clone()))
                .collect();
            by_age.sort();
            let excess = self.entries.len() - MAX_ENTRIES;
            for (_, _, key) in by_age.into_iter().take(excess) {
                self.entries.remove(&key);
            }
        }
    }
}

/// The entry `snapshot` makes of `previous`, or `None` when the snapshot
/// is older: same instance and a sequence number not above the entry's,
/// or another instance with evidence older than the entry's.
fn replace(previous: Option<&LiveEntry>, snapshot: &LiveSnapshot) -> Option<LiveEntry> {
    if let Some(entry) = previous {
        let newer = match (&entry.instance, entry.seq) {
            (Some(instance), Some(seq)) if *instance == snapshot.instance => snapshot.seq > seq,
            _ => snapshot.observed_ms >= entry.observation.observed_ms,
        };
        if !newer {
            return None;
        }
    }
    let mut next = previous.cloned().unwrap_or_else(|| {
        LiveEntry::new(
            &snapshot.harness,
            &snapshot.session_id,
            snapshot.observed_ms,
        )
    });
    next.merge_context(&snapshot.context);
    next.instance = Some(snapshot.instance.clone());
    next.seq = Some(snapshot.seq);
    next.reducer = ReducerBlock::default();
    next.observation = LiveObservation {
        state: snapshot.state,
        detail: snapshot
            .detail
            .filter(|detail| snapshot.state.accepts(*detail)),
        subagents: snapshot.subagents,
        via_subagent: snapshot.via_subagent,
        since_ms: snapshot.since_ms,
        observed_ms: snapshot.observed_ms,
        source: LiveSource::Hook,
        heartbeat_s: snapshot.heartbeat_s,
        ..next.observation
    };
    Some(next)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
