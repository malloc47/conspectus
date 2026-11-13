//! Live-state snapshots from harness plugins (ADR 0204 decision 3, ADR
//! 0206): the OpenCode plugin and the Pi extension keep a session's state
//! inside the harness and send all of it, as a `live` object beside the
//! flat hook payload's session fields:
//!
//! ```json
//! {"session_id": "...", "cwd": "/work/project", "transcript_path": "/.../session.jsonl",
//!  "hook_event_name": "...",
//!  "live": {"instance": "...", "seq": 42, "observed_ms": 1792300904859, "heartbeat_s": 60,
//!           "pid": 52817, "pid_start": 918273645, "state": "waiting", "detail": "permission",
//!           "subagents": 1, "via_subagent": true, "since_ms": 1792300900000}}
//! ```
//!
//! [`live_snapshot`] reads it as a [`LiveSnapshot`] for the store, field by
//! field against a whitelist:
//!
//! | Field | Read as |
//! | :- | :- |
//! | `session_id`, `cwd`, `transcript_path` (top level) | the session, its directory, and its transcript; the store refuses an id that is not one and drops a path that is not absolute |
//! | `instance`, `seq` | the plugin instance and its sequence number; without both there is no snapshot |
//! | `state` | a name of the closed set; any other value reads `unknown` |
//! | `detail` | a name of the closed set that `state` carries; any other is dropped |
//! | `subagents` | a count, clamped to `u16` |
//! | `via_subagent` | a flag, false unless `true` |
//! | `observed_ms`, `since_ms` | times, never later than the writer's clock or, for `since_ms`, than `observed_ms`; a missing or non-positive one is the writer's time |
//! | `heartbeat_s` | seconds, at most [`MAX_HEARTBEAT_S`] |
//! | `pid`, `pid_start` | the harness process the plugin runs in, which replaces the one the writer found by walking its parents; the start time the writer read wins when both name the same pid |
//!
//! Nothing else is read (`hook write` parses only these fields,
//! [`LIVE_FIELD`] inside the `live` object, and skips the rest unbuilt),
//! and no field can carry text into the store: a
//! state or detail outside the closed set is not copied, and every id and
//! path is checked by the store (ADR 0086 Tier 3, ADR 0206 decisions 10
//! to 12). Each harness's adapter decides which of its payloads also take
//! the attribution path.

use serde_json::{Map, Value};

use crate::hook::payload::PayloadField;
use crate::live_store::{LiveContext, LiveSnapshot};
use crate::model::live::{LiveDetail, LiveState};

/// The `live` object and every field of it [`live_snapshot`] and
/// [`reported_pid`] read.
pub(super) const LIVE_FIELD: PayloadField = PayloadField::nested(
    "live",
    &[
        PayloadField::scalar("instance"),
        PayloadField::scalar("seq"),
        PayloadField::scalar("observed_ms"),
        PayloadField::scalar("heartbeat_s"),
        PayloadField::scalar("pid"),
        PayloadField::scalar("pid_start"),
        PayloadField::scalar("state"),
        PayloadField::scalar("detail"),
        PayloadField::scalar("subagents"),
        PayloadField::scalar("via_subagent"),
        PayloadField::scalar("since_ms"),
    ],
);

/// The longest heartbeat interval a snapshot may claim, so a reader's
/// quiet window (three heartbeats) stays under half an hour.
pub(crate) const MAX_HEARTBEAT_S: u32 = 600;

/// The snapshot of `harness`'s session that `payload`'s `live` object
/// describes, with `context` (where the writer runs) filled from the
/// payload's paths and process, and `observed_ms` the writer's clock.
/// `None` without a `live` object, a session id, an instance, or a
/// sequence number.
pub(super) fn live_snapshot(
    harness: &str,
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
    let (pid, pid_start) = match reported_pid(payload) {
        Some(pid) => {
            let reported = live.get("pid_start").and_then(Value::as_u64);
            let found = context.pid_start.filter(|_| context.pid == Some(pid));
            (Some(pid), found.or(reported))
        }
        None => (context.pid, context.pid_start),
    };
    Some(LiveSnapshot {
        harness: harness.to_string(),
        session_id: session_id.to_string(),
        instance: instance.to_string(),
        seq,
        observed_ms: observed,
        context: LiveContext {
            cwd: text(fields, "cwd")
                .map(str::to_string)
                .or_else(|| context.cwd.clone()),
            transcript_path: text(fields, "transcript_path")
                .map(str::to_string)
                .or_else(|| context.transcript_path.clone()),
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

/// The process the plugin says it runs in: `live.pid`, when it is a
/// number above init's.
pub(super) fn reported_pid(payload: &Value) -> Option<i64> {
    payload
        .get("live")?
        .get("pid")?
        .as_i64()
        .filter(|pid| *pid > 1)
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
