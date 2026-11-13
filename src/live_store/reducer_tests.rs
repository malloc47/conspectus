//! Sequences from the research tables, folded through the shared
//! reducer. Each step names the harness-neutral event an adapter maps
//! the payload to; the comments say which payload that is in the
//! harnesses surveyed.

use super::*;
use crate::live_store::{FailureClass, LiveContext, LiveEntry, LiveEvent, WaitKind};
use crate::model::live::{LiveDetail as D, LiveState as S};

use LiveEventKind as K;

const CHILD: &str = "agent-1";
const OTHER_CHILD: &str = "agent-2";

fn event(at_ms: i64, kind: K) -> LiveEvent {
    LiveEvent::new("harness-a", "session-1", at_ms, kind)
}

/// Fold `steps` from no entry and return the entry after each step.
fn run(steps: Vec<(i64, K)>) -> Vec<LiveEntry> {
    let mut entry: Option<LiveEntry> = None;
    let mut seen = Vec::new();
    for (at_ms, kind) in steps {
        let next = fold(entry.as_ref(), &event(at_ms, kind));
        seen.push(next.clone());
        entry = Some(next);
    }
    seen
}

/// What a reader would show: state, detail, start.
fn shown(entry: &LiveEntry) -> (S, Option<D>, i64) {
    let observation = &entry.observation;
    (observation.state, observation.detail, observation.since_ms)
}

fn start() -> K {
    K::SessionStarted {
        after_compaction: false,
    }
}

fn wait(agent: Option<String>, kind: WaitKind) -> K {
    K::WaitStarted { agent, kind }
}

fn tool(agent: Option<String>) -> K {
    K::ToolFinished { agent }
}

fn ended() -> K {
    K::TurnEnded { interrupted: false }
}

#[test]
fn a_simple_turn_goes_ready_working_ready() {
    let seen = run(vec![
        (100, start()),
        (200, K::TurnStarted),
        (300, tool(None)),
        (400, ended()),
    ]);
    assert_eq!(shown(&seen[0]), (S::Ready, None, 100));
    assert_eq!(shown(&seen[1]), (S::Working, Some(D::Generating), 200));
    assert_eq!(shown(&seen[2]), (S::Working, Some(D::Generating), 200));
    assert_eq!(shown(&seen[3]), (S::Ready, Some(D::Finished), 400));
    assert_eq!(seen[3].observation.observed_ms, 400);
    assert_eq!(
        seen[3].observation.source,
        crate::model::live::LiveSource::Hook
    );
    assert!(seen[3].reducer.waits.is_empty());
}

#[test]
fn a_permission_wait_lasts_until_the_approved_tool_finishes() {
    // The harnesses send nothing when the operator approves; the tool's
    // end is the next signal.
    let seen = run(vec![
        (100, start()),
        (200, K::TurnStarted),
        (300, wait(None, WaitKind::Permission)),
        // The harness confirms the same prompt later; the wait keeps its
        // start.
        (350, wait(None, WaitKind::Permission)),
        (900, tool(None)),
        (1000, ended()),
    ]);
    assert_eq!(shown(&seen[2]), (S::Waiting, Some(D::Permission), 300));
    assert_eq!(shown(&seen[3]), (S::Waiting, Some(D::Permission), 300));
    assert_eq!(seen[3].observation.observed_ms, 350);
    // The turn shows again with the start it had.
    assert_eq!(shown(&seen[4]), (S::Working, Some(D::Generating), 200));
    assert_eq!(shown(&seen[5]), (S::Ready, Some(D::Finished), 1000));
}

#[test]
fn a_denied_permission_the_harness_reports_ends_the_wait() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Permission)),
        (300, K::WaitEnded { agent: None }),
    ]);
    assert_eq!(shown(&seen[2]), (S::Working, Some(D::Generating), 100));
}

#[test]
fn a_child_agents_wait_is_raised_to_the_session() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (300, wait(Some(CHILD.into()), WaitKind::Permission)),
        // The main agent's own tools do not end the child's wait.
        (400, tool(None)),
        (500, tool(Some(CHILD.into()))),
        (600, K::SubagentStopped { id: CHILD.into() }),
    ]);
    assert_eq!(seen[1].observation.subagents, 1);
    assert_eq!(shown(&seen[2]), (S::Waiting, Some(D::Permission), 300));
    assert!(seen[2].observation.via_subagent);
    assert_eq!(shown(&seen[3]), (S::Waiting, Some(D::Permission), 300));
    assert_eq!(shown(&seen[4]), (S::Working, Some(D::Generating), 100));
    assert!(!seen[4].observation.via_subagent);
    assert_eq!(seen[5].observation.subagents, 0);
}

#[test]
fn the_main_agents_own_wait_shows_before_a_childs() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, wait(Some(CHILD.into()), WaitKind::Permission)),
        (300, wait(None, WaitKind::Question)),
        (400, tool(None)),
    ]);
    assert_eq!(shown(&seen[2]), (S::Waiting, Some(D::Question), 300));
    assert!(!seen[2].observation.via_subagent);
    // The child still waits once the main agent's question is answered.
    assert_eq!(shown(&seen[3]), (S::Waiting, Some(D::Permission), 200));
    assert!(seen[3].observation.via_subagent);
}

#[test]
fn a_confirmed_wait_is_the_pending_one_whoever_it_belongs_to() {
    // A harness confirms an open dialog seconds later without naming
    // the agent; here the dialog is a child's, so the confirmation must
    // not leave a wait of the main agent behind once the child's tool
    // finishes.
    let confirm = |kind| K::WaitConfirmed { kind };
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (300, wait(Some(CHILD.into()), WaitKind::Permission)),
        (6_300, confirm(WaitKind::Permission)),
        (7_000, tool(Some(CHILD.into()))),
    ]);
    assert_eq!(shown(&seen[3]), (S::Waiting, Some(D::Permission), 300));
    assert!(seen[3].observation.via_subagent);
    assert_eq!(seen[3].observation.observed_ms, 6_300);
    assert_eq!(shown(&seen[4]), (S::Working, Some(D::Generating), 100));

    // The main agent's pending wait keeps its kind and its start.
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Question)),
        (6_200, confirm(WaitKind::Permission)),
    ]);
    assert_eq!(shown(&seen[2]), (S::Waiting, Some(D::Question), 200));

    // With nothing pending, the dialog is the main agent's: a prompt the
    // harness announces no other way.
    let seen = run(vec![
        (100, K::TurnStarted),
        (6_100, confirm(WaitKind::Permission)),
        (9_000, tool(None)),
    ]);
    assert_eq!(shown(&seen[1]), (S::Waiting, Some(D::Permission), 6_100));
    assert!(!seen[1].observation.via_subagent);
    assert_eq!(shown(&seen[2]), (S::Working, Some(D::Generating), 100));
}

#[test]
fn an_interrupt_without_a_stop_is_repaired_by_an_idle_prompt_or_the_next_turn() {
    // An interrupt fires nothing in one harness: the turn reads working
    // until the idle prompt is confirmed or the next prompt arrives.
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Permission)),
        (5_000, K::IdleConfirmed),
        (6_000, K::TurnStarted),
    ]);
    assert_eq!(shown(&seen[2]), (S::Ready, None, 5_000));
    assert_eq!(shown(&seen[3]), (S::Working, Some(D::Generating), 6_000));

    // Another harness reports the interrupt, or a tool fails because of
    // one.
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::TurnEnded { interrupted: true }),
    ]);
    assert_eq!(shown(&seen[1]), (S::Ready, Some(D::Interrupted), 200));
}

#[test]
fn an_idle_prompt_leaves_a_finished_or_failed_turn_as_it_was() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, ended()),
        (60_200, K::IdleConfirmed),
    ]);
    assert_eq!(shown(&seen[2]), (S::Ready, Some(D::Finished), 200));
    let seen = run(vec![
        (100, K::TurnStarted),
        (
            200,
            K::TurnFailed {
                class: FailureClass::Auth,
            },
        ),
        (60_200, K::IdleConfirmed),
    ]);
    assert_eq!(shown(&seen[2]), (S::Error, Some(D::Auth), 200));
}

#[test]
fn a_question_waits_until_its_tool_returns() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Question)),
        (300, tool(None)),
        (400, ended()),
    ]);
    assert_eq!(shown(&seen[1]), (S::Waiting, Some(D::Question), 200));
    assert_eq!(shown(&seen[2]), (S::Working, Some(D::Generating), 100));
    assert_eq!(shown(&seen[3]), (S::Ready, Some(D::Finished), 400));
}

#[test]
fn a_plan_waits_until_it_is_approved_or_rejected() {
    let approved = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Plan)),
        (300, tool(None)),
    ]);
    assert_eq!(shown(&approved[1]), (S::Waiting, Some(D::Plan), 200));
    assert_eq!(shown(&approved[2]), (S::Working, Some(D::Generating), 100));

    // A rejected plan fails its tool, and the turn then stops.
    let rejected = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Plan)),
        (300, tool(None)),
        (310, ended()),
    ]);
    assert_eq!(shown(&rejected[3]), (S::Ready, Some(D::Finished), 310));
}

#[test]
fn parallel_child_agents_count_once_each_and_unknown_stops_are_ignored() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (
            210,
            K::SubagentStarted {
                id: OTHER_CHILD.into(),
            },
        ),
        // A resumed child starts again under the same id.
        (220, K::SubagentStarted { id: CHILD.into() }),
        (300, K::SubagentStopped { id: CHILD.into() }),
        // Internal agents stop without ever starting.
        (
            310,
            K::SubagentStopped {
                id: "internal".into(),
            },
        ),
        (
            320,
            K::SubagentStopped {
                id: OTHER_CHILD.into(),
            },
        ),
        (400, ended()),
    ]);
    let counts: Vec<u16> = seen.iter().map(|e| e.observation.subagents).collect();
    assert_eq!(counts, vec![0, 1, 2, 2, 1, 1, 0, 0]);
    assert!(
        seen.iter()
            .take(7)
            .all(|e| e.observation.state == S::Working)
    );
    assert_eq!(shown(&seen[7]), (S::Ready, Some(D::Finished), 400));
}

#[test]
fn a_background_child_outlives_the_turn_and_its_report_starts_a_new_one() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        // The launching tool returns at once.
        (210, tool(None)),
        (300, ended()),
        // The stop lists the children still running.
        (
            300,
            K::BackgroundWork {
                subagents: vec![CHILD.into()],
            },
        ),
        // The child keeps working while the main agent is ready.
        (400, tool(Some(CHILD.into()))),
        (500, K::SubagentStopped { id: CHILD.into() }),
        // Its report arrives as a turn of its own.
        (510, K::TurnStarted),
    ]);
    assert_eq!(shown(&seen[4]), (S::Ready, Some(D::Finished), 300));
    assert_eq!(seen[4].observation.subagents, 1);
    assert_eq!(shown(&seen[5]), (S::Ready, Some(D::Finished), 300));
    assert_eq!(seen[6].observation.subagents, 0);
    assert_eq!(shown(&seen[6]), (S::Ready, Some(D::Finished), 300));
    assert_eq!(shown(&seen[7]), (S::Working, Some(D::Generating), 510));
}

#[test]
fn the_background_list_replaces_the_child_set_and_drops_their_stale_waits() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (
            210,
            K::SubagentStarted {
                id: OTHER_CHILD.into(),
            },
        ),
        (220, wait(Some(OTHER_CHILD.into()), WaitKind::Permission)),
        (300, ended()),
        // A stop was dropped for one child; the list says only the other
        // still runs.
        (
            300,
            K::BackgroundWork {
                subagents: vec![CHILD.into()],
            },
        ),
    ]);
    assert_eq!(shown(&seen[4]), (S::Waiting, Some(D::Permission), 220));
    assert!(seen[4].observation.via_subagent);
    assert_eq!(shown(&seen[5]), (S::Ready, Some(D::Finished), 300));
    assert_eq!(seen[5].observation.subagents, 1);
}

#[test]
fn compaction_mid_turn_resumes_the_turn_whichever_end_arrives_first() {
    let compact_then_start = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (300, K::CompactionStarted),
        (400, K::CompactionEnded),
        (
            410,
            K::SessionStarted {
                after_compaction: true,
            },
        ),
    ]);
    assert_eq!(shown(&compact_then_start[2]), (S::Compacting, None, 300));
    assert_eq!(compact_then_start[2].observation.subagents, 1);
    assert_eq!(
        shown(&compact_then_start[3]),
        (S::Working, Some(D::Generating), 100)
    );
    assert_eq!(
        shown(&compact_then_start[4]),
        (S::Working, Some(D::Generating), 100)
    );
    // The resume keeps the session's child agents.
    assert_eq!(compact_then_start[4].observation.subagents, 1);

    let start_then_compact = run(vec![
        (100, K::TurnStarted),
        (300, K::CompactionStarted),
        (
            400,
            K::SessionStarted {
                after_compaction: true,
            },
        ),
        (410, K::CompactionEnded),
    ]);
    assert_eq!(
        shown(&start_then_compact[2]),
        (S::Working, Some(D::Generating), 100)
    );
    assert_eq!(
        shown(&start_then_compact[3]),
        (S::Working, Some(D::Generating), 100)
    );
}

#[test]
fn compaction_while_idle_leaves_the_finished_turn_as_it_was() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, ended()),
        (5_000, K::CompactionStarted),
        (5_100, K::CompactionEnded),
    ]);
    assert_eq!(shown(&seen[2]), (S::Compacting, None, 5_000));
    // Not a new finished turn: it keeps its start.
    assert_eq!(shown(&seen[3]), (S::Ready, Some(D::Finished), 200));
    assert_eq!(seen[3].observation.observed_ms, 5_100);
}

#[test]
fn compaction_after_a_failure_ends_ready() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (
            200,
            K::TurnFailed {
                class: FailureClass::Overloaded,
            },
        ),
        (300, K::CompactionStarted),
        (400, K::CompactionEnded),
    ]);
    assert_eq!(shown(&seen[3]), (S::Ready, None, 400));
}

#[test]
fn a_new_session_start_resets_waits_and_child_agents() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, K::SubagentStarted { id: CHILD.into() }),
        (300, wait(Some(CHILD.into()), WaitKind::Permission)),
        (400, start()),
    ]);
    assert_eq!(shown(&seen[3]), (S::Ready, None, 400));
    assert_eq!(seen[3].observation.subagents, 0);
    assert!(!seen[3].observation.via_subagent);
    assert!(seen[3].reducer.waits.is_empty());
}

#[test]
fn a_rate_limit_fails_the_turn_and_an_automatic_resume_starts_working() {
    let seen = run(vec![
        (100, K::TurnStarted),
        (
            200,
            K::Retrying {
                rate_limited: false,
            },
        ),
        (300, K::Retrying { rate_limited: true }),
        (
            400,
            K::TurnFailed {
                class: FailureClass::RateLimited,
            },
        ),
        // The limit resets but the harness waits for the operator.
        (5_000, wait(None, WaitKind::Input)),
        (6_000, K::TurnStarted),
    ]);
    assert_eq!(shown(&seen[1]), (S::Working, Some(D::Retrying), 100));
    assert_eq!(shown(&seen[2]), (S::Working, Some(D::RateLimited), 100));
    assert_eq!(shown(&seen[3]), (S::Error, Some(D::RateLimited), 400));
    assert_eq!(shown(&seen[4]), (S::Waiting, Some(D::Input), 5_000));
    assert_eq!(shown(&seen[5]), (S::Working, Some(D::Generating), 6_000));
}

#[test]
fn every_failure_class_shows_as_its_error_detail() {
    for (class, detail) in [
        (FailureClass::RateLimited, D::RateLimited),
        (FailureClass::Auth, D::Auth),
        (FailureClass::Overloaded, D::Overloaded),
        (FailureClass::Failed, D::Failed),
    ] {
        let seen = run(vec![(100, K::TurnStarted), (200, K::TurnFailed { class })]);
        assert_eq!(shown(&seen[1]), (S::Error, Some(detail), 200));
    }
}

#[test]
fn a_tool_that_finishes_after_the_turn_closed_does_not_reopen_it() {
    // Two writers raced for the lock and the stop won.
    let seen = run(vec![
        (100, K::TurnStarted),
        (200, ended()),
        (201, tool(None)),
    ]);
    assert_eq!(shown(&seen[2]), (S::Ready, Some(D::Finished), 200));
}

#[test]
fn a_session_first_heard_of_mid_turn_reads_from_its_first_event() {
    let seen = run(vec![
        (100, wait(None, WaitKind::Permission)),
        (200, tool(None)),
    ]);
    assert_eq!(shown(&seen[0]), (S::Waiting, Some(D::Permission), 100));
    assert_eq!(shown(&seen[1]), (S::Working, Some(D::Generating), 200));
    let seen = run(vec![(100, K::SubagentStarted { id: CHILD.into() })]);
    assert_eq!(shown(&seen[0]), (S::Unknown, None, 100));
    assert_eq!(seen[0].observation.subagents, 1);
}

#[test]
fn a_session_end_clears_everything_and_late_events_from_its_process_are_ignored() {
    let in_process = |at_ms: i64, kind: K| {
        event(at_ms, kind).with_context(LiveContext {
            pid: Some(4242),
            pid_start: Some(7),
            ..LiveContext::default()
        })
    };
    let mut entry = None;
    for step in [
        in_process(100, K::TurnStarted),
        in_process(200, K::SubagentStarted { id: CHILD.into() }),
        in_process(300, wait(Some(CHILD.into()), WaitKind::Permission)),
        in_process(400, K::SessionEnded),
    ] {
        entry = Some(fold(entry.as_ref(), &step));
    }
    let exited = entry.unwrap();
    assert_eq!(shown(&exited), (S::Exited, Some(D::Ended), 400));
    assert_eq!(exited.observation.subagents, 0);
    assert!(!exited.observation.via_subagent);
    assert!(exited.reducer.waits.is_empty());
    assert_eq!(exited.observation.pid, Some(4242));

    // A writer that lost the race to the end changes nothing, with or
    // without the pid.
    for late in [
        in_process(450, tool(None)),
        event(450, K::TurnStarted),
        in_process(450, K::SubagentStopped { id: CHILD.into() }),
    ] {
        assert_eq!(fold(Some(&exited), &late), exited);
    }

    // A session start opens it again, and so does an event from another
    // process, or from a process that reused the pid.
    let resumed = fold(Some(&exited), &in_process(500, start()));
    assert_eq!(shown(&resumed), (S::Ready, None, 500));
    let other_process = event(500, K::TurnStarted).with_context(LiveContext {
        pid: Some(5151),
        ..LiveContext::default()
    });
    assert_eq!(
        shown(&fold(Some(&exited), &other_process)),
        (S::Working, Some(D::Generating), 500)
    );
    let reused_pid = event(500, K::TurnStarted).with_context(LiveContext {
        pid: Some(4242),
        pid_start: Some(8),
        ..LiveContext::default()
    });
    assert_eq!(
        fold(Some(&exited), &reused_pid).observation.state,
        S::Working
    );
}

/// One event of every kind, so a check can run over all of them.
fn every_kind() -> Vec<K> {
    vec![
        start(),
        K::SessionStarted {
            after_compaction: true,
        },
        K::TurnStarted,
        K::Retrying { rate_limited: true },
        wait(None, WaitKind::Plan),
        wait(Some(CHILD.into()), WaitKind::Question),
        K::WaitEnded { agent: None },
        K::WaitEnded {
            agent: Some(CHILD.into()),
        },
        K::WaitConfirmed {
            kind: WaitKind::Permission,
        },
        tool(None),
        tool(Some(CHILD.into())),
        K::SubagentStarted { id: CHILD.into() },
        K::SubagentStopped { id: CHILD.into() },
        K::BackgroundWork {
            subagents: vec![OTHER_CHILD.into()],
        },
        K::CompactionStarted,
        K::CompactionEnded,
        ended(),
        K::TurnEnded { interrupted: true },
        K::TurnFailed {
            class: FailureClass::Failed,
        },
        K::IdleConfirmed,
        K::SessionEnded,
    ]
}

#[test]
fn every_event_is_idempotent_so_a_repeat_changes_nothing() {
    // A dropped event is repaired by the next one, and a repeated one
    // must not move the state on.
    let prefixes = [
        vec![],
        vec![(100, K::TurnStarted)],
        vec![
            (100, K::TurnStarted),
            (110, K::SubagentStarted { id: CHILD.into() }),
            (120, wait(Some(CHILD.into()), WaitKind::Permission)),
        ],
        vec![(100, K::TurnStarted), (110, K::CompactionStarted)],
        vec![(100, K::TurnStarted), (110, ended())],
    ];
    for prefix in prefixes {
        let base = run(prefix.clone()).pop();
        for kind in every_kind() {
            let once = fold(base.as_ref(), &event(1_000, kind.clone()));
            let twice = fold(Some(&once), &event(1_000, kind.clone()));
            assert_eq!(once, twice, "{kind:?} after {prefix:?}");
        }
    }
}

#[test]
fn every_shown_detail_belongs_to_its_state() {
    for kind in every_kind() {
        for prefix in [vec![], vec![(100, K::TurnStarted)]] {
            let mut steps = prefix;
            steps.push((1_000, kind.clone()));
            let entry = run(steps).pop().unwrap();
            if let Some(detail) = entry.observation.detail {
                assert!(entry.observation.state.accepts(detail), "{kind:?}");
            }
        }
    }
}

#[test]
fn a_harness_reducer_overrides_one_case_and_shares_the_rest() {
    /// A harness whose idle prompt also means its turn was interrupted.
    struct InterruptOnIdle;
    impl Reducer for InterruptOnIdle {
        fn fold(&self, entry: Option<&LiveEntry>, event: &LiveEvent) -> LiveEntry {
            match event.kind {
                K::IdleConfirmed => {
                    let interrupted = LiveEvent {
                        kind: K::TurnEnded { interrupted: true },
                        ..event.clone()
                    };
                    fold(entry, &interrupted)
                }
                _ => fold(entry, event),
            }
        }
    }
    let working = InterruptOnIdle.fold(None, &event(100, K::TurnStarted));
    assert_eq!(
        working,
        SharedReducer.fold(None, &event(100, K::TurnStarted))
    );
    let idle = InterruptOnIdle.fold(Some(&working), &event(200, K::IdleConfirmed));
    assert_eq!(shown(&idle), (S::Ready, Some(D::Interrupted), 200));
}

#[test]
fn a_block_this_version_cannot_read_is_empty() {
    let entry = run(vec![
        (100, K::TurnStarted),
        (200, wait(None, WaitKind::Permission)),
    ])
    .pop()
    .unwrap();
    let mut value = serde_json::to_value(&entry).unwrap();
    value["reducer"] = serde_json::json!({ "waits": "a newer shape" });
    let read: LiveEntry = serde_json::from_value(value).unwrap();
    assert!(read.reducer.is_empty());
    assert_eq!(read.observation, entry.observation);
}
