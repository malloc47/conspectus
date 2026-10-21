//! Spawn a harness in a new tmux session and confirm a resume started
//! (ADR 0103, ADR 0133).
//!
//! `tmux new-session -d` succeeds as soon as the session exists, even
//! when the harness inside exits a moment later. Sessions Conspectus
//! creates keep a pane that exits non-zero (`remain-on-exit failed`),
//! so after spawning a resume, the launch path watches the pane
//! briefly. A dead pane's text, which is the harness's own error, goes
//! into the launch outcome, and a fresh launch replaces it. A fresh
//! launch isn't watched: the harness's first output would scroll past
//! before the operator attaches, and a fresh launch that dies stays
//! visible as a dead pane on attach.

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};

use crate::discovery::tmux::{MuxBackend, TmuxKillOutcome, TmuxPaneStatus, dead_pane_output};

use super::pin::{format_argv_for_send_keys, report_new_session};

/// How long a launch watches a resumed pane before treating the
/// harness as started. A harness rejecting a resume target takes a
/// moment to say so (about a second for Claude Code).
pub(super) const RESUME_WATCH: Duration = Duration::from_millis(1500);

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
/// `fresh_argv`. A resume is watched for `resume_watch`; one that dies
/// is replaced by a fresh launch. A fresh launch returns as soon as
/// its session exists.
pub(super) fn spawn_watched(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
    cwd: &Path,
    fresh_argv: &[OsString],
    resume_argv: Option<&[OsString]>,
    resume_watch: Duration,
) -> Result<Spawned> {
    if let Some(resume_argv) = resume_argv {
        spawn(runner, socket, name, cwd, resume_argv)?;
        let Some(dead) = watch_pane(runner, socket, name, resume_watch) else {
            return Ok(Spawned::Started);
        };
        discard_session(runner, socket, name)?;
        spawn(runner, socket, name, cwd, fresh_argv)?;
        return Ok(Spawned::FreshAfterFailedResume(dead));
    }
    spawn(runner, socket, name, cwd, fresh_argv)?;
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

/// Create tmux session `name` running `argv`, without watching it.
pub(super) fn spawn(
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
