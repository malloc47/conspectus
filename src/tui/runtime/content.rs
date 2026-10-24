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

use crate::content_search::{
    CancelToken, Progress, ScanEvent, ScanHandle, ScanOptions, SessionKey, plan_sessions, scan,
    scan_blocking,
};
use crate::tui::app::{App, Msg, ScanRequest};
use crate::viewer::search::{ContentQuery, SessionHits};

/// The longest results wait before they reach the screen.
const FLUSH_EVERY: Duration = Duration::from_millis(150);

/// The scan the live loop is running, if any.
#[derive(Default)]
pub(super) struct ContentDriver {
    active: Option<Active>,
}

struct Active {
    generation: u64,
    handle: ScanHandle,
    last_flush: Instant,
    batch: Vec<(SessionKey, SessionHits)>,
    progress: Progress,
    done: bool,
}

impl ContentDriver {
    /// Start a requested scan, stop one the app no longer wants, and
    /// deliver what the running one has found.
    pub(super) fn pump(&mut self, app: &mut App) {
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
        while let Some(event) = active.handle.try_recv() {
            if event.generation() != active.generation {
                continue;
            }
            match event {
                ScanEvent::Progress { progress, .. } => active.progress = progress,
                ScanEvent::Session { key, hits, .. } => active.batch.push((key, hits)),
                ScanEvent::Done { progress, .. } => {
                    active.progress = progress;
                    active.done = true;
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
        handle,
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
    }
}
