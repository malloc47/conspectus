//! The live-state store (ADR 0204): what each agent session is doing
//! now, one entry per harness and session id, in
//! `$XDG_STATE_HOME/conspectus/live-state.json`.
//!
//! Two kinds of writer feed it, and both end in the same [`LiveEntry`],
//! whose [`LiveObservation`](crate::model::live::LiveObservation) is what
//! readers bind to the graph:
//!
//! - An *event source* (a harness that runs a new hook process per
//!   event) maps each payload to a neutral [`LiveEvent`], and the store
//!   folds it into the session's entry with a [`Reducer`]. The entry
//!   keeps what the fold needs between events (each agent's pending
//!   wait, the child agents running, the state to resume after
//!   compaction) in a private block that no reader sees.
//! - A *snapshot source* (a plugin that keeps its own state inside the
//!   harness) sends that whole state as a [`LiveSnapshot`], which
//!   replaces the entry when it is newer.
//!
//! A writer takes `live-state.lock`, re-reads the file, applies its
//! input, collects garbage, and renames a new file into place, so the
//! file's `rev` orders writes in the order writers took the lock. One
//! that cannot take the lock within [`LOCK_WAIT`] drops its input; the
//! next event repairs what it missed. A new session id from a process
//! that already has a live entry ends that entry, so `/clear` and resume
//! leave one live session per process.
//!
//! Readers never take the lock and tolerate anything: a missing,
//! unreadable, corrupt, or newer-schema file is no live state, and an
//! entry this version cannot read is skipped.
//!
//! The file holds enums, counts, ids, paths, and times, never payload
//! (ADR 0086 Tier 3, ADR 0206): no input type has a field that could
//! carry text, and an input whose ids do not look like ids is refused.
//! Nothing here names a harness or a harness's events; each harness maps
//! its own payloads onto [`LiveEventKind`]. The store is a category 2
//! observation under the mutation envelope (ADR 0143).

mod event;
mod file;
mod process;
mod reducer;
mod store;

pub use event::{FailureClass, LiveContext, LiveEvent, LiveEventKind, LiveSnapshot, WaitKind};
pub use file::{
    DEAD_RETENTION_MS, EXITED_RETENTION_MS, LIVE_STATE_SCHEMA_VERSION, LiveEntry, LiveFile,
    LiveTmux, MAX_ENTRIES, entry_key,
};
pub use process::{process_liveness, process_start_time};
pub use reducer::{Reducer, SharedReducer, fold};
pub use store::{Applied, LIVE_STATE_FILE, LIVE_STATE_LOCK, LOCK_WAIT, LiveStore};

#[cfg(test)]
#[path = "privacy_tests.rs"]
mod privacy_tests;
