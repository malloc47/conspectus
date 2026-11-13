use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::*;
use crate::keys::{self, KeyAction, KeyRow};
use crate::testkit::keys::{alt, chord, ctrl, press, release, script, typed};

/// `keys` pressed one after another into `line`, returning the last edit.
fn run(line: &mut LineEdit, keys: &str) -> Edit {
    script(keys)
        .into_iter()
        .fold(Edit::IGNORED, |_, key| line.handle(key))
}

fn changed() -> Edit {
    Edit {
        changed: true,
        consumed: true,
    }
}

fn unchanged() -> Edit {
    Edit {
        changed: false,
        consumed: true,
    }
}

#[test]
fn typing_inserts_at_the_cursor() {
    let mut line = LineEdit::default();
    assert_eq!(run(&mut line, "abc"), changed());
    assert_eq!((line.value(), line.cursor()), ("abc", 3));
    run(&mut line, "<Left>X");
    assert_eq!((line.value(), line.cursor()), ("abXc", 3));
    assert_eq!(line.handle(typed('Q')), changed(), "a shifted letter types");
    assert_eq!(line.value(), "abXQc");
    let altgr = chord(
        KeyModifiers::CONTROL | KeyModifiers::ALT,
        KeyCode::Char('@'),
    );
    assert_eq!(line.handle(altgr), changed(), "AltGr types its character");
    assert_eq!(line.value(), "abXQ@c");
}

#[test]
fn a_new_line_puts_the_cursor_at_the_end() {
    let line = LineEdit::new("seeded");
    assert_eq!(line.cursor(), 6);
    assert_eq!(line.with_cursor(2).cursor(), 2);
    assert_eq!(LineEdit::new("ab").with_cursor(9).cursor(), 2, "clamped");
    assert!(LineEdit::default().is_empty());
}

#[test]
fn backspace_and_delete_remove_one_character() {
    let mut line = LineEdit::new("héllo");
    assert_eq!(run(&mut line, "<Left><Left><Backspace>"), changed());
    assert_eq!((line.value(), line.cursor()), ("hélo", 2));
    assert_eq!(run(&mut line, "<Delete>"), changed());
    assert_eq!(line.value(), "héo");
    assert_eq!(line.handle(ctrl('h')), changed(), "Ctrl-H is Backspace");
    assert_eq!((line.value(), line.cursor()), ("ho", 1));
    run(&mut line, "<Home>");
    assert_eq!(run(&mut line, "<Backspace>"), unchanged(), "nothing before");
    run(&mut line, "<End>");
    assert_eq!(run(&mut line, "<Delete>"), unchanged(), "nothing after");
    assert_eq!(line.value(), "ho");
}

#[test]
fn home_end_and_the_arrows_move_without_changing_the_text() {
    let mut line = LineEdit::new("abc");
    assert_eq!(run(&mut line, "<Home>"), unchanged());
    assert_eq!(line.cursor(), 0);
    assert_eq!(run(&mut line, "<Left>"), unchanged(), "stops at the start");
    assert_eq!(line.cursor(), 0);
    run(&mut line, "<Right><Right>");
    assert_eq!(line.cursor(), 2);
    run(&mut line, "<End><Right>");
    assert_eq!(line.cursor(), 3, "stops at the end");
    line.handle(ctrl('a'));
    assert_eq!(line.cursor(), 0, "Ctrl-A is Home");
    line.handle(ctrl('f'));
    assert_eq!(line.cursor(), 1, "Ctrl-F is Right");
    line.handle(ctrl('e'));
    assert_eq!(line.cursor(), 3, "Ctrl-E is End");
    line.handle(ctrl('b'));
    assert_eq!(line.cursor(), 2, "Ctrl-B is Left");
    let shifted = chord(KeyModifiers::SHIFT, KeyCode::Left);
    line.handle(shifted);
    assert_eq!(line.cursor(), 1, "a stray Shift still moves");
    assert_eq!(line.value(), "abc");
}

#[test]
fn alt_and_control_move_by_word() {
    let mut line = LineEdit::new("alpha beta-gamma");
    assert_eq!(line.handle(alt('b')), unchanged());
    assert_eq!(line.cursor(), 11, "back to the start of `gamma`");
    line.handle(alt('b'));
    assert_eq!(line.cursor(), 6, "punctuation separates words");
    line.handle(alt('f'));
    assert_eq!(line.cursor(), 11);
    line.handle(chord(KeyModifiers::CONTROL, KeyCode::Left));
    assert_eq!(line.cursor(), 6, "Ctrl-Left");
    line.handle(chord(KeyModifiers::ALT, KeyCode::Left));
    assert_eq!(line.cursor(), 0, "Alt-Left");
    line.handle(chord(KeyModifiers::CONTROL, KeyCode::Right));
    assert_eq!(line.cursor(), 6, "Ctrl-Right");
    line.handle(chord(KeyModifiers::ALT, KeyCode::Right));
    assert_eq!(line.cursor(), 11, "Alt-Right");
    assert_eq!(line.value(), "alpha beta-gamma", "motion never types");
}

#[test]
fn word_motion_skips_spaces_and_stops_at_the_ends() {
    let mut line = LineEdit::new("héllo wörld-two  ");
    line.handle(alt('b'));
    assert_eq!(line.cursor(), 12, "back over the spaces to `two`");
    line.handle(alt('b'));
    assert_eq!(line.cursor(), 6);
    line.handle(alt('b'));
    line.handle(alt('b'));
    assert_eq!(line.cursor(), 0, "stops at the start");
    line.handle(alt('f'));
    assert_eq!(line.cursor(), 6, "to the start of the next word");
    line.handle(alt('f'));
    line.handle(alt('f'));
    assert_eq!(line.cursor(), 17, "past the last word, to the end");
}

#[test]
fn words_are_deleted_before_and_after_the_cursor() {
    let mut line = LineEdit::new("one two three");
    assert_eq!(line.handle(ctrl('w')), changed());
    assert_eq!(line.value(), "one two ");
    line.handle(chord(KeyModifiers::ALT, KeyCode::Backspace));
    assert_eq!(
        line.value(),
        "one ",
        "Alt-Backspace deletes the word before"
    );
    let mut line = LineEdit::new("one two three").with_cursor(0);
    line.handle(alt('d'));
    assert_eq!(line.value(), "two three", "Alt-D deletes the word after");
    line.handle(chord(KeyModifiers::CONTROL, KeyCode::Delete));
    assert_eq!(line.value(), "three", "Ctrl-Delete too");
}

#[test]
fn kill_keys_delete_the_line_or_its_rest_and_yank_brings_it_back() {
    let mut line = LineEdit::new("keep this|drop that").with_cursor(9);
    assert_eq!(line.handle(ctrl('k')), changed());
    assert_eq!(line.value(), "keep this");
    assert_eq!(
        line.handle(ctrl('k')),
        unchanged(),
        "nothing left to kill at the end"
    );
    assert_eq!(line.handle(ctrl('y')), changed());
    assert_eq!(line.value(), "keep this|drop that");
    let mut line = LineEdit::new("whole line").with_cursor(3);
    assert_eq!(line.handle(ctrl('u')), changed(), "Ctrl-U deletes the line");
    assert_eq!((line.value(), line.cursor()), ("", 0));
    assert_eq!(line.handle(ctrl('u')), unchanged());
    line.handle(ctrl('y'));
    assert_eq!(line.value(), "whole line");
    assert_eq!(LineEdit::default().handle(ctrl('y')), unchanged());
}

#[test]
fn the_hosts_keys_are_left_alone() {
    let mut line = LineEdit::new("text");
    for key in [
        press(KeyCode::Enter),
        press(KeyCode::Esc),
        press(KeyCode::Tab),
        press(KeyCode::BackTab),
        press(KeyCode::Up),
        press(KeyCode::Down),
        press(KeyCode::PageDown),
        press(KeyCode::F(5)),
        ctrl('c'),
        ctrl('r'),
        alt('x'),
    ] {
        assert_eq!(line.handle(key), Edit::IGNORED, "{key:?}");
        assert_eq!(EditKey::of(key), None, "{key:?}");
    }
    assert_eq!(line.value(), "text");
    assert_eq!(line.cursor(), 4);
}

#[test]
fn a_release_never_edits_and_a_repeat_does() {
    let mut line = LineEdit::default();
    assert_eq!(line.handle(release(typed('a'))), Edit::IGNORED);
    let mut repeat = typed('a');
    repeat.kind = KeyEventKind::Repeat;
    assert_eq!(line.handle(repeat), changed());
    assert_eq!(line.value(), "a");
}

#[test]
fn wide_and_combined_characters_edit_as_one() {
    let mut line = LineEdit::new("東京");
    assert_eq!((line.cursor(), line.visual_cursor()), (2, 4));
    line.handle(press(KeyCode::Left));
    assert_eq!((line.cursor(), line.visual_cursor()), (1, 2));
    // A picture written with a variation selector, a joiner sequence, and
    // an accent written as a combining mark are each one step.
    let mut line = LineEdit::new("a\u{2764}\u{fe0f}b\u{1f469}\u{200d}\u{1f4bb}e\u{301}");
    line.handle(press(KeyCode::Backspace));
    assert_eq!(line.value(), "a\u{2764}\u{fe0f}b\u{1f469}\u{200d}\u{1f4bb}");
    line.handle(press(KeyCode::Backspace));
    assert_eq!(line.value(), "a\u{2764}\u{fe0f}b");
    line.handle(press(KeyCode::Left));
    assert_eq!(line.visual_cursor(), 3, "the heart takes two cells");
    line.handle(press(KeyCode::Backspace));
    assert_eq!(line.value(), "ab");
    assert_eq!(line.cursor(), 1);
}

#[test]
fn a_paste_types_one_line() {
    let mut line = LineEdit::new("[]").with_cursor(1);
    assert_eq!(line.paste("first\r\nsecond\n"), changed());
    assert_eq!(line.value(), "[first second]");
    assert_eq!(line.cursor(), 13);
    assert_eq!(line.paste("\n\r\n"), unchanged(), "nothing to type");
}

#[test]
fn a_paste_flattens_to_one_line() {
    assert_eq!(single_line_paste("one\ntwo"), "one two");
    assert_eq!(single_line_paste("one\r\ntwo\r\n"), "one two");
    assert_eq!(single_line_paste("\n\none\n\n\ntwo\n"), "one two");
    assert_eq!(single_line_paste("a\tb"), "a b");
    assert_eq!(single_line_paste("a\u{1b}[31mb"), "a[31mb");
    assert_eq!(single_line_paste("  keep  spaces  "), "  keep  spaces  ");
    assert_eq!(single_line_paste("~/src/über"), "~/src/über");
}

#[test]
fn setting_the_value_says_whether_it_changed() {
    let mut line = LineEdit::new("same").with_cursor(0);
    assert_eq!(line.set_value("same"), unchanged());
    assert_eq!(line.cursor(), 4, "the cursor goes to the end");
    assert_eq!(line.set_value("other"), changed());
    assert_eq!(line.value(), "other");
}

#[test]
fn two_lines_are_equal_with_the_same_text_and_cursor() {
    assert_eq!(LineEdit::new("ab"), LineEdit::new("ab"));
    assert_ne!(LineEdit::new("ab"), LineEdit::new("ab").with_cursor(1));
    assert_ne!(LineEdit::new("ab"), LineEdit::new("abc"));
}

#[test]
fn the_edit_a_key_asks_for_is_what_handling_it_does() {
    let mut by_key = LineEdit::new("one two");
    let mut by_edit = LineEdit::new("one two");
    for key in script("<Home><A-f>x<C-k><Left><Backspace>") {
        let edit = EditKey::of(key).expect("an editing key");
        assert_eq!(by_key.handle(key), by_edit.apply(edit), "{key:?}");
    }
    assert_eq!(by_key, by_edit);
}

#[test]
fn the_table_binds_chords_and_keys_and_is_well_formed() {
    assert!(keys::mislabeled(LINE_EDIT_KEYS).is_empty());
    assert!(keys::loose_characters(LINE_EDIT_KEYS).is_empty());
    let bound: Vec<EditKey> = keys::rows(LINE_EDIT_KEYS)
        .filter_map(|row| match row {
            KeyRow::Bind { action, .. } => Some(*action),
            _ => None,
        })
        .collect();
    for edit in [
        EditKey::Left,
        EditKey::Right,
        EditKey::WordLeft,
        EditKey::WordRight,
        EditKey::Home,
        EditKey::End,
        EditKey::DeleteBefore,
        EditKey::DeleteAfter,
        EditKey::DeleteWordBefore,
        EditKey::DeleteWordAfter,
        EditKey::DeleteLine,
        EditKey::DeleteToEnd,
        EditKey::Yank,
    ] {
        assert!(bound.contains(&edit), "{edit:?} has no key");
    }
    assert_eq!(EditKey::WordLeft.role(), keys::KeyRole::Move);
    assert_eq!(EditKey::Insert('a').role(), keys::KeyRole::Other);
    assert_eq!(
        keys::footer(LINE_EDIT_KEYS),
        "",
        "editing keys are never advertised"
    );
}
