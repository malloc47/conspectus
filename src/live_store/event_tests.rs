use std::collections::BTreeSet;

use super::*;

const AGENT: Option<&str> = Some("agent-x");

fn agent(agent: Option<&str>) -> Option<String> {
    agent.map(str::to_string)
}

/// One event kind of each shape, with every optional id both present and
/// absent where the label tells them apart.
fn every_kind() -> Vec<LiveEventKind> {
    let mut kinds = vec![
        LiveEventKind::SessionStarted {
            after_compaction: false,
        },
        LiveEventKind::SessionStarted {
            after_compaction: true,
        },
        LiveEventKind::TurnStarted,
        LiveEventKind::Retrying {
            rate_limited: false,
        },
        LiveEventKind::Retrying { rate_limited: true },
        LiveEventKind::WaitEnded { agent: None },
        LiveEventKind::WaitEnded {
            agent: agent(AGENT),
        },
        LiveEventKind::ToolFinished { agent: None },
        LiveEventKind::ToolFinished {
            agent: agent(AGENT),
        },
        LiveEventKind::SubagentStarted {
            id: "child-x".into(),
        },
        LiveEventKind::SubagentStopped {
            id: "child-x".into(),
        },
        LiveEventKind::BackgroundWork {
            subagents: vec!["child-x".into(), "child-y".into()],
        },
        LiveEventKind::CompactionStarted,
        LiveEventKind::CompactionEnded,
        LiveEventKind::TurnEnded { interrupted: false },
        LiveEventKind::TurnEnded { interrupted: true },
        LiveEventKind::IdleConfirmed,
        LiveEventKind::SessionEnded,
    ];
    for kind in [
        WaitKind::Permission,
        WaitKind::Question,
        WaitKind::Plan,
        WaitKind::Input,
    ] {
        kinds.push(LiveEventKind::WaitStarted { agent: None, kind });
        kinds.push(LiveEventKind::WaitStarted {
            agent: agent(AGENT),
            kind,
        });
        kinds.push(LiveEventKind::WaitConfirmed { kind });
    }
    for class in [
        FailureClass::RateLimited,
        FailureClass::Auth,
        FailureClass::Overloaded,
        FailureClass::Failed,
    ] {
        kinds.push(LiveEventKind::TurnFailed { class });
    }
    kinds
}

#[test]
fn a_label_names_the_event_and_its_closed_qualifier_and_nothing_else() {
    let labels: Vec<String> = every_kind().iter().map(LiveEventKind::label).collect();
    for expected in [
        "session_started",
        "session_started:after_compaction",
        "turn_started",
        "retrying",
        "retrying:rate_limited",
        "wait_ended",
        "wait_ended@child",
        "tool_finished",
        "tool_finished@child",
        "subagent_started",
        "subagent_stopped",
        "background_work:2",
        "compaction_started",
        "compaction_ended",
        "turn_ended:finished",
        "turn_ended:interrupted",
        "idle_confirmed",
        "session_ended",
        "wait_started:permission",
        "wait_started:question@child",
        "wait_confirmed:plan",
        "wait_confirmed:input",
        "turn_failed:rate_limited",
        "turn_failed:auth",
        "turn_failed:overloaded",
        "turn_failed:failed",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "no label {expected}"
        );
    }
    // Every shape has its own label, and none carries an id.
    let distinct: BTreeSet<&String> = labels.iter().collect();
    assert_eq!(distinct.len(), labels.len(), "{labels:?}");
    for label in &labels {
        assert!(
            label.chars().all(|c| c.is_ascii_lowercase()
                || matches!(c, '_' | ':' | '@')
                || c.is_ascii_digit()),
            "{label}"
        );
        assert!(
            !label.contains("agent-x") && !label.contains("child-"),
            "{label}"
        );
    }
}
