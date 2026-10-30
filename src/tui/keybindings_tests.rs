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
/// `Shift-Enter` and `Alt-Enter`. Both are bound so they cannot.
#[test]
fn g_leaves_ctrl_g_to_grouping_and_enter_leaves_its_chords_to_reveal() {
    let ctrl_g = chord(KeyModifiers::CONTROL, KeyCode::Char('g'));
    assert_eq!(reached(Focus::Left, ctrl_g), Some("grouping.cycle"));
    assert_eq!(reached(Focus::Left, typed('g')), Some("tree.first"));
    assert_eq!(
        reached(Focus::Left, chord(KeyModifiers::ALT, KeyCode::Char('g'))),
        Some("tree.first")
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
