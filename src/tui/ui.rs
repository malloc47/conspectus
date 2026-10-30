//! Ratatui draw.
//!
//! v1 render per `docs/tui-sessions-mockup.md`: a single status-bar
//! footer, a header line, and a two-panel body with the row tree on
//! the left and the selected node's detail + preview on the right.
//!
//! Locked decisions reflected here:
//!
//! - `~`-shortened paths are produced upstream in the row tree and
//!   explorer view model; the renderer just lays them out.
//! - Mux glyphs `◉` / `◯` with color carrying the primary signal
//!   (green attached / dim not attached); mux ambiguity surfaces as
//!   `▲` on group rows and the header.
//! - Header reports `updated Ns ago · N agents · M mux`; the
//!   freshness slot is the age of `App::loaded_at_epoch` on the
//!   framework's ladder (`tuikit::age`).
//! - Sessions use spare horizontal space after the mux indicator
//!   for a dim one-line last-message preview.
//! - Body switches from side-by-side to a vertical stack when the
//!   terminal is narrower than ~100 columns.
//! - Muxed-session right-panel preview shows the
//!   `--no-live-preview` banner when live extras are suppressed,
//!   while inline tree previews remain (locked decision).
//! - Empty/loading frames render minimal copy when no row tree is
//!   loaded yet.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::macros::span;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::age::Age;
use tuikit::app_frame::{AppFrame, Screen};
use tuikit::chip::{StatusChip, Tone};
use tuikit::frame::{PaneFrame, SplitPane};
use tuikit::glyphs::marks;
use tuikit::scroll::{Anchor, ScrollRegion, ScrollWindow, rows_under};
use tuikit::text;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::Theme;
use crate::tui::View;
use crate::tui::actions::{attach_disabled_reason, resolve_attach_target};
use crate::tui::app::{App, Focus, SnapshotHandle, ViewportReport};
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, MuxSessionRow, PrRow, PrStatus, RowId, RowKind,
};

mod compact;
mod header;
mod left_panel;
mod preview_pane;
mod right_panel;

use compact::*;
use header::*;
use left_panel::*;
use preview_pane::*;
use right_panel::*;

/// Render one frame and report the window each scrolled pane drew.
///
/// The screen is the framework's [`AppFrame`]: the header, the panes, and
/// the status bar draw first, unless an open modal fills the screen (a
/// full-screen viewer), which hides them. Every open modal then draws over
/// them, bottom to top (ADR 0161): each clears what it covers, so a modal
/// opened over another shows the lower one around its frame. A toast
/// draws last, over everything.
///
/// The tree, the explorer, and the Preview each work their window out
/// from the App (the window kept from the last frame and the cursor) and
/// the area the layout gave them, draw it, and return it. The caller
/// hands the [`ViewportReport`] to [`App::keep_viewports`] after the
/// frame, so the next page key moves by the rows on screen and an offset
/// past the end stays taken back. A scrolled modal (help, the value
/// modal, Messages) keeps the window it drew itself. Nothing is
/// dispatched from inside the draw.
///
/// `app` is borrowed mutably for two things: the right pane warms the
/// transcript preview's wrapped lines for the width it measured
/// (`App::prepare_preview`), and the modals keep the windows they drew.
pub fn draw(app: &mut App, frame: &mut Frame<'_>) -> ViewportReport {
    let area = frame.area();
    SCREEN.render(area, frame.buffer_mut(), &mut MainScreen(app))
}

/// The main screen's frame: a header row, the panes, and the status bar.
const SCREEN: AppFrame = AppFrame::new();

/// What the main screen draws in each part of [`SCREEN`], over the App.
struct MainScreen<'a>(&'a mut App);

impl Screen for MainScreen<'_> {
    type Report = ViewportReport;

    fn header(&mut self, area: Rect, buf: &mut Buffer) {
        draw_header(self.0, buf, area);
    }

    fn body(&mut self, area: Rect, buf: &mut Buffer) -> ViewportReport {
        draw_body(self.0, buf, area)
    }

    fn footer(&mut self, area: Rect, buf: &mut Buffer) {
        draw_status_bar(self.0, buf, area);
    }

    fn overlays_fill_screen(&self) -> bool {
        self.0.modals_fill_screen()
    }

    fn overlays(&mut self, area: Rect, buf: &mut Buffer) {
        self.0.draw_modals(area, buf);
    }

    /// The toast on screen, over everything else. An expired one was
    /// retired by `prepare_toast_for_render` before the draw.
    fn toasts(&mut self, area: Rect, buf: &mut Buffer) {
        self.0.toasts().render(area, buf, self.0.theme());
    }
}

/// Where the main screen's parts go in `area`: the header row, the two
/// panes, and the status bar.
#[cfg(any(test, feature = "snapshot"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScreenAreas {
    pub header: Rect,
    /// The tree's pane: on the left, or on top when the panes stack.
    pub left: Rect,
    /// The right pane: on the right, or below when the panes stack.
    pub right: Rect,
    pub status: Rect,
}

/// The main screen's layout in `area` with the panes stacking below
/// `narrow_layout_threshold` columns: [`SCREEN`]'s rows and the body's
/// [`SplitPane`], as the draw places them. `--snapshot-pane` and the tests
/// find a pane's cells through this, so nothing keeps a copy of the
/// layout.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn screen_areas(area: Rect, narrow_layout_threshold: u16) -> ScreenAreas {
    let chrome = SCREEN.areas(area);
    let [left, right] = SplitPane::new(narrow_layout_threshold).areas(chrome.body);
    ScreenAreas {
        header: chrome.header,
        left,
        right,
        status: chrome.footer,
    }
}

// Render clock: wall time in production, a settable clock in tests.

#[cfg(not(test))]
pub(crate) fn current_unix_epoch_for_render() -> i64 {
    // The clock the row builders age against, so an age's text and its
    // recency color agree; snapshot mode freezes it.
    crate::tui::rows::tree_current_unix_epoch()
}

/// Test override: a fixed clock so snapshot tests stay
/// deterministic without monkey-patching the system clock.
#[cfg(test)]
pub(crate) fn current_unix_epoch_for_render() -> i64 {
    test_clock::now()
}

#[cfg(test)]
pub(crate) mod test_clock {
    use std::cell::Cell;
    thread_local! {
        static NOW: Cell<i64> = const { Cell::new(0) };
    }

    pub fn set(value: i64) {
        NOW.with(|cell| cell.set(value));
    }

    pub fn now() -> i64 {
        NOW.with(std::cell::Cell::get)
    }
}

// -----------------------------------------------------------------------------
// Body: left tree + right detail
// -----------------------------------------------------------------------------

/// The two panes, side by side or stacked by the configured threshold,
/// each in a [`PaneFrame`] that shows whether it has focus.
fn draw_body(app: &mut App, buf: &mut Buffer, area: Rect) -> ViewportReport {
    let [left, right] = SplitPane::new(app.config().narrow_layout_threshold).areas(area);
    let tree = draw_left_panel(app, buf, left);
    let (explorer, preview) = draw_right_panel(app, buf, right);
    ViewportReport {
        tree,
        explorer,
        preview,
    }
}

// -----------------------------------------------------------------------------
// Snapshot helpers (test-only)
// -----------------------------------------------------------------------------

/// Render the entire UI for `app` into a fresh `Buffer` at the given
/// terminal area, with the framework's one-frame driver. Exposed for
/// snapshot tests so they can assert on the rendered shape without a real
/// TTY.
#[cfg(test)]
pub fn render_to_buffer(app: &mut App, area: Rect) -> ratatui::buffer::Buffer {
    tuikit::testkit::render::render(area.width, area.height, |frame| {
        let report = draw(app, frame);
        app.keep_viewports(report);
    })
}

/// Convert a buffer to a newline-joined string. Used by snapshot
/// tests; styling is dropped — we only assert on the characters
/// the operator would see.
#[cfg(test)]
pub fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    tuikit::testkit::buffer::text(buffer)
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
