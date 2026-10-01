use super::*;
use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn enter_confirms_with_current_value() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, InputOutcome::Confirm("hello".to_string()));
}

#[test]
fn esc_cancels_without_committing() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(key(KeyCode::Esc));
    assert_eq!(outcome, InputOutcome::Cancel);
    // Cancel must not mutate the buffer.
    assert_eq!(state.value(), "hello");
}

#[test]
fn tab_is_swallowed_while_overlay_is_open() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(key(KeyCode::Tab));
    assert_eq!(outcome, InputOutcome::Continue);
    assert_eq!(state.value(), "hello");
}

#[test]
fn ctrl_c_cancels() {
    let mut state = TextInputState::new(" rename ", "hello");
    let event = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let outcome = state.handle_key(event);
    assert_eq!(outcome, InputOutcome::Cancel);
}

#[test]
fn typing_appends_to_buffer() {
    let mut state = TextInputState::new(" rename ", "abc");
    let outcome = state.handle_key(key(KeyCode::Char('d')));
    assert_eq!(outcome, InputOutcome::Continue);
    assert_eq!(state.value(), "abcd");
}

#[test]
fn backspace_deletes_char_before_cursor() {
    let mut state = TextInputState::new(" rename ", "abc");
    state.handle_key(key(KeyCode::Backspace));
    assert_eq!(state.value(), "ab");
}

#[test]
fn enter_returns_trimmed_or_raw_value_per_caller_choice() {
    // The widget never trims itself — callers decide how to
    // treat whitespace. Round-trip an all-whitespace buffer to
    // prove the widget keeps it intact.
    let mut state = TextInputState::new(" rename ", "   ");
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, InputOutcome::Confirm("   ".to_string()));
}

#[test]
fn centered_modal_caps_width_at_60() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 30,
    };
    let modal = centered_modal_rect(area);
    assert_eq!(modal.width, 60);
    assert_eq!(modal.height, 3);
    assert!(modal.x > 0);
    assert!(modal.y > 0);
}

#[test]
fn centered_modal_scales_down_on_narrow_terminal() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 12,
    };
    let modal = centered_modal_rect(area);
    assert_eq!(modal.width, 26); // 30 - 4
    assert_eq!(modal.height, 3);
}

#[test]
fn centered_modal_floors_width_at_20() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 18,
        height: 6,
    };
    let modal = centered_modal_rect(area);
    assert_eq!(modal.width, 20);
}

#[test]
fn visible_window_keeps_cursor_in_view_for_long_values() {
    let value: String = (b'a'..=b'z').map(|b| b as char).collect();
    let cursor = value.chars().count();
    let window = visible_window(&value, cursor, 10);
    assert!(window.text.len() <= 10);
    assert!(window.cursor_offset < window.text.len() + 1);
}
