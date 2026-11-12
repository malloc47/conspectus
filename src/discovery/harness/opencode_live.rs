//! opencode plugin payloads as live-state snapshots (ADR 0204, ADR 0206).
//!
//! opencode is a snapshot source. The Conspectus plugin
//! (`plugins/opencode-hook`) keeps each session's state itself, from
//! opencode's plugin events: the 1.x plugin inside each TUI, the 2.x
//! server entrypoint inside the shared service. Whenever what a session
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
//! store, field by field against a whitelist:
//!
//! | Field | Read as |
//! | :- | :- |
//! | `session_id`, `cwd` (top level) | the session and its directory; the store refuses an id that is not one and drops a path that is not absolute |
//! | `instance`, `seq` | the plugin instance and its sequence number; without both there is no snapshot |
//! | `state` | a name of the closed set; any other value reads `unknown` |
//! | `detail` | a name of the closed set that `state` carries; any other is dropped |
//! | `subagents` | a count, clamped to `u16` |
//! | `via_subagent` | a flag, false unless `true` |
//! | `observed_ms`, `since_ms` | times, never later than the writer's clock or, for `since_ms`, than `observed_ms`; a missing or non-positive one is the writer's time |
//! | `heartbeat_s` | seconds, at most [`MAX_HEARTBEAT_S`] |
//! | `pid`, `pid_start` | the opencode process the plugin runs in, which replaces the one the writer found by walking its parents; the start time the writer read wins when both name the same pid |
//!
//! Nothing else is read, and no field can carry text into the store: a
//! state or detail outside the closed set is not copied, and every id and
//! path is checked by the store (ADR 0086 Tier 3, ADR 0206 decisions 10
//! to 12).
//!
//! [`is_attribution_event`] decides which payloads also take the
//! attribution path (the hook spool and the daemon's `hook_ingest`, ADR
//! 0146): one the plugin marks `"attribution": true`, and one without a
//! `live` object, as every payload of plugin 0.2.0 and of the 2.x TUI
//! entrypoint is. The 1.x plugin marks a root session's lifecycle events
//! (created, updated, status, idle, compacted), so attribution follows an
//! in-app session switch as before; a child session's events, and every
//! snapshot of the 2.x server entrypoint, never take it. Heartbeats,
//! permission asks, and the rest reach only the live-state store.
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
//! - Without the plugin, or with only the 2.x TUI entrypoint installed, a
//!   session has no entry, which is no evidence, not idle.

use serde_json::{Map, Value};

use crate::live_store::{LiveContext, LiveSnapshot};
use crate::model::live::{LiveDetail, LiveState};

use super::HARNESS_KEY;

/// The longest heartbeat interval a snapshot may claim, so a reader's
/// quiet window (three heartbeats) stays under half an hour.
pub(crate) const MAX_HEARTBEAT_S: u32 = 600;

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
    let fields = payload.as_object()?;
    let live = fields.get("live")?.as_object()?;
    let session_id = text(fields, "session_id")?;
    let instance = text(live, "instance")?;
    let seq = live.get("seq")?.as_u64()?;
    let observed = time(live, "observed_ms", observed_ms);
    let state = live
        .get("state")
        .and_then(Value::as_str)
        .and_then(LiveState::from_name)
        .unwrap_or(LiveState::Unknown);
    let detail = live
        .get("detail")
        .and_then(Value::as_str)
        .and_then(LiveDetail::from_name)
        .filter(|detail| state.accepts(*detail));
    let subagents = live
        .get("subagents")
        .and_then(Value::as_u64)
        .map_or(0, |count| u16::try_from(count).unwrap_or(u16::MAX));
    let heartbeat_s = live
        .get("heartbeat_s")
        .and_then(Value::as_u64)
        .map_or(0, |seconds| {
            u32::try_from(seconds.min(u64::from(MAX_HEARTBEAT_S))).unwrap_or(MAX_HEARTBEAT_S)
        });
    let (pid, pid_start) = match live
        .get("pid")
        .and_then(Value::as_i64)
        .filter(|pid| *pid > 1)
    {
        Some(pid) => {
            let reported = live.get("pid_start").and_then(Value::as_u64);
            let found = context.pid_start.filter(|_| context.pid == Some(pid));
            (Some(pid), found.or(reported))
        }
        None => (context.pid, context.pid_start),
    };
    Some(LiveSnapshot {
        harness: HARNESS_KEY.to_string(),
        session_id: session_id.to_string(),
        instance: instance.to_string(),
        seq,
        observed_ms: observed,
        context: LiveContext {
            cwd: text(fields, "cwd")
                .map(str::to_string)
                .or_else(|| context.cwd.clone()),
            pid,
            pid_start,
            ..context.clone()
        },
        state,
        detail,
        since_ms: time(live, "since_ms", observed),
        subagents,
        via_subagent: live.get("via_subagent").and_then(Value::as_bool) == Some(true),
        heartbeat_s,
    })
}

/// The string at `key`, or `None` when it is missing, empty, or not a
/// string.
fn text<'a>(fields: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// The time at `key`, no later than `latest`; `latest` when it is
/// missing or not positive.
fn time(fields: &Map<String, Value>, key: &str, latest: i64) -> i64 {
    fields
        .get(key)
        .and_then(Value::as_i64)
        .filter(|ms| *ms > 0)
        .map_or(latest, |ms| ms.min(latest))
}

#[cfg(test)]
#[path = "opencode_live_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "opencode_live_sequence_tests.rs"]
mod sequence_tests;
