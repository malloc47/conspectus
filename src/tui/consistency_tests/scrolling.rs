//! Scrolling and paging keep content on screen: a jump to the end shows
//! the last lines, a page key moves by the rows the pane or modal drew,
//! scrolling past the end is taken back at once, and every menu and pager
//! draws on a short terminal.
//!
//! Keys go in the way the live loop sends them: the screen is drawn, then
//! the key is handed to whatever owns it, so a pager knows what it showed
//! when the next key arrives.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::tui::Modal;
use crate::tui::keymap::Action;
use crate::tui::modal::{Overlay, OverlayOutcome};

/// One frame of the whole screen, the full-screen viewers included.
fn screen(app: &mut App, (width, height): (u16, u16)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| crate::tui::runtime::draw_screen(app, frame))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn key_event(code: KeyCode) -> KeyEvent {
    let modifiers = match code {
        KeyCode::Char(c) if c.is_ascii_uppercase() => KeyModifiers::SHIFT,
        _ => KeyModifiers::NONE,
    };
    KeyEvent::new(code, modifiers)
}

/// Hand a key to the top modal's own handler, closing it when it asks.
fn to_top_modal(app: &mut App, key: KeyEvent) {
    let outcome = match app.top_modal_mut() {
        Some(Modal::ValueModal(state)) => state.handle((), key),
        Some(Modal::RowMenu(state)) => state.handle((), key),
        Some(Modal::MuxMenu(state)) => state.handle((), key),
        Some(Modal::WorktreeMenu(state)) => state.handle((), key),
        other => panic!(
            "no handler here for {:?}",
            other.map(|modal| super::layout::name_of(modal))
        ),
    };
    if outcome == OverlayOutcome::Close {
        app.pop_top_modal();
    }
}

/// What the live loop does for one key: draw the screen, then give the
/// key to whatever owns it. Returns the next frame.
fn press(app: &mut App, size: (u16, u16), code: KeyCode) -> Buffer {
    screen(app, size);
    let key = key_event(code);
    match crate::tui::runtime::action_for_event(app, Event::Key(key)) {
        Some(Action::Msg(msg)) => {
            app.update(*msg);
        }
        Some(Action::MessagesOverlayKey(key)) => {
            crate::tui::runtime::handle_messages_overlay_key(app, key);
        }
        Some(Action::HelpOverlayKey(key)) => crate::tui::runtime::handle_help_overlay_key(app, key),
        Some(Action::ViewerOverlayKey(key)) => {
            crate::tui::runtime::handle_viewer_overlay_key(app, key);
        }
        Some(Action::PaneViewerKey(key)) => {
            crate::tui::runtime::handle_pane_viewer_key(app, key);
        }
        Some(
            Action::ValueModalKey(key)
            | Action::RowMenuKey(key)
            | Action::MuxMenuKey(key)
            | Action::WorktreeMenuKey(key),
        ) => to_top_modal(app, key),
        other => panic!("unexpected action for {code:?}: {other:?}"),
    }
    screen(app, size)
}

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn describe(what: &str, (width, height): (u16, u16)) -> String {
    format!("{what} at {width}x{height}")
}

// ---- the tree --------------------------------------------------------------

/// Rows inside the left pane's frame: the first box drawn from the
/// screen's left edge.
fn tree_rows(buf: &Buffer) -> usize {
    let rows = rows_of(buf);
    let top = rows
        .iter()
        .position(|row| row.starts_with('┌'))
        .expect("the left pane's top border");
    // Counted from the row below the top border, the bottom border's
    // index is the number of rows between them.
    rows.iter()
        .skip(top + 1)
        .position(|row| row.starts_with('└'))
        .expect("the left pane's bottom border")
}

fn selected_index(app: &App) -> usize {
    let selected = app.selection().expect("a selection").clone();
    app.visible_rows()
        .iter()
        .position(|row| row.id == selected)
        .expect("the selection is visible")
}

#[test]
fn paging_the_tree_moves_the_selection_by_the_rows_it_shows() {
    let world = world_with(60);
    // Side by side, and stacked under the narrow-layout threshold.
    for size in [(120_u16, 24_u16), (80, 24), (40, 10)] {
        let label = describe("tree", size);
        let mut app = app_over(View::Mux, &world);
        let rows = tree_rows(&screen(&mut app, size));
        let before = selected_index(&app);
        press(&mut app, size, KeyCode::PageDown);
        let after = selected_index(&app);
        assert_eq!(after - before, rows, "{label}: PageDown moves a page");
        press(&mut app, size, KeyCode::PageUp);
        assert_eq!(selected_index(&app), before, "{label}: PageUp moves back");
    }
}

#[test]
fn a_tree_that_shrinks_leaves_no_blank_rows_below_it() {
    let world = world_with(60);
    let size = (120, 24);
    let mut app = app_over(View::Pinned, &world);
    // Scroll to the end, then step up onto the last group row, which
    // holds the rows below it, and collapse it.
    press(&mut app, size, KeyCode::Char('G'));
    let total = app.visible_rows().len();
    let group = app
        .visible_rows()
        .iter()
        .rposition(|row| row.expandable)
        .expect("an expandable row");
    for _ in group..total - 1 {
        press(&mut app, size, KeyCode::Char('k'));
    }
    assert_eq!(selected_index(&app), group);
    let collapsed = press(&mut app, size, KeyCode::Char('h'));
    let shown = app.visible_rows().len();
    assert!(shown < total, "collapsing hid rows ({total} -> {shown})");
    let rows = tree_rows(&collapsed);
    let expected_scroll = shown.saturating_sub(rows);
    assert!(
        usize::from(app.left_scroll()) <= expected_scroll,
        "the tree scrolled past its end (offset {}, {shown} rows in {rows})\n{}",
        app.left_scroll(),
        buffer_to_string(&collapsed)
    );
}
