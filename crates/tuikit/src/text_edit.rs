//! Text editing: the one line editor every text field edits with, and
//! the one way a field is drawn.
//!
//! This is a layer 1 control (house rule 15). A program with several
//! text fields (a prompt, a form's rows, a filter query, a find prompt)
//! should edit them all the same way and draw them all the same way, or
//! a key works in one and not the next, and a wide character puts the
//! caret in the right cell of one field and the wrong cell of another.
//!
//! - **Edit**: [`LineEdit`] holds one line and its cursor, and is the
//!   only edit engine. [`LineEdit::handle`] takes one key and returns an
//!   [`Edit`] that says whether the key was an editing key
//!   ([`Edit::consumed`]) and whether the text changed
//!   ([`Edit::changed`]), so a host refilters or revalidates on a change
//!   without copying the text to compare. The editor has no `Enter`,
//!   `Esc`, or `Tab`: what those mean belongs to the prompt, form, or
//!   query that hosts the field.
//! - **Keys**: [`LINE_EDIT_KEYS`] is the editing table, readline's keys
//!   with word motion on `Alt-b`/`Alt-f` and the Control and Alt arrows;
//!   [`EditKey::of`] says which edit a key asks for, so a host that turns
//!   keys into messages can carry the edit as a value.
//! - **Paste**: [`single_line_paste`] flattens a bracketed paste to one
//!   line, so a pasted line break cannot submit the field;
//!   [`LineEdit::paste`] types it.
//! - **Draw**: [`TextField`] draws a line editor in one row with one
//!   windowing rule, measured in terminal cells, that keeps the cursor in
//!   view: an optional prefix glyph, optional `<`/`>` scroll marks, a
//!   caret from the theme, and a placeholder for an empty field.
//!
//! ```
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use tuikit::text_edit::{LineEdit, TextField};
//! use tuikit::theme::Theme;
//!
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//! let mut line = LineEdit::new("東京");
//! let typed = line.handle(press(KeyCode::Char('!')));
//! assert!(typed.consumed && typed.changed);
//! assert_eq!((line.value(), line.cursor(), line.visual_cursor()), ("東京!", 3, 5));
//!
//! // `Enter` is the host's: the editor leaves it alone.
//! assert!(!line.handle(press(KeyCode::Enter)).consumed);
//!
//! // `←` at the start is an editing key that changes nothing.
//! line.handle(press(KeyCode::Home));
//! let moved = line.handle(press(KeyCode::Left));
//! assert!(moved.consumed && !moved.changed);
//!
//! let theme = Theme::default();
//! let row = TextField::new(&line).fill(true).line(8, &theme);
//! assert_eq!(row.width(), 8);
//! ```
//!
//! # What a host keeps
//!
//! What the field is for: its label, what `Enter` and `Esc` do, what a
//! change triggers (a refilter, a validation, clearing an error), and
//! whether a paste reaches it (the overlay contract's
//! [`takes_text`](crate::overlay::Overlay::takes_text)).

mod field;
mod line_edit;

pub use field::TextField;
pub use line_edit::{Edit, LineEdit, single_line_paste};
// The editing keys are a standard key table, kept with the others in
// `keys`; a text field's host reaches them here too.
pub use crate::keys::{EditKey, LINE_EDIT_KEYS};
