//! The background refresh after a tmux hand-off, and the existence check
//! that releases a frozen mux before it lands (ADR 0201).
//!
//! A detach nearly always leaves the mux running, so the refresh's worker
//! first asks the mux's server, once, whether the session is still the
//! incarnation the screen showed (its session id and creation time). When
//! it is, the mux's rows are released at once and the refresh updates
//! everything else behind them. When it isn't, or the answer is unclear,
//! one re-check follows a moment later; a mux neither answer confirms
//! stays frozen until the refresh lands, and the refresh probes again
//! while its listing still shows that mux (a session that just ended, or a
//! server that was just killed, can still be listed for a moment).
//!
//! Discovery runs on the worker thread while the check runs beside it, so
//! the check never delays the refresh.

use std::collections::BTreeSet;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::Result;

use super::{DaemonNudge, Discover, DiscoveryResult, live_mux_ids, worker_forge_fetch};
use crate::model::{GraphSnapshot, MuxSessionId, NodeId};
use crate::tui::RunConfig;
use crate::tui::app::{HandoffRequest, MuxIncarnationTarget, ReturnCheck, ReturnedMux};

/// Asks a mux's server whether a session is still the incarnation the
/// screen showed before a hand-off. The live loop asks tmux
/// ([`TmuxIncarnation`]); tests give scripted answers.
pub(super) trait IncarnationProbe: Send + Sync {
    /// Whether the server answers for `target` with the same session id
    /// and creation time. Any other answer, or none, is `false`.
    fn still_there(&self, target: &MuxIncarnationTarget) -> bool;
}

/// What tmux prints for the session a target names.
const INCARNATION_FORMAT: &str = "#{session_id} #{session_created}";

/// The existence check against tmux: one `display-message` on the mux's
/// server, read-only.
#[derive(Debug, Default)]
pub(super) struct TmuxIncarnation {
    /// A test's private server: its socket directory, with `TMUX` unset so
    /// nothing reaches the server the tests run under.
    #[cfg(test)]
    pub(super) socket_dir: Option<std::path::PathBuf>,
}

impl IncarnationProbe for TmuxIncarnation {
    fn still_there(&self, target: &MuxIncarnationTarget) -> bool {
        if target.backend != crate::discovery::tmux::TMUX_BACKEND {
            return false;
        }
        let mut command = crate::exec::command(crate::exec::Program::Tmux, "tmux");
        #[cfg(test)]
        if let Some(dir) = &self.socket_dir {
            command.env_remove("TMUX").env("TMUX_TMPDIR", dir);
        }
        if let Some(socket) = target
            .socket
            .as_deref()
            .filter(|socket| *socket != "default")
        {
            command.args(["-L", socket]);
        }
        let output = command
            .args([
                "display-message",
                "-p",
                "-t",
                &target.session_id,
                INCARNATION_FORMAT,
            ])
            .stdin(std::process::Stdio::null())
            .output();
        output.is_ok_and(|output| {
            output.status.success()
                && answer_names(&String::from_utf8_lossy(&output.stdout), target)
        })
    }
}

/// Whether tmux's answer to [`INCARNATION_FORMAT`] names `target`. The
/// exit status alone proves nothing: tmux answers a target it can't find
/// with empty fields, and a session reusing the name is another session
/// id, so both fields must match.
fn answer_names(stdout: &str, target: &MuxIncarnationTarget) -> bool {
    let mut fields = stdout.split_whitespace();
    let (Some(session_id), Some(created), None) = (fields.next(), fields.next(), fields.next())
    else {
        return false;
    };
    session_id == target.session_id && created.parse() == Ok(target.created_epoch)
}

/// How long the check waits before asking again about a mux the first
/// answer didn't confirm.
pub(super) const RECHECK_DELAY: Duration = Duration::from_millis(150);

/// Whether `target` is still the same incarnation: one answer, and when
/// that one doesn't confirm it, one more after [`RECHECK_DELAY`].
pub(super) fn confirm(
    probe: &dyn IncarnationProbe,
    target: &MuxIncarnationTarget,
    sleep: impl FnOnce(Duration),
) -> bool {
    if probe.still_there(target) {
        return true;
    }
    sleep(RECHECK_DELAY);
    probe.still_there(target)
}

/// Check each of `checks` in turn, reporting each confirmed mux as soon
/// as it is. Returns the muxes left unconfirmed.
fn check_all(
    probe: &dyn IncarnationProbe,
    checks: Vec<(MuxSessionId, MuxIncarnationTarget)>,
    mut sleep: impl FnMut(Duration),
    mut confirmed: impl FnMut(MuxSessionId),
) -> BTreeSet<MuxSessionId> {
    let mut unconfirmed = BTreeSet::new();
    for (mux, target) in checks {
        if confirm(probe, &target, &mut sleep) {
            confirmed(mux);
        } else {
            unconfirmed.insert(mux);
        }
    }
    unconfirmed
}

/// The existence check confirmed `mux` for the refresh with generation
/// `generation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Confirmed {
    pub(super) mux: MuxSessionId,
    pub(super) generation: u64,
}

/// One refresh after a hand-off, ready to spawn.
pub(super) struct HandoffJob {
    pub(super) generation: u64,
    pub(super) config: RunConfig,
    pub(super) nudge: DaemonNudge,
    /// The muxes returned from, in order.
    pub(super) returned: Vec<ReturnedMux>,
    /// When the TUI's pane came back into view: the running muxes the
    /// screen showed.
    pub(super) unchanged_from: Option<BTreeSet<NodeId>>,
}

impl HandoffJob {
    /// The job for `request`, with generation `generation`, while the
    /// screen shows `snapshot`.
    pub(super) fn new(
        generation: u64,
        mut config: RunConfig,
        nudge: DaemonNudge,
        request: HandoffRequest,
        snapshot: Option<&GraphSnapshot>,
    ) -> Self {
        config.forge_fetch = worker_forge_fetch(nudge, config.forge_fetch);
        Self {
            generation,
            config,
            nudge,
            returned: request.returned,
            unchanged_from: snapshot.filter(|_| request.regained).map(live_mux_ids),
        }
    }
}

/// Where the worker's answers go: the refresh's result, and each mux the
/// check confirms before it.
pub(super) struct HandoffChannels {
    pub(super) results: mpsc::Sender<DiscoveryResult>,
    pub(super) confirmed: mpsc::Sender<Confirmed>,
}

/// Spawn the refresh for `job`: its existence checks on one thread, its
/// discovery (run by `discover`) on another, re-probed while a mux the
/// check left unconfirmed is still listed.
pub(super) fn spawn_refresh(
    job: HandoffJob,
    probe: Arc<dyn IncarnationProbe>,
    discover: Arc<Discover>,
    channels: HandoffChannels,
) {
    std::thread::spawn(move || {
        let HandoffJob {
            generation,
            mut config,
            nudge,
            returned,
            unchanged_from,
        } = job;
        let mut suspects = BTreeSet::new();
        let mut checks = Vec::new();
        for returned in returned {
            match returned.check {
                ReturnCheck::Incarnation(target) => checks.push((returned.mux, target)),
                ReturnCheck::Listed => {
                    suspects.insert(NodeId::MuxSession(returned.mux));
                }
                ReturnCheck::New => {}
            }
        }
        let checked: Vec<MuxSessionId> = checks.iter().map(|(mux, _)| mux.clone()).collect();
        let confirmed = channels.confirmed;
        let checking = std::thread::spawn(move || {
            check_all(probe.as_ref(), checks, std::thread::sleep, |mux| {
                // The receiver is gone only when the TUI is exiting.
                let _ = confirmed.send(Confirmed { mux, generation });
            })
        });
        let mut warning = None;
        let (snapshot, from_daemon) = {
            let mut run = || {
                let found = discover(&mut config, nudge);
                if found.warning.is_some() {
                    warning = found.warning;
                }
                (found.snapshot, found.from_daemon)
            };
            let first = run();
            // A check that panicked confirmed nothing it hadn't reported.
            let unconfirmed = checking
                .join()
                .unwrap_or_else(|_| checked.into_iter().collect());
            suspects.extend(unconfirmed.into_iter().map(NodeId::MuxSession));
            let settle = Settle {
                suspects,
                unchanged_from,
            };
            probe_until_settled(
                first,
                &settle,
                SETTLE_TRIES,
                SETTLE_DELAY,
                std::thread::sleep,
                run,
            )
        };
        let _ = channels.results.send(DiscoveryResult {
            generation,
            snapshot,
            warning,
            from_daemon,
        });
    });
}

/// What a refresh after a hand-off waits out before it reports.
#[derive(Debug, Default)]
pub(super) struct Settle {
    /// Muxes the screen showed running that the check didn't confirm (or
    /// couldn't ask about): probe again while the listing still shows one.
    pub(super) suspects: BTreeSet<NodeId>,
    /// The TUI's pane came back into view with these muxes on screen:
    /// probe again while the listing shows exactly these, since nothing
    /// was frozen to tell what the operator ended elsewhere.
    pub(super) unchanged_from: Option<BTreeSet<NodeId>>,
}

impl Settle {
    /// Whether `snapshot` may still predate what the hand-off did.
    fn open(&self, snapshot: &GraphSnapshot) -> bool {
        if self.suspects.is_empty() && self.unchanged_from.is_none() {
            return false;
        }
        let live = live_mux_ids(snapshot);
        self.suspects.iter().any(|mux| live.contains(mux))
            || self.unchanged_from.as_ref() == Some(&live)
    }
}

/// Take `first`, the result of a probe, and while its snapshot is still
/// open by `settle`, wait `delay` and probe again, at most `tries` more
/// times. Returns the last probe's result; a failed probe is returned at
/// once.
pub(super) fn probe_until_settled<T>(
    first: (Result<GraphSnapshot>, T),
    settle: &Settle,
    tries: u32,
    delay: Duration,
    mut sleep: impl FnMut(Duration),
    mut probe: impl FnMut() -> (Result<GraphSnapshot>, T),
) -> (Result<GraphSnapshot>, T) {
    let mut result = first;
    for _ in 0..tries {
        let open = matches!(&result.0, Ok(snapshot) if settle.open(snapshot));
        if !open {
            break;
        }
        sleep(delay);
        result = probe();
    }
    result
}

/// How many times a refresh after a mux hand-off re-probes while it may
/// still predate the hand-off, and how long it waits between tries. A
/// session that ended, or a server that was killed, can still answer a
/// listing for a moment; both probes rescan in well under a second.
pub(super) const SETTLE_TRIES: u32 = 2;
pub(super) const SETTLE_DELAY: Duration = Duration::from_millis(150);

#[cfg(test)]
#[path = "handoff_tests.rs"]
mod tests;
