//! Spawn a harness in a new tmux session and confirm it started
//! (ADR 0103).
//!
//! `tmux new-session -d` succeeds as soon as the session exists, even
//! when the harness inside exits a moment later. Sessions Conspectus
//! creates keep a pane that exits non-zero (`remain-on-exit failed`),
//! so after spawning, the launch path watches the pane briefly. A dead
//! pane's text, which is the harness's own error, goes into the
//! launch outcome instead of disappearing with the session.

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};

use crate::discovery::tmux::{MuxBackend, TmuxKillOutcome, TmuxPaneStatus, dead_pane_output};

use super::pin::{format_argv_for_send_keys, report_new_session};

/// How long a launch watches a new pane before treating the harness
/// as started. A harness rejecting a resume target takes a moment to
/// say so (about a second for Claude Code), and a failed resume has a
/// fresh-launch fallback, so resumes get the longer window. A fresh
/// launch only needs to catch exec-level failures such as a missing
/// binary; later failures stay visible as a dead pane.
#[derive(Clone, Copy, Debug)]
pub(super) struct WatchWindows {
    pub(super) resume: Duration,
    pub(super) fresh: Duration,
}

impl Default for WatchWindows {
    fn default() -> Self {
        Self {
            resume: Duration::from_millis(1500),
            fresh: Duration::from_millis(500),
        }
    }
}

const WATCH_POLL: Duration = Duration::from_millis(50);

/// A pane whose process exited right after launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DeadPane {
    /// Exit status when tmux kept the pane; `None` when the session
    /// was already gone, which is what a clean (zero) exit does.
    pub(super) status: Option<i32>,
    /// The pane's last lines, ANSI stripped.
    pub(super) output: String,
}

impl DeadPane {
    fn summary(&self) -> String {
        let status = match self.status {
            Some(code) => format!("exited with status {code}"),
            None => "exited".to_string(),
        };
        match self.output.lines().find(|line| !line.trim().is_empty()) {
            Some(first) => format!("{status}: {}", first.trim()),
            None => format!("{status} without output"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Spawned {
    /// The harness started (or its pane couldn't be observed).
    Started,
    /// The resume launch died; a fresh launch replaced it.
    FreshAfterFailedResume(DeadPane),
}

/// Create tmux session `name` running `resume_argv` when given, else
/// `fresh_argv`, and confirm the pane survives its watch window. A
/// resume that dies is replaced by a fresh launch. A fresh launch that
/// dies is an error carrying the pane's output; its dead session is
/// removed so the next launch starts clean.
pub(super) fn spawn_watched(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
    cwd: &Path,
    fresh_argv: &[OsString],
    resume_argv: Option<&[OsString]>,
    windows: WatchWindows,
) -> Result<Spawned> {
    if let Some(resume_argv) = resume_argv {
        spawn(runner, socket, name, cwd, resume_argv)?;
        let Some(dead) = watch_pane(runner, socket, name, windows.resume) else {
            return Ok(Spawned::Started);
        };
        discard_session(runner, socket, name)?;
        spawn_fresh(runner, socket, name, cwd, fresh_argv, windows)?;
        return Ok(Spawned::FreshAfterFailedResume(dead));
    }
    spawn_fresh(runner, socket, name, cwd, fresh_argv, windows)?;
    Ok(Spawned::Started)
}

/// One-line note for a resume that died and was replaced.
pub(super) fn resume_fallback_note(resume_argv: &[OsString], dead: &DeadPane) -> String {
    format!(
        "resume `{}` {}; launched fresh instead",
        format_argv_for_send_keys(resume_argv),
        dead.summary()
    )
}

fn spawn_fresh(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
    cwd: &Path,
    argv: &[OsString],
    windows: WatchWindows,
) -> Result<()> {
    spawn(runner, socket, name, cwd, argv)?;
    let Some(dead) = watch_pane(runner, socket, name, windows.fresh) else {
        return Ok(());
    };
    discard_session(runner, socket, name)?;
    let mut message = format!(
        "`{}` {} right after launch",
        format_argv_for_send_keys(argv),
        dead.summary()
    );
    if !dead.output.trim().is_empty() {
        message.push_str("\n--- pane output ---\n");
        message.push_str(&dead.output);
    }
    bail!(message)
}

fn spawn(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
    cwd: &Path,
    argv: &[OsString],
) -> Result<()> {
    let outcome = runner
        .new_session(socket, name, cwd, argv)
        .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
    report_new_session(outcome, name)
}

/// Poll the pane until it dies or `window` passes. `None` means the
/// harness is running, or the runner can't report pane status (in
/// which case the launch is assumed to have started, as before).
pub(super) fn watch_pane(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
    window: Duration,
) -> Option<DeadPane> {
    let deadline = Instant::now() + window;
    loop {
        match runner.pane_status(socket, name) {
            Ok(TmuxPaneStatus::Dead { status }) => {
                return Some(DeadPane {
                    status,
                    output: dead_pane_output(runner, socket, name),
                });
            }
            Ok(TmuxPaneStatus::NoTarget) => {
                return Some(DeadPane {
                    status: None,
                    output: String::new(),
                });
            }
            Ok(TmuxPaneStatus::Alive) => {}
            _ => return None,
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(WATCH_POLL);
    }
}

fn discard_session(runner: &dyn MuxBackend, socket: Option<&str>, name: &str) -> Result<()> {
    match runner
        .kill_session(socket, name)
        .map_err(|err| anyhow!("tmux kill-session failed: {err}"))?
    {
        TmuxKillOutcome::Killed | TmuxKillOutcome::NoTarget | TmuxKillOutcome::Unsupported => {
            Ok(())
        }
        TmuxKillOutcome::Unavailable(reason) => {
            bail!("tmux is unavailable on this host: {}", reason.as_str())
        }
        TmuxKillOutcome::Failed { code, message } => {
            bail!("could not remove the dead tmux session `{name}` (exit {code:?}): {message}")
        }
    }
}

#[cfg(test)]
#[path = "launch_watch_tests.rs"]
mod tests;
