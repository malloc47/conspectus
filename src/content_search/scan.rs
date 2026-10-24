//! The parallel, cancellable transcript scan.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::viewer::parser::{HarnessParser, TranscriptIndex, TranscriptSource};
use crate::viewer::search::{ContentQuery, Hit, SessionHits, match_document};

use super::prefilter::{file_may_match, safe_runs};
use super::{MAX_HITS_PER_SESSION, Progress, ScanEvent, ScanOptions, ScanSession, SessionKey};

/// Cancels a scan. Clones share one flag.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// What polling a [`ScanHandle`] found.
#[derive(Debug)]
pub enum Polled {
    /// An event was waiting.
    Event(ScanEvent),
    /// Nothing yet; the scan is still running.
    Empty,
    /// The scan's thread has ended and every event has been read. A
    /// scan always sends a [`ScanEvent::Done`] before it ends, even when a
    /// parser panics, so a caller that sees this without having seen
    /// `Done` should treat the scan as finished.
    Ended,
}

/// A scan running on its own threads. Dropping it cancels the scan.
///
/// Contract for a driver: a scan sends zero or more `Progress` and
/// `Session` events and then exactly one `Done`, all stamped with the
/// scan's generation. If the scan thread dies without sending `Done`
/// (it should not, since panics are caught), [`ScanHandle::poll`]
/// reports [`Polled::Ended`], and the driver must treat that as done.
pub struct ScanHandle {
    events: Receiver<ScanEvent>,
    cancel: CancelToken,
    thread: Option<JoinHandle<()>>,
}

impl ScanHandle {
    /// The next event if one is waiting.
    pub fn try_recv(&self) -> Option<ScanEvent> {
        self.events.try_recv().ok()
    }

    /// Poll for the next event, telling a quiet scan from an ended one.
    pub fn poll(&self) -> Polled {
        match self.events.try_recv() {
            Ok(event) => Polled::Event(event),
            Err(mpsc::TryRecvError::Empty) => Polled::Empty,
            Err(mpsc::TryRecvError::Disconnected) => Polled::Ended,
        }
    }

    /// The next event, waiting up to `timeout`. `None` on timeout or
    /// once the scan has ended and the events are drained.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<ScanEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// The next event, waiting for it. `None` once the scan has ended
    /// and the events are drained.
    pub fn recv(&self) -> Option<ScanEvent> {
        self.events.recv().ok()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// A token that cancels this scan from elsewhere.
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    /// Wait for the scan's threads to finish.
    pub fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ScanHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Everything a finished (or cancelled) blocking scan found.
#[derive(Clone, Debug, Default)]
pub struct ScanSummary {
    /// Matching sessions, ordered by key.
    pub results: Vec<(SessionKey, SessionHits)>,
    pub progress: Progress,
    pub cancelled: bool,
}

/// Start scanning `sessions` for `query` on background threads.
pub fn scan(sessions: Vec<ScanSession>, query: ContentQuery, options: ScanOptions) -> ScanHandle {
    let (tx, rx) = mpsc::channel();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let thread = std::thread::spawn(move || {
        let total = sessions.len();
        let emit = |event: ScanEvent| {
            let _ = tx.send(event);
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            run(&sessions, &query, options, &token, &emit);
        }));
        if outcome.is_err() {
            // Something outside a single session's read went wrong. The
            // scan still ends with a Done, so the caller never waits on
            // a scan that has died.
            let progress = Progress {
                scanned: total,
                total,
                skipped: total,
            };
            emit(ScanEvent::Done {
                generation: options.generation,
                progress,
                matched: 0,
                cancelled: false,
            });
        }
    });
    ScanHandle {
        events: rx,
        cancel,
        thread: Some(thread),
    }
}

/// Scan to completion on the calling thread (plus workers).
pub fn scan_blocking(
    sessions: &[ScanSession],
    query: &ContentQuery,
    options: ScanOptions,
    cancel: &CancelToken,
) -> ScanSummary {
    let summary = Mutex::new(ScanSummary::default());
    run(sessions, query, options, cancel, &|event| {
        let mut summary = summary
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match event {
            ScanEvent::Session { key, hits, .. } => summary.results.push((key, hits)),
            ScanEvent::Done {
                progress,
                cancelled,
                ..
            } => {
                summary.progress = progress;
                summary.cancelled = cancelled;
            }
            ScanEvent::Progress { .. } => {}
        }
    });
    let mut summary = summary
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    summary.results.sort_by(|a, b| a.0.cmp(&b.0));
    summary
}

/// Every hit in one session, for showing its matches, with the count
/// before any cap.
#[derive(Clone, Debug, Default)]
pub struct SessionDetail {
    /// The hits kept, in turn order. When a session has more than
    /// [`MAX_HITS_PER_SESSION`] matching turns these are the newest
    /// ones.
    pub hits: Vec<Hit>,
    /// The number of matching turns, whether or not all are in `hits`.
    pub total: usize,
}

impl SessionDetail {
    /// Whether the cap dropped some hits.
    pub fn truncated(&self) -> bool {
        self.total > self.hits.len()
    }
}

/// Every hit in one session, for showing its matches. Capped at
/// [`MAX_HITS_PER_SESSION`], keeping the newest; the count on
/// [`SessionHits`] is the true total. Reads the transcript again, since a
/// scan keeps only the best hit per session.
pub fn hits_for(session: &ScanSession, query: &ContentQuery) -> Vec<Hit> {
    session_detail(session, query).hits
}

/// [`hits_for`], also reporting how many turns matched in all.
pub fn session_detail(session: &ScanSession, query: &ContentQuery) -> SessionDetail {
    let Ok(document) = session.parser.read(&session.locator) else {
        return SessionDetail::default();
    };
    keep_newest(match_document(&document, query))
}

/// Keep at most [`MAX_HITS_PER_SESSION`] hits, the newest by time and then
/// by turn, returned in turn order.
pub(crate) fn keep_newest(mut hits: Vec<Hit>) -> SessionDetail {
    let total = hits.len();
    if total > MAX_HITS_PER_SESSION {
        hits.sort_by(|a, b| {
            b.timestamp
                .cmp(&a.timestamp)
                .then_with(|| b.turn_index.cmp(&a.turn_index))
        });
        hits.truncate(MAX_HITS_PER_SESSION);
        hits.sort_by_key(|hit| hit.turn_index);
    }
    SessionDetail { hits, total }
}

/// At most one progress event per this many sessions...
const PROGRESS_EVERY: usize = 64;

/// ...or per this long, whichever comes first.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// What one session's turn through the scan came to.
enum Outcome {
    /// The prefilter or candidate set ruled it out, so it wasn't parsed.
    RuledOut,
    /// It was parsed and had no hit.
    NoMatch,
    Matched(SessionHits),
    /// It couldn't be read.
    Skipped,
}

/// Sessions of one harness under one state root share an index.
struct Group {
    parser: &'static dyn HarnessParser,
    index: Box<dyn TranscriptIndex>,
    /// Sessions the harness's database says could match; `None` when it
    /// couldn't narrow.
    candidates: Option<std::collections::HashSet<String>>,
}

type GroupKey = (String, PathBuf);

fn run(
    sessions: &[ScanSession],
    query: &ContentQuery,
    options: ScanOptions,
    cancel: &CancelToken,
    emit: &(dyn Fn(ScanEvent) + Sync),
) {
    let total = sessions.len();
    let generation = options.generation;
    let finish = |progress: Progress, matched: usize| {
        emit(ScanEvent::Done {
            generation,
            progress,
            matched,
            cancelled: cancel.is_cancelled(),
        });
    };
    let Some(needle) = query.needle() else {
        // Nothing to look for: every session is trivially checked.
        finish(
            Progress {
                scanned: total,
                total,
                skipped: 0,
            },
            0,
        );
        return;
    };

    // The byte filter is sound only for text that is stored verbatim in
    // the file. Tool bodies are re-serialized from JSON, and thinking,
    // hidden, and aborted turns include text the parsers synthesize
    // (placeholders and metadata that appear nowhere on disk), so the
    // filter is used only when none of those are searched.
    let kinds = &query.kinds;
    let verbatim_only = !(kinds.tools || kinds.thinking || kinds.hidden || kinds.aborted);
    let runs = if options.prefilter && verbatim_only {
        safe_runs(needle)
    } else {
        Vec::new()
    };

    // Let the caller show 0/total before any database is consulted.
    emit(ScanEvent::Progress {
        generation,
        progress: Progress {
            scanned: 0,
            total,
            skipped: 0,
        },
    });

    let mut group_ids: HashMap<GroupKey, usize> = HashMap::new();
    let mut groups: Vec<Group> = Vec::new();
    // Each session's group, found once here so the workers don't build a
    // key per session.
    let mut group_of: Vec<usize> = Vec::with_capacity(total);
    for session in sessions {
        if cancel.is_cancelled() {
            finish(
                Progress {
                    scanned: 0,
                    total,
                    skipped: 0,
                },
                0,
            );
            return;
        }
        let id = *group_ids.entry(group_key(session)).or_insert_with(|| {
            groups.push(Group {
                parser: session.parser,
                index: session.parser.index(&session.locator.state_root),
                candidates: if runs.is_empty() {
                    None
                } else {
                    session
                        .parser
                        .candidates(&session.locator.state_root, &runs, &|| {
                            cancel.is_cancelled()
                        })
                },
            });
            groups.len() - 1
        });
        group_of.push(id);
    }

    let scanned = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let matched = AtomicUsize::new(0);
    let next = AtomicUsize::new(0);
    let workers = worker_count(options.workers).min(total.max(1));
    let progress = || Progress {
        scanned: scanned.load(Ordering::Relaxed),
        total,
        skipped: skipped.load(Ordering::Relaxed),
    };
    // Progress events are throttled: at most one per `PROGRESS_EVERY`
    // sessions or `PROGRESS_INTERVAL`, whichever comes first. The counters
    // stay exact, and `Done` carries the final ones.
    let started = Instant::now();
    let last_progress_ms = AtomicU64::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(session) = sessions.get(i) else {
                        break;
                    };
                    let group = &groups[group_of[i]];
                    // A parser that panics on odd data costs that one
                    // session, not the scan.
                    let outcome =
                        catch_unwind(AssertUnwindSafe(|| process(session, group, query, &runs)))
                            .unwrap_or(Outcome::Skipped);
                    match outcome {
                        Outcome::Matched(hits) => {
                            matched.fetch_add(1, Ordering::Relaxed);
                            emit(ScanEvent::Session {
                                generation,
                                key: session.key.clone(),
                                hits,
                            });
                        }
                        Outcome::Skipped => {
                            skipped.fetch_add(1, Ordering::Relaxed);
                        }
                        Outcome::RuledOut | Outcome::NoMatch => {}
                    }
                    let done_count = scanned.fetch_add(1, Ordering::Relaxed) + 1;
                    let now_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let last = last_progress_ms.load(Ordering::Relaxed);
                    let due = done_count.is_multiple_of(PROGRESS_EVERY)
                        || now_ms >= last + PROGRESS_INTERVAL.as_millis() as u64;
                    if due
                        && last_progress_ms
                            .compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed)
                            .is_ok()
                    {
                        emit(ScanEvent::Progress {
                            generation,
                            progress: progress(),
                        });
                    }
                }
            });
        }
    });
    finish(progress(), matched.load(Ordering::Relaxed));
}

fn group_key(session: &ScanSession) -> GroupKey {
    (
        session.key.harness_key.clone(),
        session.locator.state_root.clone(),
    )
}

fn worker_count(requested: usize) -> usize {
    if requested > 0 {
        return requested;
    }
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(8)
}

fn process(session: &ScanSession, group: &Group, query: &ContentQuery, runs: &[String]) -> Outcome {
    if let Some(candidates) = &group.candidates
        && !candidates.contains(&session.key.session_key)
    {
        return Outcome::RuledOut;
    }
    let Some(source) = group.index.locate(&session.key.session_key) else {
        return Outcome::Skipped;
    };
    if !runs.is_empty()
        && let TranscriptSource::File(path) = &source
    {
        match file_may_match(path, runs) {
            Ok(false) => return Outcome::RuledOut,
            Ok(true) => {}
            Err(_) => return Outcome::Skipped,
        }
    }
    let Ok(document) = group.parser.read_source(&source, &session.locator) else {
        return Outcome::Skipped;
    };
    match SessionHits::from_document(&document, query) {
        Some(summary) => Outcome::Matched(summary),
        None => Outcome::NoMatch,
    }
}
