//! The Pi payload parser: which payloads take the attribution path, how a
//! `live` object reads, which process a record names, and the payload
//! fixtures of shapes this version does not know.

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::live_sequences::{corpus_dir, read_json};
use crate::discovery::harness::{HarnessAdapter, PiAdapter};
use crate::live_store::{Applied, LiveStore, LiveTmux};
use crate::model::live::{LiveDetail, LiveState};

const SESSION: &str = "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d";
const CWD: &str = "/home/user/projects/my-project";
const FILE: &str = "/home/user/.pi/agent/sessions/--home-user-projects-my-project--/2026-11-11T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl";
const INSTANCE: &str = "mhv8k2b4-6c8e0a2b4d6f8a19";
/// The writer's clock in these tests.
const NOW: i64 = 10_000;

/// A `live` object with every field, `changes` on top.
fn live(changes: Value) -> Value {
    let mut live = json!({
        "instance": INSTANCE,
        "seq": 7,
        "observed_ms": 5000,
        "heartbeat_s": 60,
        "pid": 48213,
        "pid_start": 771_234_567,
        "state": "working",
        "detail": "tool",
        "subagents": 2,
        "since_ms": 2000,
    });
    for (key, value) in changes.as_object().expect("changes") {
        live[key] = value.clone();
    }
    live
}

fn payload(event: &str, live: Value) -> Value {
    json!({
        "session_id": SESSION,
        "cwd": CWD,
        "transcript_path": FILE,
        "hook_event_name": event,
        "live": live,
    })
}

fn read(changes: Value) -> LiveSnapshot {
    live_snapshot(
        &payload("live:tool_execution_start", live(changes)),
        &LiveContext::default(),
        NOW,
    )
    .expect("a snapshot")
}

fn fixture(name: &str) -> Value {
    read_json(&corpus_dir("pi-live").join(format!("payload-{name}.json")))
}

#[test]
fn session_starts_and_payloads_without_live_state_take_the_attribution_path() {
    let started = payload("session_start:startup", live(json!({})));
    assert!(is_attribution_event(&started), "a session start names it");
    for reason in ["reload", "new", "resume", "fork", "other"] {
        let name = format!("session_start:{reason}");
        assert!(is_attribution_event(&payload(&name, live(json!({})))));
    }
    assert!(is_attribution_event(&payload(
        "session_start",
        live(json!({}))
    )));
    for event in [
        "live:agent_start",
        "live:heartbeat",
        "live:session_shutdown",
        "live:session_start",
    ] {
        assert!(
            !is_attribution_event(&payload(event, live(json!({})))),
            "{event}"
        );
        assert!(
            !is_attribution_event(&json!({"session_id": SESSION, "hook_event_name": event})),
            "{event} without live state is no attribution either"
        );
    }
    let legacy = json!({"session_id": SESSION, "cwd": CWD, "hook_event_name": "session_start:new"});
    assert!(is_attribution_event(&legacy), "the first extension version");
    assert!(is_attribution_event(&json!({"session_id": SESSION})));
    assert!(is_attribution_event(
        &json!({"session_id": SESSION, "hook_event_name": "session.created"})
    ));
    assert!(!is_attribution_event(
        &json!({"session_id": SESSION, "hook_event_name": 7, "live": {}})
    ));
    let adapter = PiAdapter::new();
    assert!(adapter.reports_live_state());
    assert!(!adapter.is_attribution_event(&payload("live:agent_end", live(json!({})))));
    assert!(adapter.is_attribution_event(&legacy));
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
    let snapshot = live_snapshot(&payload("live:agent_start", live(json!({}))), &context, NOW)
        .expect("a snapshot");
    assert_eq!(
        snapshot,
        LiveSnapshot {
            harness: "pi".into(),
            session_id: SESSION.into(),
            instance: INSTANCE.into(),
            seq: 7,
            observed_ms: 5000,
            context: LiveContext {
                cwd: Some(CWD.into()),
                transcript_path: Some(FILE.into()),
                pid: Some(48213),
                pid_start: Some(771_234_567),
                ..context
            },
            state: LiveState::Working,
            detail: Some(LiveDetail::Tool),
            since_ms: 2000,
            subagents: 2,
            via_subagent: false,
            heartbeat_s: 60,
        }
    );
    let adapter = PiAdapter::new();
    assert_eq!(
        adapter.live_snapshot_from_payload(
            &payload("live:agent_start", live(json!({}))),
            &LiveContext::default(),
            NOW
        ),
        Some(read(json!({"detail": "tool"})))
    );
}

#[test]
fn a_state_or_detail_outside_the_closed_set_is_not_copied_and_counts_are_clamped() {
    for state in [json!("detached"), json!("SENTINEL text"), json!(null)] {
        let snapshot = read(json!({"state": state}));
        assert_eq!(
            (snapshot.state, snapshot.detail),
            (LiveState::Unknown, None),
            "{state}"
        );
    }
    let waiting = read(json!({"state": "waiting", "detail": "custom"}));
    assert_eq!((waiting.state, waiting.detail), (LiveState::Waiting, None));
    let finished = read(json!({"state": "ready", "detail": "finished"}));
    assert_eq!(finished.detail, Some(LiveDetail::Finished));
    assert_eq!(read(json!({"subagents": 1_000_000})).subagents, u16::MAX);
    assert_eq!(read(json!({"heartbeat_s": 86_400})).heartbeat_s, 600);
    assert_eq!(read(json!({"observed_ms": NOW + 1})).observed_ms, NOW);
}

#[test]
fn without_a_session_an_instance_or_a_sequence_number_there_is_no_snapshot() {
    let without = |key: &str| {
        let mut live = live(json!({}));
        live.as_object_mut().expect("live").remove(key);
        live_snapshot(
            &payload("live:agent_start", live),
            &LiveContext::default(),
            NOW,
        )
    };
    assert!(without("instance").is_none());
    assert!(without("seq").is_none());
    let mut no_session = payload("live:agent_start", live(json!({})));
    no_session
        .as_object_mut()
        .expect("payload")
        .remove("session_id");
    assert!(live_snapshot(&no_session, &LiveContext::default(), NOW).is_none());
    assert!(live_snapshot(&json!("live"), &LiveContext::default(), NOW).is_none());
}

#[test]
fn the_extensions_process_replaces_the_one_the_writer_found() {
    let writer = LiveContext {
        pid: Some(1111),
        pid_start: Some(5),
        ..LiveContext::default()
    };
    let snapshot = live_snapshot(&payload("live:agent_start", live(json!({}))), &writer, NOW)
        .expect("a snapshot");
    assert_eq!(
        (snapshot.context.pid, snapshot.context.pid_start),
        (Some(48213), Some(771_234_567))
    );
    let same = LiveContext {
        pid: Some(48213),
        ..writer.clone()
    };
    let snapshot = live_snapshot(&payload("live:agent_start", live(json!({}))), &same, NOW)
        .expect("a snapshot");
    assert_eq!(
        snapshot.context.pid_start,
        Some(5),
        "the start time the writer read"
    );
    let snapshot = live_snapshot(
        &payload("live:agent_start", live(json!({"pid": 1}))),
        &writer,
        NOW,
    )
    .expect("a snapshot");
    assert_eq!(
        (snapshot.context.pid, snapshot.context.pid_start),
        (Some(1111), Some(5))
    );
}

#[test]
fn the_attribution_record_names_the_process_the_extension_reports() {
    let adapter = PiAdapter::new();
    let started = payload("session_start:startup", live(json!({})));
    let record = |payload: &Value, pid: Option<i64>, ppid: Option<i64>| {
        let record = adapter
            .hook_record_from_payload(payload, pid, ppid, None, Some("2".into()), 5)
            .expect("a record");
        (record.pid, record.ppid)
    };
    assert_eq!(
        record(&started, Some(1111), Some(1000)),
        (Some(48213), None),
        "another process: the extension's, whose parent the writer did not read"
    );
    assert_eq!(
        record(&started, None, None),
        (Some(48213), None),
        "the walk found no process named pi"
    );
    assert_eq!(
        record(&started, Some(48213), Some(48000)),
        (Some(48213), Some(48000))
    );
    let legacy = fixture("legacy-session-start");
    assert_eq!(
        record(&legacy, Some(1111), Some(1000)),
        (Some(1111), Some(1000))
    );
    let full = adapter
        .hook_record_from_payload(&started, None, None, None, Some("2".into()), 5)
        .expect("a record");
    assert_eq!(full.harness_key, "pi");
    assert_eq!(full.session_key, SESSION);
    assert_eq!(full.cwd.as_deref(), Some(CWD));
    assert_eq!(full.transcript_path.as_deref(), Some(FILE));
    assert_eq!(
        full.hook_event_name.as_deref(),
        Some("session_start:startup")
    );
    assert_eq!(full.harness_version.as_deref(), Some("2"));
}

#[test]
fn a_payload_of_the_first_extension_version_is_attributed_and_has_no_live_state() {
    let adapter = PiAdapter::new();
    let legacy = fixture("legacy-session-start");
    assert!(adapter.is_attribution_event(&legacy));
    assert!(
        adapter
            .live_snapshot_from_payload(&legacy, &LiveContext::default(), NOW)
            .is_none()
    );
    let record = adapter
        .hook_record_from_payload(&legacy, Some(4242), None, None, Some("1".into()), 1)
        .expect("an attribution record");
    assert_eq!(
        record.hook_event_name.as_deref(),
        Some("session_start:resume")
    );
    assert_eq!(record.pid, Some(4242));
}

#[test]
fn a_newer_extensions_state_and_detail_read_as_what_this_version_knows() {
    let adapter = PiAdapter::new();
    let unknown = fixture("unknown-state");
    assert!(!adapter.is_attribution_event(&unknown));
    let snapshot = adapter
        .live_snapshot_from_payload(&unknown, &LiveContext::default(), NOW)
        .expect("a snapshot");
    assert_eq!(
        (snapshot.state, snapshot.detail),
        (LiveState::Unknown, None)
    );
    assert_eq!(snapshot.seq, 3);
    let detail = fixture("unknown-detail");
    let snapshot = adapter
        .live_snapshot_from_payload(&detail, &LiveContext::default(), NOW)
        .expect("a snapshot");
    assert_eq!(
        (snapshot.state, snapshot.detail),
        (LiveState::Waiting, None)
    );
}

#[test]
fn the_payload_the_research_sketched_is_neither_a_snapshot_nor_attribution() {
    let adapter = PiAdapter::new();
    let sketched = fixture("research-shape");
    assert!(!adapter.is_attribution_event(&sketched));
    assert!(
        adapter
            .live_snapshot_from_payload(&sketched, &LiveContext::default(), NOW)
            .is_none()
    );
}

/// A payload with a sentinel in every field the parser does not read, in
/// the fields a careless extension might forward from Pi's events, and in
/// the closed-set fields.
fn planted() -> Value {
    let text = "SENTINEL-pi-text payload words";
    let mut payload = payload(
        "live:message_end",
        live(json!({
            "state": text,
            "detail": text,
            "errorMessage": text,
            "error_class": "SENTINELpiclass",
            "title": text,
            "toolName": "mcp__SENTINELpiserver__query",
        })),
    );
    let fields = payload.as_object_mut().expect("payload");
    for key in [
        "message",
        "messages",
        "content",
        "args",
        "result",
        "title",
        "errorMessage",
        "systemPrompt",
        "compactionEntry",
        "headers",
    ] {
        fields.insert(key.into(), json!([text, {"text": text}]));
    }
    fields.insert("toolName".into(), json!("mcp__SENTINELpiserver__query"));
    payload
}

#[test]
fn no_payload_text_survives_a_snapshot_or_reaches_the_store() {
    let payload = planted();
    let snapshot = live_snapshot(&payload, &LiveContext::default(), NOW).expect("a snapshot");
    let text = serde_json::to_string(&snapshot).expect("serialize");
    assert!(!text.contains("SENTINEL"), "{text}");

    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    assert!(matches!(
        store.apply_snapshot(&snapshot).expect("write"),
        Applied::Written { .. }
    ));
    let mut text_session = payload.clone();
    text_session["session_id"] = json!("SENTINEL session text");
    let refused = live_snapshot(&text_session, &LiveContext::default(), NOW).expect("parsed");
    assert_eq!(
        store.apply_snapshot(&refused).expect("write"),
        Applied::Rejected
    );
    let mut text_path = payload;
    text_path["transcript_path"] = json!("SENTINEL transcript text");
    text_path["live"]["seq"] = json!(8);
    let dropped = live_snapshot(&text_path, &LiveContext::default(), NOW).expect("parsed");
    store.apply_snapshot(&dropped).expect("write");
    let file = std::fs::read_to_string(store.path()).expect("store file");
    assert!(!file.contains("SENTINEL"), "{file}");
    let entries = store.read();
    let entry = entries.get("pi", SESSION).expect("an entry");
    assert_eq!(
        entry.transcript_path.as_deref(),
        Some(FILE),
        "the earlier path stays"
    );
    assert_eq!(entry.seq, Some(8));
}
