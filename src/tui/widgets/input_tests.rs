use super::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::testkit::keys::{ctrl, press};

#[test]
fn enter_confirms_with_current_value() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, InputOutcome::Confirm("hello".to_string()));
}

#[test]
fn esc_cancels_without_committing() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(press(KeyCode::Esc));
    assert_eq!(outcome, InputOutcome::Cancel);
    // Cancel must not mutate the buffer.
    assert_eq!(state.value(), "hello");
}

#[test]
fn tab_is_swallowed_while_overlay_is_open() {
    let mut state = TextInputState::new(" rename ", "hello");
    let outcome = state.handle_key(press(KeyCode::Tab));
    assert_eq!(outcome, InputOutcome::Continue);
    assert_eq!(state.value(), "hello");
}

#[test]
fn ctrl_c_aborts_and_esc_cancels() {
    let mut state = TextInputState::new(" rename ", "hello");
    let event = ctrl('c');
    let outcome = state.handle_key(event);
    assert_eq!(outcome, InputOutcome::Abort, "Ctrl-C dismisses the overlay");
    assert_eq!(state.handle_key(press(KeyCode::Esc)), InputOutcome::Cancel);
}

#[test]
fn typing_appends_to_buffer() {
    let mut state = TextInputState::new(" rename ", "abc");
    let outcome = state.handle_key(press(KeyCode::Char('d')));
    assert_eq!(outcome, InputOutcome::Continue);
    assert_eq!(state.value(), "abcd");
}

#[test]
fn backspace_deletes_char_before_cursor() {
    let mut state = TextInputState::new(" rename ", "abc");
    state.handle_key(press(KeyCode::Backspace));
    assert_eq!(state.value(), "ab");
}

#[test]
fn alt_b_and_alt_f_move_by_word() {
    let alt = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
    let mut state = TextInputState::new(" rename ", "alpha beta-gamma");
    assert_eq!(state.handle_key(alt('b')), InputOutcome::Continue);
    assert_eq!(state.cursor(), 11, "back to the start of `gamma`");
    state.handle_key(alt('b'));
    assert_eq!(state.cursor(), 6, "punctuation separates words");
    state.handle_key(alt('f'));
    assert_eq!(state.cursor(), 11);
    assert_eq!(state.value(), "alpha beta-gamma", "motion never types");
}

#[test]
fn a_paste_flattens_to_one_line() {
    assert_eq!(single_line_paste("one\ntwo"), "one two");
    assert_eq!(single_line_paste("one\r\ntwo\r\n"), "one two");
    assert_eq!(single_line_paste("\n\none\n\n\ntwo\n"), "one two");
    assert_eq!(single_line_paste("a\tb"), "a b");
    assert_eq!(single_line_paste("a\u{1b}[31mb"), "a[31mb");
    assert_eq!(single_line_paste("  keep  spaces  "), "  keep  spaces  ");
    assert_eq!(single_line_paste("~/src/über"), "~/src/über");
}

#[test]
fn enter_returns_trimmed_or_raw_value_per_caller_choice() {
    // The widget never trims itself — callers decide how to
    // treat whitespace. Round-trip an all-whitespace buffer to
    // prove the widget keeps it intact.
    let mut state = TextInputState::new(" rename ", "   ");
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, InputOutcome::Confirm("   ".to_string()));
}

/// The prompt's frame on `area`: one row of text and its border.
fn prompt_rect(area: Rect) -> Rect {
    let theme = Theme::default();
    ModalFrame::new(&theme, PROMPT_SIZE).rect(area, 1)
}

#[test]
fn centered_modal_caps_width_at_60() {
    let modal = prompt_rect(Rect::new(0, 0, 120, 30));
    assert_eq!(modal.width, 60);
    assert_eq!(modal.height, 3);
    assert!(modal.x > 0);
    assert!(modal.y > 0);
}

#[test]
fn centered_modal_scales_down_on_narrow_terminal() {
    let modal = prompt_rect(Rect::new(0, 0, 30, 12));
    assert_eq!(modal.width, 26); // 30 - 4
    assert_eq!(modal.height, 3);
}

#[test]
fn centered_modal_floors_width_at_20_within_the_terminal() {
    assert_eq!(prompt_rect(Rect::new(0, 0, 22, 6)).width, 20);
    assert_eq!(prompt_rect(Rect::new(0, 0, 18, 6)).width, 18);
}

#[test]
fn visible_window_keeps_cursor_in_view_for_long_values() {
    let value: String = (b'a'..=b'z').map(|b| b as char).collect();
    let cursor = value.chars().count();
    let window = visible_window(&value, cursor, 10);
    assert!(window.text.len() <= 10);
    assert!(window.cursor_offset < window.text.len() + 1);
}
