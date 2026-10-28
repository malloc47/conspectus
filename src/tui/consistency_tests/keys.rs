//! One key convention for every modal: Esc leaves, Enter applies or
//! activates, Space changes the focused item.
//!
//! `MODAL_CONVENTION` states what each modal does with the three keys;
//! these tests check each modal's real key tables against it by the role
//! each bound action declares (`tuikit::keys::KeyAction`), not by its
//! name, and that every [`Modal`] variant has a row.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuikit::keys::{
    CHECKLIST_KEYS, CONFIRM_KEYS, KeyAction, KeyRole, KeyRow, MENU_KEYS, SpaceMeans,
    TEXT_INPUT_KEYS, resolve,
};

use super::layout::{app_with, builders, name_of};
use crate::tui::keybindings::overlay::{
    CLOSE_DOWN_KEYS, CONTROLS_KEYS, HELP_KEYS, MESSAGES_KEYS, MODAL_CONVENTION, MUX_LAUNCH_KEYS,
    NEW_MUX_KEYS, PANE_VIEWER_KEYS, PICKER_KEYS, PIN_EDIT_KEYS, SEARCH_CONTENT_KEYS, SEARCH_KEYS,
    VALUE_MODAL_KEYS, VIEWER_FIND_KEYS, VIEWER_KEYS,
};

/// The role of the action a table binds a key to.
type Probe = Box<dyn Fn(KeyEvent) -> Option<KeyRole>>;

fn table<A: KeyAction + 'static>(rows: &'static [KeyRow<A>]) -> Probe {
    Box::new(move |event| resolve(rows, event).map(KeyAction::role))
}

/// Every key table a modal consults, by modal name.
fn tables(modal: &str) -> Vec<Probe> {
    match modal {
        "Help" => vec![table(HELP_KEYS)],
        "Controls" => vec![table(CONTROLS_KEYS), table(CHECKLIST_KEYS)],
        "Rename" => vec![table(TEXT_INPUT_KEYS)],
        "Search" => vec![table(SEARCH_KEYS), table(SEARCH_CONTENT_KEYS)],
        "Picker" => vec![table(PICKER_KEYS)],
        "ValueModal" => vec![table(VALUE_MODAL_KEYS)],
        "Messages" => vec![table(MESSAGES_KEYS)],
        "Viewer" => vec![table(VIEWER_KEYS)],
        "PaneViewer" => vec![table(PANE_VIEWER_KEYS)],
        "ViewerFind" => vec![table(VIEWER_FIND_KEYS)],
        "WorktreeMenu" => vec![
            table(MENU_KEYS),
            table(CONFIRM_KEYS),
            table(CLOSE_DOWN_KEYS),
            table(TEXT_INPUT_KEYS),
        ],
        "NewMux" => vec![table(NEW_MUX_KEYS)],
        "MuxMenu" | "RowMenu" => vec![table(MENU_KEYS)],
        "MuxLaunch" => vec![table(MUX_LAUNCH_KEYS)],
        "PinEdit" => vec![table(PIN_EDIT_KEYS)],
        "CleanUp" => vec![table(CHECKLIST_KEYS), table(MENU_KEYS)],
        other => panic!("{other}: add the key tables this modal consults to tables()"),
    }
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The roles the tables `modal` consults give `event`.
fn roles(modal: &str) -> impl Fn(KeyEvent) -> Vec<KeyRole> + use<> {
    let probes = tables(modal);
    move |event| probes.iter().filter_map(|probe| probe(event)).collect()
}

#[test]
fn every_modal_has_a_row_in_the_convention() {
    let mut named: Vec<&str> = builders().iter().map(|(name, _, _)| *name).collect();
    // The transcript viewer needs a transcript on disk to build, and its
    // find prompt is a mode of the viewer rather than a modal of its own.
    named.push("Viewer");
    named.push("PaneViewer");
    named.push("ViewerFind");
    for name in &named {
        assert!(
            MODAL_CONVENTION.iter().any(|row| row.modal == *name),
            "Modal {name} has no row in MODAL_CONVENTION"
        );
    }
    for row in MODAL_CONVENTION {
        assert!(
            named.contains(&row.modal),
            "MODAL_CONVENTION names {}, which is not a Modal variant",
            row.modal
        );
    }
    // The built modals are the ones the table names (the exhaustive name
    // list in layout.rs makes a new variant fail there first).
    for (name, view, open) in builders() {
        let app = app_with(view, open);
        assert_eq!(name_of(app.modal_stack().top().expect("a modal")), name);
    }
}

/// Esc leaves every modal; Enter applies or activates where the
/// convention says it does, never leaves, and is unbound in a pager;
/// Space changes the focused item, pages, or reaches the text field as
/// the convention says.
#[test]
fn every_modal_keeps_the_convention_by_role() {
    let breaks: Vec<String> = MODAL_CONVENTION
        .iter()
        .flat_map(|row| row.breaks(&roles(row.modal)))
        .collect();
    assert!(breaks.is_empty(), "{}", breaks.join("\n"));
}

#[test]
fn a_departure_from_the_convention_gives_its_reason() {
    for row in MODAL_CONVENTION {
        let departs = matches!(row.space, SpaceMeans::Pages) || row.modal == "Controls";
        assert_eq!(
            departs,
            !row.exception.is_empty(),
            "{}: an exception needs a reason, and a reason needs an exception",
            row.modal
        );
    }
}

/// `Ctrl-C` gets out of every modal at once, or steps out of its one
/// level; no table lets an earlier row take it.
#[test]
fn ctrl_c_dismisses_every_modal() {
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    for row in MODAL_CONVENTION {
        let bound = roles(row.modal)(ctrl_c);
        assert!(
            bound
                .first()
                .is_some_and(|role| matches!(role, KeyRole::Abort | KeyRole::Leave)),
            "{}: Ctrl-C is bound to {bound:?}",
            row.modal
        );
    }
}

#[test]
fn both_search_modals_switch_reach_with_tab_and_the_contents_one_cycles_granularity_with_shift_tab()
{
    use crate::tui::keybindings::overlay::SearchKey;
    use tuikit::keys::footer;
    assert_eq!(
        resolve(SEARCH_KEYS, press(KeyCode::Tab)),
        Some(SearchKey::Scope)
    );
    assert_eq!(
        resolve(
            SEARCH_KEYS,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
        ),
        Some(SearchKey::Scope)
    );
    // The contents modal switches the same way and offers the same words.
    assert_eq!(
        resolve(SEARCH_CONTENT_KEYS, press(KeyCode::Tab)),
        Some(SearchKey::Scope)
    );
    assert_eq!(
        resolve(
            SEARCH_CONTENT_KEYS,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
        ),
        Some(SearchKey::Granularity),
        "in the contents modal Shift-Tab cycles sessions/messages instead"
    );
    assert!(footer(SEARCH_CONTENT_KEYS).contains("Tab search current/all"));
    assert!(footer(SEARCH_CONTENT_KEYS).contains("S-Tab list sessions/messages"));
    assert!(footer(SEARCH_CONTENT_KEYS).contains("Ctrl-R recall search"));
    // The hints are generated from the table, in the operator's words.
    let hints = footer(SEARCH_KEYS);
    for expected in [
        "Tab search current/all",
        "Enter pick",
        "Esc close",
        "↑/↓ move",
    ] {
        assert!(
            hints.contains(expected),
            "{expected:?} missing from {hints:?}"
        );
    }
    assert!(!hints.contains("contents"), "{hints}");
    // The list moves with Ctrl-N and Ctrl-P as well as the arrows.
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    assert_eq!(resolve(SEARCH_KEYS, ctrl('n')), Some(SearchKey::Down));
    assert_eq!(resolve(SEARCH_KEYS, ctrl('p')), Some(SearchKey::Up));
}
