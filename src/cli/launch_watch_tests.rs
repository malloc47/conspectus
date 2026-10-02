use super::*;
use crate::discovery::tmux::{DEAD_PANE_LINES, FakeTmux, TmuxCaptureOutcome};

const NO_WAIT: WatchWindows = WatchWindows {
    resume: Duration::ZERO,
    fresh: Duration::ZERO,
};

fn argv(tokens: &[&str]) -> Vec<OsString> {
    tokens.iter().map(OsString::from).collect()
}

fn dead(status: i32) -> TmuxPaneStatus {
    TmuxPaneStatus::Dead {
        status: Some(status),
    }
}

const CLAUDE_RESUME_FAILURE: &str = "\u{1b}[31mNo conversation found with session ID: abc\u{1b}[0m\n\
\n\
Resume this session with:\n\
claude --resume abc\n\
\n\
\n\
Pane is dead (status 1, Thu Oct  1 13:17:41 2026)\n";

#[test]
fn live_pane_is_started() {
    let runner = FakeTmux::with_sessions("").with_pane_statuses("w1", [TmuxPaneStatus::Alive]);

    let spawned = spawn_watched(
        &runner,
        None,
        "w1",
        Path::new("/repo"),
        &argv(&["claude"]),
        None,
        NO_WAIT,
    )
    .expect("started");

    assert_eq!(spawned, Spawned::Started);
    assert!(runner.kill_calls().is_empty());
}

#[test]
fn unobservable_pane_is_assumed_started() {
    let runner = FakeTmux::with_sessions("");

    let spawned = spawn_watched(
        &runner,
        None,
        "w1",
        Path::new("/repo"),
        &argv(&["claude"]),
        Some(&argv(&["claude", "--resume", "abc"])),
        NO_WAIT,
    )
    .expect("started");

    assert_eq!(spawned, Spawned::Started);
    assert_eq!(runner.new_session_calls().len(), 1);
}

#[test]
fn failed_resume_is_replaced_by_a_fresh_launch() {
    let runner = FakeTmux::with_sessions("")
        .with_capture(
            "w1",
            TmuxCaptureOutcome::captured(CLAUDE_RESUME_FAILURE.to_string()),
        )
        .with_pane_statuses("w1", [dead(1), TmuxPaneStatus::Alive]);
    let fresh = argv(&["claude", "--dangerously-skip-permissions"]);
    let resume = argv(&[
        "claude",
        "--dangerously-skip-permissions",
        "--resume",
        "abc",
    ]);

    let spawned = spawn_watched(
        &runner,
        None,
        "w1",
        Path::new("/repo"),
        &fresh,
        Some(&resume),
        NO_WAIT,
    )
    .expect("fresh launch succeeds");

    let Spawned::FreshAfterFailedResume(dead) = spawned else {
        panic!("expected fallback, got {spawned:?}");
    };
    assert_eq!(dead.status, Some(1));
    assert!(dead.output.starts_with("No conversation found"));
    assert!(!dead.output.contains("Pane is dead"));
    assert!(!dead.output.contains('\u{1b}'));
    let launched: Vec<Vec<OsString>> = runner
        .new_session_calls()
        .into_iter()
        .map(|(_, _, _, argv)| argv)
        .collect();
    assert_eq!(launched, [resume.clone(), fresh]);
    assert_eq!(runner.kill_calls(), [(None, "w1".to_string())]);
    let note = resume_fallback_note(&resume, &dead);
    assert!(
        note.contains("exited with status 1: No conversation found"),
        "{note}"
    );
}

#[test]
fn failed_fresh_launch_reports_pane_output_and_removes_the_dead_session() {
    let runner = FakeTmux::with_sessions("")
        .with_capture(
            "w1",
            TmuxCaptureOutcome::captured("bash: claude: command not found\n".to_string()),
        )
        .with_pane_statuses("w1", [dead(127)]);

    let err = spawn_watched(
        &runner,
        None,
        "w1",
        Path::new("/repo"),
        &argv(&["claude"]),
        None,
        NO_WAIT,
    )
    .expect_err("launch fails");

    let message = err.to_string();
    assert!(
        message.starts_with("`claude` exited with status 127: bash: claude: command not found"),
        "{message}"
    );
    assert!(message.contains("--- pane output ---"));
    assert_eq!(runner.kill_calls(), [(None, "w1".to_string())]);
}

#[test]
fn session_gone_right_after_launch_counts_as_dead() {
    let runner = FakeTmux::with_sessions("").with_pane_statuses("w1", [TmuxPaneStatus::NoTarget]);

    let err = spawn_watched(
        &runner,
        None,
        "w1",
        Path::new("/repo"),
        &argv(&["true"]),
        None,
        NO_WAIT,
    )
    .expect_err("launch fails");

    assert!(
        err.to_string()
            .starts_with("`true` exited without output right after launch"),
        "{err}"
    );
}

#[test]
fn dead_pane_output_keeps_the_tail() {
    let text: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    let runner = FakeTmux::with_sessions("").with_capture("w1", TmuxCaptureOutcome::captured(text));

    let output = dead_pane_output(&runner, None, "w1");

    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), DEAD_PANE_LINES);
    assert_eq!(lines.first(), Some(&"line 11"));
    assert_eq!(lines.last(), Some(&"line 30"));
}
