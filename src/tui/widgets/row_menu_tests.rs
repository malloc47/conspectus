use ratatui::crossterm::event::{KeyCode, KeyModifiers};

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
                Msg::SetStatus(Some("x".into())),
            )
            .confirming(),
        ],
    )
}

#[test]
fn picking_a_plain_entry_commits_its_message() {
    let mut state = menu();
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(
        outcome,
        OverlayOutcome::Commit(Box::new(Msg::SetStatus(None)))
    );
}

#[test]
fn a_confirming_entry_needs_a_second_enter() {
    let mut state = menu();
    state.handle((), key(KeyCode::Down));
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    assert_eq!(state.armed(), Some(1));
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(
        outcome,
        OverlayOutcome::Commit(Box::new(Msg::SetStatus(Some("x".into()))))
    );
}

#[test]
fn moving_disarms_a_pending_confirmation() {
    let mut state = menu();
    state.handle((), key(KeyCode::Down));
    state.handle((), key(KeyCode::Enter));
    state.handle((), key(KeyCode::Up));
    state.handle((), key(KeyCode::Down));
    assert_eq!(state.armed(), None);
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
}

#[test]
fn escape_closes_and_the_cursor_clamps() {
    let mut state = menu();
    for _ in 0..5 {
        state.handle((), key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), 1);
    assert_eq!(state.handle((), key(KeyCode::Esc)), OverlayOutcome::Close);
}
