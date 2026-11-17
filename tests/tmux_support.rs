//! The test harnesses' shared tmux helper (`tests/support/tmux.rs`): what
//! a failed call says, and the bounded retry of a server's start. The
//! calls here run a stand-in program that fails as a loaded machine makes
//! tmux fail, and counts how often it was run.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "support/program.rs"]
mod program;
#[path = "support/tmux.rs"]
mod tmux;

/// A stand-in `tmux` in `dir`: it appends a line to `calls` each time it
/// runs, fails with `stderr` for its first `failures` runs, and succeeds
/// after that.
fn stand_in(dir: &Path, failures: u32, stderr: &str) -> (PathBuf, PathBuf) {
    let calls = dir.join("calls");
    let script = program::write_program(
        dir.join("tmux"),
        &format!(
            "#!/bin/sh\necho run >> '{calls}'\nif [ \"$(wc -l < '{calls}')\" -le {failures} ]; then\n  echo '{stderr}' >&2\n  exit 1\nfi\n",
            calls = calls.display()
        ),
    );
    (script, calls)
}

fn runs(calls: &Path) -> usize {
    std::fs::read_to_string(calls).map_or(0, |text| text.lines().count())
}

fn call(script: &Path, tmpdir: &Path, args: &[&str]) -> std::process::Output {
    tmux::run(tmpdir, args, || {
        let mut command = Command::new(script);
        command.args(args).env("TMUX_TMPDIR", tmpdir);
        command
    })
}

#[test]
fn a_start_that_fails_the_way_a_loaded_machine_makes_it_fail_runs_again_and_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (script, calls) = stand_in(dir.path(), 2, "server exited unexpectedly");
    let output = call(&script, dir.path(), &["new-session", "-d", "-s", "host"]);
    assert!(output.status.success());
    assert_eq!(runs(&calls), 3, "two failed starts, then the third");
}

#[test]
fn a_start_that_keeps_failing_stops_after_the_bounded_attempts_and_says_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (script, calls) = stand_in(dir.path(), 99, "server exited unexpectedly");
    let args = ["new-session", "-d", "-s", "host"];
    let output = call(&script, dir.path(), &args);
    assert!(!output.status.success());
    assert_eq!(runs(&calls), tmux::ATTEMPTS as usize);
    let report = tmux::report(dir.path(), &args, &output);
    for needle in [
        "server exited unexpectedly",
        "new-session",
        &dir.path().display().to_string(),
    ] {
        assert!(report.contains(needle), "{needle} is in: {report}");
    }
}

#[test]
fn a_failure_that_is_not_a_failed_start_runs_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (script, calls) = stand_in(dir.path(), 99, "can't find session: host");
    let output = call(&script, dir.path(), &["new-session", "-d", "-s", "host"]);
    assert!(!output.status.success());
    assert_eq!(runs(&calls), 1, "a refusal is not a start that went wrong");
}

#[test]
fn a_call_that_starts_no_server_runs_once_whatever_it_says() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (script, calls) = stand_in(dir.path(), 99, "server exited unexpectedly");
    let output = call(&script, dir.path(), &["capture-pane", "-p", "-t", "host"]);
    assert!(!output.status.success());
    assert_eq!(runs(&calls), 1);
}

#[test]
fn the_report_lists_the_sockets_that_exist() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sockets = dir.path().join("tmux-1000");
    std::fs::create_dir_all(&sockets).expect("socket dir");
    std::fs::write(sockets.join("default"), "").expect("socket stand-in");
    let (script, _) = stand_in(dir.path(), 1, "no server running on x");
    let args = ["kill-server"];
    let output = call(&script, dir.path(), &args);
    let report = tmux::report(dir.path(), &args, &output);
    assert!(report.contains("tmux-1000/default"), "{report}");
    assert!(report.contains("no server running"), "{report}");
}
