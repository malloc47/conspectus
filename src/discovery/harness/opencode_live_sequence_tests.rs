//! The shared event scripts under `tests/fixtures/hooks/opencode-live/`.
//! The plugin's own tests replay each script through its reducer
//! (`plugins/opencode-hook/test/live.test.ts`) and check the payloads it
//! emits; here every one of those payloads goes through this parser and
//! the store in a scratch directory, and each session reads as the
//! script says after every step. The scripts are synthetic; their README
//! says how they are laid out.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::live_sequences::{self, assert_shown, corpus_dir};
use crate::discovery::harness::{HarnessAdapter, OpenCodeAdapter};
use crate::live_store::{Applied, LiveStore};

/// The scenarios the research's fixture plan and the story list.
const SCRIPTS: [&str; 19] = [
    "v1-busy-idle",
    "v1-compaction",
    "v1-delete",
    "v1-dispose",
    "v1-error",
    "v1-heartbeat",
    "v1-permission",
    "v1-question",
    "v1-retry-rate-limit",
    "v1-seq-within-one-second",
    "v1-subagent-background",
    "v1-subagent-foreground",
    "v2-compaction",
    "v2-execution",
    "v2-location",
    "v2-permission-form",
    "v2-retry",
    "v2-shutdown-dispose",
    "v2-subagent-background",
];

fn fixture_dir() -> PathBuf {
    corpus_dir("opencode-live")
}

/// Every payload a script's reducer emits, with the time of its step.
fn emitted(name: &str) -> Vec<(i64, Value)> {
    let (_, steps) = live_sequences::sequence(&fixture_dir(), name);
    steps
        .iter()
        .flat_map(|step| {
            let at_ms = step["at_ms"].as_i64().expect("at_ms");
            step["emit"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(move |payload| (at_ms, payload))
        })
        .collect()
}

#[test]
fn every_script_is_listed_and_labeled_synthetic() {
    let listed: BTreeSet<String> = SCRIPTS.iter().map(ToString::to_string).collect();
    assert_eq!(
        live_sequences::fixture_names(&fixture_dir(), "jsonl"),
        listed
    );
    for name in SCRIPTS {
        let (header, steps) = live_sequences::sequence(&fixture_dir(), name);
        assert_eq!(header["synthetic"], true, "{name}");
        assert!(!steps.is_empty(), "{name}");
    }
    let readme = fs::read_to_string(fixture_dir().join("README.md")).expect("README");
    assert!(readme.contains("**synthetic**"));
}

#[test]
fn no_emitted_payload_carries_text_from_the_events() {
    for name in SCRIPTS {
        for (_, payload) in emitted(name) {
            let text = payload.to_string();
            assert!(!text.contains("SENTINEL"), "{name}: {text}");
        }
    }
}

/// Drive the script `name` through the writer's parser and the store,
/// and check every step.
fn replay(name: &str) {
    let adapter = OpenCodeAdapter::new();
    let scratch = tempfile::tempdir().expect("scratch state directory");
    let store = LiveStore::at(scratch.path());
    let (_, steps) = live_sequences::sequence(&fixture_dir(), name);
    for (index, step) in steps.iter().enumerate() {
        let place = format!(
            "{name} step {} ({})",
            index + 1,
            step["note"].as_str().unwrap_or("")
        );
        let at_ms = step["at_ms"].as_i64().expect("at_ms");
        for payload in step["emit"].as_array().into_iter().flatten() {
            check_payload(&adapter, &store, payload, at_ms, &place);
        }
        for (session, expect) in step["shown"].as_object().into_iter().flatten() {
            assert_shown(
                &store,
                HARNESS_KEY,
                session,
                at_ms,
                expect,
                &format!("{place}, {session}"),
            );
        }
    }
}

/// One emitted payload: attributed when the plugin marked it or it has
/// no live state, read whole when it has, and written to the store.
fn check_payload(
    adapter: &OpenCodeAdapter,
    store: &LiveStore,
    payload: &Value,
    at_ms: i64,
    place: &str,
) {
    let marked = payload["attribution"] == json!(true);
    let live = payload.get("live");
    assert_eq!(
        adapter.is_attribution_event(payload),
        marked || live.is_none(),
        "{place}: attribution"
    );
    if adapter.is_attribution_event(payload) {
        let record = adapter
            .hook_record_from_payload(payload, None, None, None, None, at_ms / 1000)
            .unwrap_or_else(|err| panic!("{place}: {err}"));
        assert_eq!(
            Some(record.session_key.as_str()),
            payload["session_id"].as_str()
        );
    }
    let snapshot = adapter.live_snapshot_from_payload(payload, &LiveContext::default(), at_ms);
    let Some(live) = live else {
        assert!(snapshot.is_none(), "{place}: no live state, no snapshot");
        return;
    };
    let snapshot = snapshot.unwrap_or_else(|| panic!("{place}: no snapshot"));
    assert_eq!(snapshot.instance, live["instance"], "{place}");
    assert_eq!(Some(snapshot.seq), live["seq"].as_u64(), "{place}");
    assert!(
        matches!(
            store.apply_snapshot(&snapshot).expect("write"),
            Applied::Written { .. }
        ),
        "{place}: every snapshot is newer"
    );
    let file = store.read();
    let entry = file
        .get(HARNESS_KEY, &snapshot.session_id)
        .unwrap_or_else(|| panic!("{place}: no entry"));
    let observation = entry.observation;
    let recorded = json!({
        "instance": entry.instance,
        "seq": entry.seq,
        "observed_ms": observation.observed_ms,
        "heartbeat_s": observation.heartbeat_s,
        "pid": observation.pid,
        "pid_start": observation.pid_start,
        "state": observation.state.name(),
        "detail": observation.detail.map(LiveDetail::name),
        "subagents": observation.subagents,
        "via_subagent": observation.via_subagent,
        "since_ms": observation.since_ms,
    });
    let mut sent = live.clone();
    if sent.get("detail").is_none() {
        sent["detail"] = Value::Null;
    }
    assert_eq!(recorded, sent, "{place}: the entry holds the snapshot");
    assert_eq!(
        entry.cwd.as_deref(),
        payload["cwd"].as_str(),
        "{place}: cwd"
    );
}

#[test]
fn a_turn_of_a_v1_session() {
    replay("v1-busy-idle");
}

#[test]
fn v1_compaction_mid_turn_failed_and_while_idle() {
    replay("v1-compaction");
}

#[test]
fn a_deleted_v1_session() {
    replay("v1-delete");
}

#[test]
fn a_disposed_v1_instance() {
    replay("v1-dispose");
}

#[test]
fn v1_errors_by_class_and_an_abort() {
    replay("v1-error");
}

#[test]
fn v1_heartbeats_and_the_quiet_window() {
    replay("v1-heartbeat");
}

#[test]
fn v1_permission_asks_replies_and_rejects() {
    replay("v1-permission");
}

#[test]
fn v1_questions_under_a_permission() {
    replay("v1-question");
}

#[test]
fn a_v1_retry_and_a_rate_limit() {
    replay("v1-retry-rate-limit");
}

#[test]
fn a_v1_turn_within_one_second() {
    replay("v1-seq-within-one-second");
}

#[test]
fn a_v1_background_subagent_waiting_raises_its_ready_parent() {
    replay("v1-subagent-background");
}

#[test]
fn a_v1_foreground_subagent_waiting_raises_its_parent() {
    replay("v1-subagent-foreground");
}

#[test]
fn v2_compaction_mid_turn_failed_and_while_idle() {
    replay("v2-compaction");
}

#[test]
fn v2_executions_by_outcome() {
    replay("v2-execution");
}

#[test]
fn a_v2_instance_keeps_its_own_location() {
    replay("v2-location");
}

#[test]
fn v2_permissions_and_forms() {
    replay("v2-permission-form");
}

#[test]
fn v2_retries_and_rate_limits() {
    replay("v2-retry");
}

#[test]
fn a_v2_shutdown_and_unload() {
    replay("v2-shutdown-dispose");
}

#[test]
fn a_v2_background_subagent() {
    replay("v2-subagent-background");
}

fn snapshots(name: &str) -> Vec<LiveSnapshot> {
    emitted(name)
        .into_iter()
        .filter_map(|(at_ms, payload)| live_snapshot(&payload, &LiveContext::default(), at_ms))
        .collect()
}

fn shown(store: &LiveStore, session: &str) -> (LiveState, Option<LiveDetail>, Option<u64>) {
    let file = store.read();
    let entry = file.get(HARNESS_KEY, session).expect("an entry");
    (entry.observation.state, entry.observation.detail, entry.seq)
}

#[test]
fn writes_within_one_second_land_in_order_whatever_order_they_take_the_lock() {
    let snapshots = snapshots("v1-seq-within-one-second");
    assert_eq!(snapshots.len(), 5);
    let session = snapshots[0].session_id.clone();
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    let mut reversed = snapshots.clone();
    reversed.reverse();
    let results: Vec<Applied> = reversed
        .iter()
        .map(|snapshot| store.apply_snapshot(snapshot).expect("write"))
        .collect();
    assert!(matches!(results[0], Applied::Written { .. }));
    assert!(
        results[1..]
            .iter()
            .all(|applied| *applied == Applied::Unchanged)
    );
    let last = (LiveState::Ready, Some(LiveDetail::Finished), Some(5));
    assert_eq!(shown(&store, &session), last);

    // In order, then a late copy of an earlier one.
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    for snapshot in &snapshots {
        store.apply_snapshot(snapshot).expect("write");
    }
    assert_eq!(
        store.apply_snapshot(&snapshots[2]).expect("write"),
        Applied::Unchanged
    );
    assert_eq!(shown(&store, &session), last);
}

#[test]
fn a_reloaded_plugin_replaces_the_entry_only_with_evidence_as_new() {
    let snapshots = snapshots("v1-busy-idle");
    let ready = snapshots.last().expect("a snapshot").clone();
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    store.apply_snapshot(&ready).expect("write");

    // The reloaded instance counts from one again.
    let reloaded = |seq: u64, observed_ms: i64, state: LiveState| LiveSnapshot {
        instance: "lq2c1a99-0000000000000001".into(),
        seq,
        observed_ms,
        since_ms: observed_ms,
        state,
        detail: None,
        ..ready.clone()
    };
    assert_eq!(
        store
            .apply_snapshot(&reloaded(1, ready.observed_ms - 1, LiveState::Working))
            .expect("write"),
        Applied::Unchanged,
        "older evidence from another instance"
    );
    assert!(matches!(
        store
            .apply_snapshot(&reloaded(1, ready.observed_ms + 5, LiveState::Working))
            .expect("write"),
        Applied::Written { .. }
    ));
    assert_eq!(
        shown(&store, &ready.session_id),
        (LiveState::Working, None, Some(1))
    );
    // The first instance's late write no longer counts.
    assert_eq!(
        store.apply_snapshot(&ready).expect("write"),
        Applied::Unchanged
    );
    assert_eq!(
        store
            .apply_snapshot(&reloaded(1, ready.observed_ms + 9, LiveState::Ready))
            .expect("write"),
        Applied::Unchanged,
        "the same sequence number again"
    );
}

#[test]
fn a_payload_of_the_older_plugin_is_attributed_and_has_no_live_state() {
    let adapter = OpenCodeAdapter::new();
    for event in [
        "session.created",
        "session.updated",
        "session.status",
        "session.idle",
        "tui.route",
    ] {
        let payload = json!({
            "session_id": "ses_7Kq2mX9vB4nL1pZ8",
            "cwd": "/home/user/projects/my-project",
            "hook_event_name": event,
        });
        assert!(adapter.is_attribution_event(&payload), "{event}");
        assert!(
            adapter
                .live_snapshot_from_payload(&payload, &LiveContext::default(), 1000)
                .is_none(),
            "{event}"
        );
        let record = adapter
            .hook_record_from_payload(&payload, Some(4242), None, None, None, 1)
            .expect("an attribution record");
        assert_eq!(record.hook_event_name.as_deref(), Some(event));
        assert_eq!(
            record.cwd.as_deref(),
            Some("/home/user/projects/my-project")
        );
    }
}
