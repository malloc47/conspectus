//! One cancellable, generation-stamped event stream over several
//! content sources.
//!
//! [`search`] runs every source on its own thread and merges what they
//! report: results as they are found and progress summed across sources.
//! The sessions-only API ([`mod@super::scan`]) is unchanged; this is the
//! entry point for callers that also search other sources, such as pane
//! scrollback.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::source::{
    ContentResult, ContentSource, SourceEvent, SourceKind, cmp_results, cmp_results_by_recency,
};
use super::{CancelToken, Progress, ScanEvent, ScanOptions, ScanSession, scan};
use crate::viewer::search::ContentQuery;

/// What a merged search reports.
#[derive(Clone, Debug)]
pub enum ContentEvent {
    /// Counters moved; they are summed over every source.
    Progress { generation: u64, progress: Progress },
    /// A session or a pane matched.
    Result {
        generation: u64,
        result: ContentResult,
    },
    /// Every source ended, or the search was cancelled. Always sent last.
    Done {
        generation: u64,
        progress: Progress,
        matched: usize,
        cancelled: bool,
    },
}

impl ContentEvent {
    pub fn generation(&self) -> u64 {
        match self {
            Self::Progress { generation, .. }
            | Self::Result { generation, .. }
            | Self::Done { generation, .. } => *generation,
        }
    }
}

/// What polling a [`ContentHandle`] found.
#[derive(Debug)]
pub enum ContentPolled {
    Event(Box<ContentEvent>),
    Empty,
    /// The search's thread ended and every event has been read. A search
    /// sends `Done` first, so a caller that sees this without it should
    /// treat the search as finished.
    Ended,
}

/// A merged search running on its own threads. Dropping it cancels it.
pub struct ContentHandle {
    events: Receiver<ContentEvent>,
    cancel: CancelToken,
    thread: Option<JoinHandle<()>>,
}

impl ContentHandle {
    pub fn poll(&self) -> ContentPolled {
        match self.events.try_recv() {
            Ok(event) => ContentPolled::Event(Box::new(event)),
            Err(mpsc::TryRecvError::Empty) => ContentPolled::Empty,
            Err(mpsc::TryRecvError::Disconnected) => ContentPolled::Ended,
        }
    }

    pub fn try_recv(&self) -> Option<ContentEvent> {
        self.events.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<ContentEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    pub fn recv(&self) -> Option<ContentEvent> {
        self.events.recv().ok()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ContentHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Start searching `sources` for `query` on background threads.
pub fn search(
    sources: Vec<Box<dyn ContentSource>>,
    query: ContentQuery,
    options: ScanOptions,
) -> ContentHandle {
    let (tx, rx) = mpsc::channel();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let thread = std::thread::spawn(move || {
        let generation = options.generation;
        let latest = Mutex::new(vec![Progress::default(); sources.len()]);
        let matched = Mutex::new(0usize);
        let sum = |latest: &[Progress]| Progress {
            scanned: latest.iter().map(|p| p.scanned).sum(),
            total: latest.iter().map(|p| p.total).sum(),
            skipped: latest.iter().map(|p| p.skipped).sum(),
        };
        std::thread::scope(|scope| {
            for (index, source) in sources.iter().enumerate() {
                let (tx, latest, matched, query, token) = (&tx, &latest, &matched, &query, &token);
                scope.spawn(move || {
                    let emit = |event: SourceEvent| match event {
                        SourceEvent::Progress(progress) => {
                            let total = {
                                let mut latest = latest.lock().unwrap();
                                latest[index] = progress;
                                sum(&latest)
                            };
                            let _ = tx.send(ContentEvent::Progress {
                                generation,
                                progress: total,
                            });
                        }
                        SourceEvent::Result(result) => {
                            *matched.lock().unwrap() += 1;
                            let _ = tx.send(ContentEvent::Result { generation, result });
                        }
                    };
                    // A source that panics costs its own results, not
                    // the search.
                    let _ = catch_unwind(AssertUnwindSafe(|| {
                        source.run(query, &options, token, &emit);
                    }));
                });
            }
        });
        let progress = sum(&latest.lock().unwrap());
        let matched = *matched.lock().unwrap();
        let _ = tx.send(ContentEvent::Done {
            generation,
            progress,
            matched,
            cancelled: token.is_cancelled(),
        });
    });
    ContentHandle {
        events: rx,
        cancel,
        thread: Some(thread),
    }
}

/// Everything a finished (or cancelled) blocking search found.
#[derive(Clone, Debug, Default)]
pub struct ContentSummary {
    /// Matching results, most relevant first.
    pub results: Vec<ContentResult>,
    pub progress: Progress,
    pub cancelled: bool,
}

impl ContentSummary {
    /// Reorder the results newest first.
    pub fn sort_by_recency(&mut self) {
        self.results.sort_by(cmp_results_by_recency);
    }
}

/// Run [`search`] to completion on this thread.
pub fn search_blocking(
    sources: Vec<Box<dyn ContentSource>>,
    query: ContentQuery,
    options: ScanOptions,
    cancel: &CancelToken,
) -> ContentSummary {
    let handle = search(sources, query, options);
    let mut summary = ContentSummary::default();
    loop {
        if cancel.is_cancelled() {
            handle.cancel();
        }
        match handle.recv_timeout(Duration::from_millis(25)) {
            Some(ContentEvent::Result { result, .. }) => summary.results.push(result),
            Some(ContentEvent::Progress { progress, .. }) => summary.progress = progress,
            Some(ContentEvent::Done {
                progress,
                cancelled,
                ..
            }) => {
                summary.progress = progress;
                summary.cancelled = cancelled;
                break;
            }
            None => {
                if matches!(handle.poll(), ContentPolled::Ended) {
                    break;
                }
            }
        }
    }
    summary.results.sort_by(cmp_results);
    summary
}

/// The transcript source: the session scanner behind the
/// [`ContentSource`] seam.
pub struct SessionSource {
    sessions: Arc<Vec<ScanSession>>,
}

impl SessionSource {
    pub fn new(sessions: Vec<ScanSession>) -> Self {
        Self {
            sessions: Arc::new(sessions),
        }
    }
}

impl ContentSource for SessionSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Session
    }

    fn run(
        &self,
        query: &ContentQuery,
        options: &ScanOptions,
        cancel: &CancelToken,
        emit: &(dyn Fn(SourceEvent) + Sync),
    ) {
        let handle = scan(self.sessions.as_ref().clone(), query.clone(), *options);
        loop {
            if cancel.is_cancelled() {
                handle.cancel();
            }
            match handle.recv_timeout(Duration::from_millis(25)) {
                Some(ScanEvent::Progress { progress, .. }) => emit(SourceEvent::Progress(progress)),
                Some(ScanEvent::Session { key, hits, .. }) => {
                    emit(SourceEvent::Result(ContentResult::Session { key, hits }));
                }
                Some(ScanEvent::Done { progress, .. }) => {
                    emit(SourceEvent::Progress(progress));
                    break;
                }
                None => {
                    if matches!(handle.poll(), super::Polled::Ended) {
                        break;
                    }
                }
            }
        }
    }
}
