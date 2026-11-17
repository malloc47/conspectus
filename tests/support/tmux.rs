//! Running tmux from a test: what a failed call says, and a bounded retry
//! of a server's start.
//!
//! Each end-to-end test runs its own tmux servers, each under a
//! `TMUX_TMPDIR` of its own, and a full run starts hundreds of them beside
//! the processes the tests spawn. A start now and then fails under that
//! load (`server exited unexpectedly`, a lost connection to a server that
//! was still forking), and the harnesses asserted only the exit status, so
//! a failure said nothing of why. [`run`] keeps tmux's stderr, the
//! arguments, and the sockets that existed in the failure's report
//! ([`report`]), and runs a call that starts a server again, up to
//! [`ATTEMPTS`] times, when it failed in one of those ways. Any other
//! failure, and any call that does not start a server, runs once.
//!
//! Included with `#[path = "support/tmux.rs"] mod tmux;`.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

/// How many times a call that starts a server runs, at most.
pub const ATTEMPTS: u32 = 3;

/// What tmux prints when a server's start went wrong rather than the
/// command: the server died or never answered.
const START_FAILURES: &[&str] = &[
    "server exited unexpectedly",
    "lost server",
    "failed to connect to server",
    "no server running",
    "create window failed",
    "fork failed",
];

/// Whether `args` can start a server: `new-session` and `start-server`.
fn starts_server(args: &[&str]) -> bool {
    args.iter()
        .any(|arg| matches!(*arg, "new-session" | "start-server" | "new"))
}

/// Whether `output` is a failed start of the kind a retry can cure.
fn is_start_failure(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    !output.status.success() && START_FAILURES.iter().any(|text| stderr.contains(text))
}

/// Run the tmux call `command` builds (which must carry `args`, and
/// `tmpdir` as its `TMUX_TMPDIR`), again when a call that starts a server
/// failed to start it. The last attempt's output is the result.
pub fn run(tmpdir: &Path, args: &[&str], command: impl Fn() -> Command) -> Output {
    let mut attempt = 1;
    loop {
        let output = command().output().expect("start tmux");
        if output.status.success()
            || attempt >= ATTEMPTS
            || !starts_server(args)
            || !is_start_failure(&output)
        {
            return output;
        }
        eprintln!(
            "tmux {args:?} failed to start its server (attempt {attempt} of {ATTEMPTS}): {}",
            report(tmpdir, args, &output)
        );
        std::thread::sleep(Duration::from_millis(100 * u64::from(attempt)));
        attempt += 1;
    }
}

/// What a failed tmux call says: the arguments, the status, tmux's
/// stderr and stdout, and the sockets under the call's `TMUX_TMPDIR`
/// (tmux keeps them in `tmux-<uid>/`), which tells a server that never
/// came up from one that went away.
pub fn report(tmpdir: &Path, args: &[&str], output: &Output) -> String {
    let sockets: Vec<String> = std::fs::read_dir(tmpdir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|dir| dir.file_name().to_string_lossy().starts_with("tmux-"))
        .flat_map(|dir| {
            std::fs::read_dir(dir.path())
                .into_iter()
                .flatten()
                .flatten()
                .map(|socket| socket.path().display().to_string())
        })
        .collect();
    format!(
        "tmux {args:?} ({}) with TMUX_TMPDIR={}: stderr {:?}, stdout {:?}, sockets {sockets:?}",
        output.status,
        tmpdir.display(),
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim(),
    )
}

/// Assert that `output` of the call with `args` succeeded, with
/// [`report`] as the failure.
pub fn assert_ok(tmpdir: &Path, args: &[&str], output: &Output) {
    assert!(output.status.success(), "{}", report(tmpdir, args, output));
}

/// Whether the call succeeded, printing [`report`] to the test's stderr
/// (which the runner shows for a failed test) when it did not.
pub fn succeeded(tmpdir: &Path, args: &[&str], output: &Output) -> bool {
    if !output.status.success() {
        eprintln!("{}", report(tmpdir, args, output));
    }
    output.status.success()
}
