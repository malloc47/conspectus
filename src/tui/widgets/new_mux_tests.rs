use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn form_starts_focused_on_name_field() {
    let state = NewMuxFormState::new("dev", "/home/op/proj");
    assert_eq!(state.focus(), NewMuxField::Name);
    assert_eq!(state.name_value(), "dev");
    assert_eq!(state.cwd_value(), "/home/op/proj");
}

#[test]
fn tab_cycles_focus_between_fields() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    assert_eq!(
        state.handle((), key(KeyCode::Tab)),
        OverlayOutcome::Consumed
    );
    assert_eq!(state.focus(), NewMuxField::Cwd);
    assert_eq!(
        state.handle((), key(KeyCode::Tab)),
        OverlayOutcome::Consumed
    );
    assert_eq!(state.focus(), NewMuxField::Name);
}

#[test]
fn enter_on_name_advances_to_cwd_without_committing() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    assert_eq!(state.focus(), NewMuxField::Cwd);
}

#[test]
fn enter_on_cwd_commits_when_both_fields_populated() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    // Advance to cwd, then commit.
    let _ = state.handle((), key(KeyCode::Tab));
    let outcome = state.handle((), key(KeyCode::Enter));
    match outcome {
        OverlayOutcome::Commit(msg) => match *msg {
            Msg::CommitMuxNew { name, cwd } => {
                assert_eq!(name, "dev");
                assert_eq!(cwd, "/home/op/proj");
            }
            other => panic!("unexpected commit msg: {other:?}"),
        },
        other => panic!("expected Commit, got {other:?}"),
    }
}

#[test]
fn enter_on_empty_name_snaps_focus_back_to_name() {
    let mut state = NewMuxFormState::new("", "/tmp");
    // On cwd (advance via Tab), pressing Enter with empty name should
    // send focus back to the name field so the operator fills it.
    let _ = state.handle((), key(KeyCode::Tab));
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(outcome, OverlayOutcome::Consumed);
    assert_eq!(state.focus(), NewMuxField::Name);
}

#[test]
fn enter_on_empty_cwd_stays_on_cwd() {
    let mut state = NewMuxFormState::new("dev", "");
    let _ = state.handle((), key(KeyCode::Tab));
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(outcome, OverlayOutcome::Consumed);
    assert_eq!(state.focus(), NewMuxField::Cwd);
}

#[test]
fn esc_closes_the_form() {
    let mut state = NewMuxFormState::new("dev", "/tmp");
    assert_eq!(state.handle((), key(KeyCode::Esc)), OverlayOutcome::Close);
}

#[test]
fn ctrl_c_closes_the_form() {
    let mut state = NewMuxFormState::new("dev", "/tmp");
    assert_eq!(state.handle((), ctrl('c')), OverlayOutcome::Close);
}

#[test]
fn typing_edits_the_focused_field_only() {
    let mut state = NewMuxFormState::new("", "");
    let _ = state.handle((), key(KeyCode::Char('a')));
    let _ = state.handle((), key(KeyCode::Char('b')));
    assert_eq!(state.name_value(), "ab");
    assert_eq!(state.cwd_value(), "");
    // Move focus and type into cwd.
    let _ = state.handle((), key(KeyCode::Tab));
    let _ = state.handle((), key(KeyCode::Char('/')));
    let _ = state.handle((), key(KeyCode::Char('t')));
    assert_eq!(state.name_value(), "ab");
    assert_eq!(state.cwd_value(), "/t");
}
