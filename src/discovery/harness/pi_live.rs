//! Pi extension payloads as live-state snapshots (ADR 0204, ADR 0206).
//!
//! Pi is a snapshot source. The Conspectus extension
//! (`plugins/pi-hook/conspectus-hook.ts`) keeps the state of the session
//! its Pi process runs, from Pi's extension events, and runs `conspectus
//! hook write pi` whenever what the session shows changes, and every 60 s
//! while it is not ready, with a flat payload:
//!
//! ```json
//! {"session_id": "0199f0a2-...", "cwd": "/work/project",
//!  "transcript_path": "/home/user/.pi/agent/sessions/--work-project--/..._0199f0a2-....jsonl",
//!  "hook_event_name": "live:ui_prompt_start",
//!  "live": {"instance": "...", "seq": 42, "observed_ms": 1794400904859, "heartbeat_s": 60,
//!           "pid": 48213, "pid_start": 771234567, "state": "waiting", "detail": "permission",
//!           "subagents": 0, "since_ms": 1794400900000}}
//! ```
//!
//! [`live_snapshot`] reads the `live` object through the parser the
//! opencode plugin's snapshots share (`plugin_live.rs`), against the same
//! whitelist: a state or detail outside the closed sets is not copied,
//! counts and the heartbeat are clamped, and no time runs ahead of the
//! writer's clock. The extension maps Pi's events onto the vocabulary
//! itself and classifies an error message there, so the payload carries
//! only state names; it never carries text.
//!
//! The extension's `pid` is `process.pid` of the Pi process. It replaces
//! the one the writer finds by walking its parents for a process named
//! `pi`, which depends on Node setting the thread name from Pi's process
//! title, both for the snapshot and for the attribution record
//! ([`reported_pid`]).
//!
//! [`is_attribution_event`] decides which payloads also take the
//! attribution path (the hook spool and the daemon's `hook_ingest`, ADR
//! 0146): a `session_start:<reason>` payload, which names the session
//! the process now runs as it always has, and a payload without a `live`
//! object, as every payload of the first extension version is. A
//! `live:<event>` snapshot, a heartbeat included, reaches only the
//! live-state store.
//!
//! Some gaps come with Pi's events, and readers have to live with them:
//!
//! - Pi asks for approval only through extension dialogs: `waiting` means
//!   one is open (a confirm is a permission; a select, input, or editor a
//!   question), and a gate that asks with a select reads as a question.
//!   `ctx.ui.custom()` dialogs never read as waiting, nor does a provider
//!   login or one of Pi's own pickers. Pi before 0.84.4 fires no dialog
//!   events, so it never reads as waiting.
//! - A killed Pi says nothing; readers find its pid gone and show
//!   `exited/killed`.
//! - A `/reload` sends nothing from the old load; the new one reports the
//!   session afresh, so a finished turn reads `ready` with no detail.
//! - Print, JSON, and RPC modes and `--no-session` runs send nothing: such
//!   a session has no entry, which is no evidence, not idle.

use serde_json::Value;

use crate::discovery::harness::plugin_live;
use crate::hook::payload::PayloadField;
use crate::live_store::{LiveContext, LiveSnapshot};

use super::HARNESS_KEY;

/// Every field the snapshot, the attribution decision, and the
/// attribution record read.
pub(super) const PAYLOAD_FIELDS: [PayloadField; 5] = [
    PayloadField::scalar("session_id"),
    PayloadField::scalar("cwd"),
    PayloadField::scalar("transcript_path"),
    PayloadField::scalar("hook_event_name"),
    plugin_live::LIVE_FIELD,
];

pub(super) use plugin_live::reported_pid;

/// Whether `payload` takes the attribution path: a session start, or a
/// payload with no live state; never a `live:` snapshot.
pub(super) fn is_attribution_event(payload: &Value) -> bool {
    match payload.get("hook_event_name").and_then(Value::as_str) {
        Some(name) if name.starts_with("live:") => false,
        Some(name) if name == "session_start" || name.starts_with("session_start:") => true,
        _ => payload.get("live").is_none(),
    }
}

/// The snapshot `payload`'s `live` object describes, with `context` (where
/// the writer runs) filled from the payload's paths and process, and
/// `observed_ms` the writer's clock. `None` without a `live` object, a
/// session id, an instance, or a sequence number.
pub(super) fn live_snapshot(
    payload: &Value,
    context: &LiveContext,
    observed_ms: i64,
) -> Option<LiveSnapshot> {
    plugin_live::live_snapshot(HARNESS_KEY, payload, context, observed_ms)
}

#[cfg(test)]
#[path = "pi_live_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pi_live_sequence_tests.rs"]
mod sequence_tests;
