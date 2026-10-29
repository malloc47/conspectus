use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::*;

#[test]
fn literals_and_named_keys_are_one_press_each() {
    let presses = parse("vj<Enter>").expect("parse");
    let codes: Vec<KeyCode> = presses.iter().map(|press| press.code).collect();
    assert_eq!(
        codes,
        [KeyCode::Char('v'), KeyCode::Char('j'), KeyCode::Enter]
    );
    assert!(
        presses
            .iter()
            .all(|press| press.kind == KeyEventKind::Press)
    );
}

#[test]
fn an_uppercase_letter_carries_shift_and_nothing_else_does() {
    assert_eq!(parse("G").unwrap()[0].modifiers, KeyModifiers::SHIFT);
    for literal in ["g", "?", "1", " ", "!"] {
        assert_eq!(
            parse(literal).unwrap()[0].modifiers,
            KeyModifiers::NONE,
            "{literal:?}"
        );
    }
}

#[test]
fn every_named_key_has_its_code() {
    let names = [
        ("Up", KeyCode::Up),
        ("Down", KeyCode::Down),
        ("Left", KeyCode::Left),
        ("Right", KeyCode::Right),
        ("Enter", KeyCode::Enter),
        ("Esc", KeyCode::Esc),
        ("Tab", KeyCode::Tab),
        ("BackTab", KeyCode::BackTab),
        ("Space", KeyCode::Char(' ')),
        ("Backspace", KeyCode::Backspace),
        ("Delete", KeyCode::Delete),
        ("Insert", KeyCode::Insert),
        ("Home", KeyCode::Home),
        ("End", KeyCode::End),
        ("PageUp", KeyCode::PageUp),
        ("PageDown", KeyCode::PageDown),
        ("F1", KeyCode::F(1)),
        ("F12", KeyCode::F(12)),
    ];
    for (name, code) in names {
        assert_eq!(parse(&format!("<{name}>")), Ok(vec![press(code)]), "{name}");
    }
}

#[test]
fn modifier_prefixes_add_up_and_nest() {
    assert_eq!(parse("<C-r>"), Ok(vec![ctrl('r')]));
    assert_eq!(parse("<A-b>"), Ok(vec![alt('b')]));
    assert_eq!(
        parse("<S-Tab>"),
        Ok(vec![chord(KeyModifiers::SHIFT, KeyCode::Tab)])
    );
    assert_eq!(
        parse("<C-A-Delete>"),
        Ok(vec![chord(
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            KeyCode::Delete
        )])
    );
    assert_eq!(
        parse("<C-G>"),
        Ok(vec![chord(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('G')
        )])
    );
}

#[test]
fn one_bracketed_character_is_that_character() {
    assert_eq!(parse("<<>"), Ok(vec![typed('<')]));
    assert_eq!(parse("<v>"), Ok(vec![typed('v')]));
    assert_eq!(parse("<F>"), Ok(vec![typed('F')]));
    assert_eq!(parse("<C-/>"), Ok(vec![ctrl('/')]));
}

#[test]
fn a_bad_script_says_what_is_wrong() {
    assert_eq!(parse("<Enter"), Err(ScriptError::Unterminated));
    assert_eq!(
        parse("<Frobnicate>"),
        Err(ScriptError::UnknownFunctionKey("Frobnicate".into()))
    );
    assert_eq!(
        parse("<Nope>"),
        Err(ScriptError::UnknownName("Nope".into()))
    );
    assert_eq!(
        parse("<C-Nope>"),
        Err(ScriptError::UnknownName("Nope".into()))
    );
    assert_eq!(parse("<>"), Err(ScriptError::UnknownName(String::new())));
    assert_eq!(
        ScriptError::Unterminated.to_string(),
        "unterminated `<...>` in key script: missing `>`"
    );
    assert_eq!(
        ScriptError::UnknownName("Nope".into()).to_string(),
        "unknown key name `<Nope>`"
    );
    assert_eq!(
        ScriptError::UnknownFunctionKey("Fx".into()).to_string(),
        "unknown function key `<Fx>`"
    );
}

#[test]
fn an_empty_script_presses_nothing() {
    assert_eq!(parse(""), Ok(Vec::new()));
}

#[test]
#[should_panic(expected = "unknown key name `<Nope>`")]
fn script_panics_with_the_reason() {
    let _ = script("j<Nope>");
}

#[test]
fn a_release_is_the_same_key_let_go() {
    let key = release(ctrl('c'));
    assert_eq!(key.code, KeyCode::Char('c'));
    assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    assert_eq!(key.kind, KeyEventKind::Release);
}
