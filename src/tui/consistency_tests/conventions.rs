//! Every modal against the framework's conventions.
//!
//! The framework's conformance suite (`tuikit::testkit::conformance`)
//! holds a host's modals to the rules every modal of every host keeps: it
//! is drawn; nothing it draws escapes its frame at five widths by two
//! heights; its frame and title take the one look; moving its selection
//! moves no text; no word is cut at its edge; its keys keep the modal key
//! convention by role and pass the table checks; its footer is the one its
//! table generates; and `Ctrl-C`, `Esc`, and `q` close it by house rules
//! 4 and 5, saying nothing.
//!
//! This file is the host's half: the [`registry`] of every registered
//! modal (the layout suite's builders and the two full-screen viewers),
//! what each needs the checks to know, and the rig that draws the App and
//! routes keys the way the fixture TUI does. The checks run one per test
//! so they run in parallel. What is Conspectus's own (which modal a
//! gesture opens, what a menu offers, the levels inside a modal) stays in
//! `registry.rs`, `layout.rs`, and `keys.rs`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use tuikit::keys::{CHECKLIST_KEYS, Convention, DIALOG_KEYS, MENU_KEYS, TEXT_INPUT_KEYS};
use tuikit::testkit::conformance::{self, Case, Check, KeyTable, Registry, Rig, table};

use super::layout::{Open, app_with, builders};
use super::registry::{open_pane_viewer, open_viewer};
use super::*;
use crate::tui::keybindings::overlay::{
    CONTROLS_KEYS, HELP_KEYS, MESSAGES_KEYS, MODAL_CONVENTION, MUX_LAUNCH_KEYS, NEW_MUX_KEYS,
    PANE_VIEWER_KEYS, PICKER_KEYS, PIN_EDIT_KEYS, SEARCH_CONTENT_KEYS, SEARCH_KEYS,
    VALUE_MODAL_KEYS, VIEWER_FIND_KEYS, VIEWER_KEYS,
};

/// The App drawn and driven as the fixture TUI does: the whole screen,
/// keys through the modal stack, what a modal sends on the pure executor.
pub(super) struct Host {
    theme: crate::tui::Theme,
}

impl Rig for Host {
    type State = App;

    fn render(&self, app: &mut App, area: Rect) -> Buffer {
        render_to_buffer(app, area)
    }

    fn close_top(&self, app: &mut App) {
        app.pop_top_modal();
    }

    fn press(&self, app: &mut App, key: KeyEvent) {
        crate::tui::runtime::modal_key_pure(app, key);
    }

    fn depth(&self, app: &App) -> usize {
        app.modal_stack().len()
    }

    fn feedback(&self, app: &App) -> Vec<String> {
        app.status_message()
            .map(str::to_string)
            .into_iter()
            .chain(app.toasts().iter().map(|toast| toast.text.clone()))
            .collect()
    }

    fn note(&self, app: &mut App, text: &str) {
        app.update(Msg::SetStatus(Some(Feedback::info(text))));
    }

    fn theme(&self) -> &tuikit::theme::Theme {
        &self.theme
    }
}

/// Modals whose first level is typed into, where `q` is a letter rather
/// than a way out.
pub(super) const TYPED: &[&str] = &[
    "Rename",
    "Prompt",
    "Search",
    "Picker",
    "NewMux",
    "MuxLaunch",
    "PinEdit",
];

/// Modals whose whole body scrolls with Up and Down, so their text
/// legitimately moves.
const SCROLLS: &[&str] = &["Help", "ValueModal"];

/// Modals whose keys change the App they draw from (a setting, a search,
/// the log), so a frame after a key is not the same modal moved.
const READS_THE_APP: &[&str] = &["Controls", "Search", "Messages"];

/// Modals whose arrow keys must move a selection the check compares.
const MOVES: &[&str] = &[
    "PinEdit",
    "RowMenu",
    "MuxMenu",
    "WorktreeMenu",
    "Picker",
    "MuxLaunch",
    "NewMux",
    "CleanUp",
];

/// The modal's row of the modal key convention.
fn convention(name: &str) -> Convention {
    *MODAL_CONVENTION
        .iter()
        .find(|row| row.modal == name)
        .unwrap_or_else(|| panic!("{name} has no row in MODAL_CONVENTION"))
}

/// Every key table `modal` resolves with, at its first level and inside
/// it, in the order it consults them.
fn tables(modal: &str) -> Vec<KeyTable> {
    match modal {
        "Help" => vec![table("HELP_KEYS", HELP_KEYS)],
        "Controls" => vec![
            table("CONTROLS_KEYS", CONTROLS_KEYS),
            table("CHECKLIST_KEYS", CHECKLIST_KEYS),
        ],
        "Rename" => vec![table("TEXT_INPUT_KEYS", TEXT_INPUT_KEYS)],
        "Search" => vec![
            table("SEARCH_KEYS", SEARCH_KEYS),
            table("SEARCH_CONTENT_KEYS", SEARCH_CONTENT_KEYS),
        ],
        "Picker" => vec![table("PICKER_KEYS", PICKER_KEYS)],
        "ValueModal" => vec![table("VALUE_MODAL_KEYS", VALUE_MODAL_KEYS)],
        "Messages" => vec![table("MESSAGES_KEYS", MESSAGES_KEYS)],
        "Viewer" => vec![table("VIEWER_KEYS", VIEWER_KEYS)],
        "PaneViewer" => vec![table("PANE_VIEWER_KEYS", PANE_VIEWER_KEYS)],
        "ViewerFind" => vec![table("VIEWER_FIND_KEYS", VIEWER_FIND_KEYS)],
        // The worktree menu's steps are modals of their own: a prompt and
        // dialogs.
        "WorktreeMenu" => vec![table("MENU_KEYS", MENU_KEYS)],
        "Prompt" => vec![table("TEXT_INPUT_KEYS", TEXT_INPUT_KEYS)],
        "Dialog" => vec![table("DIALOG_KEYS", DIALOG_KEYS)],
        "NewMux" => vec![table("NEW_MUX_KEYS", NEW_MUX_KEYS)],
        "MuxMenu" | "RowMenu" => vec![table("MENU_KEYS", MENU_KEYS)],
        "MuxLaunch" => vec![table("MUX_LAUNCH_KEYS", MUX_LAUNCH_KEYS)],
        "PinEdit" => vec![table("PIN_EDIT_KEYS", PIN_EDIT_KEYS)],
        "CleanUp" => vec![table("CHECKLIST_KEYS", CHECKLIST_KEYS)],
        other => panic!("{other}: add the key tables this modal consults to tables()"),
    }
}

/// The case for one built modal: its keys and convention, whether it
/// types, how its selection is checked, and the footer it draws.
fn case(name: &'static str, variant: &'static str, view: View, open: Open) -> Case<App> {
    let mut case = Case::new(name, move || app_with(view, open))
        .variant(variant)
        .keys(convention(name), tables(name));
    if TYPED.contains(&name) {
        case = case.typed();
    }
    if SCROLLS.contains(&name) {
        case = case.unchecked_selection("its whole body scrolls with the arrow keys");
    } else if READS_THE_APP.contains(&name) {
        case = case.unchecked_selection("its keys change the App it draws from");
    } else if MOVES.contains(&name) {
        case = case.moves();
    }
    match (name, variant) {
        ("Help", _) => case.footer(HELP_KEYS),
        ("Controls", _) => case.footer(CONTROLS_KEYS),
        ("Search", "contents") => case.footer(SEARCH_CONTENT_KEYS),
        ("Search", _) => case.footer(SEARCH_KEYS),
        ("ValueModal", _) => case.footer(VALUE_MODAL_KEYS),
        ("Messages", _) => case.footer(MESSAGES_KEYS),
        ("NewMux", _) => case.footer(NEW_MUX_KEYS),
        // The worktree menu opens on its list, an action menu.
        ("MuxMenu" | "RowMenu" | "WorktreeMenu", _) => case.footer(MENU_KEYS),
        ("MuxLaunch", _) => case.footer(MUX_LAUNCH_KEYS),
        ("PinEdit", _) => case.footer(PIN_EDIT_KEYS),
        ("CleanUp", _) => case.footer(CHECKLIST_KEYS),
        // The prompts draw their keys on their bottom border.
        ("Rename" | "Prompt", _) => case.footer(TEXT_INPUT_KEYS),
        // A dialog's footer depends on its form: the one the dialog that
        // opened composes from the dialog table and its verbs.
        ("Dialog", _) => {
            let app = app_with(view, open);
            let Some(dialog) = app.dialog() else {
                panic!("Dialog ({variant}) opened no dialog");
            };
            case.footer(dialog.footer())
        }
        // The picker draws no footer of its own.
        _ => case,
    }
}

/// Every registered modal, as the conformance suite checks it.
pub(super) fn registry() -> Registry<Host> {
    let mut registry = Registry::new(Host {
        theme: crate::tui::Theme::default(),
    });
    // The variants of a modal built more than once, in the builders'
    // order.
    let mut searches = ["", "contents", "all"].into_iter();
    let mut dialogs = ["remove", "close-down", "clean up notice"].into_iter();
    for (name, view, open) in builders() {
        let variant = match name {
            "Search" => searches.next().unwrap_or(""),
            "Dialog" => dialogs.next().unwrap_or(""),
            _ => "",
        };
        registry.push(case(name, variant, view, open));
    }
    registry.push(
        Case::new("Viewer", || app_with(View::Pinned, open_viewer))
            .full_screen()
            .keys(convention("Viewer"), tables("Viewer"))
            .keys(convention("ViewerFind"), tables("ViewerFind")),
    );
    registry.push(
        Case::new("PaneViewer", || app_with(View::Pinned, open_pane_viewer))
            .full_screen()
            .keys(convention("PaneViewer"), tables("PaneViewer")),
    );
    registry
}

#[test]
fn every_registered_modal_is_in_the_conformance_registry() {
    let names = registry().names();
    for name in crate::tui::Modal::NAMES {
        assert!(
            names.contains(name),
            "Modal::{name} is registered but not in the conformance registry"
        );
    }
}

#[test]
fn every_row_of_the_convention_is_checked() {
    let checked: Vec<&str> = registry()
        .cases()
        .iter()
        .flat_map(Case::conventions)
        .map(|row| row.modal)
        .collect();
    for row in MODAL_CONVENTION {
        assert!(
            checked.contains(&row.modal),
            "MODAL_CONVENTION names {}, which no registered modal checks",
            row.modal
        );
    }
}

#[test]
fn every_modal_is_drawn() {
    conformance::assert_conventions_of(&registry(), &[Check::Drawn]);
}

#[test]
fn nothing_a_modal_draws_escapes_its_frame() {
    conformance::assert_conventions_of(&registry(), &[Check::Contained]);
}

#[test]
fn every_modal_draws_one_frame_and_one_title_form() {
    conformance::assert_conventions_of(&registry(), &[Check::Title]);
}

#[test]
fn moving_the_selection_in_a_modal_moves_no_text() {
    conformance::assert_conventions_of(&registry(), &[Check::Selection]);
}

#[test]
fn no_word_in_a_modal_is_cut_off_at_its_edge() {
    conformance::assert_conventions_of(&registry(), &[Check::Words]);
}

#[test]
fn every_modal_keeps_the_key_convention_by_role() {
    conformance::assert_conventions_of(&registry(), &[Check::Keys]);
}

#[test]
fn every_modal_draws_the_footer_its_table_generates() {
    conformance::assert_conventions_of(&registry(), &[Check::Footer]);
}

#[test]
fn every_modal_closes_by_the_one_rule_and_says_nothing() {
    conformance::assert_conventions_of(&registry(), &[Check::Close]);
}

/// `--snapshot-keys` drives every key of every modal: what a modal sends
/// goes through the reducer, and the snapshot names what it skipped
/// rather than dropping a pick or a submit.
#[cfg(feature = "snapshot")]
#[test]
fn snapshot_mode_drives_every_key_every_modal_binds() {
    let known = [
        "snapshot mode skipped a copy",
        "snapshot mode skipped an attach",
        "snapshot mode skipped a resume",
        "snapshot mode skipped a launch",
        "snapshot mode skipped a rebuild",
        "snapshot mode skipped a mux command",
        "snapshot mode skipped a store write",
        "snapshot mode reads pane scrollback only from a fixture root",
        "snapshot mode found no transcript to view",
    ];
    let mut pressed = 0;
    for case in registry().cases() {
        for key in case.bound_keys() {
            let mut app = case.open();
            let notes = crate::tui::snapshot::press_modal_key(&mut app, key);
            pressed += 1;
            for note in notes {
                assert!(
                    known.contains(&note.as_str()),
                    "{}: {key:?}: {note}",
                    case.label()
                );
            }
        }
    }
    assert!(pressed > 200, "only {pressed} keys were pressed");
}
