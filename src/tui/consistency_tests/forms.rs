//! Forms keep the capabilities the forms before them had: arrow keys
//! move between fields, Tab completes a directory in a cwd field, the
//! harness row offers the harness options (including skipping
//! permissions), and a pin's editable fields are listed, not guessed.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::Widget;

use super::*;
use crate::tui::Theme;
use crate::tui::modal::Overlay;
use crate::tui::widgets::mux_launch::{MuxLaunchField, MuxLaunchFormState, MuxLaunchFormWidget};
use crate::tui::widgets::new_mux::{NewMuxField, NewMuxFormState};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn launch_form(cwd: &str) -> MuxLaunchFormState {
    MuxLaunchFormState::new(
        "claude-code",
        cwd,
        "fresh",
        vec!["claude-code".into(), "codex".into(), "opencode".into()],
        vec![],
    )
}

fn rendered(form: &MuxLaunchFormState) -> String {
    let area = Rect::new(0, 0, 100, 30);
    let mut buffer = Buffer::empty(area);
    MuxLaunchFormWidget::new(form, &Theme::default()).render(area, &mut buffer);
    buffer_to_string(&buffer)
}

#[test]
#[ignore = "backlog CSP-692.04: the launch form moves between fields with Tab only"]
fn arrow_keys_move_between_the_launch_forms_fields() {
    let mut form = launch_form("/tmp");
    assert_eq!(form.focus(), MuxLaunchField::Harness);
    form.handle((), press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        MuxLaunchField::Cwd,
        "Down moves to the next field"
    );
    form.handle((), press(KeyCode::Up));
    assert_eq!(form.focus(), MuxLaunchField::Harness, "Up moves back");
}

#[test]
#[ignore = "backlog CSP-692.04: Tab on the launch form's cwd field moves on instead of completing"]
fn tab_completes_a_directory_in_the_launch_forms_cwd_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = launch_form(&typed);
    form.handle((), press(KeyCode::Tab));
    assert_eq!(
        form.focus(),
        MuxLaunchField::Cwd,
        "reach the cwd field first"
    );
    form.handle((), press(KeyCode::Tab));
    assert_eq!(
        form.spec().cwd().value(),
        format!("{}/alpha-dir/", dir.path().display()),
        "Tab completes the path while the cursor is in the cwd field"
    );
    assert_eq!(form.focus(), MuxLaunchField::Cwd);
}

#[test]
#[ignore = "backlog CSP-692.04: the new mux form's cwd field cannot complete a path or be reached with arrows"]
fn the_new_mux_form_completes_paths_and_moves_with_arrows() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = NewMuxFormState::new("fresh", typed);
    form.handle((), press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        NewMuxField::Cwd,
        "Down moves to the cwd field"
    );
    form.handle((), press(KeyCode::Tab));
    assert_eq!(
        form.cwd_value(),
        format!("{}/alpha-dir/", dir.path().display())
    );
}

#[test]
#[ignore = "backlog CSP-692.04: the harness row lists no options and the skip-permissions toggle is gone"]
fn the_harness_row_lists_its_options_and_the_permission_toggle() {
    let form = launch_form("/tmp");
    let text = rendered(&form).to_lowercase();
    for harness in ["claude-code", "codex", "opencode"] {
        assert!(
            text.contains(harness),
            "the harness choices include {harness}:\n{text}"
        );
    }
    assert!(
        text.contains("skip permissions") || text.contains("skip-permissions"),
        "a harness with launch options offers them:\n{text}"
    );
}

#[test]
#[ignore = "backlog CSP-692.04: editing a pin is a free-text prompt that never lists the fields it can change"]
fn editing_a_pin_lists_every_field_the_pin_can_change() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "conspectus-main");
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    let frame = frame(&mut app, 160, 40);
    let missing: Vec<&str> = PinKind::Mux
        .overridable_fields()
        .iter()
        .copied()
        .filter(|field| !frame.contains(field))
        .collect();
    assert!(
        missing.is_empty(),
        "the edit form must show {missing:?}:\n{frame}"
    );
}

#[test]
#[ignore = "backlog CSP-692.03: retargeting asks for free text instead of offering the candidates"]
fn retargeting_offers_the_other_entities_of_the_same_kind() {
    let mut app = app_in(View::Pinned);
    select(&mut app, "conspectus-main");
    app.update(Msg::OpenSlotRetarget("conspectus-main".into()));
    let frame = frame(&mut app, 160, 40);
    assert!(
        frame.contains("scratch"),
        "the unpinned mux `scratch` is a candidate to retarget to:\n{frame}"
    );
}
