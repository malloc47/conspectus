// Extracted alongside teardown.rs — see #[path = "teardown_tests.rs"].
use super::*;
use crate::discovery::tmux::{FakeTmux, TmuxKillOutcome};
use std::sync::Mutex;

/// Fake signaller: records `SIGTERM` targets and answers `is_alive`
/// from a canned per-call queue (front = earliest poll). An empty
/// queue means "still alive" so a test that never wants a graceful
/// exit can just leave it empty.
struct FakeSignaller {
    terminated: Mutex<Vec<i64>>,
    alive_answers: Mutex<std::collections::VecDeque<bool>>,
    sleeps: Mutex<u32>,
}

impl FakeSignaller {
    fn new(alive: impl IntoIterator<Item = bool>) -> Self {
        Self {
            terminated: Mutex::new(Vec::new()),
            alive_answers: Mutex::new(alive.into_iter().collect()),
            sleeps: Mutex::new(0),
        }
    }
}

impl ProcessSignaller for FakeSignaller {
    fn terminate(&self, pid: i64) -> Result<()> {
        self.terminated.lock().unwrap().push(pid);
        Ok(())
    }

    fn is_alive(&self, _pid: i64) -> bool {
        self.alive_answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(true)
    }

    fn sleep(&self, _dur: Duration) {
        *self.sleeps.lock().unwrap() += 1;
    }
}

#[test]
fn graceful_exit_signals_then_hard_reclaims() {
    let mux = FakeTmux::with_sessions("").with_kill("feat", TmuxKillOutcome::NoTarget);
    // Dead on the first poll → graceful exit.
    let sig = FakeSignaller::new([false]);
    let report = teardown_mux_session(&mux, &sig, None, "feat", Some(4242), Duration::from_secs(3))
        .expect("ok");

    assert!(report.signalled);
    assert!(report.graceful_exited);
    assert_eq!(report.kill, TmuxKillOutcome::NoTarget);
    assert_eq!(*sig.terminated.lock().unwrap(), vec![4242]);
    // Session still reclaimed even after graceful exit.
    assert_eq!(mux.kill_calls(), vec![(None, "feat".to_string())]);
}

#[test]
fn wedged_process_escalates_to_hard_kill() {
    let mux = FakeTmux::with_sessions("");
    // Always alive → grace elapses → hard kill.
    let sig = FakeSignaller::new(std::iter::empty::<bool>());
    let report = teardown_mux_session(
        &mux,
        &sig,
        Some("scratch"),
        "feat",
        Some(99),
        // Tiny window so the poll loop exits quickly on wall clock.
        Duration::from_millis(1),
    )
    .expect("ok");

    assert!(report.signalled);
    assert!(!report.graceful_exited);
    assert_eq!(report.kill, TmuxKillOutcome::Killed);
    assert_eq!(*sig.terminated.lock().unwrap(), vec![99]);
    assert_eq!(
        mux.kill_calls(),
        vec![(Some("scratch".to_string()), "feat".to_string())]
    );
}

#[test]
fn unknown_pane_pid_skips_graceful_and_hard_kills() {
    let mux = FakeTmux::with_sessions("");
    let sig = FakeSignaller::new(std::iter::empty::<bool>());
    let report =
        teardown_mux_session(&mux, &sig, None, "feat", None, Duration::from_secs(3)).expect("ok");

    assert!(!report.signalled);
    assert!(!report.graceful_exited);
    assert_eq!(report.kill, TmuxKillOutcome::Killed);
    assert!(sig.terminated.lock().unwrap().is_empty());
}

#[test]
fn zero_grace_skips_graceful_phase() {
    let mux = FakeTmux::with_sessions("");
    let sig = FakeSignaller::new(std::iter::empty::<bool>());
    let report =
        teardown_mux_session(&mux, &sig, None, "feat", Some(7), Duration::ZERO).expect("ok");

    assert!(!report.signalled);
    assert!(!report.graceful_exited);
    assert!(sig.terminated.lock().unwrap().is_empty());
    assert_eq!(report.kill, TmuxKillOutcome::Killed);
}

#[test]
fn unsupported_backend_reports_unsupported_kill() {
    struct NoKill;
    impl MuxBackend for NoKill {
        fn backend_key(&self) -> &'static str {
            "no-kill"
        }
        fn list_sessions(&self, _format: &str) -> Result<crate::discovery::tmux::TmuxOutcome> {
            Ok(crate::discovery::tmux::TmuxOutcome::Sessions(String::new()))
        }
    }
    let sig = FakeSignaller::new([false]);
    let report = teardown_mux_session(&NoKill, &sig, None, "feat", Some(7), Duration::from_secs(3))
        .expect("ok");
    assert_eq!(report.kill, TmuxKillOutcome::Unsupported);
}
