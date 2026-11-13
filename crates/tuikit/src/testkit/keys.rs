//! Key scripts: a run of key presses written the way a vim user writes
//! them, and constructors for single presses.
//!
//! A script is literal characters plus `<Name>` brackets for keys that
//! are not characters:
//!
//! - A literal character is that key (`v` is `v`). An uppercase ASCII
//!   letter carries Shift, as a keyboard reports it.
//! - Named keys: `<Up> <Down> <Left> <Right> <Enter> <Esc> <Tab>
//!   <BackTab> <Space> <Backspace> <Delete> <Insert> <Home> <End>
//!   <PageUp> <PageDown>`, and the function keys `<F1>` to `<F12>`.
//! - One character in brackets is that character (`<<>` is `<`, `<v>` is
//!   `v`), so a script can press `<` itself.
//! - Modifiers prefix a name: `<C-x>` (Control), `<A-x>` (Alt), `<S-Tab>`
//!   (Shift). They nest, so `<C-A-Delete>` is Control and Alt with Delete.
//!
//! An unclosed `<` and an unknown name are errors ([`ScriptError`]), so a
//! typo fails loudly instead of pressing something else.
//!
//! ```
//! use ratatui::crossterm::event::{KeyCode, KeyModifiers};
//! use tuikit::testkit::keys::{self, ScriptError};
//!
//! let presses = keys::parse("j<Tab>G<C-r>").expect("a valid script");
//! assert_eq!(presses[0], keys::press(KeyCode::Char('j')));
//! assert_eq!(presses[1], keys::press(KeyCode::Tab));
//! assert_eq!(presses[2].modifiers, KeyModifiers::SHIFT);
//! assert_eq!(presses[3], keys::ctrl('r'));
//! assert_eq!(keys::parse("<Enter"), Err(ScriptError::Unterminated));
//! ```

use std::fmt;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Why a key script does not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptError {
    /// A `<` with no `>` after it.
    Unterminated,
    /// `<F...>` with something other than a number after the `F`.
    UnknownFunctionKey(String),
    /// A `<Name>` the notation does not have.
    UnknownName(String),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unterminated => write!(f, "unterminated `<...>` in key script: missing `>`"),
            Self::UnknownFunctionKey(name) => write!(f, "unknown function key `<{name}>`"),
            Self::UnknownName(name) => write!(f, "unknown key name `<{name}>`"),
        }
    }
}

impl std::error::Error for ScriptError {}

/// The presses `script` writes, in order.
pub fn parse(script: &str) -> Result<Vec<KeyEvent>, ScriptError> {
    let mut out = Vec::new();
    let mut chars = script.chars();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut name = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == '>' {
                    closed = true;
                    break;
                }
                name.push(next);
            }
            if !closed {
                return Err(ScriptError::Unterminated);
            }
            out.push(named(&name)?);
        } else {
            out.push(typed(ch));
        }
    }
    Ok(out)
}

/// The presses `script` writes, for a test that writes a script it knows
/// is valid.
///
/// # Panics
///
/// When `script` does not parse, with the reason.
pub fn script(script: &str) -> Vec<KeyEvent> {
    parse(script).unwrap_or_else(|error| panic!("key script {script:?}: {error}"))
}

/// A press of `code` with no modifier.
pub const fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A press of `code` with `modifiers`.
pub const fn chord(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// The press that types `c`, as a script writes it: an uppercase ASCII
/// letter carries Shift, everything else no modifier.
pub const fn typed(c: char) -> KeyEvent {
    typed_key(KeyCode::Char(c))
}

/// A press of `code` as a script writes it: an uppercase ASCII letter
/// carries Shift, any other key no modifier.
pub const fn typed_key(code: KeyCode) -> KeyEvent {
    let modifiers = match code {
        KeyCode::Char(c) if c.is_ascii_uppercase() => KeyModifiers::SHIFT,
        _ => KeyModifiers::NONE,
    };
    KeyEvent::new(code, modifiers)
}

/// Control and the character `c` (`ctrl('c')` is `Ctrl-C`).
pub const fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Alt and the character `c`.
pub const fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

/// The release of `key`: the same key, reported as let go rather than
/// pressed.
pub const fn release(key: KeyEvent) -> KeyEvent {
    let mut key = key;
    key.kind = KeyEventKind::Release;
    key
}

/// The press a bracketed `<name>` writes.
fn named(name: &str) -> Result<KeyEvent, ScriptError> {
    for (prefix, modifier) in [
        ("C-", KeyModifiers::CONTROL),
        ("A-", KeyModifiers::ALT),
        ("S-", KeyModifiers::SHIFT),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            let inner = named(rest)?;
            return Ok(KeyEvent::new(inner.code, inner.modifiers | modifier));
        }
    }
    let code = match name {
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "BackTab" => KeyCode::BackTab,
        "Space" => KeyCode::Char(' '),
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        other if other.starts_with('F') && other.len() > 1 => {
            let number = other[1..]
                .parse()
                .map_err(|_| ScriptError::UnknownFunctionKey(name.to_string()))?;
            KeyCode::F(number)
        }
        other => {
            let mut chars = other.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(typed(c)),
                _ => Err(ScriptError::UnknownName(name.to_string())),
            };
        }
    };
    Ok(press(code))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
