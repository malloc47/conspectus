// Extracted from multi_select.rs H-HYG-011 rolling wave via #[path = "multi_select_tests.rs"] mod tests;
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

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn cursor_starts_at_zero_and_wraps() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    assert_eq!(state.cursor(), 0);
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 2);
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 0, "down wraps");
    state.handle_key(key(KeyCode::Up));
    assert_eq!(state.cursor(), 2, "up wraps");
}

#[test]
fn vim_keys_move_cursor() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    state.handle_key(key(KeyCode::Char('j')));
    assert_eq!(state.cursor(), 1);
    state.handle_key(key(KeyCode::Char('k')));
    assert_eq!(state.cursor(), 0);
}

#[test]
fn space_toggles_item_under_cursor() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    state.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(state.selected_indices(), vec![0]);
    state.handle_key(key(KeyCode::Char(' ')));
    assert!(state.selected_indices().is_empty());
}

#[test]
fn enter_confirms_with_current_selection() {
    let mut state = MultiSelectState::new("harness", 3, &[1]);
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![1]));
}

#[test]
fn esc_cancels_without_confirming() {
    let mut state = MultiSelectState::new("harness", 3, &[1]);
    let outcome = state.handle_key(key(KeyCode::Esc));
    assert_eq!(outcome, MultiSelectOutcome::Cancel);
    assert_eq!(state.selected_indices(), vec![1]);
}

#[test]
fn ctrl_c_cancels() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    let outcome = state.handle_key(ctrl(KeyCode::Char('c')));
    assert_eq!(outcome, MultiSelectOutcome::Cancel);
}

#[test]
fn initial_selection_round_trips() {
    let state = MultiSelectState::new("harness", 4, &[0, 2]);
    assert!(state.is_selected(0));
    assert!(!state.is_selected(1));
    assert!(state.is_selected(2));
    assert!(!state.is_selected(3));
    assert_eq!(state.selected_indices(), vec![0, 2]);
}

#[test]
fn out_of_range_initial_indices_are_ignored() {
    let state = MultiSelectState::new("harness", 2, &[1, 5, 99]);
    assert_eq!(state.selected_indices(), vec![1]);
}

#[test]
fn empty_list_handles_keys_without_panicking() {
    let mut state = MultiSelectState::new("harness", 0, &[]);
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Up));
    state.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(state.cursor(), 0);
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![]));
}

#[test]
fn unrecognized_keys_are_ignored() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    let outcome = state.handle_key(key(KeyCode::Tab));
    assert_eq!(outcome, MultiSelectOutcome::Continue);
    assert_eq!(state.cursor(), 0);
}

#[test]
fn selected_last_row_scrolls_into_short_multi_select_body() {
    let offset = compute_scroll(9, 4, 10);
    assert_eq!(offset, 6);
    assert!(9 >= offset);
    assert!(9 < offset + 4);
}
