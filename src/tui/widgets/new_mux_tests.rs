use super::*;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::keys::{ctrl, press};

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
    assert_eq!(state.handle_key(press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(state.focus(), NewMuxField::Cwd);
    assert_eq!(state.handle_key(press(KeyCode::Tab)), Outcome::Consumed);
    assert_eq!(state.focus(), NewMuxField::Name);
}

#[test]
fn enter_on_name_advances_to_cwd_without_committing() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    assert_eq!(state.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(state.focus(), NewMuxField::Cwd);
}

#[test]
fn enter_on_cwd_commits_when_both_fields_populated() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    // Advance to cwd, then commit.
    let _ = state.handle_key(press(KeyCode::Tab));
    let outcome = state.handle_key(press(KeyCode::Enter));
    match outcome {
        Outcome::Commit(msg) => match msg {
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
    let _ = state.handle_key(press(KeyCode::Tab));
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, missing("name"));
    assert_eq!(state.focus(), NewMuxField::Name);
}

#[test]
fn enter_on_empty_cwd_stays_on_cwd() {
    let mut state = NewMuxFormState::new("dev", "");
    let _ = state.handle_key(press(KeyCode::Tab));
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, missing("cwd"));
    assert_eq!(state.focus(), NewMuxField::Cwd);
}

/// The form stays open and a toast names the empty field.
fn missing(field: &str) -> Outcome<Msg> {
    Outcome::Stay(Msg::Toast(tuikit::feedback::Feedback::warning(format!(
        "mux: {field} is required"
    ))))
}

#[test]
fn esc_closes_the_form() {
    let mut state = NewMuxFormState::new("dev", "/tmp");
    assert_eq!(state.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn ctrl_c_closes_the_form() {
    let mut state = NewMuxFormState::new("dev", "/tmp");
    assert_eq!(state.handle_key(ctrl('c')), Outcome::Close);
}

#[test]
fn typing_edits_the_focused_field_only() {
    let mut state = NewMuxFormState::new("", "");
    let _ = state.handle_key(press(KeyCode::Char('a')));
    let _ = state.handle_key(press(KeyCode::Char('b')));
    assert_eq!(state.name_value(), "ab");
    assert_eq!(state.cwd_value(), "");
    // Move focus and type into cwd.
    let _ = state.handle_key(press(KeyCode::Tab));
    let _ = state.handle_key(press(KeyCode::Char('/')));
    let _ = state.handle_key(press(KeyCode::Char('t')));
    assert_eq!(state.name_value(), "ab");
    assert_eq!(state.cwd_value(), "/t");
}

#[test]
fn up_and_down_move_between_name_and_cwd() {
    let mut state = NewMuxFormState::new("dev", "/home/op/proj");
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), NewMuxField::Cwd);
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), NewMuxField::Name);
}

#[test]
fn tab_completes_a_directory_in_the_cwd_field() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("alpha-dir")).unwrap();
    let mut state = NewMuxFormState::new("dev", format!("{}/alp", dir.path().display()));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), NewMuxField::Cwd, "completed, so it stayed");
    assert_eq!(
        state.cwd_value(),
        format!("{}/alpha-dir", dir.path().display())
    );
}

#[test]
fn candidates_offer_known_directories() {
    let mut state = NewMuxFormState::new("dev", "/srv/wor").with_cwd_candidates(vec![
        crate::tui::widgets::path_omnibox::PathCandidate::new("/srv/work/known", "repo", 300),
    ]);
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.cwd_value(), "/srv/work/known");
}

#[test]
fn typing_in_the_cwd_field_edits_the_path() {
    let mut state = NewMuxFormState::new("dev", "/a");
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Char('b')));
    assert_eq!(state.cwd_value(), "/ab");
}

#[test]
fn the_form_shows_the_path_mark_and_its_keys() {
    let state = NewMuxFormState::new("dev", "/tmp");
    let area = ratatui::layout::Rect::new(0, 0, 90, 14);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    NewMuxFormWidget::new(&state, &crate::tui::Theme::default()).render(area, &mut buf);
    let frame: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        frame.contains('√') && frame.contains("↑/↓ move") && frame.contains("Tab complete/next"),
        "{frame}"
    );
}
