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
use ratatui::layout::Rect;
use ratatui::macros::{horizontal, span, vertical};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use tuikit::age::Age;
use tuikit::chip::{StatusChip, Tone};
use tuikit::glyphs::marks;
use tuikit::scroll::{Anchor, ScrollGutter, ScrollRegion, ScrollWindow, WrappedRows, rows_under};
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
/// The header, the panes, and the status bar draw first, unless an open
/// modal fills the screen (a full-screen viewer), which hides them. Every
/// open modal then draws over them, bottom to top (ADR 0161): each clears
/// what it covers, so a modal opened over another shows the lower one
/// around its frame. A toast draws last, over everything.
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
    let mut report = ViewportReport::default();
    if !app.modals_fill_screen() {
        let layout = vertical![==1, >=3, ==1].split(area);
        draw_header(app, frame, layout[0]);
        report = draw_body(app, frame, layout[1]);
        draw_status_bar(app, frame, layout[2]);
    }
    app.draw_modals(area, frame.buffer_mut());
    draw_toast(app, frame, area);
    report
}

/// The toast on screen, over everything else. An expired one was
/// retired by `prepare_toast_for_render` before the draw.
fn draw_toast(app: &App, frame: &mut Frame<'_>, area: Rect) {
    app.toasts().render(area, frame.buffer_mut(), app.theme());
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

fn draw_body(app: &mut App, frame: &mut Frame<'_>, area: Rect) -> ViewportReport {
    let threshold = app.config().narrow_layout_threshold;
    let split = if area.width < threshold {
        vertical![==50%, ==50%].split(area)
    } else {
        horizontal![==50%, ==50%].split(area)
    };
    let tree = draw_left_panel(app, frame, split[0]);
    let (explorer, preview) = draw_right_panel(app, frame, split[1]);
    ViewportReport {
        tree,
        explorer,
        preview,
    }
}

fn focus_marker_width(app: &App, focus: Focus) -> usize {
    text::width(&focus_marker_span(app, focus).content)
}

/// `► ` when the given panel has focus, two-space pad otherwise.
/// Keeps title widths consistent across focus states so the tab
/// strip and right-pane label sit at the same offset before and
/// after `Tab`.
fn focus_marker_span(app: &App, panel: Focus) -> Span<'static> {
    let focused = app.focus() == panel;
    let marker = if focused { marks::POINTER } else { " " };
    span!(Style::default().fg(app.theme().panel_focus_accent); "{marker} ")
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
