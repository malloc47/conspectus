use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::model::HistoryKind;
use tuikit::testkit::keys::press;

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
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.selected(), [0, 2]);
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Commit(Msg::ReapRecords(vec!["h_a".into(), "h_c".into()]))
    );
}

#[test]
fn deselecting_everything_closes_without_reaping() {
    let mut state = CleanUpState::new(vec![candidate("h_a", "alpha")], 100);
    state.handle_key(press(KeyCode::Char(' ')));
    assert!(state.selected().is_empty());
    assert_eq!(state.handle_key(press(KeyCode::Enter)), Outcome::Close);
}

#[test]
fn escape_cancels() {
    let mut state = state();
    assert_eq!(state.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_dismisses_the_checklist() {
    let mut state = state();
    assert_eq!(state.handle_key(press(KeyCode::Char('q'))), Outcome::Close);
}

#[test]
fn the_empty_notice_is_a_dialog_any_dismiss_key_closes() {
    use tuikit::overlay::Overlay;
    let mut notice = empty_notice();
    assert_eq!(notice.title(), "Clean up");
    assert_eq!(notice.text()[0], "Nothing to clean up.");
    assert_eq!(notice.footer().text(), "Enter/Esc close");
    for close in [press(KeyCode::Enter), press(KeyCode::Esc)] {
        assert_eq!(
            notice.clone().handle(close, &()),
            Outcome::Close,
            "{close:?}"
        );
    }
    assert_eq!(
        notice.clone().handle(press(KeyCode::Char('q')), &()),
        Outcome::Dismiss
    );
    assert_eq!(
        notice.handle(press(KeyCode::Char('x')), &()),
        Outcome::Consumed
    );
}
