//! Live state on the rows between snapshots (ADR 0205).
//!
//! The App holds the live overlay; the runtime drives it, since it reads
//! the store and `/proc`: [`App::rebind_live`] before each new snapshot's
//! rows are built, [`App::poll_live`] once a tick. When the overlay's
//! table changes, or time alone moves a state, the rows already built
//! take their new live values in place. Nothing else about a row depends
//! on its live state, so the tree is not rebuilt, and the selection and
//! folds stay as they are.

use super::*;
use crate::tui::live_overlay::LiveOverlay;

impl App {
    /// Read live state through `overlay` from now on: the live runtime's
    /// overlay over the store. Until it is set, rows read each
    /// snapshot's own table.
    pub(crate) fn set_live_overlay(&mut self, overlay: LiveOverlay) {
        self.live = overlay;
    }

    /// The live table the rows of `snapshot` read.
    pub(crate) fn live_table<'a>(
        &'a self,
        snapshot: &'a GraphSnapshot,
    ) -> &'a BTreeMap<NodeId, crate::model::live::LiveBinding> {
        self.live.table(snapshot)
    }

    /// Bind the overlay to `snapshot` before its rows are built and it
    /// replaces the snapshot on screen.
    pub(crate) fn rebind_live(&mut self, snapshot: &GraphSnapshot, now: Instant) {
        self.live.rebind(snapshot, now);
    }

    /// One tick of the overlay at `now` (`now_ms` by the wall clock):
    /// pick up a rewritten store, or a state time alone moved, and fill
    /// the rows on screen with it.
    pub(crate) fn poll_live(&mut self, now: Instant, now_ms: i64) {
        let snapshot = self.handle.as_ref().map(SnapshotHandle::snapshot);
        let rebound = self.live.poll(now, snapshot);
        let aged = snapshot.is_some_and(|snapshot| self.live.aged(snapshot, now, now_ms));
        if rebound || aged {
            self.refill_live(now_ms);
        }
    }

    /// Give the rows on screen their live values at `now_ms`.
    fn refill_live(&mut self, now_ms: i64) {
        let Some(handle) = &self.handle else {
            return;
        };
        let snapshot = handle.snapshot();
        let values =
            crate::tui::rows::live::LiveValues::at(snapshot, self.live.table(snapshot), now_ms);
        crate::tui::rows::live::annotate(&mut self.tree, snapshot, &values);
    }
}
