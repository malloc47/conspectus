//! The fixture corpus under `tests/fixtures/hooks/codex/`: each single
//! payload maps as its event documents, and each sequence, driven through
//! the whole writer path (payload, mapping, the store in a scratch
//! directory, the entry read back, the state a reader shows), shows the
//! expected state after every step. The fixtures are synthetic; their
//! README says how they are laid out.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::discovery::harness::CodexAdapter;
use crate::discovery::harness::live_sequences::{self, corpus_dir, read_json};

use LiveEventKind as K;

const SESSION: &str = "019a7c3e-5b2d-7f41-9c8e-2d4b6a1f0e37";
const CHILD: &str = "019a7c41-0c6e-7d2a-b5f3-8e9d0a1b2c3d";

fn fixture_dir() -> PathBuf {
    corpus_dir("codex")
}

fn fixture(name: &str) -> Value {
    read_json(&fixture_dir().join(format!("{name}.json")))
}

fn fixture_names(extension: &str) -> BTreeSet<String> {
    live_sequences::fixture_names(&fixture_dir(), extension)
}

fn kinds(payload: &Value) -> Vec<K> {
    live_events(payload, &LiveContext::default(), 1)
        .into_iter()
        .map(|event| event.kind)
        .collect()
}

/// What each single payload maps to.
fn expected_kinds() -> Vec<(&'static str, Vec<K>)> {
    let start = |after_compaction| K::SessionStarted { after_compaction };
    let permission = |agent: Option<&str>| K::WaitStarted {
        agent: agent.map(str::to_string),
        kind: WaitKind::Permission,
    };
    let tool = |agent: Option<&str>| K::ToolFinished {
        agent: agent.map(str::to_string),
    };
    vec![
        ("sessionstart-startup", vec![start(false)]),
        ("sessionstart-resume", vec![start(false)]),
        ("sessionstart-clear", vec![start(false)]),
        ("sessionstart-compact", vec![start(true)]),
        ("sessionstart-fork", vec![start(false)]),
        ("sessionstart-ephemeral", vec![start(false)]),
        ("userpromptsubmit", vec![K::TurnStarted]),
        ("permissionrequest-bash", vec![permission(None)]),
        ("permissionrequest-apply-patch", vec![permission(None)]),
        ("permissionrequest-mcp", vec![permission(None)]),
        ("permissionrequest-subagent", vec![permission(Some(CHILD))]),
        ("posttooluse-bash", vec![tool(None)]),
        ("posttooluse-subagent-bash", vec![tool(Some(CHILD))]),
        ("pretooluse-request-user-input", vec![]),
        ("stop", vec![K::TurnEnded { interrupted: false }]),
        ("stop-plan-mode", vec![K::TurnEnded { interrupted: false }]),
        ("interrupt", vec![K::TurnEnded { interrupted: true }]),
        ("sessionend", vec![K::SessionEnded]),
        (
            "subagentstart",
            vec![K::SubagentStarted { id: CHILD.into() }],
        ),
        (
            "subagentstop",
            vec![K::SubagentStopped { id: CHILD.into() }],
        ),
        ("precompact-auto", vec![K::CompactionStarted]),
        ("precompact-manual", vec![K::CompactionStarted]),
        ("postcompact-auto", vec![K::CompactionEnded]),
        ("postcompact-manual", vec![K::CompactionEnded]),
        ("premodelrequest", vec![]),
        ("postmodelrequest-failed", vec![]),
        ("unknown-event", vec![]),
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
        for event in live_events(&payload, &LiveContext::default(), 1) {
            assert_eq!(event.harness, HARNESS_KEY, "{name}");
            assert_eq!(event.session_id, SESSION, "{name}");
        }
    }
}

#[test]
fn every_subscribed_event_has_a_payload_fixture_and_every_session_start_source_too() {
    let names: BTreeSet<String> = fixture_names("json")
        .iter()
        .filter_map(|name| {
            fixture(name)["hook_event_name"]
                .as_str()
                .map(str::to_string)
        })
        .collect();
    for event in HOOK_EVENTS {
        assert!(names.contains(event), "{event}");
    }
    let sources: BTreeSet<String> = fixture_names("json")
        .iter()
        .filter_map(|name| fixture(name)["source"].as_str().map(str::to_string))
        .collect();
    let documented: BTreeSet<String> = ["startup", "resume", "clear", "compact", "fork"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(sources, documented);
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
    assert_eq!(context("sessionstart-ephemeral").transcript_path, None);
}

#[test]
fn every_payload_bearing_field_of_the_fixtures_holds_a_sentinel() {
    let payload_fields = [
        "prompt",
        "tool_input",
        "tool_response",
        "last_assistant_message",
        "error",
        "summary",
    ];
    for name in fixture_names("json") {
        let payload = fixture(&name);
        for field in payload_fields {
            if let Some(value) = payload.get(field) {
                assert!(
                    value.to_string().contains("SENTINEL-codex-"),
                    "{name}: {field}"
                );
            }
        }
    }
}

/// The sequences the research's fixture plan and the story list.
const SCENARIOS: [&str; 11] = [
    "seq-simple-turn",
    "seq-permission-approve",
    "seq-permission-decline",
    "seq-subagent-waiting",
    "seq-subagent-overlay",
    "seq-auto-compact-midturn",
    "seq-manual-compact-idle",
    "seq-plan-mode-stop",
    "seq-crash-no-sessionend",
    "seq-clear",
    "seq-session-end",
];

#[test]
fn every_scenario_has_a_sequence_and_every_fixture_says_it_is_synthetic() {
    let expected: BTreeSet<String> = SCENARIOS.iter().map(ToString::to_string).collect();
    assert_eq!(fixture_names("jsonl"), expected);
    for name in SCENARIOS {
        let (header, _) = live_sequences::sequence(&fixture_dir(), name);
        assert_eq!(header["synthetic"], true, "{name}");
    }
    let readme = fs::read_to_string(fixture_dir().join("README.md")).expect("README");
    assert!(readme.contains("**synthetic**"));
}

fn run_sequence(name: &str) {
    live_sequences::run_sequence(&CodexAdapter::new(), &fixture_dir(), name);
}

#[test]
fn a_simple_turn() {
    run_sequence("seq-simple-turn");
}

#[test]
fn an_approval_granted_then_the_tool_finishing() {
    run_sequence("seq-permission-approve");
}

#[test]
fn approvals_declined_then_an_interrupt_or_a_stop() {
    run_sequence("seq-permission-decline");
}

#[test]
fn a_subagent_waiting_for_approval() {
    run_sequence("seq-subagent-waiting");
}

#[test]
fn subagents_on_a_working_and_a_ready_session() {
    run_sequence("seq-subagent-overlay");
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
fn a_stop_in_plan_mode() {
    run_sequence("seq-plan-mode-stop");
}

#[test]
fn a_crash_without_a_session_end() {
    run_sequence("seq-crash-no-sessionend");
}

#[test]
fn a_clear() {
    run_sequence("seq-clear");
}

#[test]
fn a_session_end() {
    run_sequence("seq-session-end");
}
