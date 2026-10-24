//! Runs the content search scans the reducer asks for.
//!
//! The reducer leaves a [`ScanRequest`] on the app; the live loop starts
//! it on worker threads and folds what comes back into
//! [`Msg::ContentHits`] at most every [`FLUSH_EVERY`], so a scan that
//! finds many sessions does not rebuild the tree for each one. Modes
//! with no event loop of their own (snapshots, tests) scan to completion
//! on the calling thread.

use std::path::Path;
use std::time::{Duration, Instant};

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;

use crate::content_search::{
    CancelToken, Polled, Progress, ScanEvent, ScanHandle, ScanOptions, SessionKey, hits_for,
    plan_sessions, scan, scan_blocking,
};
use crate::tui::app::{App, DetailRequest, Msg, ScanRequest};
use crate::viewer::search::{ContentQuery, Hit, SessionHits};

/// The longest results wait before they reach the screen.
pub(super) const FLUSH_EVERY: Duration = Duration::from_millis(150);

/// What a running scan has to say.
pub(super) enum Poll {
    Event(ScanEvent),
    /// Nothing yet; the scan is still running.
    Empty,
    /// The scan's threads are gone and nothing more will come.
    Disconnected,
}

/// Where a scan's events come from: the real handle, or a stand-in in
/// tests.
pub(super) trait EventSource {
    fn poll(&mut self) -> Poll;
}

impl EventSource for ScanHandle {
    fn poll(&mut self) -> Poll {
        match ScanHandle::poll(self) {
            Polled::Event(event) => Poll::Event(event),
            Polled::Empty => Poll::Empty,
            Polled::Ended => Poll::Disconnected,
        }
    }
}

/// The scan the live loop is running, if any.
#[derive(Default)]
pub(super) struct ContentDriver {
    pub(super) active: Option<Active>,
    pub(super) details: Option<DetailActive>,
}

/// The read of matching turns for the per-message grouping.
pub(super) struct DetailActive {
    generation: u64,
    cancel: CancelToken,
    events: Receiver<(SessionKey, Vec<Hit>)>,
    last_flush: Instant,
    batch: Vec<(SessionKey, Vec<Hit>)>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for DetailActive {
    fn drop(&mut self) {
        self.cancel.cancel();
        // The thread notices the token at its next session; leave it.
        drop(self.thread.take());
    }
}

pub(super) struct Active {
    generation: u64,
    handle: Box<dyn EventSource + Send>,
    last_flush: Instant,
    batch: Vec<(SessionKey, SessionHits)>,
    progress: Progress,
    done: bool,
}

impl ContentDriver {
    /// Start a requested scan, stop one the app no longer wants, and
    /// deliver what the running one has found.
    pub(super) fn pump(&mut self, app: &mut App) {
        self.pump_details(app);
        if let Some(active) = &self.active
            && app.content_generation() != Some(active.generation)
        {
            // The query changed or was cleared: dropping the handle
            // cancels the scan.
            self.active = None;
        }
        if self.active.is_none()
            && let Some(request) = app.take_scan_request()
        {
            self.active = Some(start(&request));
        }
        let Some(active) = &mut self.active else {
            return;
        };
        loop {
            match active.handle.poll() {
                Poll::Event(event) if event.generation() != active.generation => {}
                Poll::Event(ScanEvent::Progress { progress, .. }) => active.progress = progress,
                Poll::Event(ScanEvent::Session { key, hits, .. }) => active.batch.push((key, hits)),
                Poll::Event(ScanEvent::Done { progress, .. }) => {
                    active.progress = progress;
                    active.done = true;
                }
                Poll::Empty => break,
                // A scan whose threads died without saying so (a parser
                // panicked) is over: finish what it found instead of
                // searching forever.
                Poll::Disconnected => {
                    active.done = true;
                    break;
                }
            }
        }
        if active.done || active.last_flush.elapsed() >= FLUSH_EVERY {
            active.last_flush = Instant::now();
            let batch = std::mem::take(&mut active.batch);
            let (generation, progress, done) = (active.generation, active.progress, active.done);
            if done || !batch.is_empty() || progress.scanned > 0 {
                app.update(Msg::ContentHits {
                    generation,
                    batch,
                    progress,
                    done,
                });
            }
            if done {
                self.active = None;
            }
        }
    }
}

impl ContentDriver {
    /// Read the matching turns of the sessions a finished scan found,
    /// off the UI thread, and hand them over in batches.
    fn pump_details(&mut self, app: &mut App) {
        if let Some(active) = &self.details
            && app.content_generation() != Some(active.generation)
        {
            self.details = None;
        }
        if self.details.is_none()
            && let Some(request) = app.take_detail_request()
        {
            self.details = Some(start_details(request));
        }
        let Some(active) = &mut self.details else {
            return;
        };
        let mut finished = false;
        loop {
            match active.events.try_recv() {
                Ok(item) => active.batch.push(item),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if finished || active.last_flush.elapsed() >= FLUSH_EVERY {
            active.last_flush = Instant::now();
            let batch = std::mem::take(&mut active.batch);
            if !batch.is_empty() {
                app.update(Msg::ContentDetails {
                    generation: active.generation,
                    batch,
                });
            }
        }
        if finished {
            self.details = None;
        }
    }
}

fn start_details(request: DetailRequest) -> DetailActive {
    let (sender, events) = mpsc::channel();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let thread = std::thread::spawn(move || {
        let plan = plan_sessions(&request.sessions);
        let query = ContentQuery::new(request.text.clone(), request.kinds);
        for session in &plan.sessions {
            if token.is_cancelled() {
                return;
            }
            let hits = hits_for(session, &query);
            let key = session.key.clone();
            if sender.send((key, hits)).is_err() {
                return;
            }
        }
    });
    DetailActive {
        generation: request.generation,
        cancel,
        events,
        last_flush: Instant::now(),
        batch: Vec::new(),
        thread: Some(thread),
    }
}

fn start(request: &ScanRequest) -> Active {
    let plan = plan_sessions(&request.sessions);
    let query = ContentQuery::new(request.text.clone(), request.kinds);
    let handle = scan(
        plan.sessions,
        query,
        ScanOptions {
            generation: request.generation,
            ..ScanOptions::default()
        },
    );
    Active {
        generation: request.generation,
        handle: Box::new(handle),
        last_flush: Instant::now(),
        batch: Vec::new(),
        progress: Progress::default(),
        done: false,
    }
}

/// Run every requested scan to completion on this thread. Snapshot mode
/// and tests use it so a search finishes before the next frame.
///
/// `rebase` maps a scenario world's recorded state paths (the first
/// path) onto where the world was written (the second), as the viewer
/// does when snapshot mode opens a transcript.
pub(in crate::tui) fn run_blocking(app: &mut App, rebase: Option<(&Path, &Path)>) {
    while let Some(mut request) = app.take_scan_request() {
        if let Some((from, to)) = rebase {
            for id in &mut request.sessions {
                if let Ok(rest) = Path::new(&id.state_scope).strip_prefix(from) {
                    id.state_scope = to.join(rest).display().to_string();
                }
            }
        }
        let plan = plan_sessions(&request.sessions);
        let query = ContentQuery::new(request.text.clone(), request.kinds);
        let summary = scan_blocking(
            &plan.sessions,
            &query,
            ScanOptions {
                generation: request.generation,
                ..ScanOptions::default()
            },
            &CancelToken::new(),
        );
        app.update(Msg::ContentHits {
            generation: request.generation,
            batch: summary.results,
            progress: summary.progress,
            done: true,
        });
        run_details_blocking(app, rebase);
    }
    run_details_blocking(app, rebase);
}

fn run_details_blocking(app: &mut App, rebase: Option<(&Path, &Path)>) {
    while let Some(mut details) = app.take_detail_request() {
        if let Some((from, to)) = rebase {
            for id in &mut details.sessions {
                if let Ok(rest) = Path::new(&id.state_scope).strip_prefix(from) {
                    id.state_scope = to.join(rest).display().to_string();
                }
            }
        }
        let plan = plan_sessions(&details.sessions);
        let query = ContentQuery::new(details.text.clone(), details.kinds);
        let batch = plan
            .sessions
            .iter()
            .map(|session| (session.key.clone(), hits_for(session, &query)))
            .collect();
        app.update(Msg::ContentDetails {
            generation: details.generation,
            batch,
        });
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
