//! Runs the content search scans the reducer asks for.
//!
//! The reducer leaves a [`ScanRequest`] on the app; the live loop starts
//! it on worker threads (session transcripts, and mux pane scrollback when
//! the request asks) and folds what comes back into [`Msg::ContentPanes`]
//! and [`Msg::ContentHits`] at most every [`FLUSH_EVERY`], so a scan that
//! finds many sessions does not rebuild the tree for each one. Modes
//! with no event loop of their own (snapshots, tests) scan to completion
//! on the calling thread.

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
use std::path::Path;
use std::time::{Duration, Instant};

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;

use std::sync::Arc;

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
use crate::content_search::search_blocking;
use crate::content_search::{
    Attribution, CancelToken, ContentEvent, ContentHandle, ContentPolled, ContentResult,
    ContentSource, PaneHits, PaneSource, Progress, ScanOptions, ScanSession, SessionKey,
    SessionSource, attribute_panes, hits_for, plan_sessions, search, sockets_in,
};
use crate::discovery::tmux::MuxBackend;
use crate::tui::app::{App, DetailRequest, Msg, PaneScope, ScanRequest};
use crate::viewer::search::{ContentQuery, Hit, SessionHits};

/// The longest results wait before they reach the screen.
pub(super) const FLUSH_EVERY: Duration = Duration::from_millis(150);

/// What a running scan has to say.
pub(super) enum Poll {
    Event(Box<ContentEvent>),
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

impl EventSource for ContentHandle {
    fn poll(&mut self) -> Poll {
        match ContentHandle::poll(self) {
            ContentPolled::Event(event) => Poll::Event(event),
            ContentPolled::Empty => Poll::Empty,
            ContentPolled::Ended => Poll::Disconnected,
        }
    }
}

/// Reads the matching turns of one session. Tests substitute a reader
/// they can hold still; the live loop uses [`hits_for`].
pub(super) type DetailReader = Arc<dyn Fn(&ScanSession, &ContentQuery) -> Vec<Hit> + Send + Sync>;

/// The scan the live loop is running, if any.
#[derive(Default)]
pub(super) struct ContentDriver {
    pub(super) active: Option<Active>,
    pub(super) details: Option<DetailActive>,
    /// Replaces [`hits_for`] for the detail reads; `None` outside tests.
    pub(super) detail_reader: Option<DetailReader>,
    /// Reads the scrollback of mux panes; without one a request for panes
    /// searches the sessions only.
    pub(super) pane_backend: Option<Arc<dyn MuxBackend>>,
}

/// The read of matching turns for the per-message grouping.
pub(super) struct DetailActive {
    generation: u64,
    /// Set when the turns are for a remembered search.
    serial: Option<u64>,
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
    pane_batch: Vec<PaneHits>,
    progress: Progress,
    done: bool,
}

/// What a scan of mux panes needs from the app: the servers to ask and
/// the sessions each mux hosts, for deduplication.
fn pane_source(
    app: &App,
    backend: &Arc<dyn MuxBackend>,
    scope: &PaneScope,
) -> Option<Box<dyn ContentSource>> {
    let handle = app.snapshot_handle()?;
    let snapshot = handle.snapshot();
    let attribution: Attribution = attribute_panes(snapshot);
    // Ages are drawn against the same clock as the rows, which snapshot
    // mode freezes.
    let source = PaneSource::new(Arc::clone(backend), sockets_in(snapshot), attribution)
        .at(crate::tui::rows::tree_current_unix_epoch());
    Some(Box::new(match &scope.muxes {
        Some(muxes) => source.only_muxes(muxes.clone()),
        None => source,
    }))
}

impl ContentDriver {
    /// A driver that reads pane scrollback through tmux.
    pub(super) fn live() -> Self {
        Self {
            pane_backend: Some(Arc::new(crate::discovery::tmux::SystemTmux::new())),
            ..Self::default()
        }
    }

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
            let panes = request
                .panes
                .as_ref()
                .zip(self.pane_backend.as_ref())
                .and_then(|(scope, backend)| pane_source(app, backend, scope));
            self.active = Some(start(&request, panes));
        }
        let Some(active) = &mut self.active else {
            return;
        };
        loop {
            match active.handle.poll() {
                Poll::Event(event) if event.generation() != active.generation => {}
                Poll::Event(event) => match *event {
                    ContentEvent::Progress { progress, .. } => active.progress = progress,
                    ContentEvent::Result { result, .. } => match result {
                        ContentResult::Session { key, hits } => active.batch.push((key, hits)),
                        ContentResult::Pane(pane) => active.pane_batch.push(pane),
                    },
                    ContentEvent::Done { progress, .. } => {
                        active.progress = progress;
                        active.done = true;
                    }
                },
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
            let panes = std::mem::take(&mut active.pane_batch);
            let (generation, progress, done) = (active.generation, active.progress, active.done);
            if !panes.is_empty() {
                app.update(Msg::ContentPanes {
                    generation,
                    batch: panes,
                });
            }
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
            && app.detail_target() != Some((active.generation, active.serial))
        {
            self.details = None;
        }
        if self.details.is_none()
            && let Some(request) = app.take_detail_request()
        {
            self.details = Some(start_details(request, self.detail_reader.clone()));
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
                app.update(match active.serial {
                    Some(serial) => Msg::HistoryDetails { serial, batch },
                    None => Msg::ContentDetails {
                        generation: active.generation,
                        batch,
                    },
                });
            }
        }
        if finished {
            self.details = None;
        }
    }
}

fn start_details(request: DetailRequest, reader: Option<DetailReader>) -> DetailActive {
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
            let hits = match &reader {
                Some(read) => read(session, &query),
                None => hits_for(session, &query),
            };
            let key = session.key.clone();
            if sender.send((key, hits)).is_err() {
                return;
            }
        }
    });
    DetailActive {
        generation: request.generation,
        serial: request.serial,
        cancel,
        events,
        last_flush: Instant::now(),
        batch: Vec::new(),
        thread: Some(thread),
    }
}

fn start(request: &ScanRequest, panes: Option<Box<dyn ContentSource>>) -> Active {
    let plan = plan_sessions(&request.sessions);
    let query = ContentQuery::new(request.text.clone(), request.kinds);
    let mut sources: Vec<Box<dyn ContentSource>> =
        vec![Box::new(SessionSource::new(plan.sessions))];
    sources.extend(panes);
    let handle = search(
        sources,
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
        pane_batch: Vec::new(),
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
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub(in crate::tui) fn run_blocking(app: &mut App, rebase: Option<(&Path, &Path)>) {
    run_blocking_with(app, rebase, None);
}

/// [`run_blocking`] reading pane scrollback through `panes`; without a
/// backend a request for panes searches the sessions only.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub(in crate::tui) fn run_blocking_with(
    app: &mut App,
    rebase: Option<(&Path, &Path)>,
    panes: Option<&Arc<dyn MuxBackend>>,
) {
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
        let mut sources: Vec<Box<dyn ContentSource>> =
            vec![Box::new(SessionSource::new(plan.sessions))];
        sources.extend(
            request
                .panes
                .as_ref()
                .zip(panes)
                .and_then(|(scope, backend)| pane_source(app, backend, scope)),
        );
        let summary = search_blocking(
            sources,
            query,
            ScanOptions {
                generation: request.generation,
                ..ScanOptions::default()
            },
            &CancelToken::new(),
        );
        let mut sessions = Vec::new();
        let mut found_panes = Vec::new();
        for result in summary.results {
            match result {
                ContentResult::Session { key, hits } => sessions.push((key, hits)),
                ContentResult::Pane(pane) => found_panes.push(pane),
            }
        }
        if !found_panes.is_empty() {
            app.update(Msg::ContentPanes {
                generation: request.generation,
                batch: found_panes,
            });
        }
        app.update(Msg::ContentHits {
            generation: request.generation,
            batch: sessions,
            progress: summary.progress,
            done: true,
        });
        run_details_blocking(app, rebase);
    }
    run_details_blocking(app, rebase);
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
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
        app.update(match details.serial {
            Some(serial) => Msg::HistoryDetails { serial, batch },
            None => Msg::ContentDetails {
                generation: details.generation,
                batch,
            },
        });
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
