use ratatui::crossterm::event::KeyCode;
use tuikit::overlay::Overlay;

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

fn list() -> Checklist<Msg> {
    checklist(
        &[
            candidate("h_a", "alpha"),
            candidate("h_b", "beta"),
            candidate("h_c", "gamma"),
        ],
        86_400 * 20,
    )
}

#[test]
fn every_candidate_starts_selected_with_its_age() {
    let list = list();
    assert_eq!(list.title(), TITLE);
    assert_eq!(list.selected(), [0, 1, 2]);
    // In days, as the History view's age column shows the same record.
    assert_eq!(list.items()[0].label, "mux      alpha · last seen 20d ago");
    assert_eq!(list.summary(), "3 of 3 selected");
}

#[test]
fn confirming_reaps_exactly_the_selected_records() {
    let mut list = list();
    // Deselect the middle one.
    list.handle(press(KeyCode::Down), &());
    list.handle(press(KeyCode::Char(' ')), &());
    assert_eq!(list.selected(), [0, 2]);
    let outcome = list.handle(press(KeyCode::Enter), &());
    assert_eq!(
        outcome,
        Outcome::Commit(Msg::ReapRecords(vec!["h_a".into(), "h_c".into()]))
    );
}

#[test]
fn deselecting_everything_closes_without_reaping() {
    let mut one = checklist(&[candidate("h_a", "alpha")], 100);
    one.handle(press(KeyCode::Char(' ')), &());
    assert!(one.selected().is_empty());
    assert_eq!(one.handle(press(KeyCode::Enter), &()), Outcome::Close);
    // Select none does the same in one key.
    let mut list = list();
    list.handle(press(KeyCode::Char('n')), &());
    assert_eq!(list.handle(press(KeyCode::Enter), &()), Outcome::Close);
}

#[test]
fn select_all_puts_every_record_back() {
    let mut list = list();
    list.handle(press(KeyCode::Char('n')), &());
    list.handle(press(KeyCode::Char('a')), &());
    assert_eq!(
        list.handle(press(KeyCode::Enter), &()),
        Outcome::Commit(Msg::ReapRecords(vec![
            "h_a".into(),
            "h_b".into(),
            "h_c".into()
        ]))
    );
}

#[test]
fn escape_cancels() {
    let mut list = list();
    assert_eq!(list.handle(press(KeyCode::Esc), &()), Outcome::Close);
}

#[test]
fn q_dismisses_the_checklist() {
    let mut list = list();
    assert_eq!(
        list.handle(press(KeyCode::Char('q')), &()),
        Outcome::Dismiss
    );
}

#[test]
fn the_empty_notice_is_a_dialog_any_dismiss_key_closes() {
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
