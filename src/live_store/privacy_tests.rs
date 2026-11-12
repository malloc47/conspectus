//! No payload text reaches the live-state file (ADR 0086 Tier 3).
//!
//! The inputs hold only ids, paths, numbers, and closed enums, so a
//! payload has nowhere to go; these tests keep it that way. One walks
//! every string the inputs and the file can hold and fails on a string
//! in a field that is neither an id or a path nor a closed name, which
//! is what a new text-capable field looks like. The others plant a
//! sentinel in every string input and check where each one ends up:
//! ids and paths only under their own field, ids the reducer no longer
//! needs nowhere, and payload-like text nowhere at all.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde_json::Value;

use super::*;
use crate::model::live::{LiveDetail, LiveSource, LiveState};

/// Fields whose strings are ids or paths, which the store may keep.
/// `entries` stands for the keys of the entry map.
const ID_AND_PATH_FIELDS: &[&str] = &[
    "harness",
    "session_id",
    "cwd",
    "transcript_path",
    "socket",
    "session",
    "pane",
    "instance",
    "agent",
    "id",
    "subagents",
    "entries",
];

/// Fields whose strings are names from a closed set.
const NAME_FIELDS: &[&str] = &["state", "detail", "source", "kind", "class"];

const WAIT_KINDS: [WaitKind; 4] = [
    WaitKind::Permission,
    WaitKind::Question,
    WaitKind::Plan,
    WaitKind::Input,
];

const FAILURE_CLASSES: [FailureClass; 4] = [
    FailureClass::RateLimited,
    FailureClass::Auth,
    FailureClass::Overloaded,
    FailureClass::Failed,
];

/// Every closed name a string field may hold.
fn closed_names() -> BTreeSet<String> {
    let mut names: BTreeSet<String> = LiveState::ALL.iter().map(|s| s.name().into()).collect();
    names.extend(LiveDetail::ALL.iter().map(|d| d.name().to_string()));
    names.extend(LiveSource::ALL.iter().map(|s| s.name().to_string()));
    let serialized = WAIT_KINDS
        .iter()
        .map(|kind| serde_json::to_value(kind).unwrap())
        .chain(
            FAILURE_CLASSES
                .iter()
                .map(|c| serde_json::to_value(c).unwrap()),
        )
        .chain(
            every_kind("agent-x")
                .into_iter()
                .map(|k| serde_json::to_value(k).unwrap()),
        );
    names.extend(serialized.filter_map(|value| value.as_str().map(str::to_string)));
    names
}

/// One event kind of each variant, every optional id present.
fn every_kind(agent: &str) -> Vec<LiveEventKind> {
    let agent = Some(agent.to_string());
    let mut kinds = vec![
        LiveEventKind::SessionStarted {
            after_compaction: true,
        },
        LiveEventKind::TurnStarted,
        LiveEventKind::Retrying { rate_limited: true },
        LiveEventKind::WaitEnded {
            agent: agent.clone(),
        },
        LiveEventKind::ToolFinished {
            agent: agent.clone(),
        },
        LiveEventKind::SubagentStarted {
            id: "child-x".into(),
        },
        LiveEventKind::SubagentStopped {
            id: "child-x".into(),
        },
        LiveEventKind::BackgroundWork {
            subagents: vec!["child-x".into()],
        },
        LiveEventKind::CompactionStarted,
        LiveEventKind::CompactionEnded,
        LiveEventKind::TurnEnded { interrupted: true },
        LiveEventKind::IdleConfirmed,
        LiveEventKind::SessionEnded,
    ];
    kinds.extend(WAIT_KINDS.map(|kind| LiveEventKind::WaitStarted {
        agent: agent.clone(),
        kind,
    }));
    kinds.extend(FAILURE_CLASSES.map(|class| LiveEventKind::TurnFailed { class }));
    kinds
}

/// Every string in `value` with the field it sits in.
fn strings(value: &Value, field: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::String(text) => out.push((field.to_string(), text.clone())),
        Value::Array(items) => items.iter().for_each(|item| strings(item, field, out)),
        Value::Object(map) if field == "entries" => {
            for (key, entry) in map {
                out.push((field.to_string(), key.clone()));
                strings(entry, "", out);
            }
        }
        Value::Object(map) => map.iter().for_each(|(key, item)| strings(item, key, out)),
        _ => {}
    }
}

fn assert_only_ids_paths_and_names(value: &Value) {
    let names = closed_names();
    let mut found = Vec::new();
    strings(value, "", &mut found);
    assert!(!found.is_empty());
    for (field, text) in found {
        if ID_AND_PATH_FIELDS.contains(&field.as_str()) {
            continue;
        }
        assert!(
            NAME_FIELDS.contains(&field.as_str()) && names.contains(&text),
            "`{field}` holds {text:?}, which is neither an id, a path, nor a closed name"
        );
    }
}

/// A context with every field set.
fn full_context(tag: &str) -> LiveContext {
    LiveContext {
        cwd: Some(format!("/work/{tag}-cwd")),
        transcript_path: Some(format!("/transcripts/{tag}-transcript.jsonl")),
        tmux: Some(LiveTmux {
            socket: Some(format!("/tmp/{tag}-socket")),
            session: Some(format!("{tag}-tmux-session")),
            pane: Some(format!("%{tag}-pane")),
        }),
        pid: Some(4242),
        pid_start: Some(17),
    }
}

fn full_snapshot(instance: &str, context: LiveContext) -> LiveSnapshot {
    LiveSnapshot {
        harness: "harness-b".into(),
        session_id: "session-b".into(),
        instance: instance.into(),
        seq: 3,
        observed_ms: 1_000,
        context,
        state: LiveState::Waiting,
        detail: Some(LiveDetail::Permission),
        since_ms: 900,
        subagents: 2,
        via_subagent: true,
        heartbeat_s: 60,
    }
}

#[test]
fn no_input_or_file_holds_a_string_outside_ids_paths_and_closed_names() {
    for kind in every_kind("agent-x") {
        let event = LiveEvent {
            harness: "harness-a".into(),
            session_id: "session-a".into(),
            observed_ms: 100,
            context: full_context("walk"),
            kind,
        };
        assert_only_ids_paths_and_names(&serde_json::to_value(&event).unwrap());
    }
    assert_only_ids_paths_and_names(
        &serde_json::to_value(full_snapshot("instance-1", full_context("walk"))).unwrap(),
    );

    // A file whose reducer block holds everything it can: the main
    // agent's state and its resume, waits of both kinds of agent, and
    // child agents.
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    for kind in [
        LiveEventKind::TurnStarted,
        LiveEventKind::SubagentStarted {
            id: "child-x".into(),
        },
        LiveEventKind::WaitStarted {
            agent: Some("child-x".into()),
            kind: WaitKind::Permission,
        },
        LiveEventKind::WaitStarted {
            agent: None,
            kind: WaitKind::Question,
        },
        LiveEventKind::CompactionStarted,
    ] {
        let event =
            LiveEvent::new("harness-a", "session-a", 100, kind).with_context(full_context("walk"));
        store.apply_event(&event).unwrap();
    }
    store
        .apply_snapshot(&full_snapshot("instance-1", full_context("walk")))
        .unwrap();
    let text = fs::read_to_string(store.path()).unwrap();
    for field in ["\"resume\"", "\"waits\"", "\"subagents\"", "\"main\""] {
        assert!(text.contains(field), "the walk saw no {field}");
    }
    assert_only_ids_paths_and_names(&serde_json::from_str(&text).unwrap());
}

#[test]
fn a_kept_id_or_path_lands_only_in_its_own_field_and_spent_ids_leave() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let kept: BTreeMap<&str, &[&str]> = BTreeMap::from([
        ("sentinel-harness", &["harness", "entries"][..]),
        ("sentinel-session", &["session_id", "entries"][..]),
        ("sentinel-cwd", &["cwd"][..]),
        ("sentinel-transcript", &["transcript_path"][..]),
        ("sentinel-socket", &["socket"][..]),
        ("sentinel-tmux-session", &["session"][..]),
        ("sentinel-pane", &["pane"][..]),
        ("sentinel-child", &["subagents", "agent"][..]),
        ("sentinel-background", &["subagents"][..]),
    ]);
    let send = |kind: LiveEventKind| {
        let event = LiveEvent::new("sentinel-harness", "sentinel-session", 100, kind)
            .with_context(full_context("sentinel"));
        assert!(matches!(
            store.apply_event(&event).unwrap(),
            Applied::Written { .. } | Applied::Unchanged
        ));
    };
    for kind in [
        LiveEventKind::TurnStarted,
        LiveEventKind::SubagentStarted {
            id: "sentinel-child".into(),
        },
        LiveEventKind::BackgroundWork {
            subagents: vec!["sentinel-child".into(), "sentinel-background".into()],
        },
        LiveEventKind::WaitStarted {
            agent: Some("sentinel-child".into()),
            kind: WaitKind::Permission,
        },
    ] {
        send(kind);
    }
    let text = fs::read_to_string(store.path()).unwrap();
    let mut found = Vec::new();
    strings(&serde_json::from_str(&text).unwrap(), "", &mut found);
    for (sentinel, fields) in &kept {
        let homes: BTreeSet<&str> = found
            .iter()
            .filter(|(_, text)| text.contains(sentinel))
            .map(|(field, _)| field.as_str())
            .collect();
        assert!(!homes.is_empty(), "{sentinel} was never stored");
        assert!(
            homes.iter().all(|home| fields.contains(home)),
            "{sentinel} landed in {homes:?}"
        );
    }

    // Once a child's wait and the child itself are over, the reducer
    // keeps no trace of its id.
    for kind in [
        LiveEventKind::ToolFinished {
            agent: Some("sentinel-child".into()),
        },
        LiveEventKind::SubagentStopped {
            id: "sentinel-child".into(),
        },
        LiveEventKind::BackgroundWork { subagents: vec![] },
        LiveEventKind::TurnEnded { interrupted: false },
    ] {
        send(kind);
    }
    let text = fs::read_to_string(store.path()).unwrap();
    assert!(!text.contains("sentinel-child"));
    assert!(!text.contains("sentinel-background"));
    assert!(text.contains("sentinel-session"));
}

/// Text as a payload carries it: words, spaces, and a line break, with a
/// leading slash so that only the line break stops it passing for a
/// path.
const PAYLOAD: &str = "/SENTINEL-PAYLOAD please summarize\nthe launch plan";

#[test]
fn payload_text_in_any_string_input_never_reaches_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let payload = || PAYLOAD.to_string();
    let event =
        |harness: String, session: String, context: LiveContext, kind: LiveEventKind| LiveEvent {
            harness,
            session_id: session,
            observed_ms: 100,
            context,
            kind,
        };
    let ok = || "ok".to_string();
    let clean = LiveContext::default;

    // An id that carries text refuses the whole input.
    let mut refused = vec![
        event(payload(), ok(), clean(), LiveEventKind::TurnStarted),
        event(ok(), payload(), clean(), LiveEventKind::TurnStarted),
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::SubagentStarted { id: payload() },
        ),
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::SubagentStopped { id: payload() },
        ),
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::BackgroundWork {
                subagents: vec![ok(), payload()],
            },
        ),
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::WaitEnded {
                agent: Some(payload()),
            },
        ),
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::ToolFinished {
                agent: Some(payload()),
            },
        ),
    ];
    refused.extend(WAIT_KINDS.map(|kind| {
        event(
            ok(),
            ok(),
            clean(),
            LiveEventKind::WaitStarted {
                agent: Some(payload()),
                kind,
            },
        )
    }));
    for input in &refused {
        assert_eq!(
            store.apply_event(input).unwrap(),
            Applied::Rejected,
            "{input:?}"
        );
    }
    let mut snapshots = vec![
        full_snapshot(&payload(), clean()),
        LiveSnapshot {
            harness: payload(),
            ..full_snapshot("instance-1", clean())
        },
        LiveSnapshot {
            session_id: payload(),
            ..full_snapshot("instance-1", clean())
        },
    ];
    for snapshot in &snapshots {
        assert_eq!(store.apply_snapshot(snapshot).unwrap(), Applied::Rejected);
    }

    // A path or name that carries text is dropped and the rest is kept.
    let texty_context = || LiveContext {
        cwd: Some(payload()),
        transcript_path: Some(payload()),
        tmux: Some(LiveTmux {
            socket: Some(payload()),
            session: Some(payload()),
            pane: Some(payload()),
        }),
        pid: Some(4242),
        pid_start: None,
    };
    for kind in every_kind("agent-x") {
        let written = store
            .apply_event(&event(ok(), ok(), texty_context(), kind))
            .unwrap();
        assert!(matches!(
            written,
            Applied::Written { .. } | Applied::Unchanged
        ));
    }
    snapshots = vec![full_snapshot("instance-2", texty_context())];
    for snapshot in &snapshots {
        assert!(matches!(
            store.apply_snapshot(snapshot).unwrap(),
            Applied::Written { .. }
        ));
    }

    let text = fs::read_to_string(store.path()).unwrap();
    assert!(!text.contains("SENTINEL-PAYLOAD"));
    assert!(!text.contains("summarize"));
    let file = store.read();
    let entry = file.get("ok", "ok").unwrap();
    assert_eq!(entry.observation.pid, Some(4242));
    assert_eq!((entry.cwd.as_ref(), entry.tmux.as_ref()), (None, None));
    assert!(file.get("harness-b", "session-b").is_some());
    assert_only_ids_paths_and_names(&serde_json::from_str(&text).unwrap());
}
