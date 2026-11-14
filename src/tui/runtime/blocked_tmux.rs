//! A mux backend for tests whose pane captures wait for the test to let
//! them finish, so a test can hold a capture in flight and see what the
//! loop does meanwhile.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::discovery::tmux::{MuxBackend, TmuxCaptureOutcome, TmuxOutcome};

/// How long a blocked capture waits for its gate before it gives up,
/// so a test that forgets to open it fails instead of hanging.
const PATIENCE: Duration = Duration::from_secs(5);

/// The gate the captures of a [`BlockedTmux`] wait at.
#[derive(Default)]
pub(super) struct Gate {
    open: AtomicBool,
    started: AtomicUsize,
    finished: AtomicUsize,
}

impl Gate {
    pub(super) fn open(&self) {
        self.open.store(true, Ordering::SeqCst);
    }

    pub(super) fn close(&self) {
        self.open.store(false, Ordering::SeqCst);
    }

    /// Captures that have begun.
    pub(super) fn started(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }

    /// Captures that got through the gate.
    pub(super) fn finished(&self) -> usize {
        self.finished.load(Ordering::SeqCst)
    }

    /// Wait, in real time, until `count` captures have begun.
    pub(super) fn wait_started(&self, count: usize) {
        let begun = Instant::now();
        while self.started() < count {
            assert!(begun.elapsed() < PATIENCE, "no capture began");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// A backend that lists no sessions and captures a pane only once its
/// gate is open: the text is `<target> capture <n>`, `n` counting
/// captures from 1.
pub(super) struct BlockedTmux {
    gate: Arc<Gate>,
}

impl BlockedTmux {
    /// A backend with a closed gate, and the gate.
    pub(super) fn closed() -> (Arc<dyn MuxBackend>, Arc<Gate>) {
        let gate = Arc::new(Gate::default());
        let tmux = Arc::new(Self {
            gate: Arc::clone(&gate),
        });
        (tmux, gate)
    }
}

impl MuxBackend for BlockedTmux {
    fn backend_key(&self) -> &'static str {
        "tmux"
    }

    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(TmuxOutcome::Sessions(String::new()))
    }

    fn capture_pane(&self, _socket_name: Option<&str>, target: &str) -> Result<TmuxCaptureOutcome> {
        let n = self.gate.started.fetch_add(1, Ordering::SeqCst) + 1;
        let begun = Instant::now();
        while !self.gate.open.load(Ordering::SeqCst) {
            if begun.elapsed() > PATIENCE {
                return Ok(TmuxCaptureOutcome::Failed {
                    code: None,
                    message: "the test never opened the gate".to_string(),
                });
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.gate.finished.fetch_add(1, Ordering::SeqCst);
        Ok(TmuxCaptureOutcome::captured(format!(
            "{target} capture {n}"
        )))
    }
}
