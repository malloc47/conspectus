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
//!   freshness slot uses [`format_recency`] over
//!   `App::loaded_at_epoch`.
//! - Sessions use spare horizontal space after the mux indicator
//!   for a dim one-line last-message preview.
//! - Body switches from side-by-side to a vertical stack when the
//!   terminal is narrower than ~100 columns.
//! - Muxed-session right-panel preview shows the
//!   `--no-live-preview` banner when live extras are suppressed,
//!   while inline tree previews remain (locked decision).
//! - Empty/loading frames render minimal copy when no row tree is
//!   loaded yet.

use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::macros::{horizontal, span, vertical};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use tuikit::glyphs::marks;
use tuikit::scroll::{ScrollGutter, ScrollWindow, WrappedRows};
use tuikit::text;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::SessionsGrouping;
use crate::tui::Theme;
use crate::tui::View;
use crate::tui::actions::{attach_disabled_reason, resolve_attach_target};
use crate::tui::app::{App, Focus, SnapshotHandle, ViewportReport};
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, MuxSessionRow, PrRow, PrStatus, RowId, RowKind,
    format_recency, recency_bucket,
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

/// Render one frame and report the window each scrolled surface drew.
///
/// The tree, the explorer, the Preview, and the scrolling overlays (help,
/// the value modal, Messages) each work their window out from the App
/// (the window kept from the last frame and the cursor) and the area the
/// layout gave them, draw it, and return it. The caller hands the
/// [`ViewportReport`] to [`App::keep_viewports`] after the frame, so the
/// next page key moves by the rows on screen and an offset past the end
/// stays taken back. Nothing is dispatched from inside the draw.
///
/// `app` is borrowed mutably for one thing: the right pane warms the
/// transcript preview's wrapped lines for the width it measured
/// (`App::prepare_preview`). Every other draw helper takes `&App`.
pub fn draw(app: &mut App, frame: &mut Frame<'_>) -> ViewportReport {
    let area = frame.area();
    let layout = vertical![==1, >=3, ==1].split(area);

    draw_header(app, frame, layout[0]);
    let mut report = draw_body(app, frame, layout[1]);
    draw_status_bar(app, frame, layout[2]);
    draw_controls_overlay(app, frame, area);
    draw_search_overlay(app, frame, area);
    report.help = draw_help_overlay(app, frame, area);
    draw_rename_overlay(app, frame, area);
    draw_worktree_menu(app, frame, area);
    draw_new_mux_form(app, frame, area);
    draw_mux_menu(app, frame, area);
    draw_row_menu(app, frame, area);
    draw_clean_up(app, frame, area);
    draw_picker(app, frame, area);
    draw_mux_launch_form(app, frame, area);
    draw_pin_edit_form(app, frame, area);
    report.value = draw_value_modal(app, frame, area);
    report.messages = draw_messages_overlay(app, frame, area);
    draw_toast(app, frame, area);
    report
}

fn draw_mux_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::mux_menu::MuxMenuWidget;

    let Some(state) = app.mux_menu() else {
        return;
    };
    frame.render_widget(MuxMenuWidget::new(state, app.theme()), area);
}

fn draw_row_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::row_menu::RowMenuWidget;

    let Some(state) = app.row_menu() else {
        return;
    };
    frame.render_widget(RowMenuWidget::new(state, app.theme()), area);
}

fn draw_picker(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::picker::PickerWidget;

    let Some(state) = app.picker() else {
        return;
    };
    frame.render_widget(PickerWidget::new(state, app.theme()), area);
}

fn draw_clean_up(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::clean_up::CleanUpWidget;

    let Some(state) = app.clean_up() else {
        return;
    };
    frame.render_widget(CleanUpWidget::new(state, app.theme()), area);
}

fn draw_mux_launch_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::mux_launch::MuxLaunchFormWidget;

    let Some(state) = app.mux_launch_form() else {
        return;
    };
    frame.render_widget(MuxLaunchFormWidget::new(state, app.theme()), area);
}

fn draw_pin_edit_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::pin_edit::PinEditFormWidget;

    let Some(state) = app.pin_edit_form() else {
        return;
    };
    frame.render_widget(PinEditFormWidget::new(state, app.theme()), area);
}

fn draw_worktree_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::worktree_menu::WorktreeMenuWidget;

    let Some(state) = app.worktree_menu() else {
        return;
    };
    frame.render_widget(WorktreeMenuWidget::new(state, app.theme()), area);
}

fn draw_new_mux_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::new_mux::NewMuxFormWidget;

    let Some(state) = app.new_mux_form() else {
        return;
    };
    frame.render_widget(NewMuxFormWidget::new(state, app.theme()), area);
}

fn draw_toast(app: &App, frame: &mut Frame<'_>, area: Rect) {
    if !app.toast().has_toast() {
        return;
    }
    // The upstream engine impls `Widget for &ToastEngine`, so we
    // render through the shared borrow without any wrapper widget.
    // `set_area` + `tick` already ran via `prepare_toast_for_render`
    // before `terminal.draw` was called.
    frame.render_widget(app.toast(), area);
}

fn draw_value_modal(app: &App, frame: &mut Frame<'_>, area: Rect) -> Option<ScrollWindow> {
    use crate::tui::widgets::value_modal::ValueModalWidget;

    let state = app.value_modal()?;
    Some(ValueModalWidget::new(state, app.theme()).draw(area, frame.buffer_mut()))
}

fn draw_messages_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) -> Option<ScrollWindow> {
    use crate::tui::widgets::messages::MessagesWidget;

    let state = app.messages_overlay()?;
    let widget = MessagesWidget::new(
        state,
        app.messages(),
        app.theme(),
        crate::discovery::current_epoch(),
    );
    Some(widget.draw(area, frame.buffer_mut()))
}

fn draw_help_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) -> Option<ScrollWindow> {
    use crate::tui::widgets::help::HelpOverlayWidget;

    let state = app.help_overlay()?;
    Some(HelpOverlayWidget::new(state, app.theme()).draw(area, frame.buffer_mut()))
}

fn draw_search_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::search::items_from_rows;
    use crate::tui::widgets::search::{Reach, SearchMode, SearchOverlayWidget};

    let Some(state) = app.search_overlay() else {
        return;
    };
    // Recompute items from the live row tree each frame so the
    // overlay's label lookup never lags behind a refresh. Building
    // them copies a few strings per row, so only the scope that lists
    // rows pays for it.
    let rows_scope = state.mode() == SearchMode::Quick && state.reach() == Reach::Current;
    let items = if rows_scope {
        items_from_rows(&app.tree().rows)
    } else {
        Vec::new()
    };
    let view = (state.mode() == SearchMode::Content).then(|| app.content_view());
    // Outcome words for the wider reach: one walk out from the focus,
    // then a lookup per row the list actually draws.
    let reach = (state.reach() == Reach::All && state.mode() == SearchMode::Quick)
        .then(|| app.search_reachability())
        .flatten();
    let outcome = |node: &crate::model::NodeId| app.open_plan(reach.as_ref(), node).word();
    let mut widget = SearchOverlayWidget::new(state, &items, app.theme()).with_outcomes(&outcome);
    if let Some(view) = &view {
        widget = widget.with_content(view);
    }
    frame.render_widget(widget, area);
}

fn draw_rename_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::input::TextInputWidget;

    let Some(state) = app.rename_overlay() else {
        return;
    };
    let widget = TextInputWidget::new(state).theme(app.theme());
    frame.render_widget(widget, area);
}

fn draw_controls_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::controls::ControlsOverlayWidget;

    let Some(state) = app.controls_overlay() else {
        return;
    };
    let widget = ControlsOverlayWidget::new(state, app.controls_context(), app.theme());
    frame.render_widget(widget, area);
}

// Render clock: wall time in production, a settable clock in tests.

#[cfg(not(test))]
fn current_unix_epoch_for_render() -> i64 {
    // The clock the row builders age against, so an age's text and its
    // recency color agree; snapshot mode freezes it.
    crate::tui::rows::tree_current_unix_epoch()
}

/// Test override: a fixed clock so snapshot tests stay
/// deterministic without monkey-patching the system clock.
#[cfg(test)]
fn current_unix_epoch_for_render() -> i64 {
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
        ..ViewportReport::default()
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
/// terminal area. Exposed for snapshot tests so they can assert on
/// the rendered shape without a real TTY.
#[cfg(test)]
pub fn render_to_buffer(app: &mut App, area: Rect) -> ratatui::buffer::Buffer {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("test terminal");
    terminal
        .draw(|frame| {
            let report = draw(app, frame);
            app.keep_viewports(report);
        })
        .expect("draw on test backend");
    terminal.backend().buffer().clone()
}

/// Convert a buffer to a newline-joined string. Used by snapshot
/// tests; styling is dropped — we only assert on the characters
/// the operator would see.
#[cfg(test)]
pub fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let width = buffer.area.width as usize;
    let mut out = String::with_capacity((width + 1) * buffer.area.height as usize);
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
