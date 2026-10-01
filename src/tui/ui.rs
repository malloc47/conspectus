//! Ratatui draw.
//!
//! v1 render per `docs/tui-sessions-mockup.md`: a single status-bar
//! footer, a header line, and a two-panel body with the row tree on
//! the left and the selected node's detail + preview on the right.
//!
//! Locked decisions reflected here:
//!
//! - `~`-shortened paths are produced upstream in the row tree and
//!   detail view-models; the renderer just lays them out.
//! - Mux glyphs `◉` / `◐` / `◯` with color carrying the primary
//!   signal (green attached / yellow ambiguous / dim un-muxed).
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
use ratatui::layout::{Margin, Rect};
use ratatui::macros::{horizontal, span, vertical};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use unicode_width::UnicodeWidthStr;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::Msg;
use crate::tui::SessionsGrouping;
use crate::tui::Theme;
use crate::tui::View;
use crate::tui::actions::{attach_disabled_reason, resolve_attach_target, target_label};
use crate::tui::app::{App, Focus, SnapshotHandle};
use crate::tui::detail::{HeaderField, NodeDetail, SectionKind};
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxCandidateRow, MuxIndicator, MuxSessionRow, PrRow, RowId, RowKind,
    format_recency, recency_bucket,
};

mod detail_header;
mod header;
mod left_panel;
mod preview_pane;
mod right_panel;
mod text;

use detail_header::*;
use header::*;
use left_panel::*;
use preview_pane::*;
use right_panel::*;
use text::*;

/// Render one frame.
///
/// Takes `&mut App` because the draw path dispatches
/// [`Msg::LeftViewportChanged`] / [`Msg::ExplorerViewportChanged`]
/// mid-frame so the reducer can reconcile scroll offsets. The
/// reconciliation math itself lives in the
/// reducer; draw only measures viewport height + the post-wrap
/// explorer cursor row span and dispatches those measurements as
/// Msgs. A strict `&App → buffer` shape would require restructuring
/// draw into separate measure + render passes so the runtime can
/// dispatch the Msgs upstream — that's an ADR 0085 contract-5
/// optional cleanup, not a correctness need. The mutation surface
/// today is bounded to those two Msg dispatches; every subsequent
/// draw helper (`draw_header` / `draw_status_bar` / overlays /
/// toast) takes `&App` and is byte-identical over the same App
/// state.
pub fn draw(app: &mut App, frame: &mut Frame<'_>) {
    let area = frame.area();
    let layout = vertical![==1, >=3, ==1].split(area);

    draw_header(app, frame, layout[0]);
    draw_body(app, frame, layout[1]);
    draw_status_bar(app, frame, layout[2]);
    draw_controls_overlay(app, frame, area);
    draw_pins_overlay(app, frame, area);
    draw_search_overlay(app, frame, area);
    draw_help_overlay(app, frame, area);
    draw_rename_overlay(app, frame, area);
    draw_worktree_menu(app, frame, area);
    draw_new_mux_form(app, frame, area);
    draw_mux_menu(app, frame, area);
    draw_mux_launch_form(app, frame, area);
    draw_value_modal(app, frame, area);
    draw_toast(app, frame, area);
}

fn draw_mux_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::mux_menu::MuxMenuWidget;

    let Some(state) = app.mux_menu() else {
        return;
    };
    frame.render_widget(MuxMenuWidget::new(state, app.theme()), area);
}

fn draw_mux_launch_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::mux_launch::MuxLaunchFormWidget;

    let Some(state) = app.mux_launch_form() else {
        return;
    };
    frame.render_widget(MuxLaunchFormWidget::new(state, app.theme()), area);
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

fn draw_value_modal(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::value_modal::ValueModalWidget;

    let Some(state) = app.value_modal() else {
        return;
    };
    frame.render_widget(ValueModalWidget::new(state, app.theme()), area);
}

fn draw_help_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::help::HelpOverlayWidget;

    let Some(state) = app.help_overlay() else {
        return;
    };
    frame.render_widget(HelpOverlayWidget::new(state, app.theme()), area);
}

fn draw_search_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::search::items_from_rows;
    use crate::tui::widgets::search::SearchOverlayWidget;

    let Some(state) = app.search_overlay() else {
        return;
    };
    // Recompute items from the live visible row tree each frame so
    // the search overlay's label lookup never lags behind a
    // refresh. The trade is cheap (visible_rows is already
    // materialized; items_from_rows just clones a few strings per
    // row).
    let visible: Vec<crate::tui::rows::Row> = app.visible_rows().into_iter().cloned().collect();
    let items = items_from_rows(&visible);
    let widget = SearchOverlayWidget::new(state, &items, app.theme());
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

fn draw_pins_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    use crate::tui::widgets::pins::PinsOverlayWidget;

    let Some(state) = app.pins_overlay() else {
        return;
    };
    let widget = PinsOverlayWidget::new(state, app.theme());
    frame.render_widget(widget, area);
}

// Render clock: wall time in production, a settable clock in tests.

#[cfg(not(test))]
fn current_unix_epoch_for_render() -> i64 {
    crate::discovery::current_epoch()
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

fn draw_body(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let threshold = app.config().narrow_layout_threshold;
    let split = if area.width < threshold {
        vertical![==50%, ==50%].split(area)
    } else {
        horizontal![==50%, ==50%].split(area)
    };
    draw_left_panel(app, frame, split[0]);
    draw_right_panel(app, frame, split[1]);
}

fn focus_marker_width(app: &App, focus: Focus) -> usize {
    focus_marker_span(app, focus).content.chars().count()
}

/// `▸ ` when the given panel has focus, two-space pad otherwise.
/// Keeps title widths consistent across focus states so the tab
/// strip and right-pane label sit at the same offset before and
/// after `Tab`.
fn focus_marker_span(app: &App, panel: Focus) -> Span<'static> {
    let focused = app.focus() == panel;
    let glyph = if focused { "▸ " } else { "  " };
    span!(Style::default().fg(app.theme().panel_focus_accent); "{glyph}")
}

/// Sum of post-wrap terminal rows the given lines occupy when
/// rendered into a paragraph `width` wide. Mirrors the per-line
/// count used by the explorer header budget — used here to drive
/// scrollbar `content_length` (ADR 0076).
fn wrapped_line_count(lines: &[Line<'_>], width: u16) -> usize {
    let pane_width = width.max(1) as usize;
    lines
        .iter()
        .map(|line| {
            let line_width = line
                .spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum::<usize>()
                .max(1);
            line_width.div_ceil(pane_width).max(1)
        })
        .sum()
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
        .draw(|frame| draw(app, frame))
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
