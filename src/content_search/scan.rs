//! The parallel, cancellable transcript scan.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::viewer::parser::{HarnessParser, TranscriptIndex, TranscriptSource};
use crate::viewer::search::{ContentQuery, Hit, SessionHits, match_document};

use super::prefilter::{contains_all, safe_runs};
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

/// A scan running on its own threads. Dropping it cancels the scan.
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
        run(&sessions, &query, options, &token, &|event| {
            let _ = tx.send(event);
        });
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

/// Every hit in one session, for showing its matches. Capped at
/// [`MAX_HITS_PER_SESSION`]; the count on [`SessionHits`] is the true
/// total. Reads the transcript again, since a scan keeps only the best
/// hit per session.
pub fn hits_for(session: &ScanSession, query: &ContentQuery) -> Vec<Hit> {
    let Ok(document) = session.parser.read(&session.locator) else {
        return Vec::new();
    };
    let mut hits = match_document(&document, query);
    hits.truncate(MAX_HITS_PER_SESSION);
    hits
}

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

    // Tool bodies are re-serialized from JSON, so a number can read
    // differently than it was stored; only filter when they aren't
    // searched.
    let runs = if options.prefilter && !query.kinds.tools {
        safe_runs(needle)
    } else {
        Vec::new()
    };

    let mut groups: HashMap<GroupKey, Group> = HashMap::new();
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
        groups.entry(group_key(session)).or_insert_with(|| Group {
            parser: session.parser,
            index: session.parser.index(&session.locator.state_root),
            candidates: if runs.is_empty() {
                None
            } else {
                session
                    .parser
                    .candidates(&session.locator.state_root, &runs)
            },
        });
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
                    let group = &groups[&group_key(session)];
                    match process(session, group, query, &runs) {
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
                    scanned.fetch_add(1, Ordering::Relaxed);
                    emit(ScanEvent::Progress {
                        generation,
                        progress: progress(),
                    });
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
        match std::fs::read(path) {
            Ok(bytes) if !contains_all(&bytes, runs) => return Outcome::RuledOut,
            Ok(_) => {}
            Err(_) => return Outcome::Skipped,
        }
    }
    let Ok(document) = group.parser.read_source(&source, &session.locator) else {
        return Outcome::Skipped;
    };
    let hits = match_document(&document, query);
    match SessionHits::from_hits(&hits) {
        Some(summary) => Outcome::Matched(summary),
        None => Outcome::NoMatch,
    }
}
