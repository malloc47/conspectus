//! When the selection's preview loads, and the workers that load it.
//!
//! A cursor moving through rows loads nothing for the rows it passes: a
//! load starts once the cursor has rested on its row for [`REST`]
//! ([`PreviewRest`], ADR 0237). A load that is still running when the
//! cursor leaves its row stops starting subprocesses ([`PreviewLoads`]).
//! Every load, a mux pane's capture included, runs on a worker thread and
//! reaches the reducer through [`PreviewLoads::drain`], so nothing the
//! Preview reads can hold up a key or a frame (ADR 0024).

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ratatui::style::Modifier;

use crate::content_search::CancelToken;
use crate::discovery::tmux::MuxBackend;
use crate::model::MuxSessionId;
use crate::tui::RunConfig;
use crate::tui::app::{App, Msg};
use crate::tui::preview::{
    PreviewContent, PreviewSource, Stop, Stopped, capture_via, load_offline_preview_unless,
    pane_native_id, preview_source,
};

/// How long the cursor stays on a row before its preview loads. Short
/// enough that a pause shows the preview within about a tenth of a
/// second, long enough that a held key (about 30 a second) or a scroll
/// never lets a load start for the rows it passes.
pub(super) const REST: Duration = Duration::from_millis(100);

/// Where the cursor is, for deciding when a preview may load.
#[derive(Debug, Default)]
pub(super) struct PreviewRest {
    /// The source the last pass that was allowed to load resolved: what
    /// the selection last showed, which a selection that moved onto
    /// another source loads again for (ADR 0112).
    shown: Option<PreviewSource>,
    /// The source the cursor is on.
    cursor: Option<PreviewSource>,
    /// When input moved the cursor onto `cursor`, until it has rested
    /// there. A source that changes with no input behind it, such as a
    /// snapshot landing, is not a cursor passing through and has none.
    moved_at: Option<Instant>,
}

impl PreviewRest {
    /// One pass of the loop at `now`: the source to load, if the
    /// selection needs one now. `keyed` says the pass followed input;
    /// `force` says that input asked for a refresh, which loads the
    /// selection whether or not the cursor has rested.
    pub(super) fn pass(
        &mut self,
        app: &App,
        config: &RunConfig,
        now: Instant,
        keyed: bool,
        force: bool,
    ) -> Option<PreviewSource> {
        let current = preview_source(app);
        if current != self.cursor {
            self.cursor = current.clone();
            self.moved_at = keyed.then_some(now);
        }
        if !force && self.moving(now) {
            return None;
        }
        self.moved_at = None;
        let plan = super::plan_preview_load(app, config, self.shown.as_ref(), force);
        self.shown = current;
        plan
    }

    /// The source the cursor is on.
    pub(super) fn cursor(&self) -> Option<&PreviewSource> {
        self.cursor.as_ref()
    }

    /// How long until the cursor counts as at rest, while it is moving:
    /// the loop wakes then, not at its next timeout.
    pub(super) fn until_rest(&self, now: Instant) -> Option<Duration> {
        let moved = self.moved_at?;
        Some(REST.saturating_sub(now.saturating_duration_since(moved)))
    }

    fn moving(&self, now: Instant) -> bool {
        self.until_rest(now).is_some_and(|left| !left.is_zero())
    }
}

/// What a preview says when its loader crashed.
const CRASHED: &str = "the preview reader crashed";

/// A finished load, on its way from a worker to the reducer.
enum Landed {
    Loaded {
        source: PreviewSource,
        content: PreviewContent,
    },
    /// The load stopped before it finished.
    Abandoned(PreviewSource),
}

/// What a worker runs to load a source that needs no mux backend.
pub(super) type Loader = Arc<
    dyn Fn(&PreviewSource, Modifier, Stop<'_>) -> Result<Option<PreviewContent>, Stopped>
        + Send
        + Sync,
>;

/// The loads a live loop has running on worker threads, and how their
/// results get back: through [`PreviewLoads::drain`], which hands each to
/// the reducer (ADR 0024).
pub(super) struct PreviewLoads {
    tx: mpsc::Sender<Landed>,
    rx: mpsc::Receiver<Landed>,
    /// Captures the panes of muxes, through the same backend as every
    /// other mux call.
    tmux: Arc<dyn MuxBackend>,
    loader: Loader,
    /// Each running load's flag, which stops it starting subprocesses.
    running: BTreeMap<PreviewSource, CancelToken>,
    /// The panes with a capture in flight. A capture is one `tmux` call,
    /// so it is never stopped, and a pane has at most one at a time.
    capturing: BTreeSet<MuxSessionId>,
}

impl PreviewLoads {
    pub(super) fn new(tmux: Arc<dyn MuxBackend>) -> Self {
        Self::with_loader(tmux, Arc::new(load_offline_preview_unless))
    }

    /// Loads that run through `loader`, which tests hold still.
    pub(super) fn with_loader(tmux: Arc<dyn MuxBackend>, loader: Loader) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            tmux,
            loader,
            running: BTreeMap::new(),
            capturing: BTreeSet::new(),
        }
    }

    /// Start loading `source` on a worker thread. A loader panic still
    /// ends the load, or the preview would wait on it forever and `r`
    /// couldn't retry.
    pub(super) fn start(&mut self, app: &mut App, source: PreviewSource) {
        match source {
            PreviewSource::Pane(mux) => self.capture(app, mux),
            source => self.read(app, source),
        }
    }

    /// Capture the pane of `mux` on a worker thread. A mux with no pane
    /// to capture answers so at once, with no `tmux` call; a pane already
    /// being captured is left to that capture.
    fn capture(&mut self, app: &mut App, mux: MuxSessionId) {
        let Some(native_id) = pane_native_id(app, &mux) else {
            app.update(Msg::SetPreview {
                source: PreviewSource::Pane(mux),
                content: PreviewContent::NoTarget,
            });
            return;
        };
        if !self.capturing.insert(mux.clone()) {
            return;
        }
        let (tx, tmux) = (self.tx.clone(), Arc::clone(&self.tmux));
        std::thread::spawn(move || {
            let content = catch_unwind(AssertUnwindSafe(|| capture_via(tmux.as_ref(), &native_id)))
                .unwrap_or_else(|_| PreviewContent::Failed(CRASHED.to_string()));
            // The receiver is gone only when the TUI is exiting.
            let _ = tx.send(Landed::Loaded {
                source: PreviewSource::Pane(mux),
                content,
            });
        });
    }

    /// Read `source`, a transcript, listing, file, git query, process
    /// tree, or pull request, on a worker thread.
    fn read(&mut self, app: &App, source: PreviewSource) {
        let cancel = CancelToken::new();
        self.running.insert(source.clone(), cancel.clone());
        let (tx, loader, muted) = (self.tx.clone(), Arc::clone(&self.loader), app.theme().muted);
        std::thread::spawn(move || {
            let stop = || cancel.is_cancelled();
            let landed = match catch_unwind(AssertUnwindSafe(|| loader(&source, muted, &stop))) {
                Ok(Ok(Some(content))) => Landed::Loaded { source, content },
                Ok(Ok(None)) | Err(_) => Landed::Loaded {
                    source,
                    content: PreviewContent::Failed(CRASHED.to_string()),
                },
                Ok(Err(Stopped)) => Landed::Abandoned(source),
            };
            // The receiver is gone only when the TUI is exiting.
            let _ = tx.send(landed);
        });
    }

    /// The cursor is on `cursor`: every other load still running stops
    /// before it starts another subprocess.
    pub(super) fn keep_only(&self, cursor: Option<&PreviewSource>) {
        for (source, cancel) in &self.running {
            if Some(source) != cursor {
                cancel.cancel();
            }
        }
    }

    /// Hand the reducer what the workers have finished.
    pub(super) fn drain(&mut self, app: &mut App) {
        while let Ok(landed) = self.rx.try_recv() {
            match landed {
                Landed::Loaded { source, content } => {
                    self.running.remove(&source);
                    if let PreviewSource::Pane(mux) = &source {
                        self.capturing.remove(mux);
                    }
                    app.update(Msg::SetPreview { source, content });
                }
                Landed::Abandoned(source) => {
                    self.running.remove(&source);
                    app.update(Msg::PreviewLoadAbandoned(source));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "previews_tests.rs"]
mod tests;
