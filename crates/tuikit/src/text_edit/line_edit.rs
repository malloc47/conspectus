//! The line editor: one line of text, a cursor, and the keys that edit
//! them.

use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::{Input, InputRequest};

use crate::keys::{EditKey, LINE_EDIT_KEYS, resolve, typed_char};

/// What one key did to a line, so the host reacts to an edit instead of
/// comparing the text before and after.
///
/// A key can be consumed without changing anything (`←` at the start,
/// `Delete` at the end): it was an editing key, so the host should not
/// treat it as one of its own, but there is nothing to refilter or
/// revalidate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    /// The text changed: something was typed, deleted, yanked, or pasted.
    pub changed: bool,
    /// The key is an editing key or types a character, whether or not it
    /// changed the text. A key the editor does not consume (`Enter`,
    /// `Esc`, `Tab`, `↑`, an unbound chord) is the host's.
    pub consumed: bool,
}

impl Edit {
    /// Neither consumed nor changed: the key is not an editing key.
    pub const IGNORED: Edit = Edit {
        changed: false,
        consumed: false,
    };
}

impl EditKey {
    /// The edit `key` asks for: a row of [`LINE_EDIT_KEYS`], or a typed
    /// character. `None` when the key is not an editing key, so a host
    /// can tell before it hands a key on whether the line editor takes
    /// it. A key that is released never edits.
    pub fn of(key: KeyEvent) -> Option<EditKey> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        resolve(LINE_EDIT_KEYS, key).or_else(|| typed(key).map(EditKey::Insert))
    }
}

/// The character `key` types into a line, if it types one: a character
/// alone or with Shift, or with exactly Control and Alt, which is how a
/// terminal reports AltGr on some keyboards.
fn typed(key: KeyEvent) -> Option<char> {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    if key.modifiers == altgr {
        return typed_char(KeyEvent {
            modifiers: KeyModifiers::NONE,
            ..key
        });
    }
    typed_char(key)
}

/// One line of text being edited, and its cursor: the only text-edit
/// engine, behind every text field.
///
/// The value is any text; the cursor sits between characters and moves
/// and deletes one grapheme at a time, so a letter keeps its accent and
/// an emoji is never split. [`cursor`](Self::cursor) counts the
/// characters before it and [`visual_cursor`](Self::visual_cursor) the
/// terminal cells, which is what a renderer needs; [`TextField`] draws a
/// line with both.
///
/// The editor knows nothing of submitting, leaving, or moving between
/// fields: [`handle`](Self::handle) takes the keys of [`LINE_EDIT_KEYS`]
/// and typed characters, and leaves every other key (`Enter`, `Esc`,
/// `Tab`, `↑`) to its host, saying which it took in the [`Edit`] it
/// returns.
///
/// Two editors are equal when they hold the same text with the cursor in
/// the same place.
///
/// [`TextField`]: super::TextField
#[derive(Clone, Debug, Default)]
pub struct LineEdit {
    input: Input,
}

impl LineEdit {
    /// A line holding `value`, with the cursor at its end, so typing
    /// appends to a value the host seeded.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            input: Input::new(value.into()),
        }
    }

    /// The same line with the cursor after `cursor` characters (at the
    /// end when the line is shorter).
    #[must_use]
    pub fn with_cursor(self, cursor: usize) -> Self {
        Self {
            input: self.input.with_cursor(cursor),
        }
    }

    /// The text.
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Whether the line holds no text.
    pub fn is_empty(&self) -> bool {
        self.input.value().is_empty()
    }

    /// The characters (Unicode scalar values) before the cursor.
    pub fn cursor(&self) -> usize {
        self.input.cursor()
    }

    /// The terminal cells before the cursor: where a renderer that draws
    /// the whole line from its first cell puts the caret. A wide
    /// character counts two cells, a combining accent none.
    pub fn visual_cursor(&self) -> usize {
        self.input.visual_cursor()
    }

    /// Replace the text, with the cursor at its end, and say whether it
    /// changed.
    pub fn set_value(&mut self, value: impl Into<String>) -> Edit {
        let value = value.into();
        let changed = value != self.input.value();
        self.input = Input::new(value);
        Edit {
            changed,
            consumed: true,
        }
    }

    /// Edit the line with one key press.
    ///
    /// A key of [`LINE_EDIT_KEYS`] or a typed character is consumed, and
    /// [`Edit::changed`] says whether the text changed; any other key is
    /// [`Edit::IGNORED`], left for the host to act on.
    pub fn handle(&mut self, key: KeyEvent) -> Edit {
        EditKey::of(key).map_or(Edit::IGNORED, |edit| self.apply(edit))
    }

    /// Apply one edit, as a key that asks for it would.
    pub fn apply(&mut self, edit: EditKey) -> Edit {
        let request = match edit {
            EditKey::Insert(c) => InputRequest::InsertChar(c),
            EditKey::Left => InputRequest::GoToPrevChar,
            EditKey::Right => InputRequest::GoToNextChar,
            EditKey::WordLeft => InputRequest::GoToPrevWord,
            EditKey::WordRight => InputRequest::GoToNextWord,
            EditKey::Home => InputRequest::GoToStart,
            EditKey::End => InputRequest::GoToEnd,
            EditKey::DeleteBefore => InputRequest::DeletePrevChar,
            EditKey::DeleteAfter => InputRequest::DeleteNextChar,
            EditKey::DeleteWordBefore => InputRequest::DeletePrevWord,
            EditKey::DeleteWordAfter => InputRequest::DeleteNextWord,
            EditKey::DeleteLine => InputRequest::DeleteLine,
            // Deleting to the end from the end deletes nothing, though
            // the engine reports a change.
            EditKey::DeleteToEnd if self.cursor() == self.value().chars().count() => {
                return Edit {
                    changed: false,
                    consumed: true,
                };
            }
            EditKey::DeleteToEnd => InputRequest::DeleteTillEnd,
            EditKey::Yank => InputRequest::Yank,
        };
        let changed = self.input.handle(request).is_some_and(|state| state.value);
        Edit {
            changed,
            consumed: true,
        }
    }

    /// Type a bracketed paste at the cursor, flattened to one line by
    /// [`single_line_paste`], so a paste can never submit or leave the
    /// field the way a typed line break or tab would.
    pub fn paste(&mut self, text: &str) -> Edit {
        let mut changed = false;
        for c in single_line_paste(text).chars() {
            changed |= self.apply(EditKey::Insert(c)).changed;
        }
        Edit {
            changed,
            consumed: true,
        }
    }
}

impl PartialEq for LineEdit {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value() && self.cursor() == other.cursor()
    }
}

impl Eq for LineEdit {}

/// The text a paste types into a one-line field.
///
/// A line break would press `Enter` and submit the field, and one line
/// cannot show it, so the paste is flattened instead of cut at its first
/// line: line breaks at either end are dropped (a copied line usually
/// ends in one), each run of line breaks inside becomes one space, a tab
/// becomes a space (`Tab` would complete or move to the next field), and
/// any other control character is dropped. Everything else, spaces
/// included, is kept as it is.
///
/// A host that types a paste one key at a time (so each field completes
/// and refilters as it does for typing) flattens it with this first;
/// [`LineEdit::paste`] does both.
///
/// ```
/// use tuikit::text_edit::single_line_paste;
///
/// assert_eq!(single_line_paste("first\r\nsecond\n"), "first second");
/// assert_eq!(single_line_paste("a\tb"), "a b");
/// ```
pub fn single_line_paste(text: &str) -> String {
    let is_break = |c: char| c == '\n' || c == '\r';
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for c in text.trim_matches(is_break).chars() {
        if is_break(c) {
            if !in_break {
                out.push(' ');
            }
            in_break = true;
            continue;
        }
        in_break = false;
        if c == '\t' {
            out.push(' ');
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
#[path = "line_edit_tests.rs"]
mod tests;
