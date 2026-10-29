use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn menu() -> RowMenuState {
    RowMenuState::new(
        "Record",
        vec![
            RowMenuEntry::new("Rebuild", "bring it back", Msg::SetStatus(None)),
            RowMenuEntry::new(
                "Forget",
                "remove the record",
                Msg::SetStatus(Some(tuikit::feedback::Feedback::info("x"))),
            )
            .confirming(),
        ],
    )
}

#[test]
fn picking_a_plain_entry_commits_its_message() {
    let mut state = menu();
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Commit(Msg::SetStatus(None)));
}

#[test]
fn a_confirming_entry_needs_a_second_enter() {
    let mut state = menu();
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.handle_key(key(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(state.armed(), Some(1));
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Commit(Msg::SetStatus(Some(tuikit::feedback::Feedback::info("x"))))
    );
}

#[test]
fn moving_disarms_a_pending_confirmation() {
    let mut state = menu();
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Enter));
    state.handle_key(key(KeyCode::Up));
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.armed(), None);
    assert_eq!(state.handle_key(key(KeyCode::Enter)), Outcome::Consumed);
}

#[test]
fn escape_closes() {
    let mut state = menu();
    assert_eq!(state.handle_key(key(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_and_ctrl_c_dismiss_even_an_armed_entry() {
    // Nothing is typed in a menu, so q dismisses it like Ctrl-C.
    for dismiss in [
        key(KeyCode::Char('q')),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        let mut state = menu();
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Enter));
        assert_eq!(state.armed(), Some(1));
        assert_eq!(state.handle_key(dismiss), Outcome::Close);
    }
}

#[test]
fn the_cursor_wraps_at_both_ends() {
    let mut state = menu();
    state.handle_key(key(KeyCode::Up));
    assert_eq!(state.cursor(), 1, "up from the first entry is the last");
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 0, "down from the last entry is the first");
}

/// The menu drawn on an 80x20 screen.
fn drawn(state: &RowMenuState, theme: &Theme) -> Buffer {
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    RowMenuWidget::new(state, theme).render(area, &mut buf);
    buf
}

/// The rows that hold the pointer mark, with the cell it is drawn in.
fn marked(buf: &Buffer) -> Vec<(u16, u16)> {
    let area = buf.area;
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|(x, y)| buf[(*x, *y)].symbol() == tuikit::glyphs::marks::POINTER)
        .collect()
}

#[test]
fn the_cursor_entry_shows_the_marker_and_the_highlight() {
    let theme = Theme::default();
    let mut state = menu();
    state.handle_key(key(KeyCode::Down));
    let buf = drawn(&state, &theme);
    let marks = marked(&buf);
    assert_eq!(marks.len(), 1, "one marked row");
    let (x, y) = marks[0];
    let label: String = (x + 2..x + 8).map(|x| buf[(x, y)].symbol()).collect();
    assert_eq!(label, "Forget");
    for x in x..x + 8 {
        assert!(buf[(x, y)].modifier.contains(theme.selection_active));
    }
    // The entry's explanation below is not part of the highlight.
    assert!(
        !buf[(x + 5, y + 1)]
            .modifier
            .contains(theme.selection_active)
    );
}

#[test]
fn an_armed_entry_keeps_the_highlight_in_the_error_color() {
    let theme = Theme::default();
    let mut state = menu();
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Enter));
    let buf = drawn(&state, &theme);
    let (x, y) = marked(&buf)[0];
    let cell = &buf[(x + 2, y)];
    assert_eq!(cell.fg, theme.error);
    assert!(cell.modifier.contains(theme.selection_active));
}
