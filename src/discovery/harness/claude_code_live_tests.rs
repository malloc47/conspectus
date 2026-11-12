//! The Claude Code mapping, one event at a time, from payloads shaped as
//! the hooks reference documents them.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::*;
use crate::discovery::harness::HarnessAdapter;
use crate::live_store::LiveTmux;

use LiveEventKind as K;

const SESSION: &str = "11111111-2222-3333-4444-555555555555";
const CHILD: &str = "a1b2c3d4e5f6a7b8c";

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
        "transcript_path": "/home/user/.claude/projects/-work-project/1111.jsonl",
        "cwd": "/work/project",
        "permission_mode": "default",
        "hook_event_name": event,
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

#[test]
fn a_session_start_is_a_live_session_start_carrying_the_payload_paths() {
    let payload = payload(
        "SessionStart",
        json!({"source": "startup", "model": "a-model"}),
    );
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
                transcript_path: Some(
                    "/home/user/.claude/projects/-work-project/1111.jsonl".to_string()
                ),
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
    ] {
        assert_eq!(
            kinds(&payload("SessionStart", json!({"source": source}))),
            vec![K::SessionStarted { after_compaction }],
            "source {source}"
        );
    }
    // A session started with `--agent` names the agent type and no
    // agent id; it is the session's own start.
    assert_eq!(
        kinds(&payload(
            "SessionStart",
            json!({"source": "startup", "agent_type": "reviewer"})
        )),
        vec![K::SessionStarted {
            after_compaction: false
        }]
    );
}

#[test]
fn a_prompt_starts_the_main_agents_turn() {
    assert_eq!(
        kinds(&payload(
            "UserPromptSubmit",
            json!({"prompt": "do the thing", "prompt_id": "p-1"})
        )),
        vec![K::TurnStarted]
    );
    // A child agent's prompt is the child's own turn.
    assert!(kinds(&payload("UserPromptSubmit", json!({"agent_id": CHILD}))).is_empty());
}

#[test]
fn a_permission_request_is_a_wait_of_the_agent_that_asks() {
    assert_eq!(
        kinds(&payload("PermissionRequest", json!({"tool_name": "Bash"}))),
        vec![K::WaitStarted {
            agent: None,
            kind: WaitKind::Permission
        }]
    );
    assert_eq!(
        kinds(&payload(
            "PermissionRequest",
            json!({"tool_name": "Edit", "agent_id": CHILD, "agent_type": "Explore"})
        )),
        vec![K::WaitStarted {
            agent: Some(CHILD.into()),
            kind: WaitKind::Permission
        }]
    );
    // Should the dialog of a question or a plan come as a permission
    // request too, it stays the wait its tool is.
    for (tool, kind) in [
        ("AskUserQuestion", WaitKind::Question),
        ("ExitPlanMode", WaitKind::Plan),
    ] {
        assert_eq!(
            kinds(&payload("PermissionRequest", json!({"tool_name": tool}))),
            vec![K::WaitStarted { agent: None, kind }]
        );
    }
}

#[test]
fn only_the_dialog_tools_pre_tool_use_is_a_wait() {
    for (tool, kind) in [
        ("AskUserQuestion", WaitKind::Question),
        ("ExitPlanMode", WaitKind::Plan),
    ] {
        assert_eq!(
            kinds(&payload(
                "PreToolUse",
                json!({"tool_name": tool, "tool_use_id": "toolu_01"})
            )),
            vec![K::WaitStarted { agent: None, kind }]
        );
    }
    for tool in ["Bash", "mcp__server__tool", "Agent"] {
        assert!(
            kinds(&payload("PreToolUse", json!({"tool_name": tool}))).is_empty(),
            "{tool}"
        );
    }
}

#[test]
fn a_finished_tool_ends_its_agents_wait() {
    assert_eq!(
        kinds(&payload(
            "PostToolUse",
            json!({"tool_name": "Bash", "tool_use_id": "toolu_01", "duration_ms": 12})
        )),
        vec![K::ToolFinished { agent: None }]
    );
    assert_eq!(
        kinds(&payload(
            "PostToolUse",
            json!({"tool_name": "Bash", "agent_id": CHILD})
        )),
        vec![K::ToolFinished {
            agent: Some(CHILD.into())
        }]
    );
    // Another tool's response is never read, whatever its shape.
    for response in [json!("plain text"), json!({"status": 200}), json!([1, 2])] {
        assert_eq!(
            kinds(&payload(
                "PostToolUse",
                json!({"tool_name": "WebFetch", "tool_response": response})
            )),
            vec![K::ToolFinished { agent: None }]
        );
    }
}

#[test]
fn the_subagent_tools_result_starts_a_background_child_or_stops_a_finished_one() {
    for tool in SUBAGENT_TOOLS {
        assert_eq!(
            kinds(&payload(
                "PostToolUse",
                json!({
                    "tool_name": tool,
                    "tool_response": {"status": "async_launched", "agentId": CHILD, "prompt": "x"},
                })
            )),
            vec![
                K::ToolFinished { agent: None },
                K::SubagentStarted { id: CHILD.into() }
            ],
            "{tool}"
        );
    }
    assert_eq!(
        kinds(&payload(
            "PostToolUse",
            json!({
                "tool_name": "Agent",
                "tool_response": {"status": "completed", "agentId": CHILD, "content": []},
            })
        )),
        vec![
            K::ToolFinished { agent: None },
            K::SubagentStopped { id: CHILD.into() }
        ]
    );
    // An odd result, or an id that does not look like one, leaves only
    // the finished tool.
    for response in [
        json!({"status": "async_launched"}),
        json!({"status": "async_launched", "agentId": "two words"}),
        json!({"status": "teleported", "agentId": CHILD}),
        json!({"status": 3, "agentId": CHILD}),
        json!("async_launched"),
    ] {
        assert_eq!(
            kinds(&payload(
                "PostToolUse",
                json!({"tool_name": "Agent", "tool_response": response})
            )),
            vec![K::ToolFinished { agent: None }],
            "{response}"
        );
    }
}

#[test]
fn a_failed_tool_ends_the_wait_and_an_interrupted_one_ends_the_turn() {
    assert_eq!(
        kinds(&payload(
            "PostToolUseFailure",
            json!({"tool_name": "Bash", "error": "exit 1", "is_interrupt": false})
        )),
        vec![K::ToolFinished { agent: None }]
    );
    assert_eq!(
        kinds(&payload(
            "PostToolUseFailure",
            json!({"tool_name": "Bash", "is_interrupt": true})
        )),
        vec![
            K::ToolFinished { agent: None },
            K::TurnEnded { interrupted: true }
        ]
    );
    // A child's interrupted tool says nothing about the main turn.
    assert_eq!(
        kinds(&payload(
            "PostToolUseFailure",
            json!({"tool_name": "Bash", "is_interrupt": true, "agent_id": CHILD})
        )),
        vec![K::ToolFinished {
            agent: Some(CHILD.into())
        }]
    );
}

#[test]
fn each_subscribed_notification_type_has_its_meaning() {
    let confirmed = |kind| K::WaitConfirmed { kind };
    for (notification, kind) in [
        ("permission_prompt", confirmed(WaitKind::Permission)),
        ("elicitation_dialog", confirmed(WaitKind::Input)),
        ("elicitation_url_dialog", confirmed(WaitKind::Input)),
        ("agent_needs_input", confirmed(WaitKind::Input)),
        ("idle_prompt", K::IdleConfirmed),
        ("quota_auto_resume_disabled", K::IdleConfirmed),
        ("quota_auto_resume_fired", K::TurnStarted),
        (
            "quota_auto_resume_stale",
            K::WaitStarted {
                agent: None,
                kind: WaitKind::Input,
            },
        ),
    ] {
        assert_eq!(
            kinds(&payload(
                "Notification",
                json!({"notification_type": notification, "message": "m", "title": "t"})
            )),
            vec![kind],
            "{notification}"
        );
    }
    for notification in [
        "elicitation_complete",
        "elicitation_response",
        "agent_completed",
        "auth_success",
        "auth_storage_failure",
        "plugin_notification",
    ] {
        assert!(
            kinds(&payload(
                "Notification",
                json!({"notification_type": notification})
            ))
            .is_empty(),
            "{notification}"
        );
    }
    assert!(kinds(&payload("Notification", json!({"message": "m"}))).is_empty());
}

#[test]
fn child_agents_start_and_stop_by_id() {
    assert_eq!(
        kinds(&payload(
            "SubagentStart",
            json!({"agent_id": CHILD, "agent_type": "Explore"})
        )),
        vec![K::SubagentStarted { id: CHILD.into() }]
    );
    assert_eq!(
        kinds(&payload(
            "SubagentStop",
            json!({
                "agent_id": CHILD,
                "agent_type": "Explore",
                "stop_hook_active": false,
                "agent_transcript_path": "/home/user/.claude/projects/-work-project/1111/subagents/agent-a1.jsonl",
                "last_assistant_message": "done",
            })
        )),
        vec![K::SubagentStopped { id: CHILD.into() }]
    );
    assert!(kinds(&payload("SubagentStart", json!({}))).is_empty());
    assert!(kinds(&payload("SubagentStop", json!({"agent_type": ""}))).is_empty());
}

#[test]
fn a_stop_ends_the_turn_and_lists_the_children_still_running() {
    assert_eq!(
        kinds(&payload(
            "Stop",
            json!({"stop_hook_active": false, "last_assistant_message": "done"})
        )),
        vec![K::TurnEnded { interrupted: false }]
    );
    // A response a stop hook forced is finished all the same.
    assert_eq!(
        kinds(&payload("Stop", json!({"stop_hook_active": true}))),
        vec![K::TurnEnded { interrupted: false }]
    );
    assert_eq!(
        kinds(&payload(
            "Stop",
            json!({
                "stop_hook_active": false,
                "background_tasks": [
                    {"id": "agent-b1", "type": "subagent", "status": "running", "agent_type": "Explore", "description": "d"},
                    {"id": "agent-b2", "type": "subagent", "status": "completed"},
                    {"id": "agent-b3", "type": "subagent"},
                    {"id": "agent-b4", "type": "subagent", "status": "queued"},
                    {"id": "bash-1", "type": "shell", "status": "running", "command": "sleep 100"},
                    {"id": "two words", "type": "subagent", "status": "running"},
                    {"type": "subagent", "status": "running"},
                    "not a task",
                ],
                "session_crons": [{"id": "c1", "schedule": "*/5 * * * *", "recurring": true, "prompt": "p"}],
            })
        )),
        vec![
            K::TurnEnded { interrupted: false },
            K::BackgroundWork {
                subagents: vec!["agent-b1".into(), "agent-b3".into(), "agent-b4".into()]
            }
        ]
    );
    // An empty list says no child runs.
    assert_eq!(
        kinds(&payload("Stop", json!({"background_tasks": []}))),
        vec![
            K::TurnEnded { interrupted: false },
            K::BackgroundWork { subagents: vec![] }
        ]
    );
    assert!(kinds(&payload("Stop", json!({"agent_id": CHILD}))).is_empty());
}

#[test]
fn a_stop_failure_is_classed_by_its_error_type() {
    for (error, class) in [
        ("rate_limit", FailureClass::RateLimited),
        ("authentication_failed", FailureClass::Auth),
        ("oauth_org_not_allowed", FailureClass::Auth),
        ("cloud_credential_error", FailureClass::Auth),
        ("overloaded", FailureClass::Overloaded),
        ("billing_error", FailureClass::Failed),
        ("account_on_hold", FailureClass::Failed),
        ("invalid_request", FailureClass::Failed),
        ("model_not_found", FailureClass::Failed),
        ("server_error", FailureClass::Failed),
        ("max_output_tokens", FailureClass::Failed),
        ("unknown", FailureClass::Failed),
        ("a_type_from_a_newer_version", FailureClass::Failed),
    ] {
        assert_eq!(
            kinds(&payload(
                "StopFailure",
                json!({"error": error, "error_details": "429", "last_assistant_message": "x"})
            )),
            vec![K::TurnFailed { class }],
            "{error}"
        );
    }
    assert_eq!(
        kinds(&payload("StopFailure", json!({}))),
        vec![K::TurnFailed {
            class: FailureClass::Failed
        }]
    );
    assert!(
        kinds(&payload(
            "StopFailure",
            json!({"error": "rate_limit", "agent_id": CHILD})
        ))
        .is_empty()
    );
}

#[test]
fn compaction_starts_and_ends_and_a_childs_compaction_is_its_own() {
    for trigger in ["auto", "manual"] {
        assert_eq!(
            kinds(&payload(
                "PreCompact",
                json!({"trigger": trigger, "custom_instructions": "keep it short"})
            )),
            vec![K::CompactionStarted]
        );
        assert_eq!(
            kinds(&payload(
                "PostCompact",
                json!({"trigger": trigger, "compact_summary": "summary"})
            )),
            vec![K::CompactionEnded]
        );
    }
    for event in ["PreCompact", "PostCompact"] {
        assert!(
            kinds(&payload(
                event,
                json!({"trigger": "auto", "agent_id": CHILD})
            ))
            .is_empty(),
            "{event}"
        );
    }
}

#[test]
fn every_session_end_reason_ends_the_session() {
    for reason in ["clear", "resume", "logout", "prompt_input_exit", "other"] {
        assert_eq!(
            kinds(&payload("SessionEnd", json!({"reason": reason}))),
            vec![K::SessionEnded],
            "{reason}"
        );
    }
}

#[test]
fn a_childs_events_keep_the_paths_the_main_agent_recorded() {
    let events = live_events(
        &payload(
            "PostToolUse",
            json!({"agent_id": CHILD, "cwd": "/work/project/.claude/worktrees/child"}),
        ),
        &writer_context(),
        1,
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].context, writer_context());
}

#[test]
fn a_field_of_another_type_reads_as_absent_without_costing_the_event() {
    let events = live_events(
        &json!({
            "hook_event_name": "SessionStart",
            "session_id": SESSION,
            "source": 7,
            "cwd": ["/work"],
            "transcript_path": null,
            "agent_id": false,
        }),
        &writer_context(),
        1,
    );
    assert_eq!(
        events,
        vec![
            LiveEvent::new(
                HARNESS_KEY,
                SESSION,
                1,
                K::SessionStarted {
                    after_compaction: false
                }
            )
            .with_context(writer_context())
        ]
    );
    assert_eq!(
        kinds(&payload(
            "PostToolUseFailure",
            json!({"is_interrupt": "yes"})
        )),
        vec![K::ToolFinished { agent: None }]
    );
}

#[test]
fn events_the_mapping_does_not_cover_and_unreadable_payloads_say_nothing() {
    for payload in [
        payload("MessageDisplay", json!({"delta": "text"})),
        payload("PostToolBatch", json!({})),
        payload("TeammateIdle", json!({})),
        payload("CwdChanged", json!({"old_cwd": "/a", "new_cwd": "/b"})),
        payload("PermissionDenied", json!({"tool_name": "Bash"})),
        json!({"hook_event_name": "Stop"}),
        json!({"hook_event_name": "SessionStart"}),
        json!({"hook_event_name": "SessionStart", "session_id": ""}),
        json!({"hook_event_name": "SessionStart", "session_id": 7}),
        json!({"hook_event_name": 7, "session_id": SESSION}),
        json!({"session_id": SESSION}),
        json!(["SessionStart"]),
        json!("SessionStart"),
    ] {
        assert!(
            live_events(&payload, &LiveContext::default(), 1).is_empty(),
            "{payload}"
        );
    }
}

#[test]
fn only_a_session_start_is_recorded_for_attribution() {
    let adapter = super::super::ClaudeCodeAdapter::new();
    let event = |name: &str| json!({"hook_event_name": name, "session_id": "s"});
    assert!(adapter.is_attribution_event(&event("SessionStart")));
    for name in [
        "UserPromptSubmit",
        "PermissionRequest",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "Notification",
        "SubagentStart",
        "SubagentStop",
        "Stop",
        "StopFailure",
        "PreCompact",
        "PostCompact",
        "SessionEnd",
    ] {
        assert!(!adapter.is_attribution_event(&event(name)), "{name}");
    }
    assert!(!adapter.is_attribution_event(&json!({"hook_event_name": 3})));
    // A caller older than per-event hooks sends no event name.
    assert!(adapter.is_attribution_event(&json!({"session_id": "s"})));
}

#[test]
fn the_adapter_hands_its_payloads_to_this_mapping() {
    let adapter = super::super::ClaudeCodeAdapter::new();
    let payload = payload("Stop", json!({}));
    assert_eq!(
        adapter.live_events_from_payload(&payload, &writer_context(), 5),
        live_events(&payload, &writer_context(), 5)
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
            "tool_name": "Read",
            "tool_input": {"file_path": "/work/project/big.log"},
            "tool_response": {"file": {"content": large}},
        }),
    );
    let started = Instant::now();
    for _ in 0..1_000 {
        assert_eq!(kinds(&payload), vec![K::ToolFinished { agent: None }]);
    }
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}
