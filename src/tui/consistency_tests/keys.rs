//! One key convention for every modal: Esc leaves, Enter applies or
//! activates, Space changes the focused item.
//!
//! `MODAL_CONVENTION` states what each modal does with the three keys;
//! these tests check each modal's real key tables against it, and that
//! every [`Modal`] variant has a row.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::layout::{app_with, builders, name_of};
use crate::tui::keybindings::overlay::{
    CLOSE_DOWN_KEYS, CONFIRM_KEYS, CONTROLS_KEYS, EnterMeans, HELP_KEYS, KeyRow, MENU_KEYS,
    MESSAGES_KEYS, MODAL_CONVENTION, MULTI_SELECT_KEYS, MUX_LAUNCH_KEYS, NEW_MUX_KEYS, PICKER_KEYS,
    PIN_EDIT_KEYS, SEARCH_CONTENT_KEYS, SEARCH_KEYS, SpaceMeans, TEXT_INPUT_KEYS, VALUE_MODAL_KEYS,
    VIEWER_FIND_KEYS, VIEWER_KEYS, resolve,
};

/// What a table makes of a key, by the name of the action it binds.
type Probe = Box<dyn Fn(KeyEvent) -> Option<String>>;

fn table<A: Copy + std::fmt::Debug + 'static>(rows: &'static [KeyRow<A>]) -> Probe {
    Box::new(move |event| resolve(rows, event).map(|action| format!("{action:?}")))
}

/// Every key table a modal consults, by modal name.
fn tables(modal: &str) -> Vec<Probe> {
    match modal {
        "Help" => vec![table(HELP_KEYS)],
        "Controls" => vec![table(CONTROLS_KEYS), table(MULTI_SELECT_KEYS)],
        "Rename" => vec![table(TEXT_INPUT_KEYS)],
        "Search" => vec![table(SEARCH_KEYS), table(SEARCH_CONTENT_KEYS)],
        "Picker" => vec![table(PICKER_KEYS)],
        "ValueModal" => vec![table(VALUE_MODAL_KEYS)],
        "Messages" => vec![table(MESSAGES_KEYS)],
        "Viewer" => vec![table(VIEWER_KEYS)],
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
        "CleanUp" => vec![table(MULTI_SELECT_KEYS), table(MENU_KEYS)],
        other => panic!("{other}: add the key tables this modal consults to tables()"),
    }
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The action names the first table that binds `code` gives it.
fn actions(modal: &str, code: KeyCode) -> Vec<String> {
    tables(modal)
        .iter()
        .filter_map(|probe| probe(press(code)))
        .collect()
}

const LEAVES: &[&str] = &["Cancel", "Close", "No", "Back"];
const APPLIES: &[&str] = &[
    "Confirm", "Submit", "Pick", "Yes", "Done", "Merge", "Discard",
];
const CHANGES: &[&str] = &["Toggle", "OptionNext"];

#[test]
fn every_modal_has_a_row_in_the_convention() {
    let mut named: Vec<&str> = builders().iter().map(|(name, _, _)| *name).collect();
    // The transcript viewer needs a transcript on disk to build, and its
    // find prompt is a mode of the viewer rather than a modal of its own.
    named.push("Viewer");
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
        assert_eq!(name_of(app.modal_stack().last().expect("a modal")), name);
    }
}

#[test]
fn esc_leaves_every_modal() {
    for row in MODAL_CONVENTION {
        let bound = actions(row.modal, KeyCode::Esc);
        assert!(
            bound.iter().any(|name| LEAVES.contains(&name.as_str())),
            "{}: Esc is bound to {bound:?}, not to leaving the modal",
            row.modal
        );
    }
}

#[test]
fn enter_applies_or_activates_where_the_convention_says_it_does() {
    for row in MODAL_CONVENTION {
        let bound = actions(row.modal, KeyCode::Enter);
        match row.enter {
            EnterMeans::Submits | EnterMeans::Activates => assert!(
                bound.iter().any(|name| APPLIES.contains(&name.as_str())),
                "{}: Enter is bound to {bound:?}, not to applying",
                row.modal
            ),
            EnterMeans::Nothing => assert!(
                bound.is_empty(),
                "{}: Enter is bound to {bound:?} in a modal where it does nothing special",
                row.modal
            ),
        }
        assert!(
            !bound.iter().any(|name| LEAVES.contains(&name.as_str())),
            "{}: Enter cancels; Esc is the key that leaves without applying",
            row.modal
        );
    }
}

#[test]
fn space_changes_the_focused_item_where_the_convention_says_it_does() {
    for row in MODAL_CONVENTION {
        let bound = actions(row.modal, KeyCode::Char(' '));
        match row.space {
            SpaceMeans::Toggles => assert!(
                bound.iter().any(|name| CHANGES.contains(&name.as_str())),
                "{}: Space is bound to {bound:?}, not to toggling or selecting",
                row.modal
            ),
            SpaceMeans::Pages => assert!(
                bound.iter().any(|name| name.contains("Page")),
                "{}: Space is bound to {bound:?}, not to paging",
                row.modal
            ),
            SpaceMeans::Types => assert!(
                bound.is_empty(),
                "{}: Space is bound to {bound:?}; it should reach the text field",
                row.modal
            ),
        }
    }
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

#[test]
fn both_search_modals_switch_reach_with_tab_and_shift_tab() {
    use crate::tui::keybindings::overlay::{SearchKey, footer};
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
        Some(SearchKey::Scope)
    );
    assert!(footer(SEARCH_CONTENT_KEYS).contains("Tab current/all"));
    // The hints are generated from the table, in the operator's words.
    let hints = footer(SEARCH_KEYS);
    for expected in ["Tab current/all", "Enter pick", "Esc close", "↑/↓ move"] {
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
