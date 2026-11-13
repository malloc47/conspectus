//! The fixture corpus under `tests/fixtures/hooks/claude-code/`: each
//! single payload maps as its event documents, and each sequence, driven
//! through the whole writer path (payload, mapping, the store in a
//! scratch directory, the entry read back, the state a reader shows),
//! shows the expected state after every step. The fixtures are synthetic;
//! their README says how they are laid out.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::*;
use crate::discovery::harness::ClaudeCodeAdapter;
use crate::discovery::harness::live_sequences::{self, corpus_dir, read_json};

use LiveEventKind as K;

const CHILD: &str = "a3f9c27e1b4d5068";
const CHILD_BG: &str = "c5d2a8f1e0b7c364";
const INTERNAL: &str = "d0e1f2a3b4c5d6e7";

fn fixture_dir() -> PathBuf {
    corpus_dir("claude-code")
}

fn fixture(name: &str) -> Value {
    read_json(&fixture_dir().join(format!("{name}.json")))
}

/// The fixture files with `extension`, by stem.
fn fixture_names(extension: &str) -> BTreeSet<String> {
    live_sequences::fixture_names(&fixture_dir(), extension)
}

fn kinds(payload: &Value) -> Vec<K> {
    live_events(payload, &LiveContext::default(), 1)
        .into_iter()
        .map(|event| event.kind)
        .collect()
}

fn wait(agent: Option<&str>, kind: WaitKind) -> K {
    K::WaitStarted {
        agent: agent.map(str::to_string),
        kind,
    }
}

fn tool(agent: Option<&str>) -> K {
    K::ToolFinished {
        agent: agent.map(str::to_string),
    }
}

/// What each single payload maps to.
fn expected_kinds() -> Vec<(&'static str, Vec<K>)> {
    let start = |after_compaction| K::SessionStarted { after_compaction };
    let confirmed = |kind| K::WaitConfirmed { kind };
    let finished = K::TurnEnded { interrupted: false };
    let failed = |class| K::TurnFailed { class };
    vec![
        ("sessionstart-startup", vec![start(false)]),
        ("sessionstart-resume", vec![start(false)]),
        ("sessionstart-clear", vec![start(false)]),
        ("sessionstart-compact", vec![start(true)]),
        ("sessionstart-fork", vec![start(false)]),
        ("sessionstart-agent", vec![start(false)]),
        ("sessionstart-no-transcript", vec![start(false)]),
        ("userpromptsubmit", vec![K::TurnStarted]),
        (
            "pretooluse-askuserquestion",
            vec![wait(None, WaitKind::Question)],
        ),
        ("pretooluse-exitplanmode", vec![wait(None, WaitKind::Plan)]),
        ("pretooluse-bash", vec![]),
        ("pretooluse-subagent-bash", vec![]),
        ("pretooluse-mcp", vec![]),
        (
            "permissionrequest-bash",
            vec![wait(None, WaitKind::Permission)],
        ),
        (
            "permissionrequest-subagent",
            vec![wait(Some(CHILD), WaitKind::Permission)],
        ),
        ("posttooluse-bash", vec![tool(None)]),
        ("posttooluse-subagent-bash", vec![tool(Some(CHILD))]),
        (
            "posttooluse-agent-completed",
            vec![tool(None), K::SubagentStopped { id: CHILD.into() }],
        ),
        (
            "posttooluse-agent-async-launched",
            vec![
                tool(None),
                K::SubagentStarted {
                    id: CHILD_BG.into(),
                },
            ],
        ),
        ("posttoolusefailure", vec![tool(None)]),
        (
            "posttoolusefailure-interrupt",
            vec![tool(None), K::TurnEnded { interrupted: true }],
        ),
        (
            "notification-permission-prompt",
            vec![confirmed(WaitKind::Permission)],
        ),
        ("notification-idle-prompt", vec![K::IdleConfirmed]),
        (
            "notification-elicitation-dialog",
            vec![confirmed(WaitKind::Input)],
        ),
        (
            "notification-elicitation-url-dialog",
            vec![confirmed(WaitKind::Input)],
        ),
        (
            "notification-agent-needs-input",
            vec![confirmed(WaitKind::Input)],
        ),
        ("notification-quota-auto-resume-fired", vec![K::TurnStarted]),
        (
            "notification-quota-auto-resume-stale",
            vec![wait(None, WaitKind::Input)],
        ),
        (
            "notification-quota-auto-resume-disabled",
            vec![K::IdleConfirmed],
        ),
        (
            "subagentstart",
            vec![K::SubagentStarted { id: CHILD.into() }],
        ),
        (
            "subagentstop",
            vec![K::SubagentStopped { id: CHILD.into() }],
        ),
        (
            "subagentstop-internal-empty-type",
            vec![K::SubagentStopped {
                id: INTERNAL.into(),
            }],
        ),
        (
            "stop",
            vec![finished.clone(), K::BackgroundWork { subagents: vec![] }],
        ),
        (
            "stop-background-subagent",
            vec![
                finished.clone(),
                K::BackgroundWork {
                    subagents: vec![CHILD_BG.into()],
                },
            ],
        ),
        (
            "stop-hook-active",
            vec![finished.clone(), K::BackgroundWork { subagents: vec![] }],
        ),
        (
            "stop-session-crons",
            vec![finished, K::BackgroundWork { subagents: vec![] }],
        ),
        (
            "stopfailure-rate-limit",
            vec![failed(FailureClass::RateLimited)],
        ),
        (
            "stopfailure-overloaded",
            vec![failed(FailureClass::Overloaded)],
        ),
        (
            "stopfailure-authentication-failed",
            vec![failed(FailureClass::Auth)],
        ),
        ("precompact-auto", vec![K::CompactionStarted]),
        ("precompact-manual", vec![K::CompactionStarted]),
        ("postcompact-auto", vec![K::CompactionEnded]),
        ("postcompact-manual", vec![K::CompactionEnded]),
        ("sessionend-prompt-input-exit", vec![K::SessionEnded]),
        ("sessionend-clear", vec![K::SessionEnded]),
        ("sessionend-resume", vec![K::SessionEnded]),
        ("sessionend-other", vec![K::SessionEnded]),
        ("sessionend-logout", vec![K::SessionEnded]),
        ("payload-large-tool-response", vec![tool(None)]),
    ]
}

#[test]
fn every_payload_fixture_maps_as_its_event_documents() {
    let expected = expected_kinds();
    let listed: BTreeSet<String> = expected.iter().map(|(name, _)| name.to_string()).collect();
    assert_eq!(
        listed,
        fixture_names("json"),
        "every payload fixture has an expectation here"
    );
    for (name, kinds_expected) in expected {
        let payload = fixture(name);
        assert_eq!(kinds(&payload), kinds_expected, "{name}");
        let events = live_events(&payload, &LiveContext::default(), 1);
        for event in &events {
            assert_eq!(event.harness, HARNESS_KEY, "{name}");
            assert_eq!(
                event.session_id, "3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f",
                "{name}"
            );
        }
    }
}

#[test]
fn every_subscribed_event_has_a_payload_fixture() {
    let events: BTreeSet<String> = fixture_names("json")
        .iter()
        .filter_map(|name| {
            fixture(name)["hook_event_name"]
                .as_str()
                .map(str::to_string)
        })
        .collect();
    for subscription in HOOK_SUBSCRIPTIONS {
        assert!(
            events.contains(subscription.event),
            "{}",
            subscription.event
        );
    }
    let notifications: BTreeSet<String> = fixture_names("json")
        .iter()
        .filter_map(|name| {
            fixture(name)["notification_type"]
                .as_str()
                .map(str::to_string)
        })
        .collect();
    let matcher = HOOK_SUBSCRIPTIONS[6].matcher.expect("notification matcher");
    for notification in matcher.split('|') {
        assert!(notifications.contains(notification), "{notification}");
    }
}

#[test]
fn a_main_agent_fixture_records_its_paths_and_a_childs_does_not() {
    let context = |name: &str| {
        live_events(&fixture(name), &LiveContext::default(), 1)
            .remove(0)
            .context
    };
    let main = context("posttooluse-bash");
    assert_eq!(main.cwd.as_deref(), Some("/home/user/projects/my-project"));
    assert!(main.transcript_path.is_some());
    assert_eq!(context("posttooluse-subagent-bash"), LiveContext::default());
    assert_eq!(context("sessionstart-no-transcript").transcript_path, None);
}

#[test]
fn the_large_tool_response_fixture_costs_nothing_to_map() {
    let mut payload = fixture("payload-large-tool-response");
    payload["tool_response"]["file"]["content"] =
        Value::String("line of a large log file\n".repeat(1 << 20));
    let started = Instant::now();
    for _ in 0..1_000 {
        assert_eq!(kinds(&payload), vec![tool(None)]);
    }
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

/// The sequences the research's fixture plan lists, plus none it does not.
const SCENARIOS: [&str; 17] = [
    "seq-simple-turn",
    "seq-permission-approve",
    "seq-permission-approve-long-tool",
    "seq-permission-deny-esc",
    "seq-esc-interrupt",
    "seq-askuserquestion",
    "seq-plan-approve",
    "seq-plan-reject",
    "seq-parallel-foreground-subagents",
    "seq-background-subagent-outlives-stop",
    "seq-auto-compact-midturn",
    "seq-manual-compact-idle",
    "seq-clear",
    "seq-resume-in-process",
    "seq-rate-limit-auto-resume",
    "seq-kill9-no-sessionend",
    "seq-old-version-subagentstop-spam",
];

#[test]
fn every_scenario_has_a_sequence_and_every_fixture_says_it_is_synthetic() {
    let expected: BTreeSet<String> = SCENARIOS.iter().map(ToString::to_string).collect();
    assert_eq!(fixture_names("jsonl"), expected);
    for name in SCENARIOS {
        let (header, _) = sequence(name);
        assert_eq!(header["synthetic"], true, "{name}");
    }
    let readme = fs::read_to_string(fixture_dir().join("README.md")).expect("README");
    assert!(readme.contains("**synthetic**"));
}

/// A sequence's header and steps.
fn sequence(name: &str) -> (Value, Vec<Value>) {
    live_sequences::sequence(&fixture_dir(), name)
}

/// Drive `name` through the writer path and check every step.
fn run_sequence(name: &str) {
    live_sequences::run_sequence(&ClaudeCodeAdapter::new(), &fixture_dir(), name);
}

#[test]
fn a_simple_turn() {
    run_sequence("seq-simple-turn");
}

#[test]
fn a_permission_approved() {
    run_sequence("seq-permission-approve");
}

#[test]
fn a_permission_approved_for_a_long_tool() {
    run_sequence("seq-permission-approve-long-tool");
}

#[test]
fn a_permission_denied_with_esc() {
    run_sequence("seq-permission-deny-esc");
}

#[test]
fn an_esc_interrupt() {
    run_sequence("seq-esc-interrupt");
}

#[test]
fn a_question_answered() {
    run_sequence("seq-askuserquestion");
}

#[test]
fn a_plan_approved() {
    run_sequence("seq-plan-approve");
}

#[test]
fn a_plan_rejected() {
    run_sequence("seq-plan-reject");
}

#[test]
fn parallel_foreground_subagents() {
    run_sequence("seq-parallel-foreground-subagents");
}

#[test]
fn a_background_subagent_outliving_the_stop() {
    run_sequence("seq-background-subagent-outlives-stop");
}

#[test]
fn an_automatic_compaction_mid_turn() {
    run_sequence("seq-auto-compact-midturn");
}

#[test]
fn a_manual_compaction_while_idle() {
    run_sequence("seq-manual-compact-idle");
}

#[test]
fn a_clear() {
    run_sequence("seq-clear");
}

#[test]
fn a_resume_in_the_same_process() {
    run_sequence("seq-resume-in-process");
}

#[test]
fn a_rate_limit_and_its_automatic_resume() {
    run_sequence("seq-rate-limit-auto-resume");
}

#[test]
fn a_kill_without_a_session_end() {
    run_sequence("seq-kill9-no-sessionend");
}

#[test]
fn an_old_versions_subagent_stop_spam() {
    run_sequence("seq-old-version-subagentstop-spam");
}
