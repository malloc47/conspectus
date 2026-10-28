//! Key matchers, their one label notation, and the const builders a key
//! table is written with.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::glyphs::marks;

/// Which key events a row binds.
///
/// An overlay binds a character with [`KeyMatcher::Char`] (the [`ch`]
/// builder), a key that is not a character with [`KeyMatcher::AnyMod`]
/// ([`key`]), and a chord with [`KeyMatcher::Exact`] ([`ctrl`], [`alt`],
/// [`ctrl_key`]). That is the one rule for characters: a character
/// binding fires on the character as typed and never on a Control or Alt
/// chord of it, so `g` and `G` jump to the ends of a pager while `Ctrl-G`
/// and `Alt-G` stay free for whatever the program binds them to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMatcher {
    /// A character as it is typed: alone, or with Shift, since terminals
    /// differ on whether they report Shift with `G` or `?`. A Control or
    /// Alt chord of the character does not match.
    Char(char),
    /// A key pressed with exactly these modifiers: a chord such as
    /// `Ctrl-R`, `Alt-B`, or `Shift-Enter`.
    Exact {
        /// The modifiers that must be held, and no others.
        modifiers: KeyModifiers,
        /// The key.
        code: KeyCode,
    },
    /// A character with any modifiers but Control, so its Alt form fires
    /// too while the Control chord stays free for a binding of its own.
    /// For a program-wide keymap that tolerates Alt; an overlay binds
    /// characters with [`KeyMatcher::Char`].
    AnyModExceptCtrl(char),
    /// A key whatever modifiers come with it. For keys that are not
    /// characters (arrows, `Enter`, `Esc`, `Tab`, `Home`, `End`, the page
    /// keys), where a stray Shift or Alt should not stop the key.
    AnyMod(KeyCode),
}

impl KeyMatcher {
    /// Whether a key event with `modifiers` and `code` fires this matcher.
    pub fn matches(&self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        match *self {
            KeyMatcher::Char(c) => {
                code == KeyCode::Char(c)
                    && (modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT)
            }
            KeyMatcher::Exact {
                modifiers: wanted,
                code: key,
            } => wanted == modifiers && key == code,
            KeyMatcher::AnyModExceptCtrl(c) => {
                code == KeyCode::Char(c) && !modifiers.contains(KeyModifiers::CONTROL)
            }
            KeyMatcher::AnyMod(key) => key == code,
        }
    }

    /// Whether `event` fires this matcher.
    pub fn matches_event(&self, event: KeyEvent) -> bool {
        self.matches(event.modifiers, event.code)
    }

    /// The plainest key event this matcher fires on: the key with the
    /// matcher's own modifiers, or none. The checks that a footer's keys
    /// resolve press it.
    pub fn example(&self) -> KeyEvent {
        match *self {
            KeyMatcher::Char(c) | KeyMatcher::AnyModExceptCtrl(c) => {
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
            }
            KeyMatcher::Exact { modifiers, code } => KeyEvent::new(code, modifiers),
            KeyMatcher::AnyMod(code) => KeyEvent::new(code, KeyModifiers::NONE),
        }
    }

    /// Whether this matcher binds a character but ignores its modifiers,
    /// against the overlay rule (see the type's documentation).
    pub fn ignores_chords_of_a_character(&self) -> bool {
        matches!(
            self,
            KeyMatcher::AnyMod(KeyCode::Char(_)) | KeyMatcher::AnyModExceptCtrl(_)
        )
    }
}

/// The key a matcher binds, written the one way footers, help pages, and
/// messages write keys: `↑` `↓` `←` `→` for the arrows, `Space`, `S-Tab`,
/// `PgUp` and `PgDn`, `Enter`, `Esc`, and chords as `Ctrl-R` (the letter
/// upper-cased, since a terminal sends `Ctrl-r` and `Ctrl-R` alike),
/// `Alt-b`, and `Shift-Enter`.
pub fn key_label(key: &KeyMatcher) -> String {
    match *key {
        KeyMatcher::Char(c) | KeyMatcher::AnyModExceptCtrl(c) => code_label(KeyCode::Char(c)),
        KeyMatcher::AnyMod(code) => code_label(code),
        KeyMatcher::Exact { modifiers, code } => {
            let mut label = String::new();
            let control = modifiers.contains(KeyModifiers::CONTROL);
            if control {
                label.push_str("Ctrl-");
            }
            if modifiers.contains(KeyModifiers::ALT) {
                label.push_str("Alt-");
            }
            let shifted_by_its_name = matches!(code, KeyCode::BackTab)
                || matches!(code, KeyCode::Char(c) if c.is_uppercase());
            if modifiers.contains(KeyModifiers::SHIFT) && !shifted_by_its_name {
                label.push_str("Shift-");
            }
            match code {
                KeyCode::Char(c) if control && c.is_ascii_lowercase() => {
                    label.push(c.to_ascii_uppercase());
                }
                other => label.push_str(&code_label(other)),
            }
            label
        }
    }
}

fn code_label(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up => marks::ARROW_UP.to_string(),
        KeyCode::Down => marks::ARROW_DOWN.to_string(),
        KeyCode::Left => marks::ARROW_LEFT.to_string(),
        KeyCode::Right => marks::ARROW_RIGHT.to_string(),
        KeyCode::BackTab => "S-Tab".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    }
}

/// A key that is not a character, whatever modifiers come with it:
/// `key(KeyCode::Enter)`.
///
/// # Panics
///
/// When `code` is a character, which the overlay rule binds with [`ch`].
/// A table is a `const`, so the mistake fails to compile.
pub const fn key(code: KeyCode) -> KeyMatcher {
    if let KeyCode::Char(_) = code {
        panic!("bind a character with `ch`, which ignores no chord of it");
    }
    KeyMatcher::AnyMod(code)
}

/// A character as typed, alone or with Shift: `ch('j')`, `ch('G')`,
/// `ch('?')`.
pub const fn ch(c: char) -> KeyMatcher {
    KeyMatcher::Char(c)
}

/// A Control chord of a character: `ctrl('r')` is `Ctrl-R`.
pub const fn ctrl(c: char) -> KeyMatcher {
    KeyMatcher::Exact {
        modifiers: KeyModifiers::CONTROL,
        code: KeyCode::Char(c),
    }
}

/// An Alt chord of a character: `alt('b')` is `Alt-b`.
pub const fn alt(c: char) -> KeyMatcher {
    KeyMatcher::Exact {
        modifiers: KeyModifiers::ALT,
        code: KeyCode::Char(c),
    }
}

/// A Control chord of a key that is not a character: `Ctrl-←`.
pub const fn ctrl_key(code: KeyCode) -> KeyMatcher {
    KeyMatcher::Exact {
        modifiers: KeyModifiers::CONTROL,
        code,
    }
}
