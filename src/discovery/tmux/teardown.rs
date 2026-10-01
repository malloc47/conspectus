//! Operator-initiated mux teardown mechanism (ADR 0093).
//!
//! The [`MuxBackend::kill_session`] primitive is the *hard* phase of
//! teardown. This module owns the *graceful-first* wrapper the ADR
//! mandates: send `SIGTERM` to the session's foreground process (the
//! pane PID discovery already observed), give it a short, configurable
//! grace window to flush and exit, then reclaim the session with the
//! hard `kill-session`. The escalation is guaranteed — teardown never
//! hangs on a wedged agent.
//!
//! Signalling and liveness are behind [`ProcessSignaller`] so the whole
//! two-phase flow is testable without spawning a real process: the
//! production [`SystemSignaller`] uses `libc::kill`, and tests inject a
//! fake that records the `SIGTERM` and drives a canned liveness
//! sequence.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::{MuxBackend, TmuxKillOutcome};

/// Sends `SIGTERM` to a pid and probes whether it is still alive.
/// Abstracted so the grace-poll is deterministic under test.
pub trait ProcessSignaller {
    /// Send `SIGTERM` to `pid`. Best-effort: a missing process (the
    /// agent already exited) is success, not an error.
    fn terminate(&self, pid: i64) -> Result<()>;
    /// Whether `pid` is still a live process.
    fn is_alive(&self, pid: i64) -> bool;
    /// Sleep between liveness polls. Kept on the trait so tests
    /// advance instantly instead of waiting real wall-clock time.
    fn sleep(&self, dur: Duration);
}

/// `pid` as a single-process `kill(2)` target. `None` for values that
/// aren't one: 0 and negative pids address process groups (`-1` is
/// every process the user owns), and values outside `pid_t` would wrap
/// into some other pid.
fn signal_target(pid: i64) -> Option<libc::pid_t> {
    libc::pid_t::try_from(pid).ok().filter(|pid| *pid > 0)
}

/// Production signaller backed by `libc::kill`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemSignaller;

impl ProcessSignaller for SystemSignaller {
    fn terminate(&self, pid: i64) -> Result<()> {
        let Some(pid) = signal_target(pid) else {
            return Ok(());
        };
        // SAFETY: `kill(2)` with a plain pid + signal has no memory
        // safety concerns. ESRCH (no such process) means the agent
        // already exited — the goal state — so it isn't an error.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        Ok(())
    }

    fn is_alive(&self, pid: i64) -> bool {
        // `kill(pid, 0)` performs the permission/existence check
        // without delivering a signal: 0 => alive, ESRCH => gone.
        let Some(pid) = signal_target(pid) else {
            return false;
        };
        let rc = unsafe { libc::kill(pid, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    fn sleep(&self, dur: Duration) {
        std::thread::sleep(dur);
    }
}

/// Outcome of a two-phase mux teardown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeardownReport {
    /// Whether the pane process exited on its own during the grace
    /// window (a clean graceful shutdown). `false` when the pane PID
    /// was unknown, the grace window was `0s`, or the process was
    /// still alive when the window elapsed.
    pub graceful_exited: bool,
    /// Whether a `SIGTERM` was actually delivered (pane PID known and
    /// grace window non-zero).
    pub signalled: bool,
    /// Result of the hard `kill-session` reclaim. `NoTarget` counts
    /// as success — the goal state is "session not running".
    pub kill: TmuxKillOutcome,
}

/// Interval between liveness polls during the grace window.
const POLL_STEP: Duration = Duration::from_millis(100);

/// Two-phase teardown of one mux session (ADR 0093):
///
/// 1. **Graceful.** When `pane_pid` is known and `grace > 0`, send
///    `SIGTERM` to the pane's foreground process and poll for it to
///    exit for up to `grace`.
/// 2. **Hard.** Reclaim the session with `kill_session` regardless —
///    even after a graceful agent exit the (now idle) session must be
///    ended to free the working tree. `NoTarget` is treated as
///    success by the caller.
///
/// The hard phase always runs, so a wedged agent can never leave a
/// dangling session. When the backend can't terminate sessions it
/// returns [`TmuxKillOutcome::Unsupported`] and the caller degrades to
/// "close the session yourself".
pub fn teardown_mux_session(
    mux: &dyn MuxBackend,
    signaller: &dyn ProcessSignaller,
    socket_name: Option<&str>,
    target: &str,
    pane_pid: Option<i64>,
    grace: Duration,
) -> Result<TeardownReport> {
    let mut graceful_exited = false;
    let mut signalled = false;

    if let Some(pid) = pane_pid
        && !grace.is_zero()
    {
        signaller.terminate(pid)?;
        signalled = true;
        let deadline = Instant::now() + grace;
        loop {
            if !signaller.is_alive(pid) {
                graceful_exited = true;
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
            // Don't overshoot the deadline on the final nap.
            let remaining = deadline.saturating_duration_since(Instant::now());
            signaller.sleep(POLL_STEP.min(remaining));
        }
    }

    let kill = mux.kill_session(socket_name, target)?;
    Ok(TeardownReport {
        graceful_exited,
        signalled,
        kill,
    })
}

#[cfg(test)]
#[path = "teardown_tests.rs"]
mod tests;
