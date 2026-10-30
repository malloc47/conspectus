use ratatui::crossterm::event::{KeyEvent, KeyEventKind};
use tuikit::keys::Binding;
use tuikit::testkit::keys::{chord, press, typed};

use super::*;

/// The keymap's id of the binding `event` reaches with `focus`.
fn reached(focus: Focus, event: KeyEvent) -> Option<&'static str> {
    keymap()
        .resolve(active_scopes(focus), event)
        .map(Binding::id)
}

/// The keymap builds: no two keys of one scope can match one key event
/// (neither a collision nor a key an earlier one shadows), every id is
/// unique and well formed, and every binding has keys and help.
#[test]
fn the_keymap_builds_without_conflicts() {
    let built = Keymap::builder()
        .rows(PROGRAM_KEYS)
        .table(RIGHT_PANE_KEYS, right_pane_binding)
        .rows(AFTER_RIGHT_PANE_KEYS)
        .build();
    if let Err(conflicts) = &built {
        panic!("{conflicts}");
    }
    // The right pane's table is taken in whole: one binding per explorer
    // action, after the program's and the left pane's.
    let right = keymap()
        .bindings()
        .iter()
        .filter(|binding| binding.scope() == RIGHT_PANE)
        .count();
    assert_eq!(right, 13);
}

/// The keys that once sat outside the table, matched by hand, are
/// bindings now.
#[test]
fn every_key_the_program_takes_is_a_binding() {
    let none = KeyModifiers::NONE;
    for (focus, event, id) in [
        (
            Focus::Left,
            chord(none, KeyCode::PageDown),
            "tree.page-down",
        ),
        (Focus::Left, chord(none, KeyCode::PageUp), "tree.page-up"),
        (Focus::Left, typed('o'), "value.inspect"),
        (Focus::Right, typed('o'), "explorer.inspect"),
        (Focus::Left, typed('i'), "row.copy-id"),
        (Focus::Right, press(KeyCode::Tab), "focus.cycle"),
        (Focus::Left, typed('J'), "preview.down"),
        (Focus::Right, typed('K'), "preview.up"),
    ] {
        assert_eq!(reached(focus, event), Some(id), "{focus:?} {event:?}");
    }
}

/// `F`, `Enter`, and `Backspace` mean one thing on each pane, and the
/// keymap holds both meanings, a binding in each pane's scope.
#[test]
fn the_keys_that_change_meaning_by_focus_are_a_binding_in_each_panes_scope() {
    for (event, left, right) in [
        (typed('F'), "filters.clear", "explorer.toggle-full"),
        (press(KeyCode::Enter), "row.default", "explorer.activate"),
        (press(KeyCode::Backspace), "reveal.back", "explorer.back"),
    ] {
        assert_eq!(reached(Focus::Left, event), Some(left));
        assert_eq!(reached(Focus::Right, event), Some(right));
        for (id, scope) in [(left, LEFT_PANE), (right, RIGHT_PANE)] {
            assert_eq!(keymap().get(id).map(Binding::scope), Some(scope), "{id}");
        }
    }
}

/// The only key a pane takes from the program is `o`, which means the
/// same on the right pane as everywhere: the explorer's inspect.
#[test]
fn the_only_program_key_a_pane_takes_is_the_inspect_key() {
    assert_eq!(
        keymap().overrides(),
        vec![("explorer.inspect", "value.inspect", INSPECT_KEY.to_string())]
    );
    let inspect = Command::Explorer(ExplorerKey::Inspect);
    for id in ["explorer.inspect", "value.inspect"] {
        assert_eq!(keymap().get(id).map(Binding::action), Some(inspect));
    }
}

/// The shadowing the build refuses: `g` with any modifier took `Ctrl-G`
/// from the grouping key's row, and `Enter` with any modifier overlapped
/// `Shift-Enter` and `Alt-Enter`. Both are bound so they cannot: `g` binds
/// as typed, so `Ctrl-G` groups and `Alt-g` does nothing.
#[test]
fn g_leaves_ctrl_g_to_grouping_and_enter_leaves_its_chords_to_reveal() {
    let ctrl_g = chord(KeyModifiers::CONTROL, KeyCode::Char('g'));
    assert_eq!(reached(Focus::Left, ctrl_g), Some("grouping.cycle"));
    assert_eq!(reached(Focus::Left, typed('g')), Some("tree.first"));
    assert_eq!(
        reached(Focus::Left, chord(KeyModifiers::ALT, KeyCode::Char('g'))),
        None
    );
    for modifiers in [KeyModifiers::SHIFT, KeyModifiers::ALT] {
        assert_eq!(
            reached(Focus::Left, chord(modifiers, KeyCode::Enter)),
            Some("row.reveal")
        );
    }
    assert_eq!(
        reached(Focus::Left, chord(KeyModifiers::CONTROL, KeyCode::Enter)),
        None,
        "Enter binds no chord of itself"
    );
}

#[test]
fn a_view_key_switches_to_its_view() {
    assert_eq!(
        keymap()
            .get("view.sessions")
            .map(|binding| binding.action().action()),
        Some(Action::SwitchView(View::Sessions))
    );
    let mut release = typed('3');
    release.kind = KeyEventKind::Release;
    assert_eq!(
        crate::tui::keymap::translate(ratatui::crossterm::event::Event::Key(release)),
        None,
        "a release is no key press"
    );
}

/// The status bar's and the chip's hints name the keymap's keys in its
/// words.
#[test]
fn the_messages_hint_names_the_messages_key() {
    let app = App::new(crate::tui::RunConfig::defaults());
    assert_eq!(messages_hint(&app, "details"), "! details");
}

/// The main screen binds characters as every overlay does: alone or with
/// Shift, never with Control or Alt, and a chord is a binding of its own,
/// spelled out. So a chord of a bound character reaches nothing unless a
/// binding names that chord.
#[test]
fn every_character_binds_as_typed_and_every_chord_is_spelled_out() {
    assert!(
        keymap().loose_characters().is_empty(),
        "{:?}",
        keymap().loose_characters()
    );
    let chords = [
        KeyModifiers::ALT,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT | KeyModifiers::CONTROL,
        KeyModifiers::SUPER,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    ];
    for binding in keymap().bindings() {
        for key in binding.keys() {
            let KeyMatcher::Char(c) = *key else { continue };
            for focus in [Focus::Left, Focus::Right] {
                for modifiers in chords {
                    let event = chord(modifiers, KeyCode::Char(c));
                    let spelled = keymap().resolve(active_scopes(focus), event).map(|found| {
                        found.keys().iter().any(|key| {
                            *key == KeyMatcher::Exact {
                                modifiers,
                                code: KeyCode::Char(c),
                            }
                        })
                    });
                    assert!(
                        spelled.is_none_or(|spelled| spelled),
                        "{focus:?}: {modifiers:?} {c:?} reaches a binding that does not name it"
                    );
                }
            }
        }
    }
    // Among them, the chords that used to work: `Alt-a` attached, `Alt-p`
    // pinned, and `Ctrl-j` (what many terminals send for `Ctrl-Enter`)
    // moved the tree.
    for (modifiers, c) in [
        (KeyModifiers::ALT, 'a'),
        (KeyModifiers::ALT, 'p'),
        (KeyModifiers::ALT, 'r'),
        (KeyModifiers::ALT, '/'),
        (KeyModifiers::CONTROL, 'j'),
        (KeyModifiers::CONTROL, 'k'),
        (KeyModifiers::CONTROL, 'J'),
    ] {
        assert_eq!(
            reached(Focus::Left, chord(modifiers, KeyCode::Char(c))),
            None,
            "{modifiers:?} {c}"
        );
    }
    // The chords the program binds on purpose still reach their bindings,
    // and a character typed with Shift is the character.
    for (event, id) in [
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('r')),
            "refresh.hard",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('g')),
            "grouping.cycle",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('/')),
            "search.contents",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('7')),
            "search.contents",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('l')),
            "search.granularity",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('o')),
            "search.options",
        ),
        (
            chord(KeyModifiers::CONTROL, KeyCode::Char('f')),
            "search.keep",
        ),
        (chord(KeyModifiers::CONTROL, KeyCode::Char('c')), "quit"),
        (chord(KeyModifiers::ALT, KeyCode::Enter), "row.reveal"),
        (chord(KeyModifiers::SHIFT, KeyCode::Enter), "row.reveal"),
        (chord(KeyModifiers::SHIFT, KeyCode::Char('?')), "help"),
        (chord(KeyModifiers::SHIFT, KeyCode::Char('G')), "tree.last"),
        (chord(KeyModifiers::SHIFT, KeyCode::Char('R')), "rename"),
        (chord(KeyModifiers::NONE, KeyCode::Char('q')), "quit"),
    ] {
        assert_eq!(reached(Focus::Left, event), Some(id), "{event:?}");
    }
}

/// A message names a key as hints write it, from the binding that takes
/// it: the words the messages wrote by hand before, in one notation.
#[test]
fn a_message_names_the_key_its_binding_takes() {
    for (id, label) in [
        ("reveal.back", "Backspace"),
        ("explorer.back", "Backspace"),
        ("focus.cycle", "Tab"),
        ("search.keep", "Ctrl-F"),
        ("view.search", "6"),
        ("row.copy-id", "i"),
        ("row.default", "Enter"),
        ("refresh", "r"),
        ("value.inspect", "o"),
    ] {
        assert_eq!(key_name(id), label, "{id}");
    }
}

#[test]
#[should_panic(expected = "the keymap binds no `no.such-key`")]
fn a_message_cannot_name_a_key_the_keymap_does_not_bind() {
    key_name("no.such-key");
}
