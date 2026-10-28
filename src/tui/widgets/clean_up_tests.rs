use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::*;
use crate::model::HistoryKind;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn candidate(id: &str, label: &str) -> Candidate {
    Candidate {
        kind: HistoryKind::Mux,
        id: id.into(),
        label: label.into(),
        last_seen: 0,
    }
}

fn state() -> CleanUpState {
    CleanUpState::new(
        vec![
            candidate("h_a", "alpha"),
            candidate("h_b", "beta"),
            candidate("h_c", "gamma"),
        ],
        86_400 * 20,
    )
}

#[test]
fn every_candidate_starts_selected_with_its_age() {
    let state = state();
    assert_eq!(state.selected(), [0, 1, 2]);
    // In days, as the History view's age column shows the same record.
    assert_eq!(state.labels()[0], "mux      alpha · last seen 20d ago");
}

#[test]
fn confirming_reaps_exactly_the_selected_records() {
    let mut state = state();
    // Deselect the middle one.
    state.handle_key(key(KeyCode::Down));
    state.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(state.selected(), [0, 2]);
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Commit(Msg::ReapRecords(vec!["h_a".into(), "h_c".into()]))
    );
}

#[test]
fn deselecting_everything_closes_without_reaping() {
    let mut state = CleanUpState::new(vec![candidate("h_a", "alpha")], 100);
    state.handle_key(key(KeyCode::Char(' ')));
    assert!(state.selected().is_empty());
    assert_eq!(state.handle_key(key(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn escape_cancels() {
    let mut state = state();
    assert_eq!(state.handle_key(key(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_dismisses_the_checklist_and_its_empty_notice() {
    let mut state = state();
    assert_eq!(state.handle_key(key(KeyCode::Char('q'))), Outcome::Close);
    let mut state = CleanUpState::new(Vec::new(), 100);
    assert_eq!(state.handle_key(key(KeyCode::Char('q'))), Outcome::Close);
}

#[test]
fn with_no_candidates_enter_and_escape_dismiss_the_notice() {
    let mut state = CleanUpState::new(Vec::new(), 100);
    assert!(state.is_empty());
    assert_eq!(state.handle_key(key(KeyCode::Enter)), Outcome::Close);
    let mut state = CleanUpState::new(Vec::new(), 100);
    assert_eq!(state.handle_key(key(KeyCode::Esc)), Outcome::Close);
    assert_eq!(state.handle_key(key(KeyCode::Char('x'))), Outcome::Consumed);
}
