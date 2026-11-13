//! The live-state repair poll (ADR 0218): a harness's own list of its
//! sessions, asked for only when the harness's hooks have left a state
//! that may be stuck.
//!
//! A [`LivePoller`] belongs to a process that discovers in the background
//! and keeps it for its lifetime (the daemon, a TUI with no daemon); a
//! one-shot command has none. On each discovery the live-state pass hands
//! it the hook bindings of its harness that could be stuck
//! ([`PollCandidate`]) and how many running agents of its harness no hook
//! entry reports, and the poller runs its program when both hold:
//!
//! 1. [`POLL_INTERVAL_MS`] have passed since its last attempt, whatever
//!    that attempt came to;
//! 2. a candidate is *due*: it has had no event for [`SETTLE_MS`], and no
//!    poll since has answered it ([`answered`]); or some running agent
//!    has no hook entry, which only a poll can show (ADR 0222).
//!
//! Either way it returns the last poll that succeeded, which the pass
//! puts on the snapshot for the binding to fold in. A poll that fails (no
//! program, a non-zero exit, a timeout, output too long or unreadable)
//! changes nothing and says nothing.

use std::fmt;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::live_store::{PidProbe, PidStatus};
use crate::model::live::{LivePoll, LiveState};

/// The least time between two attempts of one poller.
pub const POLL_INTERVAL_MS: i64 = 30_000;

/// How long a hook state must go without an event before a poll may
/// speak for it: a hook event less than this older than a poll wins.
pub const SETTLE_MS: i64 = 10_000;

/// How often a running poll checks whether its program has exited.
const WAIT_STEP: Duration = Duration::from_millis(10);

/// What running a poll's program came to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    /// It exited 0 and printed this.
    Output(Vec<u8>),
    /// The program is not installed.
    Unavailable,
    /// It could not be started, exited otherwise than 0, or printed more
    /// than it may.
    Failed,
    /// It ran past its timeout and was killed.
    TimedOut,
}

/// Runs a poll's program. Tests stand in their own.
pub trait PollRunner: Send + Sync {
    fn run(&self) -> RunOutcome;
}

/// Run `command` with stdin closed and stderr dropped unread, read at
/// most `cap` bytes of its stdout, and kill it (`SIGKILL`) once it has
/// run for `timeout`. Output past `cap` is drained and the run counts as
/// [`RunOutcome::Failed`], so a long listing cannot stall the program.
/// Nothing about the run is logged, and stderr is never read.
pub fn run_bounded(mut command: Command, timeout: Duration, cap: usize) -> RunOutcome {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return RunOutcome::Unavailable,
        Err(_) => return RunOutcome::Failed,
    };
    let deadline = Instant::now() + timeout;
    let (sent, received) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut stdout = stdout;
            let read = (&mut stdout)
                .take(u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1))
                .read_to_end(&mut bytes);
            let _ = std::io::copy(&mut stdout, &mut std::io::sink());
            let _ = sent.send(read.ok().map(|_| bytes));
        });
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return RunOutcome::TimedOut;
            }
            Ok(None) => std::thread::sleep(WAIT_STEP),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return RunOutcome::Failed;
            }
        }
    };
    // A process the program left behind can hold the pipe open; the
    // reader is then left to end on its own.
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .max(WAIT_STEP);
    let Ok(Some(bytes)) = received.recv_timeout(remaining) else {
        return RunOutcome::Failed;
    };
    if !status.success() || bytes.len() > cap {
        return RunOutcome::Failed;
    }
    RunOutcome::Output(bytes)
}

/// Reads a program's output, taken at `observed_ms`, as a poll: `None`
/// when it says nothing readable.
pub type PollParser = fn(&[u8], i64) -> Option<LivePoll>;

/// A hook binding of the poller's harness that a poll might repair, as
/// the live-state pass found it: its store entry's session id, the
/// process it recorded, and its state and latest event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollCandidate {
    pub session_id: String,
    pub pid: Option<i64>,
    pub pid_start: Option<u64>,
    /// `working` or `waiting`, as the store recorded it.
    pub state: LiveState,
    /// When its latest event arrived, Unix epoch milliseconds.
    pub observed_ms: i64,
}

/// Whether `poll` has already answered `candidate`: it ran at least
/// [`SETTLE_MS`] after the candidate's latest event, so the binding used
/// it, and it either did not list the agent or read it in a state that
/// only a hook event ends (`ready`, `error`, `exited`). A poll that read
/// `working` or `waiting` leaves the candidate due, since either can end
/// without an event.
pub fn answered(candidate: &PollCandidate, poll: &LivePoll) -> bool {
    if poll.observed_ms.saturating_sub(candidate.observed_ms) < SETTLE_MS {
        return false;
    }
    match poll.agent_for(
        candidate.pid,
        candidate.pid_start,
        Some(&candidate.session_id),
    ) {
        None => true,
        Some(agent) => matches!(
            agent.state,
            LiveState::Ready | LiveState::Error | LiveState::Exited
        ),
    }
}

/// Whether `candidate` is due at `now_ms` given the last poll that
/// succeeded: silent for [`SETTLE_MS`] and not [`answered`].
pub fn due(candidate: &PollCandidate, last: Option<&LivePoll>, now_ms: i64) -> bool {
    now_ms.saturating_sub(candidate.observed_ms) >= SETTLE_MS
        && !last.is_some_and(|poll| answered(candidate, poll))
}

/// One harness's repair poll, with what it remembers between discoveries:
/// when it last tried and the last poll that succeeded.
pub struct LivePoller {
    harness: &'static str,
    runner: Box<dyn PollRunner>,
    parse: PollParser,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    memory: Mutex<Memory>,
}

#[derive(Default)]
struct Memory {
    last_attempt_ms: Option<i64>,
    last: Option<LivePoll>,
}

impl fmt::Debug for LivePoller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LivePoller")
            .field("harness", &self.harness)
            .finish_non_exhaustive()
    }
}

impl LivePoller {
    /// A poller for `harness` that runs `runner` and reads its output
    /// with `parse`, on the system clock.
    pub fn new(harness: &'static str, runner: Box<dyn PollRunner>, parse: PollParser) -> Self {
        Self {
            harness,
            runner,
            parse,
            clock: Box::new(crate::live_store::now_ms),
            memory: Mutex::new(Memory::default()),
        }
    }

    /// The same poller on `clock`, Unix epoch milliseconds.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// The harness whose hook bindings this poller repairs.
    pub fn harness(&self) -> &'static str {
        self.harness
    }

    /// The poll the binding should fold in now: a new one when the
    /// limits of the module documentation let it run and it succeeds,
    /// else the last that succeeded. `unreported` counts the running
    /// agents of the harness that no hook entry reports. Each listed
    /// pid's start time is read through `probe`, and each agent's run is
    /// carried on from the last poll.
    pub fn poll(
        &self,
        candidates: &[PollCandidate],
        unreported: usize,
        probe: &dyn PidProbe,
    ) -> Option<LivePoll> {
        let now_ms = (self.clock)();
        {
            let mut memory = self.memory();
            let rested = memory.last_attempt_ms.is_none_or(|last| {
                now_ms < last || now_ms.saturating_sub(last) >= POLL_INTERVAL_MS
            });
            let wanted = unreported > 0
                || candidates
                    .iter()
                    .any(|candidate| due(candidate, memory.last.as_ref(), now_ms));
            if !(rested && wanted) {
                return memory.last.clone();
            }
            // Claimed before the program runs, so a discovery running
            // beside this one does not run it too.
            memory.last_attempt_ms = Some(now_ms);
        }
        let read = match self.runner.run() {
            RunOutcome::Output(bytes) => (self.parse)(&bytes, now_ms),
            RunOutcome::Unavailable | RunOutcome::Failed | RunOutcome::TimedOut => None,
        };
        let mut memory = self.memory();
        if let Some(mut poll) = read {
            for agent in &mut poll.agents {
                agent.pid_start = agent.pid.and_then(|pid| match probe.status(pid) {
                    PidStatus::Running { start } => start,
                    PidStatus::Gone | PidStatus::Unknown => None,
                });
            }
            let poll = poll.after(memory.last.as_ref());
            memory.last = Some(poll);
        }
        memory.last.clone()
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The repair polls of every registered harness that offers one, unless
/// `CONSPECTUS_DISABLE_LIVE_POLL` is set. A process that discovers in the
/// background builds these once and passes them to every discovery
/// (`LocalDiscoveryConfig::with_live_pollers`).
pub fn from_env() -> Vec<Arc<LivePoller>> {
    if std::env::var_os("CONSPECTUS_DISABLE_LIVE_POLL").is_some() {
        return Vec::new();
    }
    crate::discovery::harness::registered_adapters()
        .filter_map(crate::discovery::harness::HarnessAdapter::live_poller)
        .map(Arc::new)
        .collect()
}

#[cfg(test)]
#[path = "live_poller_tests.rs"]
mod tests;
