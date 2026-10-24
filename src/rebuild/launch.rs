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

use crate::discovery::tmux::{
    MuxBackend, TmuxAttachOutcome, TmuxKillOutcome, TmuxNewSessionOutcome, TmuxPaneStatus,
    TmuxSendKeysOutcome, dead_pane_output,
};

/// How long a launch watches a resumed pane before treating the
/// harness as started. A harness rejecting a resume target takes a
/// moment to say so (about a second for Claude Code).
pub(crate) const RESUME_WATCH: Duration = Duration::from_millis(1500);

const WATCH_POLL: Duration = Duration::from_millis(50);

/// A pane whose process exited right after launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeadPane {
    /// Exit status when tmux kept the pane; `None` when the session
    /// was already gone, which is what a clean (zero) exit does.
    pub(crate) status: Option<i32>,
    /// The pane's last lines, ANSI stripped.
    pub(crate) output: String,
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
pub(crate) enum Spawned {
    /// The harness started (or its pane couldn't be observed).
    Started,
    /// The resume launch died; a fresh launch replaced it.
    FreshAfterFailedResume(DeadPane),
}

/// Create tmux session `name` running `resume_argv` when given, else
/// `fresh_argv`. A resume is watched for `resume_watch`; one that dies
/// is replaced by a fresh launch. A fresh launch returns as soon as
/// its session exists.
pub(crate) fn spawn_watched(
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
pub(crate) fn resume_fallback_note(resume_argv: &[OsString], dead: &DeadPane) -> String {
    format!(
        "resume `{}` {}; launched fresh instead",
        format_argv_for_send_keys(resume_argv),
        dead.summary()
    )
}

/// Create tmux session `name` running `argv`, without watching it.
pub(crate) fn spawn(
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
pub(crate) fn watch_pane(
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

pub(crate) fn discard_session(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
) -> Result<()> {
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

pub(crate) fn attach_and_report(
    runner: &dyn MuxBackend,
    socket: Option<&str>,
    name: &str,
) -> Result<()> {
    let backend = runner.backend_key();
    let outcome = runner
        .attach_session(socket, name)
        .map_err(|err| anyhow!("{backend} attach failed: {err}"))?;
    match outcome {
        TmuxAttachOutcome::Detached => Ok(()),
        TmuxAttachOutcome::NoTarget => {
            bail!(
                "{backend} session `{name}` is gone; the process in it may have exited right after launch"
            )
        }
        TmuxAttachOutcome::Unavailable(reason) => bail!(
            "{backend} is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxAttachOutcome::Failed { code, message } => {
            bail!("{backend} attach failed (exit {code:?}): {message}")
        }
        TmuxAttachOutcome::Unsupported => {
            bail!("the {backend} runner does not support attach; attach to `{name}` manually")
        }
    }
}

pub(crate) fn report_send_keys(
    outcome: TmuxSendKeysOutcome,
    backend: &str,
    name: &str,
) -> Result<()> {
    match outcome {
        TmuxSendKeysOutcome::Sent => Ok(()),
        TmuxSendKeysOutcome::NoTarget => {
            bail!("{backend} session `{name}` disappeared before send-keys reached it")
        }
        TmuxSendKeysOutcome::Unavailable(reason) => bail!(
            "{backend} is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxSendKeysOutcome::Failed { code, message } => {
            bail!("{backend} send-keys failed (exit {code:?}): {message}")
        }
        TmuxSendKeysOutcome::Unsupported => {
            bail!("the {backend} runner does not support send-keys")
        }
    }
}

pub(crate) fn report_new_session(outcome: TmuxNewSessionOutcome, name: &str) -> Result<()> {
    match outcome {
        TmuxNewSessionOutcome::Created => Ok(()),
        TmuxNewSessionOutcome::NameTaken => bail!(
            "a tmux session named `{name}` already exists; rename the existing tmux or pick a different `mux.name`"
        ),
        TmuxNewSessionOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxNewSessionOutcome::Failed { code, message } => {
            bail!("tmux new-session failed (exit {code:?}): {message}")
        }
        TmuxNewSessionOutcome::Unsupported => {
            bail!("this tmux runner does not support new-session")
        }
    }
}

/// Build the `tmux [-L <socket>] attach-session -t <name>` command
/// string used in `--no-attach` output. Single-arg quoting is
/// intentionally minimal — `mux.name` is operator-typed and the
/// schema rejects shell metacharacters by virtue of tmux's own
/// session-name rules (alphanumeric + a small set of punctuation).
pub(crate) fn format_attach_command(socket: Option<&str>, name: &str) -> String {
    match socket {
        None => format!("tmux attach-session -t {name}"),
        Some(socket) => format!("tmux -L {socket} attach-session -t {name}"),
    }
}

/// Render an argv slice as a tmux `send-keys` literal so the
/// harness command lands in the existing pane, quoted so a shell reads
/// back the same arguments.
pub(crate) fn format_argv_for_send_keys(argv: &[std::ffi::OsString]) -> String {
    crate::argv::shell_join(argv)
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
