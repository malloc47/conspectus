//! The Codex mapping, one event at a time, from payloads shaped as the
//! Codex hook schemas document them.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::HarnessAdapter;
use crate::live_store::LiveTmux;

use LiveEventKind as K;

const SESSION: &str = "019a7c3e-5b2d-7f41-9c8e-2d4b6a1f0e37";
const CHILD: &str = "019a7c41-0c6e-7d2a-b5f3-8e9d0a1b2c3d";
const TRANSCRIPT: &str =
    "/home/user/.codex/sessions/2026/11/12/rollout-2026-11-12T09-14-03-019a7c3e.jsonl";

fn writer_context() -> LiveContext {
    LiveContext {
        pid: Some(4242),
        pid_start: Some(77),
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: None,
            pane: Some("%3".into()),
        }),
        ..LiveContext::default()
    }
}

/// A payload of `event` with the common fields and `extra` on top.
fn payload(event: &str, extra: Value) -> Value {
    let mut payload = json!({
        "session_id": SESSION,
        "transcript_path": TRANSCRIPT,
        "cwd": "/work/project",
        "hook_event_name": event,
        "model": "gpt-5.4",
        "permission_mode": "default",
    });
    let fields = payload.as_object_mut().expect("payload object");
    for (key, value) in extra.as_object().expect("extra object") {
        fields.insert(key.clone(), value.clone());
    }
    payload
}

/// The kinds `payload` maps to.
fn kinds(payload: &Value) -> Vec<K> {
    live_events(payload, &writer_context(), 900)
        .into_iter()
        .map(|event| event.kind)
        .collect()
}

fn child(event: &str) -> Value {
    payload(event, json!({"agent_id": CHILD, "agent_type": "worker"}))
}

#[test]
fn a_session_start_is_a_live_session_start_carrying_the_payload_paths() {
    let payload = payload("SessionStart", json!({"source": "startup"}));
    assert_eq!(
        live_events(&payload, &writer_context(), 900),
        vec![
            LiveEvent::new(
                HARNESS_KEY,
                SESSION,
                900,
                K::SessionStarted {
                    after_compaction: false
                },
            )
            .with_context(LiveContext {
                cwd: Some("/work/project".to_string()),
                transcript_path: Some(TRANSCRIPT.to_string()),
                ..writer_context()
            })
        ]
    );
}

#[test]
fn only_the_start_after_compaction_resumes_the_session() {
    for (source, after_compaction) in [
        ("startup", false),
        ("resume", false),
        ("clear", false),
        ("fork", false),
        ("compact", true),
        ("a-source-codex-adds-later", false),
    ] {
        assert_eq!(
            kinds(&payload("SessionStart", json!({"source": source}))),
            vec![K::SessionStarted { after_compaction }],
            "source {source}"
        );
    }
}

#[test]
fn an_ephemeral_session_has_no_transcript_and_keeps_what_the_writer_knew() {
    let payload = payload(
        "SessionStart",
        json!({"source": "startup", "transcript_path": null}),
    );
    let context = LiveContext {
        transcript_path: Some("/known/rollout.jsonl".into()),
        ..writer_context()
    };
    let events = live_events(&payload, &context, 900);
    assert_eq!(
        events[0].context.transcript_path.as_deref(),
        Some("/known/rollout.jsonl")
    );
    let events = live_events(&payload, &writer_context(), 900);
    assert_eq!(events[0].context.transcript_path, None);
}

#[test]
fn a_prompt_starts_the_main_agents_turn() {
    assert_eq!(
        kinds(&payload(
            "UserPromptSubmit",
            json!({"turn_id": "019a7c3f-0000-7000-8000-000000000001", "prompt": "do the thing"})
        )),
        vec![K::TurnStarted]
    );
}

#[test]
fn a_permission_request_is_a_wait_of_the_agent_that_asks() {
    for tool in ["Bash", "apply_patch", "mcp__docs__search"] {
        assert_eq!(
            kinds(&payload(
                "PermissionRequest",
                json!({"tool_name": tool, "tool_input": {"command": "x"}})
            )),
            vec![K::WaitStarted {
                agent: None,
                kind: WaitKind::Permission
            }],
            "{tool}"
        );
    }
    assert_eq!(
        kinds(&child("PermissionRequest")),
        vec![K::WaitStarted {
            agent: Some(CHILD.into()),
            kind: WaitKind::Permission
        }]
    );
}

#[test]
fn a_finished_tool_ends_the_wait_of_the_agent_it_ran_for() {
    assert_eq!(
        kinds(&payload("PostToolUse", json!({"tool_name": "Bash"}))),
        vec![K::ToolFinished { agent: None }]
    );
    assert_eq!(
        kinds(&child("PostToolUse")),
        vec![K::ToolFinished {
            agent: Some(CHILD.into())
        }]
    );
}

#[test]
fn a_stop_finishes_the_turn_in_plan_mode_as_in_any_other() {
    for mode in [
        "default",
        "acceptEdits",
        "plan",
        "dontAsk",
        "bypassPermissions",
    ] {
        assert_eq!(
            kinds(&payload(
                "Stop",
                json!({"permission_mode": mode, "stop_hook_active": false})
            )),
            vec![K::TurnEnded { interrupted: false }],
            "{mode}"
        );
    }
    // A continuation another Stop hook forced ends the same way.
    assert_eq!(
        kinds(&payload("Stop", json!({"stop_hook_active": true}))),
        vec![K::TurnEnded { interrupted: false }]
    );
}

#[test]
fn an_interrupt_ends_the_turn_as_interrupted() {
    assert_eq!(
        kinds(&payload("Interrupt", json!({}))),
        vec![K::TurnEnded { interrupted: true }]
    );
}

#[test]
fn subagent_events_start_and_stop_a_child_by_its_id() {
    assert_eq!(
        kinds(&child("SubagentStart")),
        vec![K::SubagentStarted { id: CHILD.into() }]
    );
    assert_eq!(
        kinds(&child("SubagentStop")),
        vec![K::SubagentStopped { id: CHILD.into() }]
    );
    // Without an id there is no child to count.
    assert!(kinds(&payload("SubagentStart", json!({"agent_type": "worker"}))).is_empty());
    assert!(kinds(&payload("SubagentStop", json!({"agent_id": ""}))).is_empty());
}

#[test]
fn compaction_and_the_session_end_are_the_main_agents() {
    for (trigger, event, kind) in [
        ("auto", "PreCompact", K::CompactionStarted),
        ("manual", "PreCompact", K::CompactionStarted),
        ("auto", "PostCompact", K::CompactionEnded),
        ("manual", "PostCompact", K::CompactionEnded),
    ] {
        assert_eq!(
            kinds(&payload(event, json!({"trigger": trigger}))),
            vec![kind],
            "{event} {trigger}"
        );
    }
    assert_eq!(
        kinds(&payload("SessionEnd", json!({"reason": "other"}))),
        vec![K::SessionEnded]
    );
}

#[test]
fn a_childs_turn_events_are_dropped_and_its_paths_never_recorded() {
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "Stop",
        "Interrupt",
        "PreCompact",
        "PostCompact",
        "SessionEnd",
    ] {
        assert!(kinds(&child(event)).is_empty(), "{event}");
    }
    let events = live_events(&child("PermissionRequest"), &writer_context(), 900);
    assert_eq!(events[0].context, writer_context());
}

#[test]
fn an_agent_type_alone_names_no_child() {
    assert_eq!(
        kinds(&payload(
            "PermissionRequest",
            json!({"agent_type": "worker"})
        )),
        vec![K::WaitStarted {
            agent: None,
            kind: WaitKind::Permission
        }]
    );
}

#[test]
fn an_event_the_mapping_does_not_cover_is_ignored() {
    for event in [
        "PreToolUse",
        "PreModelRequest",
        "PostModelRequest",
        "Notification",
        "AnEventCodexAddsLater",
    ] {
        assert!(
            kinds(&payload(
                event,
                json!({"status": "failed", "error": "upstream text"})
            ))
            .is_empty(),
            "{event}"
        );
    }
    for payload in [
        json!({"session_id": SESSION}),
        json!({"session_id": SESSION, "hook_event_name": 3}),
        json!({"hook_event_name": "Stop"}),
        json!({"hook_event_name": "Stop", "session_id": ""}),
        json!({"hook_event_name": "Stop", "session_id": 7}),
        json!("Stop"),
        json!([1, 2]),
    ] {
        assert!(kinds(&payload).is_empty(), "{payload}");
    }
}

#[test]
fn every_subscribed_event_maps_to_something() {
    for event in HOOK_EVENTS {
        let extra = match event {
            "SubagentStart" | "SubagentStop" => json!({"agent_id": CHILD}),
            _ => json!({}),
        };
        assert!(!kinds(&payload(event, extra)).is_empty(), "{event}");
    }
}

#[test]
fn only_a_session_start_is_recorded_for_attribution() {
    let adapter = super::super::CodexAdapter::new();
    let event = |name: &str| json!({"hook_event_name": name, "session_id": "s"});
    assert!(adapter.is_attribution_event(&event("SessionStart")));
    for name in HOOK_EVENTS.iter().skip(1) {
        assert!(!adapter.is_attribution_event(&event(name)), "{name}");
    }
    assert!(!adapter.is_attribution_event(&event("PreModelRequest")));
    assert!(!adapter.is_attribution_event(&json!({"hook_event_name": 3})));
    // A caller older than per-event hooks sends no event name.
    assert!(adapter.is_attribution_event(&json!({"session_id": "s"})));
}

#[test]
fn the_adapter_hands_its_payloads_to_this_mapping_and_reports_live_state() {
    let adapter = super::super::CodexAdapter::new();
    let payload = payload("Stop", json!({}));
    assert_eq!(
        adapter.live_events_from_payload(&payload, &writer_context(), 5),
        live_events(&payload, &writer_context(), 5)
    );
    assert!(adapter.reports_live_state());
    assert!(
        adapter
            .live_snapshot_from_payload(&payload, &writer_context(), 5)
            .is_none()
    );
}

#[test]
fn a_huge_payload_field_costs_nothing_to_map() {
    // A tool's response can run to megabytes; the mapping looks fields
    // up by name and never reads this one.
    let large = "x".repeat(32 * 1024 * 1024);
    let payload = payload(
        "PostToolUse",
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat big.log"},
            "tool_response": large,
        }),
    );
    let started = Instant::now();
    for _ in 0..1_000 {
        assert_eq!(kinds(&payload), vec![K::ToolFinished { agent: None }]);
    }
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}
