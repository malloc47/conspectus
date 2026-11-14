//! opencode plugin payloads as live-state snapshots (ADR 0204, ADR 0206).
//!
//! opencode is a snapshot source. The Conspectus plugin
//! (`plugins/opencode-hook`) keeps each session's state itself, from
//! opencode's plugin events: the 1.x plugin inside each TUI, the 2.x
//! server entrypoint inside the shared service, and, when no server
//! entrypoint is active, the 2.x TUI entrypoint inside each client for
//! the sessions that client has shown. Whenever what a session
//! shows changes, and every 60 s while it is not ready, the plugin runs
//! `conspectus hook write opencode` with a flat payload:
//!
//! ```json
//! {"session_id": "ses_...", "cwd": "/work/project", "hook_event_name": "session.status",
//!  "attribution": true,
//!  "live": {"instance": "...", "seq": 42, "observed_ms": 1792300904859, "heartbeat_s": 60,
//!           "pid": 52817, "pid_start": 918273645, "state": "waiting", "detail": "permission",
//!           "subagents": 1, "via_subagent": true, "since_ms": 1792300900000}}
//! ```
//!
//! [`live_snapshot`] reads the `live` object as a [`LiveSnapshot`] for the
//! store through the parser the Pi extension's snapshots share
//! (`plugin_live.rs`), field by field against a whitelist: a state or
//! detail outside the closed sets is not copied, counts and the heartbeat
//! are clamped, no time runs ahead of the writer's clock, and the plugin's
//! own pid replaces the one the writer found by walking its parents.
//!
//! [`is_attribution_event`] decides which payloads also take the
//! attribution path (the hook spool and the daemon's `hook_ingest`, ADR
//! 0146): one the plugin marks `"attribution": true`, and one without a
//! `live` object, as every payload of plugin 0.2.0 and every attribution
//! record of the 2.x TUI entrypoint is. The 1.x plugin marks a root
//! session's lifecycle events (created, updated, status, idle,
//! compacted), so attribution follows an in-app session switch as
//! before; a child session's events, and every snapshot of a 2.x
//! entrypoint, never take it. Heartbeats, permission asks, and the rest
//! reach only the live-state store.
//!
//! Some gaps come with the events, and readers have to live with them:
//!
//! - opencode 1.x publishes no event for an Esc interrupt, so an
//!   interrupted turn reads `ready/finished`, not `interrupted`; 2.x says
//!   which.
//! - Not every 1.x failure publishes `session.error`, so a failed turn may
//!   read `ready/finished`.
//! - A killed opencode says nothing; readers find its pid gone and show
//!   `exited/killed`. With 2.x that pid is the shared service's, so
//!   closing a client leaves its sessions as the service runs them.
//! - Without the plugin a session has no entry, which is no evidence, not
//!   idle. With only the 2.x TUI entrypoint installed, so has a session
//!   no client has shown; one a client showed reads its client's process,
//!   and so `exited/killed` once every client that showed it has closed.

use serde_json::Value;

use crate::discovery::harness::plugin_live;
use crate::hook::payload::PayloadField;
use crate::live_store::{LiveContext, LiveSnapshot};

use super::HARNESS_KEY;

/// Every field the snapshot, the attribution decision, and the
/// attribution record read.
pub(super) const PAYLOAD_FIELDS: [PayloadField; 6] = [
    PayloadField::scalar("session_id"),
    PayloadField::scalar("cwd"),
    PayloadField::scalar("transcript_path"),
    PayloadField::scalar("hook_event_name"),
    PayloadField::scalar("attribution"),
    plugin_live::LIVE_FIELD,
];

/// Whether `payload` takes the attribution path: when the plugin marks it
/// with `attribution`, as it says; else when it carries no live state.
pub(super) fn is_attribution_event(payload: &Value) -> bool {
    match payload.get("attribution") {
        Some(Value::Bool(flag)) => *flag,
        _ => payload.get("live").is_none(),
    }
}

/// The snapshot `payload`'s `live` object describes, with `context` (where
/// the writer runs) filled from the payload's cwd and process, and
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
#[path = "opencode_live_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "opencode_live_sequence_tests.rs"]
mod sequence_tests;
