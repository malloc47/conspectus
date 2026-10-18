use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Up,
    Down,
    Pick,
    Close,
}

const ROWS: &[KeyRow<Key>] = &[
    KeyRow::Bind {
        key: KeyMatcher::Exact {
            modifiers: KeyModifiers::CONTROL,
            code: KeyCode::Char('c'),
        },
        action: Key::Close,
        hint: Hint::Alias,
    },
    KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Up),
        action: Key::Up,
        hint: Hint::Shown {
            label: "↑",
            verb: "move",
        },
    },
    KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Char('k')),
        action: Key::Up,
        hint: Hint::Alias,
    },
    KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Down),
        action: Key::Down,
        hint: Hint::Shown {
            label: "↓",
            verb: "move",
        },
    },
    KeyRow::Note("type to filter"),
    KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Enter),
        action: Key::Pick,
        hint: Hint::Shown {
            label: "Enter",
            verb: "pick",
        },
    },
    KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Esc),
        action: Key::Close,
        hint: Hint::Shown {
            label: "Esc",
            verb: "close",
        },
    },
];

fn press(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn resolve_returns_the_first_matching_row() {
    assert_eq!(
        resolve(ROWS, press(KeyModifiers::NONE, KeyCode::Up)),
        Some(Key::Up)
    );
    assert_eq!(
        resolve(ROWS, press(KeyModifiers::NONE, KeyCode::Char('k'))),
        Some(Key::Up)
    );
    assert_eq!(
        resolve(ROWS, press(KeyModifiers::CONTROL, KeyCode::Char('c'))),
        Some(Key::Close)
    );
    assert_eq!(
        resolve(ROWS, press(KeyModifiers::NONE, KeyCode::Char('x'))),
        None
    );
}

#[test]
fn footer_groups_labels_by_verb_and_skips_aliases() {
    assert_eq!(
        footer(ROWS),
        "↑/↓ move · type to filter · Enter pick · Esc close"
    );
}

#[test]
fn mislabeled_flags_a_label_that_names_another_key() {
    assert!(mislabeled(ROWS).is_empty());
    let wrong: &[KeyRow<Key>] = &[KeyRow::Bind {
        key: KeyMatcher::AnyMod(KeyCode::Char('n')),
        action: Key::Close,
        hint: Hint::Shown {
            label: "y",
            verb: "to confirm",
        },
    }];
    assert_eq!(mislabeled(wrong), vec![("y", "n".to_string())]);
}

#[test]
fn labels_accept_arrow_symbols_and_named_keys() {
    let rows: &[KeyRow<Key>] = &[
        KeyRow::Bind {
            key: KeyMatcher::AnyMod(KeyCode::BackTab),
            action: Key::Up,
            hint: Hint::Shown {
                label: "S-Tab",
                verb: "back",
            },
        },
        KeyRow::Bind {
            key: KeyMatcher::AnyMod(KeyCode::Char(' ')),
            action: Key::Pick,
            hint: Hint::Shown {
                label: "Space",
                verb: "toggle",
            },
        },
        KeyRow::Bind {
            key: KeyMatcher::AnyMod(KeyCode::Left),
            action: Key::Up,
            hint: Hint::Shown {
                label: "←",
                verb: "toggle",
            },
        },
    ];
    assert!(mislabeled(rows).is_empty(), "{:?}", mislabeled(rows));
}

#[test]
fn every_overlay_table_labels_its_keys_truthfully() {
    let tables: Vec<(&str, Vec<(&'static str, String)>)> = vec![
        ("menu", mislabeled(MENU_KEYS)),
        ("multi-select", mislabeled(MULTI_SELECT_KEYS)),
        ("text input", mislabeled(TEXT_INPUT_KEYS)),
        ("new mux", mislabeled(NEW_MUX_KEYS)),
        ("search", mislabeled(SEARCH_KEYS)),
        ("help", mislabeled(HELP_KEYS)),
        ("value modal", mislabeled(VALUE_MODAL_KEYS)),
        ("messages", mislabeled(MESSAGES_KEYS)),
        ("confirm", mislabeled(CONFIRM_KEYS)),
        ("close-down", mislabeled(CLOSE_DOWN_KEYS)),
        ("mux launch", mislabeled(MUX_LAUNCH_KEYS)),
        ("omnibox", mislabeled(OMNIBOX_KEYS)),
        ("pin create", mislabeled(PIN_CREATE_KEYS)),
        ("pin edit", mislabeled(PIN_EDIT_KEYS)),
        ("pin bind", mislabeled(PIN_BIND_KEYS)),
        ("pin rebind", mislabeled(PIN_REBIND_KEYS)),
        ("pin remove", mislabeled(PIN_REMOVE_KEYS)),
    ];
    for (name, wrong) in tables {
        assert!(
            wrong.is_empty(),
            "{name} footer labels name other keys: {wrong:?}"
        );
    }
}

#[test]
fn generated_footers_keep_the_established_wording() {
    assert_eq!(footer(MENU_KEYS), "↑/↓ move · Enter pick · Esc close");
    assert_eq!(
        footer(MULTI_SELECT_KEYS),
        "Space toggle · Enter confirm · Esc cancel"
    );
    assert_eq!(footer(TEXT_INPUT_KEYS), "Enter confirm · Esc cancel");
    assert_eq!(
        footer(NEW_MUX_KEYS),
        "Tab switch field · Enter next/commit · Esc cancel"
    );
    assert_eq!(
        footer(MESSAGES_KEYS),
        "j/k select · J/K scroll detail · y copy · Esc close"
    );
    assert_eq!(
        footer(HELP_KEYS),
        "j/k/PgDn/PgUp to scroll · Esc/q/? to close"
    );
    assert_eq!(footer(CONFIRM_KEYS), "Enter/y to confirm · Esc/n to cancel");
    assert_eq!(
        footer(CLOSE_DOWN_KEYS),
        "m = merge & close · d = discard & close · Esc cancel"
    );
    assert_eq!(
        footer(MUX_LAUNCH_KEYS),
        "Tab/Shift-Tab move · ←/→ toggle · Enter launch · Esc cancel"
    );
    assert_eq!(
        footer(PIN_CREATE_KEYS),
        "↑/↓/Tab move · S-Tab back · ←/→/Space option · Enter create · Esc cancel"
    );
    assert_eq!(
        footer(PIN_EDIT_KEYS),
        "↑/↓/Tab move · S-Tab back · ←/→/Space option · Enter save · Esc cancel"
    );
    assert_eq!(
        footer(PIN_BIND_KEYS),
        "Up/Down choose · Enter bind · Esc cancel"
    );
    assert_eq!(
        footer(PIN_REBIND_KEYS),
        "Up/Down field · type to edit · Enter save · Esc cancel"
    );
    assert_eq!(footer(PIN_REMOVE_KEYS), "Enter remove · Esc cancel");
}

#[test]
fn viewer_keys_resolve_as_the_old_match_did() {
    use crate::viewer::input::ViewerMsg as V;
    let none = KeyModifiers::NONE;
    let shift = KeyModifiers::SHIFT;
    let ctrl_mod = KeyModifiers::CONTROL;
    let cases = [
        (ctrl_mod, KeyCode::Char('c'), Some(V::Close)),
        (none, KeyCode::Esc, Some(V::Close)),
        (none, KeyCode::Char('q'), Some(V::Close)),
        (none, KeyCode::Char('j'), Some(V::ScrollDown)),
        (none, KeyCode::Down, Some(V::ScrollDown)),
        (none, KeyCode::Char('k'), Some(V::ScrollUp)),
        (none, KeyCode::Up, Some(V::ScrollUp)),
        (none, KeyCode::PageDown, Some(V::PageDown)),
        (none, KeyCode::Char(' '), Some(V::PageDown)),
        (none, KeyCode::PageUp, Some(V::PageUp)),
        (ctrl_mod, KeyCode::Char('d'), Some(V::HalfPageDown)),
        (ctrl_mod, KeyCode::Char('u'), Some(V::HalfPageUp)),
        (none, KeyCode::Char('d'), None),
        (none, KeyCode::Char('g'), Some(V::JumpToStart)),
        (none, KeyCode::Home, Some(V::JumpToStart)),
        (shift, KeyCode::Char('G'), Some(V::JumpToEnd)),
        (none, KeyCode::Char('G'), Some(V::JumpToEnd)),
        (ctrl_mod, KeyCode::Char('G'), None),
        (none, KeyCode::End, Some(V::JumpToEnd)),
        (none, KeyCode::Char('t'), Some(V::CycleToolDetail)),
        (shift, KeyCode::Char('T'), Some(V::ToggleThinking)),
        (shift, KeyCode::Char('I'), Some(V::ToggleAborted)),
        (shift, KeyCode::Char('H'), Some(V::ToggleHidden)),
        (none, KeyCode::Char('?'), Some(V::ToggleHelp)),
        (none, KeyCode::Char('x'), None),
    ];
    for (modifiers, code, expected) in cases {
        assert_eq!(
            resolve(VIEWER_KEYS, press(modifiers, code)),
            expected,
            "{modifiers:?} {code:?}"
        );
    }
}
