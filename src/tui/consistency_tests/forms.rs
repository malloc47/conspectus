//! Forms keep the capabilities the forms before them had: arrow keys
//! move between fields, Tab completes a directory in a cwd field, the
//! harness row offers the harness options (including skipping
//! permissions), and a pin's editable fields are listed, not guessed.
//! And they follow the form house rules: `Enter` submits from any field,
//! and the keys stay in view on a short terminal.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::widgets::Widget;

use super::*;
use crate::tui::Theme;
use crate::tui::widgets::mux_launch::{MuxLaunchField, MuxLaunchFormState, MuxLaunchFormWidget};
use crate::tui::widgets::new_mux::{self, CWD};
use tuikit::overlay::Overlay;
use tuikit::testkit::keys::press;

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
fn arrow_keys_move_between_the_launch_forms_fields() {
    let mut form = launch_form("/tmp");
    assert_eq!(form.focus(), MuxLaunchField::MuxName, "the name is first");
    form.handle_key(press(KeyCode::Down));
    assert_eq!(form.focus(), MuxLaunchField::Harness);
    form.handle_key(press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        MuxLaunchField::LaunchOptions,
        "Down moves to the next field: the harness's launch options"
    );
    form.handle_key(press(KeyCode::Down));
    assert_eq!(form.focus(), MuxLaunchField::Cwd);
    form.handle_key(press(KeyCode::Up));
    form.handle_key(press(KeyCode::Up));
    form.handle_key(press(KeyCode::Up));
    assert_eq!(form.focus(), MuxLaunchField::MuxName, "Up moves back");
}

#[test]
fn tab_completes_a_directory_in_the_launch_forms_cwd_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = launch_form(&typed);
    form.handle_key(press(KeyCode::Down));
    form.handle_key(press(KeyCode::Down));
    form.handle_key(press(KeyCode::Down));
    assert_eq!(
        form.focus(),
        MuxLaunchField::Cwd,
        "reach the cwd field first"
    );
    form.handle_key(press(KeyCode::Tab));
    assert_eq!(
        form.spec().cwd().value(),
        format!("{}/alpha-dir", dir.path().display()),
        "Tab completes the path while the cursor is in the cwd field"
    );
    assert_eq!(form.focus(), MuxLaunchField::Cwd);
}

#[test]
fn the_new_mux_form_completes_paths_and_moves_with_arrows() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("alpha-dir")).expect("subdir");
    let typed = format!("{}/alp", dir.path().display());
    let mut form = new_mux::form("fresh", typed, Vec::new());
    form.handle(press(KeyCode::Down), &());
    assert_eq!(form.focus(), Some(CWD), "Down moves to the cwd field");
    form.handle(press(KeyCode::Tab), &());
    assert_eq!(
        form.values().text(CWD),
        format!("{}/alpha-dir", dir.path().display())
    );
}

/// The new mux form and the launch form submit from any field, so `Enter`
/// on the first field does what it does on the last (house rule 6).
#[test]
fn enter_submits_a_form_from_its_first_field() {
    let mut form = new_mux::form("fresh", "/tmp", Vec::new());
    assert!(
        matches!(
            form.handle(press(KeyCode::Enter), &()),
            tuikit::overlay::Outcome::Commit(Msg::CommitMuxNew { .. })
        ),
        "the new mux form creates from its name field"
    );
    let mut launch = launch_form("/tmp");
    assert_eq!(launch.focus(), MuxLaunchField::MuxName);
    assert!(matches!(
        launch.handle_key(press(KeyCode::Enter)),
        tuikit::overlay::Outcome::Commit(Msg::CommitMuxLaunch(_))
    ));
}

/// A form's footer is fixed: on a terminal too short for the whole form,
/// its keys stay at the bottom and its rows scroll (house rule 8).
#[test]
fn a_forms_keys_stay_in_view_on_a_short_terminal() {
    let mut launch = launch_form("/tmp");
    // The last field, so the rows have scrolled to it.
    launch.handle_key(press(KeyCode::Up));
    let area = Rect::new(0, 0, 60, 12);
    let mut buffer = Buffer::empty(area);
    MuxLaunchFormWidget::new(&launch, &Theme::default()).render(area, &mut buffer);
    let frame = buffer_to_string(&buffer);
    for segment in
        tuikit::keys::Footer::new(crate::tui::keybindings::overlay::MUX_LAUNCH_KEYS).segments()
    {
        assert!(
            frame.contains(&segment.text),
            "{} scrolled away:\n{frame}",
            segment.text
        );
    }
    assert!(
        frame.contains("pin"),
        "the focused row is in view:\n{frame}"
    );
}

#[test]
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

fn command_row_cells(form: &MuxLaunchFormState) -> Vec<(char, ratatui::style::Modifier)> {
    let area = Rect::new(0, 0, 100, 30);
    let mut buffer = Buffer::empty(area);
    MuxLaunchFormWidget::new(form, &Theme::default()).render(area, &mut buffer);
    for y in 0..area.height {
        let text: String = (0..area.width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        if text.contains("command") {
            return (0..area.width)
                .map(|x| {
                    let cell = &buffer[(x, y)];
                    (
                        cell.symbol().chars().next().unwrap_or(' '),
                        cell.style().add_modifier,
                    )
                })
                .collect();
        }
    }
    panic!("no command row in the form:\n{}", rendered(form));
}

#[test]
fn the_launch_forms_command_row_is_read_only_and_never_takes_focus() {
    use ratatui::style::Modifier;
    let mut form = launch_form("/tmp");
    // Walk the whole focus ring both ways with arrows and Tab: the
    // command row is not a field, so focus never rests on a row that
    // renders the command.
    for code in [KeyCode::Down, KeyCode::Tab, KeyCode::Up, KeyCode::BackTab] {
        for _ in 0..16 {
            form.handle_key(press(code));
            let cells = command_row_cells(&form);
            let marker: String = cells.iter().take(2).map(|(c, _)| *c).collect();
            assert!(
                !marker.contains('>'),
                "focus marker on the command row after {code:?}"
            );
        }
    }
    let cells = command_row_cells(&form);
    let text: String = cells.iter().map(|(c, _)| *c).collect();
    assert!(!text.contains("read only"), "no words spent on it: {text}");
    assert!(text.contains("└ command"), "led in by a dim corner: {text}");
    // Two cells deeper than the field labels, with its value in the
    // same column as theirs.
    let column = |needle: &str| {
        let frame = rendered(&form);
        let line = frame.lines().find(|l| l.contains(needle)).expect("row");
        line.find(needle).map(|byte| line[..byte].chars().count())
    };
    assert_eq!(
        column("command").map(|c| c - 2),
        column("harness  "),
        "the command label sits two cells inside the field labels"
    );
    let styled = cells
        .iter()
        .filter(|(c, _)| !c.is_whitespace() && !matches!(c, '│' | '┃' | '|'))
        .all(|(_, m)| m.contains(Modifier::ITALIC) && m.contains(Modifier::DIM));
    assert!(styled, "the command row is dim italic, unlike input rows");
}
