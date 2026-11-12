//! The shared event scripts under `tests/fixtures/hooks/pi-live/`. The
//! extension's own tests replay each script through its state machine
//! (`plugins/pi-hook/test/conspectus-hook.test.ts`) and check the
//! payloads it emits; here every one of those payloads goes through this
//! parser and the store in a scratch directory, and each session reads as
//! the script says after every step. The scripts are synthetic; their
//! README says how they are laid out.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::live_sequences::{self, assert_shown, corpus_dir};
use crate::discovery::harness::{HarnessAdapter, PiAdapter};
use crate::live_store::{Applied, LiveStore};
use crate::model::live::{LiveDetail, LiveState};

/// The scenarios the research's fixture plan and the story list, and the
/// two the extension's own choices add (`custom-dialog`, `no-pane`).
const SCRIPTS: [&str; 18] = [
    "abort",
    "compaction-failed",
    "custom-dialog",
    "kill-no-shutdown",
    "new-session-switch",
    "no-pane",
    "overflow-compaction-retry",
    "parallel-tools",
    "permission-gate-during-tool",
    "plan-mode-prompt-in-agent-end",
    "rate-limit-429",
    "reload",
    "retry-exhausted",
    "retry-then-success",
    "shutdown-quit",
    "subagent-parallel",
    "threshold-compaction",
    "tool-run",
];

fn fixture_dir() -> PathBuf {
    corpus_dir("pi-live")
}

/// Every payload a script's extension emits, with the time of its step.
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

/// The text the scripts' events carry that must never leave the
/// extension: every error message and every tool name, quoted as JSON
/// would hold them.
fn event_words() -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    for name in SCRIPTS {
        let (_, steps) = live_sequences::sequence(&fixture_dir(), name);
        for step in steps {
            let event = &step["event"];
            for text in [
                &event["toolName"],
                &event["message"]["errorMessage"],
                &event["errorMessage"],
            ] {
                if let Some(text) = text.as_str() {
                    words.insert(Value::String(text.to_string()).to_string());
                }
            }
        }
    }
    words
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
fn no_emitted_payload_carries_text_tool_names_or_errors_from_the_events() {
    let words = event_words();
    for needle in [
        "\"bash\"",
        "\"subagent\"",
        "\"mcp__SENTINELpiserver__query\"",
    ] {
        assert!(words.contains(needle), "{needle} is planted");
    }
    assert!(words.iter().any(|word| word.contains("Too Many Requests")));
    for name in SCRIPTS {
        for (_, payload) in emitted(name) {
            let text = payload.to_string();
            assert!(!text.contains("SENTINEL"), "{name}: {text}");
            for word in &words {
                assert!(!text.contains(word.as_str()), "{name}: {word} in {text}");
            }
        }
    }
}

/// Drive the script `name` through the writer's parser and the store,
/// and check every step. Returns the store's scratch directory.
fn replay(name: &str) -> tempfile::TempDir {
    let adapter = PiAdapter::new();
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
    scratch
}

/// One emitted payload: attributed when it is a session start, read whole,
/// and written to the store, which then holds what it said.
fn check_payload(adapter: &PiAdapter, store: &LiveStore, payload: &Value, at_ms: i64, place: &str) {
    let event = payload["hook_event_name"].as_str().expect("an event name");
    assert_eq!(
        adapter.is_attribution_event(payload),
        event.starts_with("session_start:"),
        "{place}: attribution of {event}"
    );
    assert!(
        event.starts_with("session_start:") || event.starts_with("live:"),
        "{place}: {event}"
    );
    if adapter.is_attribution_event(payload) {
        let record = adapter
            .hook_record_from_payload(payload, None, None, None, None, at_ms / 1000)
            .unwrap_or_else(|err| panic!("{place}: {err}"));
        assert_eq!(
            Some(record.session_key.as_str()),
            payload["session_id"].as_str()
        );
        assert_eq!(record.pid, payload["live"]["pid"].as_i64(), "{place}: pid");
    }
    let snapshot = adapter
        .live_snapshot_from_payload(payload, &LiveContext::default(), at_ms)
        .unwrap_or_else(|| panic!("{place}: no snapshot"));
    let live = &payload["live"];
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
        "since_ms": observation.since_ms,
    });
    let mut sent = live.clone();
    if sent.get("detail").is_none() {
        sent["detail"] = Value::Null;
    }
    assert_eq!(recorded, sent, "{place}: the entry holds the snapshot");
    assert!(!observation.via_subagent, "{place}");
    assert_eq!(
        entry.cwd.as_deref(),
        payload["cwd"].as_str(),
        "{place}: cwd"
    );
    assert_eq!(
        entry.transcript_path.as_deref(),
        payload["transcript_path"].as_str(),
        "{place}: transcript path"
    );
}

#[test]
fn a_tool_call_in_a_turn() {
    replay("tool-run");
}

#[test]
fn parallel_tool_calls() {
    replay("parallel-tools");
}

#[test]
fn a_gate_extensions_dialogs_during_tool_calls() {
    replay("permission-gate-during-tool");
}

#[test]
fn a_plan_approval_asked_before_the_run_settles() {
    replay("plan-mode-prompt-in-agent-end");
}

#[test]
fn a_retry_that_succeeds() {
    replay("retry-then-success");
}

#[test]
fn retries_that_all_fail() {
    replay("retry-exhausted");
}

#[test]
fn rate_limits_and_an_auth_failure() {
    replay("rate-limit-429");
}

#[test]
fn compaction_at_the_threshold_and_by_hand() {
    replay("threshold-compaction");
}

#[test]
fn compaction_for_a_context_overflow_and_its_retry() {
    replay("overflow-compaction-retry");
}

#[test]
fn compactions_cancelled_and_failed() {
    replay("compaction-failed");
}

#[test]
fn interrupts() {
    replay("abort");
}

#[test]
fn session_switches_inside_one_pi() {
    replay("new-session-switch");
}

#[test]
fn reloads() {
    replay("reload");
}

#[test]
fn subagent_tool_calls_in_parallel() {
    replay("subagent-parallel");
}

#[test]
fn a_quit_ends_the_session() {
    replay("shutdown-quit");
}

#[test]
fn a_killed_pi_turns_quiet_and_reads_killed_once_its_pid_is_gone() {
    replay("kill-no-shutdown");
}

#[test]
fn custom_dialogs_are_not_a_wait() {
    replay("custom-dialog");
}

#[test]
fn runs_outside_a_pane_leave_no_entry() {
    assert!(emitted("no-pane").is_empty());
    let scratch = replay("no-pane");
    assert!(LiveStore::at(scratch.path()).read().entries.is_empty());
}

#[test]
fn no_text_reaches_the_store_from_any_script() {
    let words = event_words();
    for name in SCRIPTS {
        let scratch = replay(name);
        let path = LiveStore::at(scratch.path()).path();
        let Ok(file) = fs::read_to_string(&path) else {
            continue;
        };
        assert!(!file.contains("SENTINEL"), "{name}: {file}");
        for word in &words {
            assert!(!file.contains(word.as_str()), "{name}: {word} in {file}");
        }
    }
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
fn writers_that_finish_out_of_order_leave_the_latest_snapshot() {
    // The extension's writers are detached, so two started a moment apart
    // can take the store's lock in either order.
    let snapshots = snapshots("tool-run");
    let last = snapshots.last().expect("a snapshot").clone();
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    let results: Vec<Applied> = snapshots
        .iter()
        .rev()
        .map(|snapshot| store.apply_snapshot(snapshot).expect("write"))
        .collect();
    assert!(matches!(results[0], Applied::Written { .. }));
    assert!(
        results[1..]
            .iter()
            .all(|applied| *applied == Applied::Unchanged)
    );
    let expected = (LiveState::Ready, Some(LiveDetail::Finished), Some(last.seq));
    assert_eq!(shown(&store, &last.session_id), expected);
    assert_eq!(
        store.apply_snapshot(&snapshots[2]).expect("write"),
        Applied::Unchanged,
        "a late copy of an earlier one"
    );
    assert_eq!(shown(&store, &last.session_id), expected);
}

#[test]
fn a_reloaded_extension_takes_over_and_the_old_loads_late_writes_do_not_count() {
    let snapshots = snapshots("reload");
    let session = snapshots[0].session_id.clone();
    let first = snapshots[0].instance.clone();
    let reloaded = snapshots
        .iter()
        .find(|snapshot| snapshot.instance != first)
        .expect("a snapshot of the reloaded extension")
        .clone();
    assert_eq!(reloaded.seq, 1, "a new load counts from one");
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    store.apply_snapshot(&reloaded).expect("write");
    // The old load's last write, finishing after the new load's first.
    let old_last = snapshots
        .iter()
        .rfind(|snapshot| snapshot.instance == first)
        .expect("an old snapshot")
        .clone();
    assert_eq!(
        store.apply_snapshot(&old_last).expect("write"),
        Applied::Unchanged
    );
    assert_eq!(shown(&store, &session), (LiveState::Ready, None, Some(1)));
}

#[test]
fn a_session_resumed_in_the_same_process_is_live_again() {
    let snapshots = snapshots("new-session-switch");
    let first = snapshots[0].session_id.clone();
    let scratch = tempfile::tempdir().expect("scratch");
    let store = LiveStore::at(scratch.path());
    for snapshot in &snapshots {
        store.apply_snapshot(snapshot).expect("write");
    }
    let file = store.read();
    let sessions: Vec<(&str, LiveState)> = file
        .entries
        .values()
        .map(|entry| (entry.session_id.as_str(), entry.observation.state))
        .collect();
    assert_eq!(sessions.len(), 3, "{sessions:?}");
    assert_eq!(
        sessions
            .iter()
            .filter(|(_, state)| *state != LiveState::Exited)
            .count(),
        1,
        "one live session per process: {sessions:?}"
    );
    assert_eq!(shown(&store, &first).0, LiveState::Ready);
}
