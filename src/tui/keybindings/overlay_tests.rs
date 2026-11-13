use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::keys::{
    CHECKLIST_KEYS, ChecklistKey, DIALOG_KEYS, DialogKey, FIND_KEYS, FORM_KEYS, FORM_OPTION_KEYS,
    HINT_SEPARATOR, LINE_EDIT_KEYS, MENU_KEYS, MenuKey, PAGER_HELP_KEYS, PAGER_KEYS,
    PAGER_SCROLL_KEYS, PALETTE_KEYS, PALETTE_OPTION_KEYS, PATH_FIELD_KEYS, PaletteKey,
    PaletteOptionKey, READER_KEYS, SETTINGS_KEYS, SettingsKey, TEXT_INPUT_KEYS, ctrl_c_first,
    footer, loose_characters, mislabeled, resolve, unresolved,
};

use super::*;
use crate::tui::keybindings::RIGHT_PANE_KEYS;
use crate::viewer::keys::{TRANSCRIPT_KEYS, TranscriptKey};
use tuikit::explorer::EXPLORER_KEYS;
use tuikit::testkit::keys::chord;

/// What a table gets wrong, by rule.
struct Problems {
    mislabeled: Vec<(&'static str, String)>,
    unresolved: Vec<(&'static str, String)>,
    loose: Vec<String>,
    ctrl_c_first: bool,
}

fn problems<A: KeyAction + PartialEq>(rows: &'static [KeyRow<A>]) -> Problems {
    Problems {
        mislabeled: mislabeled(rows),
        unresolved: unresolved(rows),
        loose: loose_characters(rows),
        ctrl_c_first: ctrl_c_first(rows),
    }
}

/// Every key table an overlay resolves with, the standard ones included,
/// by the name its constant has. `the_registry_names_every_table` keeps
/// this list complete.
fn registry() -> Vec<(&'static str, Problems)> {
    vec![
        ("MENU_KEYS", problems(MENU_KEYS)),
        ("DIALOG_KEYS", problems(DIALOG_KEYS)),
        ("PAGER_KEYS", problems(PAGER_KEYS)),
        ("PAGER_SCROLL_KEYS", problems(PAGER_SCROLL_KEYS)),
        ("READER_KEYS", problems(READER_KEYS)),
        ("PAGER_HELP_KEYS", problems(PAGER_HELP_KEYS)),
        ("FIND_KEYS", problems(FIND_KEYS)),
        ("TEXT_INPUT_KEYS", problems(TEXT_INPUT_KEYS)),
        ("CHECKLIST_KEYS", problems(CHECKLIST_KEYS)),
        ("LOG_LIST_KEYS", problems(LOG_LIST_KEYS)),
        ("SETTINGS_KEYS", problems(SETTINGS_KEYS)),
        ("PALETTE_KEYS", problems(PALETTE_KEYS)),
        ("PALETTE_OPTION_KEYS", problems(PALETTE_OPTION_KEYS)),
        ("FORM_KEYS", problems(FORM_KEYS)),
        ("FORM_OPTION_KEYS", problems(FORM_OPTION_KEYS)),
        ("LINE_EDIT_KEYS", problems(LINE_EDIT_KEYS)),
        ("PATH_FIELD_KEYS", problems(PATH_FIELD_KEYS)),
        ("SEARCH_KEYS", problems(SEARCH_KEYS)),
        ("SEARCH_CONTENT_KEYS", problems(SEARCH_CONTENT_KEYS)),
        ("HELP_KEYS", problems(HELP_KEYS)),
        ("VALUE_MODAL_KEYS", problems(VALUE_MODAL_KEYS)),
        ("MESSAGES_KEYS", problems(MESSAGES_KEYS)),
        ("NEW_MUX_KEYS", problems(NEW_MUX_KEYS)),
        ("MUX_LAUNCH_KEYS", problems(MUX_LAUNCH_KEYS)),
        ("PIN_EDIT_KEYS", problems(PIN_EDIT_KEYS)),
        ("PANE_KEYS", problems(PANE_KEYS)),
        ("TRANSCRIPT_KEYS", problems(TRANSCRIPT_KEYS)),
        ("EXPLORER_KEYS", problems(EXPLORER_KEYS)),
        ("RIGHT_PANE_KEYS", problems(RIGHT_PANE_KEYS)),
    ]
}

/// Tables that are a part of another table or of a field, not a whole
/// overlay's, so they need not start with `Ctrl-C`: the form's option
/// rows are included after `FORM_KEYS`, the pager's scroll rows after
/// `PAGER_KEYS` and `PAGER_HELP_KEYS` start with it, the pane viewer's and
/// the transcript viewer's own keys after the pager's `READER_KEYS`, the
/// path field's one row is
/// consulted by a form after the form's own table, and the line editor's
/// keys are every text field's, consulted after its overlay's table.
const FRAGMENTS: &[&str] = &[
    "FORM_OPTION_KEYS",
    "PAGER_SCROLL_KEYS",
    "PANE_KEYS",
    "TRANSCRIPT_KEYS",
    "LINE_EDIT_KEYS",
    "PATH_FIELD_KEYS",
];

/// Tables a pane resolves while it has focus, not an overlay: a pane
/// leaves `q` and `Ctrl-C` to the program, so its table need not start
/// with `Ctrl-C`; it takes every other check.
const PANES: &[&str] = &["EXPLORER_KEYS", "RIGHT_PANE_KEYS"];

#[test]
fn every_advertised_key_resolves_to_what_its_footer_says() {
    for (name, found) in registry() {
        assert!(
            found.unresolved.is_empty(),
            "{name}: the footer names keys an earlier row takes: {:?}",
            found.unresolved
        );
    }
}

#[test]
fn every_table_labels_its_keys_in_the_one_notation() {
    for (name, found) in registry() {
        assert!(
            found.mislabeled.is_empty(),
            "{name}: footer labels name other keys: {:?}",
            found.mislabeled
        );
    }
}

#[test]
fn every_table_binds_characters_as_typed() {
    for (name, found) in registry() {
        assert!(
            found.loose.is_empty(),
            "{name}: characters bound with their chords: {:?}",
            found.loose
        );
    }
}

#[test]
fn every_overlay_table_starts_with_ctrl_c() {
    for (name, found) in registry() {
        assert_eq!(
            found.ctrl_c_first,
            !FRAGMENTS.contains(&name) && !PANES.contains(&name),
            "{name}: Ctrl-C first (a fragment or a pane is exempt, and says so)"
        );
    }
}

/// The registry above names every table the host and the framework
/// define, so a new table cannot skip the checks.
#[test]
fn the_registry_names_every_table() {
    let registered: Vec<&str> = registry().into_iter().map(|(name, _)| name).collect();
    for source in [
        include_str!("overlay.rs"),
        include_str!("../keybindings.rs"),
        include_str!("../../../crates/tuikit/src/keys/standard.rs"),
        include_str!("../../../crates/tuikit/src/explorer/keys.rs"),
    ] {
        for line in source.lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub const ") else {
                continue;
            };
            let Some((name, kind)) = rest.split_once(':') else {
                continue;
            };
            if kind.trim_start().starts_with("&[KeyRow<") {
                assert!(
                    registered.contains(&name),
                    "{name} is not in the overlay table registry"
                );
            }
        }
    }
}

#[test]
fn generated_footers_read_in_the_one_grammar() {
    let joined = |segments: &[&str]| segments.join(HINT_SEPARATOR);
    // The search options pane is the palette's.
    assert_eq!(
        footer(PALETTE_OPTION_KEYS),
        joined(&["↑/↓ move", "Space toggle", "Esc back"])
    );
    // The picker is a palette: the standard palette footer.
    assert_eq!(
        footer(PALETTE_KEYS),
        joined(&["↑/↓ move", "Enter pick", "Esc close"])
    );
    assert_eq!(
        footer(SEARCH_KEYS),
        joined(&[
            "↑/↓ move",
            "Enter pick",
            "Tab search current/all",
            "Esc close"
        ])
    );
    assert_eq!(
        footer(SEARCH_CONTENT_KEYS),
        joined(&[
            "↑/↓ move",
            "Enter open hit",
            "Tab search current/all",
            "S-Tab list sessions/messages",
            "Ctrl-R recall search",
            "Ctrl-O set options",
            "Ctrl-F filter lists",
            "Ctrl-S open Search view",
            "Esc close",
        ])
    );
    assert_eq!(
        footer(SETTINGS_KEYS),
        joined(&[
            "changes apply as you make them",
            "↑/↓ move",
            "Space select/toggle",
            "Enter apply & close",
            "Esc close",
        ])
    );
    assert_eq!(
        footer(HELP_KEYS),
        joined(&["j/k/PgDn/PgUp scroll", "Esc/q/? close"])
    );
    assert_eq!(
        footer(VALUE_MODAL_KEYS),
        joined(&["j/k/PgDn/PgUp scroll", "Esc/q/o close", "y copy"])
    );
    // In table order the copy key comes after the standard log list's
    // rows; the log list draws it before the keys that leave.
    assert_eq!(
        footer(MESSAGES_KEYS),
        joined(&["j/k select", "J/K scroll detail", "Esc close", "y copy"])
    );
    assert_eq!(
        footer(NEW_MUX_KEYS),
        joined(&[
            "↑/↓ move",
            "Tab complete/next",
            "S-Tab back",
            "Enter create",
            "Esc cancel",
        ])
    );
    assert_eq!(
        footer(MUX_LAUNCH_KEYS),
        joined(&[
            "↑/↓ move",
            "Tab complete/next",
            "S-Tab back",
            "←/→/Space change",
            "Enter launch",
            "Esc cancel",
        ])
    );
    assert_eq!(
        footer(PIN_EDIT_KEYS),
        joined(&[
            "↑/↓ move",
            "Tab complete/next",
            "S-Tab back",
            "←/→/Space change",
            "Ctrl-R follow again",
            "Enter save",
            "Esc cancel",
        ])
    );
    assert_eq!(footer(FIND_KEYS), joined(&["Enter find", "Esc cancel"]));
    // One grammar: a short imperative verb after the keys, never "to"
    // or "=".
    for text in [
        footer(PALETTE_OPTION_KEYS),
        footer(SEARCH_CONTENT_KEYS),
        footer(SETTINGS_KEYS),
        footer(HELP_KEYS),
        footer(VALUE_MODAL_KEYS),
        footer(MESSAGES_KEYS),
        footer(PIN_EDIT_KEYS),
        footer(DIALOG_KEYS),
    ] {
        assert!(!text.contains(" to ") && !text.contains(" = "), "{text}");
    }
}

#[test]
fn a_character_never_fires_on_its_control_or_alt_chord() {
    use tuikit::keys::PagerKey;
    let ctrl = KeyModifiers::CONTROL;
    let alt = KeyModifiers::ALT;
    // Pagers go to their ends on plain `g` and `G` only.
    for table in [HELP_KEYS, VALUE_MODAL_KEYS] {
        let at = |modifiers, c| resolve(table, chord(modifiers, KeyCode::Char(c)));
        assert_eq!(at(KeyModifiers::NONE, 'g'), Some(PagerKey::Top));
        assert_eq!(at(KeyModifiers::SHIFT, 'G'), Some(PagerKey::Bottom));
        assert_eq!(at(ctrl, 'g'), None);
        assert_eq!(at(alt, 'g'), None);
        assert_eq!(at(alt, 'G'), None);
        assert_eq!(at(ctrl, 'q'), None);
        assert_eq!(at(ctrl, ' '), None);
    }
    assert_eq!(
        resolve(VALUE_MODAL_KEYS, chord(ctrl, KeyCode::Char('y'))),
        None
    );
    assert_eq!(resolve(MESSAGES_KEYS, chord(alt, KeyCode::Char('y'))), None);
    assert_eq!(
        resolve(MESSAGES_KEYS, chord(ctrl, KeyCode::Char('g'))),
        None
    );
    // A dialog confirms on its plain letter only (a choice's verbs keep
    // to the same rule; the dialog's own tests check them).
    assert_eq!(resolve(DIALOG_KEYS, chord(ctrl, KeyCode::Char('y'))), None);
    assert_eq!(
        resolve(DIALOG_KEYS, chord(KeyModifiers::NONE, KeyCode::Char('y'))),
        Some(DialogKey::Confirm)
    );
    // Menus and settings move with plain `j` and `k`, not their chords.
    assert_eq!(resolve(MENU_KEYS, chord(ctrl, KeyCode::Char('j'))), None);
    assert_eq!(resolve(SETTINGS_KEYS, chord(alt, KeyCode::Char('k'))), None);
}

#[test]
fn every_list_moves_with_ctrl_n_and_ctrl_p() {
    let ctrl_n = chord(KeyModifiers::CONTROL, KeyCode::Char('n'));
    let ctrl_p = chord(KeyModifiers::CONTROL, KeyCode::Char('p'));
    assert_eq!(resolve(MENU_KEYS, ctrl_n), Some(MenuKey::Down));
    assert_eq!(resolve(MENU_KEYS, ctrl_p), Some(MenuKey::Up));
    assert_eq!(resolve(SETTINGS_KEYS, ctrl_n), Some(SettingsKey::Down));
    assert_eq!(resolve(SETTINGS_KEYS, ctrl_p), Some(SettingsKey::Up));
    assert_eq!(resolve(CHECKLIST_KEYS, ctrl_n), Some(ChecklistKey::Down));
    assert_eq!(resolve(MESSAGES_KEYS, ctrl_n), Some(LogListKey::Down));
    assert_eq!(resolve(MESSAGES_KEYS, ctrl_p), Some(LogListKey::Up));
    for form in [NEW_MUX_KEYS, MUX_LAUNCH_KEYS, PIN_EDIT_KEYS] {
        assert_eq!(resolve(form, ctrl_n), Some(FormKey::Down));
        assert_eq!(resolve(form, ctrl_p), Some(FormKey::Up));
    }
    for search in [SEARCH_KEYS, SEARCH_CONTENT_KEYS] {
        assert_eq!(resolve(search, ctrl_n), Some(SearchKey::Down));
        assert_eq!(resolve(search, ctrl_p), Some(SearchKey::Up));
    }
    // The picker's table.
    assert_eq!(resolve(PALETTE_KEYS, ctrl_n), Some(PaletteKey::Down));
    assert_eq!(resolve(PALETTE_KEYS, ctrl_p), Some(PaletteKey::Up));
    assert_eq!(
        resolve(PALETTE_OPTION_KEYS, ctrl_n),
        Some(PaletteOptionKey::Down)
    );
    // The viewers are pagers with no cursor; Ctrl-N is not their `n`.
    assert_eq!(resolve(TRANSCRIPT_KEYS, ctrl_n), None);
    assert_eq!(resolve(READER_KEYS, ctrl_n), None);
    assert_eq!(resolve(PANE_KEYS, ctrl_n), None);
}

/// What the transcript viewer does with a key: the pager's table
/// first, then the transcript's toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
    Pager(PagerKey),
    Toggle(TranscriptKey),
}

#[test]
fn the_transcript_viewer_resolves_the_readers_keys_then_its_toggles() {
    use PagerKey as P;
    use TranscriptKey as T;
    let none = KeyModifiers::NONE;
    let shift = KeyModifiers::SHIFT;
    let ctrl_mod = KeyModifiers::CONTROL;
    let alt_mod = KeyModifiers::ALT;
    let pager = |key| Some(Read::Pager(key));
    let toggle = |key| Some(Read::Toggle(key));
    let cases = [
        (ctrl_mod, KeyCode::Char('c'), pager(P::Abort)),
        (none, KeyCode::Esc, pager(P::Close)),
        (none, KeyCode::Char('q'), pager(P::Abort)),
        (none, KeyCode::Char('j'), pager(P::Down)),
        (none, KeyCode::Down, pager(P::Down)),
        (none, KeyCode::Char('k'), pager(P::Up)),
        (none, KeyCode::Up, pager(P::Up)),
        (none, KeyCode::PageDown, pager(P::PageDown)),
        (none, KeyCode::Char(' '), pager(P::PageDown)),
        (none, KeyCode::PageUp, pager(P::PageUp)),
        (ctrl_mod, KeyCode::Char('d'), pager(P::HalfPageDown)),
        (ctrl_mod, KeyCode::Char('u'), pager(P::HalfPageUp)),
        (none, KeyCode::Char('d'), None),
        (none, KeyCode::Char('g'), pager(P::Top)),
        (none, KeyCode::Home, pager(P::Top)),
        (shift, KeyCode::Char('G'), pager(P::Bottom)),
        (none, KeyCode::Char('G'), pager(P::Bottom)),
        (ctrl_mod, KeyCode::Char('G'), None),
        (none, KeyCode::End, pager(P::Bottom)),
        (none, KeyCode::Char('t'), toggle(T::Tools)),
        (shift, KeyCode::Char('T'), toggle(T::Thinking)),
        (shift, KeyCode::Char('I'), toggle(T::Aborted)),
        (shift, KeyCode::Char('H'), toggle(T::Hidden)),
        (none, KeyCode::Char('?'), pager(P::Help)),
        (none, KeyCode::Char('/'), pager(P::Find)),
        (none, KeyCode::Char('n'), pager(P::FindNext)),
        (shift, KeyCode::Char('N'), pager(P::FindPrev)),
        (none, KeyCode::Char('N'), pager(P::FindPrev)),
        (none, KeyCode::Char('x'), None),
        // The one rule for characters: no chord of a character fires it.
        (ctrl_mod, KeyCode::Char('g'), None),
        (alt_mod, KeyCode::Char('j'), None),
        (ctrl_mod, KeyCode::Char('q'), None),
        (ctrl_mod, KeyCode::Char('t'), None),
        (ctrl_mod, KeyCode::Char('/'), None),
        (ctrl_mod, KeyCode::Char('n'), None),
    ];
    for (modifiers, code, expected) in cases {
        let key = chord(modifiers, code);
        let read = resolve(READER_KEYS, key)
            .map(Read::Pager)
            .or_else(|| resolve(TRANSCRIPT_KEYS, key).map(Read::Toggle));
        assert_eq!(read, expected, "{modifiers:?} {code:?}");
    }
}

#[test]
fn find_prompt_keys_edit_confirm_and_leave() {
    use tuikit::keys::FindKey as F;
    use tuikit::text_edit::EditKey;
    let none = KeyModifiers::NONE;
    let ctrl_mod = KeyModifiers::CONTROL;
    // The prompt's own keys.
    let cases = [
        (none, KeyCode::Enter, Some(F::Confirm)),
        (none, KeyCode::Esc, Some(F::Cancel)),
        (ctrl_mod, KeyCode::Char('c'), Some(F::Abort)),
    ];
    for (modifiers, code, expected) in cases {
        assert_eq!(
            resolve(FIND_KEYS, chord(modifiers, code)),
            expected,
            "{modifiers:?} {code:?}"
        );
    }
    // Everything else is the line editor's, the shared editing keys and
    // typed characters (Space included) alike.
    let edits = [
        (none, KeyCode::Backspace, EditKey::DeleteBefore),
        (none, KeyCode::Delete, EditKey::DeleteAfter),
        (none, KeyCode::Left, EditKey::Left),
        (none, KeyCode::Right, EditKey::Right),
        (ctrl_mod, KeyCode::Left, EditKey::WordLeft),
        (ctrl_mod, KeyCode::Right, EditKey::WordRight),
        (KeyModifiers::ALT, KeyCode::Char('b'), EditKey::WordLeft),
        (KeyModifiers::ALT, KeyCode::Char('f'), EditKey::WordRight),
        (none, KeyCode::Home, EditKey::Home),
        (none, KeyCode::End, EditKey::End),
        (ctrl_mod, KeyCode::Char('u'), EditKey::DeleteLine),
        (ctrl_mod, KeyCode::Char('w'), EditKey::DeleteWordBefore),
        (ctrl_mod, KeyCode::Char('k'), EditKey::DeleteToEnd),
        (none, KeyCode::Char(' '), EditKey::Insert(' ')),
        (none, KeyCode::Char('n'), EditKey::Insert('n')),
    ];
    for (modifiers, code, expected) in edits {
        let key = chord(modifiers, code);
        assert_eq!(resolve(FIND_KEYS, key), None, "{modifiers:?} {code:?}");
        assert_eq!(EditKey::of(key), Some(expected), "{modifiers:?} {code:?}");
    }
}
