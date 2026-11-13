//! The opencode payload parser: which payloads take the attribution
//! path, and how a `live` object reads, field by field.

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::plugin_live::MAX_HEARTBEAT_S;
use crate::discovery::harness::{HarnessAdapter, OpenCodeAdapter};
use crate::live_store::{Applied, LiveStore, LiveTmux};
use crate::model::live::{LiveDetail, LiveState};

const SESSION: &str = "ses_7Kq2mX9vB4nL1pZ8";
const CWD: &str = "/home/user/projects/my-project";
const INSTANCE: &str = "lq2c1a00-8f3e6b2d9c4a7e15";
/// The writer's clock in these tests.
const NOW: i64 = 10_000;

/// A `live` object with every field, `changes` on top.
fn live(changes: Value) -> Value {
    let mut live = json!({
        "instance": INSTANCE,
        "seq": 7,
        "observed_ms": 5000,
        "heartbeat_s": 60,
        "pid": 52817,
        "pid_start": 918_273_645,
        "state": "working",
        "detail": "generating",
        "subagents": 1,
        "via_subagent": false,
        "since_ms": 2000,
    });
    for (key, value) in changes.as_object().expect("changes") {
        live[key] = value.clone();
    }
    live
}

fn payload(live: Value) -> Value {
    json!({
        "session_id": SESSION,
        "cwd": CWD,
        "hook_event_name": "session.status",
        "live": live,
    })
}

fn snapshot(payload: &Value) -> Option<LiveSnapshot> {
    live_snapshot(payload, &LiveContext::default(), NOW)
}

fn read(changes: Value) -> LiveSnapshot {
    snapshot(&payload(live(changes))).expect("a snapshot")
}

#[test]
fn only_payloads_marked_for_it_or_without_live_state_take_the_attribution_path() {
    let legacy = json!({"session_id": SESSION, "cwd": CWD, "hook_event_name": "session.status"});
    assert!(
        is_attribution_event(&legacy),
        "plugin 0.2.0 and the 2.x TUI"
    );
    let route = json!({"session_id": SESSION, "hook_event_name": "tui.route"});
    assert!(is_attribution_event(&route));
    let mut marked = payload(live(json!({})));
    assert!(!is_attribution_event(&marked), "a snapshot alone");
    marked["attribution"] = json!(true);
    assert!(is_attribution_event(&marked), "a root's lifecycle event");
    marked["attribution"] = json!(false);
    assert!(!is_attribution_event(&marked));
    marked["attribution"] = json!("yes");
    assert!(!is_attribution_event(&marked), "only a boolean marks it");
    let mut unmarked = legacy;
    unmarked["attribution"] = json!(false);
    assert!(!is_attribution_event(&unmarked));
    assert!(OpenCodeAdapter::new().reports_live_state());
    assert!(!OpenCodeAdapter::new().is_attribution_event(&payload(live(json!({})))));
}

#[test]
fn a_snapshot_reads_every_field_it_names() {
    let context = LiveContext {
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: Some("work".into()),
            pane: Some("%3".into()),
        }),
        ..LiveContext::default()
    };
    let snapshot = live_snapshot(&payload(live(json!({}))), &context, NOW).expect("a snapshot");
    assert_eq!(
        snapshot,
        LiveSnapshot {
            harness: "opencode".into(),
            session_id: SESSION.into(),
            instance: INSTANCE.into(),
            seq: 7,
            observed_ms: 5000,
            context: LiveContext {
                cwd: Some(CWD.into()),
                pid: Some(52817),
                pid_start: Some(918_273_645),
                ..context
            },
            state: LiveState::Working,
            detail: Some(LiveDetail::Generating),
            since_ms: 2000,
            subagents: 1,
            via_subagent: false,
            heartbeat_s: 60,
        }
    );
    assert!(read(json!({"via_subagent": true})).via_subagent);
    assert!(!read(json!({"via_subagent": "true"})).via_subagent);
    let without_cwd = json!({"session_id": SESSION, "live": live(json!({}))});
    let context = LiveContext {
        cwd: Some("/from/the/writer".into()),
        ..LiveContext::default()
    };
    assert_eq!(
        live_snapshot(&without_cwd, &context, NOW)
            .expect("a snapshot")
            .context
            .cwd
            .as_deref(),
        Some("/from/the/writer")
    );
}

#[test]
fn a_state_or_detail_outside_the_closed_set_is_not_copied() {
    for state in [
        json!("napping"),
        json!("SENTINEL text"),
        json!(7),
        json!(null),
    ] {
        let snapshot = read(json!({"state": state, "detail": "generating"}));
        assert_eq!(snapshot.state, LiveState::Unknown, "{state}");
        assert_eq!(snapshot.detail, None, "{state}");
    }
    assert_eq!(read(json!({"detail": "permission"})).detail, None);
    assert_eq!(read(json!({"detail": "SENTINEL"})).detail, None);
    assert_eq!(read(json!({"detail": 3})).detail, None);
    let waiting = read(json!({"state": "waiting", "detail": "input"}));
    assert_eq!(
        (waiting.state, waiting.detail),
        (LiveState::Waiting, Some(LiveDetail::Input))
    );
    let mut without = live(json!({}));
    without.as_object_mut().expect("live").remove("state");
    assert_eq!(
        snapshot(&payload(without)).expect("a snapshot").state,
        LiveState::Unknown
    );
}

#[test]
fn counts_and_times_are_clamped() {
    assert_eq!(read(json!({"subagents": 100_000})).subagents, u16::MAX);
    assert_eq!(read(json!({"subagents": -1})).subagents, 0);
    assert_eq!(read(json!({"subagents": 1.5})).subagents, 0);
    assert_eq!(
        read(json!({"heartbeat_s": 86_400})).heartbeat_s,
        MAX_HEARTBEAT_S
    );
    assert_eq!(read(json!({"heartbeat_s": -60})).heartbeat_s, 0);
    let future = read(json!({"observed_ms": NOW + 60_000, "since_ms": NOW + 30_000}));
    assert_eq!((future.observed_ms, future.since_ms), (NOW, NOW));
    for observed in [json!(0), json!(-5), json!("5000"), json!(null)] {
        assert_eq!(
            read(json!({"observed_ms": observed})).observed_ms,
            NOW,
            "{observed}"
        );
    }
    let late_start = read(json!({"since_ms": 6000}));
    assert_eq!(late_start.since_ms, 5000, "never after the evidence");
    assert_eq!(read(json!({"since_ms": 0})).since_ms, 5000);
}

#[test]
fn without_a_session_an_instance_or_a_sequence_number_there_is_no_snapshot() {
    let without = |key: &str| {
        let mut live = live(json!({}));
        live.as_object_mut().expect("live").remove(key);
        snapshot(&payload(live))
    };
    assert!(without("instance").is_none());
    assert!(without("seq").is_none());
    assert!(snapshot(&payload(live(json!({"seq": -1})))).is_none());
    assert!(snapshot(&payload(live(json!({"seq": "7"})))).is_none());
    assert!(snapshot(&payload(live(json!({"instance": ""})))).is_none());
    assert!(snapshot(&payload(json!("working"))).is_none());
    assert!(snapshot(&json!({"live": live(json!({}))})).is_none());
    assert!(snapshot(&json!({"session_id": "", "live": live(json!({}))})).is_none());
    assert!(snapshot(&json!([1, 2, 3])).is_none());
    let legacy = json!({"session_id": SESSION, "cwd": CWD, "hook_event_name": "session.idle"});
    assert!(snapshot(&legacy).is_none());
}

#[test]
fn the_plugins_process_replaces_the_one_the_writer_found() {
    let writer = |pid: i64, start: u64| LiveContext {
        pid: Some(pid),
        pid_start: Some(start),
        ..LiveContext::default()
    };
    let process = |payload: &Value, context: &LiveContext| {
        let snapshot = live_snapshot(payload, context, NOW).expect("a snapshot");
        (snapshot.context.pid, snapshot.context.pid_start)
    };
    let full = payload(live(json!({})));
    assert_eq!(
        process(&full, &writer(1111, 5)),
        (Some(52817), Some(918_273_645)),
        "another process: the plugin's own"
    );
    assert_eq!(
        process(&full, &writer(52817, 5)),
        (Some(52817), Some(5)),
        "the same process: the start time the writer read"
    );
    let mut no_start = live(json!({}));
    no_start.as_object_mut().expect("live").remove("pid_start");
    assert_eq!(
        process(&payload(no_start.clone()), &writer(1111, 5)),
        (Some(52817), None)
    );
    assert_eq!(
        process(&payload(no_start), &writer(52817, 5)),
        (Some(52817), Some(5))
    );
    for pid in [json!(1), json!(0), json!(-3), json!("52817"), json!(null)] {
        assert_eq!(
            process(&payload(live(json!({"pid": pid}))), &writer(1111, 5)),
            (Some(1111), Some(5)),
            "{pid}: the writer's"
        );
    }
}

/// A payload with a sentinel in every field the parser does not read,
/// in the fields a careless plugin might forward from opencode's events,
/// and in the closed-set fields.
fn planted() -> Value {
    let text = "SENTINEL-opencode-text payload words";
    let mut payload = payload(live(json!({
        "state": text,
        "detail": text,
        "message": text,
        "title": text,
        "reason": "SENTINELopencodereason",
        "error": {"type": "api", "message": text},
        "retry": {"message": text, "reason": "SENTINELaccountratelimit"},
    })));
    let fields = payload.as_object_mut().expect("payload");
    for key in [
        "title",
        "message",
        "error",
        "patterns",
        "resources",
        "metadata",
        "questions",
        "fields",
    ] {
        fields.insert(key.into(), json!([text, {"message": text}]));
    }
    fields.insert("hook_event_name".into(), json!("SENTINELopencodeevent"));
    payload
}

#[test]
fn no_payload_text_survives_a_snapshot_or_reaches_the_store() {
    let payload = planted();
    let snapshot = snapshot(&payload).expect("a snapshot");
    let text = serde_json::to_string(&snapshot).expect("serialize");
    assert!(!text.contains("SENTINEL"), "{text}");
    assert_eq!(
        (snapshot.state, snapshot.detail),
        (LiveState::Unknown, None)
    );

    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    assert!(matches!(
        store.apply_snapshot(&snapshot).expect("write"),
        Applied::Written { .. }
    ));
    let file = std::fs::read_to_string(store.path()).expect("store file");
    assert!(!file.contains("SENTINEL"), "{file}");

    // Text where an id or a path belongs is refused or dropped by the
    // store.
    let mut text_id = payload.clone();
    text_id["live"]["instance"] = json!("SENTINEL instance text");
    let refused = super::live_snapshot(&text_id, &LiveContext::default(), NOW).expect("parsed");
    assert_eq!(
        store.apply_snapshot(&refused).expect("write"),
        Applied::Rejected
    );
    let mut text_session = payload.clone();
    text_session["session_id"] = json!("SENTINEL session text");
    let refused =
        super::live_snapshot(&text_session, &LiveContext::default(), NOW).expect("parsed");
    assert_eq!(
        store.apply_snapshot(&refused).expect("write"),
        Applied::Rejected
    );
    let mut text_cwd = payload;
    text_cwd["cwd"] = json!("SENTINEL cwd text");
    text_cwd["live"]["seq"] = json!(8);
    let dropped = super::live_snapshot(&text_cwd, &LiveContext::default(), NOW).expect("parsed");
    store.apply_snapshot(&dropped).expect("write");
    let file = std::fs::read_to_string(store.path()).expect("store file");
    assert!(!file.contains("SENTINEL"), "{file}");
}
