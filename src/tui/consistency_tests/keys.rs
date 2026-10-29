//! One key convention for every modal: Esc leaves, Enter applies or
//! activates, Space changes the focused item.
//!
//! `MODAL_CONVENTION` states what each modal does with the three keys.
//! The framework's conformance suite checks each registered modal's real
//! key tables against its row by the role each bound action declares
//! (`tuikit::keys::KeyAction`), and that `Ctrl-C` dismisses it
//! (`conventions.rs`). These tests keep the rows themselves honest and
//! hold the two search modals to one meaning of Tab.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::keys::{SpaceMeans, resolve};

use crate::tui::keybindings::overlay::{MODAL_CONVENTION, SEARCH_CONTENT_KEYS, SEARCH_KEYS};
use tuikit::testkit::keys::{chord, ctrl, press};

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
fn both_search_modals_switch_reach_with_tab_and_the_contents_one_cycles_granularity_with_shift_tab()
{
    use crate::tui::keybindings::overlay::SearchKey;
    use tuikit::keys::footer;
    assert_eq!(
        resolve(SEARCH_KEYS, press(KeyCode::Tab)),
        Some(SearchKey::Scope)
    );
    assert_eq!(
        resolve(SEARCH_KEYS, chord(KeyModifiers::SHIFT, KeyCode::BackTab)),
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
            chord(KeyModifiers::SHIFT, KeyCode::BackTab)
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
    assert_eq!(resolve(SEARCH_KEYS, ctrl('n')), Some(SearchKey::Down));
    assert_eq!(resolve(SEARCH_KEYS, ctrl('p')), Some(SearchKey::Up));
}
