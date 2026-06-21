//! Filesystem watcher abstraction (P7-009 / ADR 0081).
//!
//! The daemon's per-class scheduler uses a [`Watcher`] to wait
//! for filesystem changes with a timeout. When the OS reports
//! activity on any registered path the watcher returns
//! [`WatcherEvent::Changed`] and the scheduler runs its cycle
//! immediately; otherwise the timeout fires (returning
//! [`WatcherEvent::Timeout`]) and the scheduler runs a normal
//! interval-driven cycle.
//!
//! Two implementations ship in this module:
//!
//! * [`NotifyWatcher`] — wraps the `notify` crate to install
//!   per-path watchers via inotify (Linux), kqueue (BSD), or
//!   FSEvents (macOS). Failure to install (rlimit, EACCES,
//!   unsupported fs) bubbles up as an `Err`; the caller is
//!   expected to swallow it into a one-line warning and fall
//!   back to interval polling. The watcher itself is best-
//!   effort: if the OS drops events, polling still catches up
//!   on the next interval.
//! * [`FakeWatcher`] — a test-only deterministic implementation
//!   driven by an internal queue of pre-canned events. Used by
//!   the scheduler integration tests to exercise watcher-
//!   available, watcher-fallback, and watcher-saturation
//!   paths without depending on real OS behavior.

use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{
    Config as NotifyConfig, Event as NotifyEventStruct, EventKind, RecommendedWatcher,
    RecursiveMode, Watcher as NotifyWatcherTrait,
};

/// Event surfaced by a single [`Watcher::wait`] call.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WatcherEvent {
    /// At least one registered path observed a filesystem
    /// change before the timeout. Caller should run its cycle
    /// immediately.
    Changed,
    /// The timeout elapsed without any change. Caller falls
    /// back to its interval-driven cycle.
    Timeout,
    /// The caller's shutdown latch flipped while the wait was
    /// in progress. Caller should exit cleanly.
    ShuttingDown,
}

/// Wait-with-timeout abstraction the scheduler uses. The trait
/// is intentionally narrow so a real (notify-backed) and a
/// fake (queue-backed) implementation can both satisfy it
/// without leaking notify types into the test surface.
pub trait Watcher: Send {
    fn wait(&mut self, timeout: Duration) -> WatcherEvent;
}

/// Fallback watcher used when notify installation fails or
/// when a class doesn't have a filesystem signal worth watching
/// (e.g. forge — the upstream `gh` calls have no local
/// filesystem signal). Every [`Watcher::wait`] call sleeps the
/// full timeout and returns [`WatcherEvent::Timeout`], so the
/// scheduler degrades cleanly to its plain interval cadence.
pub struct NullWatcher;

impl Watcher for NullWatcher {
    fn wait(&mut self, timeout: Duration) -> WatcherEvent {
        std::thread::sleep(timeout);
        WatcherEvent::Timeout
    }
}

/// notify-backed watcher. Holds the per-path watchers and the
/// channel they write into; [`wait`](Self::wait) just blocks on
/// the channel with a timeout. The `notify::RecommendedWatcher`
/// picks the right backend per platform (inotify / kqueue /
/// FSEvents) per ADR 0081.
pub struct NotifyWatcher {
    rx: Receiver<()>,
    _watcher: RecommendedWatcher,
}

impl NotifyWatcher {
    /// Install non-recursive watchers on each of `paths`.
    /// Missing paths are skipped silently (a harness whose
    /// state dir doesn't exist yet doesn't block the others);
    /// the caller can recover when the path appears by simply
    /// running the next polling cycle and re-installing on the
    /// subsequent daemon restart. Returns an error only on
    /// catastrophic notify-setup failure (rlimit, OOM, etc.).
    pub fn new<I, P>(paths: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let (tx, rx) = channel::<()>();
        let bridge = ChannelBridge { tx };
        let mut watcher = RecommendedWatcher::new(bridge, NotifyConfig::default())
            .context("create notify::RecommendedWatcher")?;
        for path in paths {
            let path = path.as_ref();
            if !path.exists() {
                // Skip non-existent paths rather than failing
                // the whole watcher. The poll path will pick up
                // the directory when it appears.
                continue;
            }
            // NonRecursive: harness state dirs are flat, and
            // recursive watching would unnecessarily inflate the
            // inotify-descriptor count (see ADR 0081's
            // watch-descriptor budget note).
            if let Err(err) = watcher.watch(path, RecursiveMode::NonRecursive) {
                eprintln!(
                    "conspectus serve: failed to install watcher on {}: {err:#}",
                    path.display()
                );
            }
        }
        Ok(Self {
            rx,
            _watcher: watcher,
        })
    }
}

impl Watcher for NotifyWatcher {
    fn wait(&mut self, timeout: Duration) -> WatcherEvent {
        match self.rx.recv_timeout(timeout) {
            Ok(()) => {
                // Drain any further events that arrived in
                // quick succession so the caller does not get
                // woken twice for a burst of changes (e.g. an
                // editor's atomic-write rename sequence).
                while self.rx.try_recv().is_ok() {}
                WatcherEvent::Changed
            }
            Err(RecvTimeoutError::Timeout) => WatcherEvent::Timeout,
            // Channel disconnected — the watcher handle is
            // gone. Treat as shutting down so the scheduler
            // exits cleanly rather than spinning.
            Err(RecvTimeoutError::Disconnected) => WatcherEvent::ShuttingDown,
        }
    }
}

/// Bridges notify's `EventHandler` trait into a channel send.
/// We deliberately collapse every change into a unit signal —
/// the scheduler only needs to know "something changed";
/// downstream discovery re-reads the directory anyway.
struct ChannelBridge {
    tx: Sender<()>,
}

impl notify::EventHandler for ChannelBridge {
    fn handle_event(&mut self, event: notify::Result<NotifyEventStruct>) {
        let Ok(event) = event else {
            // Notify-side error (rare; usually a watcher being
            // moved out from under us). Ignore — the polling
            // fallback covers any missed signal.
            return;
        };
        // Coalesce: notify reports very granular events
        // (Create / Modify / Remove / Access). For our wake-up
        // purpose any of them is "something changed." We drop
        // Access events because they're noisy and don't
        // indicate state change.
        if matches!(event.kind, EventKind::Access(_) | EventKind::Other) {
            return;
        }
        // Send is best-effort: a full channel (which never
        // happens in practice; the receiver is always waiting)
        // would just drop the signal, and the next change
        // would re-fire.
        let _ = self.tx.send(());
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Test-only watcher driven by a pre-canned event queue.
    /// Each `wait` call pops the next queued event; an empty
    /// queue returns [`WatcherEvent::Timeout`] so tests don't
    /// have to enumerate every wait the scheduler will make.
    pub struct FakeWatcher {
        queue: Arc<Mutex<VecDeque<WatcherEvent>>>,
    }

    impl FakeWatcher {
        pub fn new() -> Self {
            Self {
                queue: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        /// Handle the caller can use to push events from a
        /// peer thread without holding a `&mut` reference to
        /// the watcher.
        pub fn handle(&self) -> FakeWatcherHandle {
            FakeWatcherHandle {
                queue: Arc::clone(&self.queue),
            }
        }
    }

    impl Default for FakeWatcher {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Watcher for FakeWatcher {
        fn wait(&mut self, _timeout: Duration) -> WatcherEvent {
            self.queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(WatcherEvent::Timeout)
        }
    }

    /// Pusher side. Cloneable; multiple peers can enqueue
    /// events into the same fake watcher.
    #[derive(Clone)]
    pub struct FakeWatcherHandle {
        queue: Arc<Mutex<VecDeque<WatcherEvent>>>,
    }

    impl FakeWatcherHandle {
        pub fn push(&self, event: WatcherEvent) {
            self.queue.lock().unwrap().push_back(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeWatcher;
    use super::*;

    #[test]
    fn fake_watcher_returns_queued_events_then_timeout() {
        // The fake's contract: pops queued events in FIFO
        // order, then returns Timeout once exhausted so the
        // scheduler doesn't need to enumerate every wait.
        let mut watcher = FakeWatcher::new();
        let handle = watcher.handle();
        handle.push(WatcherEvent::Changed);
        handle.push(WatcherEvent::Changed);
        assert_eq!(
            watcher.wait(Duration::from_millis(10)),
            WatcherEvent::Changed
        );
        assert_eq!(
            watcher.wait(Duration::from_millis(10)),
            WatcherEvent::Changed
        );
        assert_eq!(
            watcher.wait(Duration::from_millis(10)),
            WatcherEvent::Timeout
        );
    }

    #[test]
    fn notify_watcher_with_empty_path_set_is_a_pure_timeout() {
        // No watchers installed → every wait times out. The
        // scheduler's interval cadence carries on as if the
        // watcher weren't there.
        let mut watcher = NotifyWatcher::new(Vec::<&Path>::new()).expect("create");
        assert_eq!(
            watcher.wait(Duration::from_millis(50)),
            WatcherEvent::Timeout
        );
    }

    #[test]
    fn notify_watcher_fires_on_file_create_in_watched_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let mut watcher = NotifyWatcher::new([tmp.path()]).expect("create");

        // Spawn a peer that creates a file after a short
        // delay; the wait should return Changed before its
        // timeout fires.
        let path = tmp.path().to_owned();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _ = std::fs::write(path.join("triggered"), b"hello");
        });

        let event = watcher.wait(Duration::from_secs(3));
        assert_eq!(
            event,
            WatcherEvent::Changed,
            "watcher should fire on file create within the timeout"
        );
    }
}
