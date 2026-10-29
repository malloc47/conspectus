//! Every text field edits the same way: Alt-B and Alt-F move by word, a
//! pasted value cannot submit the field it lands in, a path typed with
//! `~` reaches the command expanded, and input a field refuses says why
//! (in a prompt or a form, on its error line).
//!
//! Each field is opened the way the operator opens it and driven by the
//! keys the live loop routes to it (`action_for_event`), and the checks
//! read what the operator sees: the frame, the status line, a toast, or
//! the command a commit asks for. Nothing here depends on which control a
//! field is built on.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::tui::effect::{Effect, ExecSpec, StoreOp};
use crate::tui::keymap::Action;
use crate::tui::widgets::controls::ControlsOverlayState;
use crate::tui::widgets::mux_launch::MuxLaunchFormState;
use tuikit::testkit::keys::{alt, chord, press};

const SIZE: (u16, u16) = (120, 40);

/// One frame of the whole screen, toasts and full-screen viewers
/// included.
fn screen(app: &mut App) -> String {
    let (width, height) = SIZE;
    app.prepare_toast_for_render();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| crate::tui::runtime::draw_screen(app, frame))
        .expect("draw");
    buffer_to_string(terminal.backend().buffer())
}

/// Run `msg` through the reducer and the pure executor (which posts
/// toasts and drops what would reach outside the test), returning the
/// effects so a test can read the command a commit asks for.
fn apply(app: &mut App, msg: Msg) -> Vec<Effect> {
    let effects = app.update(msg);
    crate::tui::runtime::execute_effects(app, effects.clone());
    effects
}

/// Hand one routed action to the modal that owns it, as the live loop
/// does: through the stack, with what it commits or sends while it stays
/// open run on the pure executor. Returns the effects those asked for.
fn run(app: &mut App, action: Action) -> Vec<Effect> {
    let Action::ModalKey(key) = action else {
        panic!("not a text field's key: {action:?}");
    };
    let mut effects = Vec::new();
    crate::tui::runtime::modal_key(app, key, &mut |app, msg| {
        effects.extend(apply(app, msg));
    });
    effects
}

/// One key, the way the live loop sends it.
fn send(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let action = crate::tui::runtime::action_for_event(app, Event::Key(key))
        .unwrap_or_else(|| panic!("nothing takes {key:?}"));
    run(app, action)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        send(app, press(KeyCode::Char(c)));
    }
}

/// Empty the focused field: `Ctrl-U` at the end of the text.
fn clear(app: &mut App) {
    send(app, press(KeyCode::End));
    send(app, chord(KeyModifiers::CONTROL, KeyCode::Char('u')));
}

fn home() -> String {
    std::env::var("HOME").expect("HOME must be set for the `~` checks")
}

// ---- opening each field ----------------------------------------------------

fn worktree_context() -> crate::tui::widgets::worktree_menu::WorktreeMenuContext {
    crate::tui::widgets::worktree_menu::WorktreeMenuContext {
        context: crate::tui::worktree_actions::WorktreeContext::Worktree,
        repo_root: Some("/home/op/src/conspectus".to_string()),
        branch: Some("main".to_string()),
        guard_sessions: Vec::new(),
        can_mutate: true,
        reveal_target: None,
    }
}

fn open_rename(app: &mut App) {
    app.update(Msg::OpenSlotRename("conspectus-main".into()));
}

fn open_branch_prompt(app: &mut App) {
    let menu = crate::tui::widgets::worktree_menu::WorktreeMenuState::new(worktree_context())
        .expect("a worktree offers New worktree");
    app.open_worktree_menu(menu);
    // New worktree is the first action.
    send(app, press(KeyCode::Enter));
}

fn open_controls_editor(app: &mut App, row: crate::tui::widgets::controls::ControlsCursor) {
    app.open_controls_overlay();
    for _ in 0..80 {
        if app.controls_overlay().map(ControlsOverlayState::cursor) == Some(row) {
            send(app, press(KeyCode::Char(' ')));
            return;
        }
        send(app, press(KeyCode::Down));
    }
    panic!("the controls have no {row:?} row");
}

fn open_max_age(app: &mut App) {
    open_controls_editor(
        app,
        crate::tui::widgets::controls::ControlsCursor::FilterMaxAge,
    );
}

fn open_contains(app: &mut App) {
    open_controls_editor(
        app,
        crate::tui::widgets::controls::ControlsCursor::FilterContent,
    );
}

fn open_quick_select(app: &mut App) {
    app.open_search_entry();
}

fn open_content_search(app: &mut App) {
    app.open_content_search_entry();
}

fn open_picker(app: &mut App) {
    app.update(Msg::OpenSlotRetarget("conspectus-main".into()));
}

fn open_new_mux_name(app: &mut App) {
    app.update(Msg::OpenNewMuxForm);
}

fn open_new_mux_cwd(app: &mut App) {
    app.update(Msg::OpenNewMuxForm);
    send(app, press(KeyCode::Down));
}

fn open_mux_launch_name(app: &mut App) {
    app.update(Msg::OpenMuxLaunchForm);
}

fn focus_launch_field(app: &mut App, field: crate::tui::widgets::mux_launch::MuxLaunchField) {
    for _ in 0..20 {
        if app.mux_launch_form().map(MuxLaunchFormState::focus) == Some(field) {
            return;
        }
        send(app, press(KeyCode::Down));
    }
    panic!("the launch form has no {field:?} field");
}

fn open_mux_launch_cwd(app: &mut App) {
    app.update(Msg::OpenMuxLaunchForm);
    focus_launch_field(app, crate::tui::widgets::mux_launch::MuxLaunchField::Cwd);
}

fn focus_pin_row(app: &mut App, label: &str) {
    for _ in 0..20 {
        if app.pin_edit_form().and_then(|f| f.focused_label()) == Some(label) {
            return;
        }
        send(app, press(KeyCode::Down));
    }
    panic!("the pin edit form has no {label} row");
}

fn open_pin_label(app: &mut App) {
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    focus_pin_row(app, "label");
}

fn open_pin_cwd(app: &mut App) {
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    focus_pin_row(app, "cwd");
}

fn open_viewer_find(app: &mut App) {
    app.open_viewer_modal(crate::viewer::state::ViewerState::new(
        crate::viewer::model::TranscriptDocument::default(),
    ));
    send(app, press(KeyCode::Char('/')));
}

fn open_pane_viewer_find(app: &mut App) {
    use crate::content_search::{MuxRef, PaneRef};
    use crate::tui::widgets::pane_viewer::PaneViewerState;
    let pane = PaneRef {
        mux: MuxRef {
            backend: "tmux".into(),
            socket: None,
            session: "shell".into(),
        },
        pane_id: "%0".into(),
        window_index: "0".into(),
        pane_index: "0".into(),
        command: Some("bash".into()),
    };
    let lines = (1..=10).map(|n| format!("output {n}")).collect();
    app.open_pane_viewer(PaneViewerState::open(pane, lines, 1, ""));
    send(app, press(KeyCode::Char('/')));
}

/// Puts the cursor in a text field.
type Open = fn(&mut App);

/// Every text field, by the name a failure reports, and how to put the
/// cursor in it.
const FIELDS: &[(&str, Open)] = &[
    ("rename prompt", open_rename),
    ("worktree branch prompt", open_branch_prompt),
    ("controls max age", open_max_age),
    ("controls contains", open_contains),
    ("quick select", open_quick_select),
    ("content search", open_content_search),
    ("picker filter", open_picker),
    ("new mux name", open_new_mux_name),
    ("new mux cwd", open_new_mux_cwd),
    ("mux launch name", open_mux_launch_name),
    ("mux launch cwd", open_mux_launch_cwd),
    ("pin edit label", open_pin_label),
    ("pin edit cwd", open_pin_cwd),
    ("transcript find", open_viewer_find),
    ("pane scrollback find", open_pane_viewer_find),
];

fn opened(open: Open) -> App {
    let mut app = app_in(View::Sessions);
    open(&mut app);
    app
}

// ---- word motion -----------------------------------------------------------

#[test]
fn word_motion_works_in_every_text_field() {
    // From the end, a word back lands on `beta`; from the start, a word
    // forward lands there too.
    let motions: [(&str, Option<KeyEvent>, KeyEvent); 4] = [
        ("Alt-B", None, alt('b')),
        (
            "Ctrl-Left",
            None,
            chord(KeyModifiers::CONTROL, KeyCode::Left),
        ),
        ("Alt-F", Some(press(KeyCode::Home)), alt('f')),
        (
            "Ctrl-Right",
            Some(press(KeyCode::Home)),
            chord(KeyModifiers::CONTROL, KeyCode::Right),
        ),
    ];
    let mut failures = Vec::new();
    for (field, open) in FIELDS {
        for (name, first, motion) in &motions {
            let mut app = opened(*open);
            clear(&mut app);
            type_text(&mut app, "alpha beta");
            if let Some(first) = first {
                send(&mut app, *first);
            }
            send(&mut app, *motion);
            type_text(&mut app, "X");
            let frame = screen(&mut app);
            if !frame.contains("alpha Xbeta") {
                failures.push(format!("{field}: {name}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "word motion does nothing in: {failures:#?}"
    );
}

// ---- `~` on commit ---------------------------------------------------------

#[test]
fn the_new_mux_form_runs_the_command_in_the_expanded_directory() {
    let mut app = opened(open_new_mux_name);
    type_text(&mut app, "dev");
    send(&mut app, press(KeyCode::Down));
    clear(&mut app);
    type_text(&mut app, "~/projects");
    let effects = send(&mut app, press(KeyCode::Enter));
    let cwd = effects.iter().find_map(|effect| match effect {
        Effect::Exec(ExecSpec::MuxNew { cwd, .. }) => Some(cwd.clone()),
        _ => None,
    });
    assert_eq!(cwd, Some(format!("{}/projects", home())), "{effects:?}");
}

fn launch_request(effects: &[Effect]) -> crate::tui::widgets::mux_launch::MuxLaunchRequest {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Exec(ExecSpec::MuxLaunch { request }) => Some(request.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the form did not launch: {effects:?}"))
}

#[test]
fn the_mux_launch_form_runs_the_command_in_the_expanded_directory() {
    use crate::tui::widgets::mux_launch::MuxLaunchField;
    let mut app = opened(open_mux_launch_name);
    clear(&mut app);
    type_text(&mut app, "fresh-mux");
    focus_launch_field(&mut app, MuxLaunchField::Cwd);
    clear(&mut app);
    type_text(&mut app, "~/projects");
    let effects = send(&mut app, press(KeyCode::Enter));
    assert_eq!(launch_request(&effects).cwd, format!("{}/projects", home()));
}

#[test]
fn the_mux_launch_form_expands_the_worktree_repo() {
    use crate::tui::widgets::mux_launch::MuxLaunchField;
    let mut app = opened(open_mux_launch_name);
    clear(&mut app);
    type_text(&mut app, "fresh-mux");
    focus_launch_field(&mut app, MuxLaunchField::WorktreeToggle);
    send(&mut app, press(KeyCode::Char(' ')));
    focus_launch_field(&mut app, MuxLaunchField::WorktreeBranch);
    type_text(&mut app, "feature");
    focus_launch_field(&mut app, MuxLaunchField::WorktreeRepo);
    clear(&mut app);
    type_text(&mut app, "~/projects");
    let effects = send(&mut app, press(KeyCode::Enter));
    assert_eq!(
        launch_request(&effects).worktree_repo,
        Some(format!("{}/projects", home()))
    );
}

#[test]
fn the_pin_edit_form_saves_the_expanded_directory() {
    let mut app = opened(open_pin_cwd);
    clear(&mut app);
    type_text(&mut app, "~/projects");
    let effects = send(&mut app, press(KeyCode::Enter));
    let saved = effects.iter().find_map(|effect| match effect {
        Effect::WriteStore(StoreOp::SavePin { request }) => request
            .set
            .iter()
            .find(|(field, _)| field == "cwd")
            .map(|(_, value)| value.clone()),
        _ => None,
    });
    assert_eq!(
        saved,
        Some(crate::model::FieldValue::Text(format!(
            "{}/projects",
            home()
        ))),
        "{effects:?}"
    );
}

// ---- refused input says why ------------------------------------------------

/// Whether `effects` change anything outside the TUI.
fn acts(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Exec(_) | Effect::WriteStore(_)))
}

#[test]
fn an_invalid_max_age_says_why_and_keeps_the_editor_open() {
    let mut app = opened(open_max_age);
    type_text(&mut app, "15");
    send(&mut app, press(KeyCode::Enter));
    assert!(
        app.controls_overlay()
            .is_some_and(|controls| controls.sub_editor().is_some()),
        "the editor stays open for a fix"
    );
    assert_eq!(app.filter().max_age, None, "no filter is applied");
    says_in_the_prompt(&mut app, "max age: missing unit (expected ms/s/m/h/d)");
    // The next edit clears the reason.
    type_text(&mut app, "m");
    assert!(!screen(&mut app).contains("missing unit"));
}

/// The prompt or form on top explains a refusal on its own error line:
/// the reason is drawn inside its frame and no toast is posted.
fn says_in_the_prompt(app: &mut App, reason: &str) {
    let frame = screen(app);
    let row = frame
        .lines()
        .find(|row| row.contains(reason))
        .unwrap_or_else(|| panic!("the prompt says {reason:?}:\n{frame}"));
    assert!(
        row.contains(&format!("│{reason}")),
        "the reason starts inside the prompt's border:\n{frame}"
    );
    assert!(app.toasts().is_empty(), "no toast: {:?}", app.toasts());
}

#[test]
fn an_empty_branch_name_says_why_and_keeps_the_prompt_open() {
    let mut app = opened(open_branch_prompt);
    let effects = send(&mut app, press(KeyCode::Enter));
    assert!(!acts(&effects), "nothing is created: {effects:?}");
    assert!(app.worktree_menu().is_some(), "the prompt stays open");
    says_in_the_prompt(&mut app, "worktree: branch name is required");
}

#[test]
fn the_new_mux_form_says_which_field_is_missing() {
    let mut app = opened(open_new_mux_name);
    let effects = send(&mut app, press(KeyCode::Enter));
    assert!(!acts(&effects), "nothing is launched: {effects:?}");
    says_in_the_prompt(&mut app, "mux: name is required");

    type_text(&mut app, "dev");
    send(&mut app, press(KeyCode::Down));
    clear(&mut app);
    let effects = send(&mut app, press(KeyCode::Enter));
    assert!(!acts(&effects), "nothing is launched: {effects:?}");
    assert!(app.new_mux_form().is_some(), "the form stays open");
    says_in_the_prompt(&mut app, "mux: cwd is required");
}

#[test]
fn an_empty_pin_rename_says_nothing_was_entered() {
    let mut app = opened(open_rename);
    clear(&mut app);
    let effects = send(&mut app, press(KeyCode::Enter));
    assert!(!acts(&effects), "nothing is renamed: {effects:?}");
    assert!(app.rename_overlay().is_some(), "the prompt stays open");
    says_in_the_prompt(&mut app, "pin: nothing entered");
}

// ---- paste -----------------------------------------------------------------

/// A bracketed paste, the way the live loop types it into the field that
/// has the keys.
fn paste(app: &mut App, text: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    crate::tui::runtime::type_paste(app, text, &mut |app, action| {
        effects.extend(run(app, action));
    });
    effects
}

#[test]
fn a_multi_line_paste_types_one_line_and_submits_nothing() {
    let mut failures = Vec::new();
    for (field, open) in FIELDS {
        let mut app = opened(*open);
        clear(&mut app);
        let effects = paste(&mut app, "alpha\r\nbeta\n\ngamma\tdelta\n");
        let frame = screen(&mut app);
        if acts(&effects) || !frame.contains("alpha beta gamma delta") {
            failures.push(format!("{field}: {effects:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "a paste submitted the field or lost its text: {failures:#?}"
    );
}

#[test]
fn a_pasted_value_commits_like_a_typed_one() {
    let mut app = opened(open_new_mux_name);
    paste(&mut app, "dev\n");
    send(&mut app, press(KeyCode::Down));
    clear(&mut app);
    paste(&mut app, "~/projects\n");
    let effects = send(&mut app, press(KeyCode::Enter));
    assert!(
        effects.contains(&Effect::Exec(ExecSpec::MuxNew {
            name: "dev".into(),
            cwd: format!("{}/projects", home()),
        })),
        "{effects:?}"
    );
}

#[test]
fn a_paste_outside_a_text_field_does_nothing() {
    let mut app = app_in(View::Sessions);
    let before = screen(&mut app);
    let effects = paste(&mut app, "q\nr");
    assert!(effects.is_empty(), "{effects:?}");
    assert!(!app.should_quit(), "a pasted `q` is not the quit key");
    assert_eq!(screen(&mut app), before, "the tree");

    // A menu's letters are its keys: `d` would discard the worktree.
    let mut app = app_in(View::Sessions);
    let menu =
        crate::tui::widgets::worktree_menu::WorktreeMenuState::new_close_down(worktree_context())
            .expect("a worktree can be closed down");
    app.open_worktree_menu(menu);
    let before = screen(&mut app);
    let effects = paste(&mut app, "discard");
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(screen(&mut app), before, "the close-down choice");

    // A choice row of a form: Space would step the option.
    let mut app = opened(open_mux_launch_name);
    focus_launch_field(
        &mut app,
        crate::tui::widgets::mux_launch::MuxLaunchField::Pin,
    );
    let before = screen(&mut app);
    paste(&mut app, "  ");
    assert_eq!(screen(&mut app), before, "the launch form's pin row");

    // The controls list: `j` would move and Space would apply.
    let mut app = app_in(View::Sessions);
    app.open_controls_overlay();
    let before = screen(&mut app);
    paste(&mut app, "j j");
    assert_eq!(screen(&mut app), before, "the controls list");
}
