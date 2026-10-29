//! The standard key tables and the action enums they bind: one table per
//! standard control, so every menu, dialog, pager, prompt, checklist, and
//! form takes the same keys.
//!
//! Each table starts with `Ctrl-C`, bound to its `Abort` action, and a
//! table where nothing is typed binds `q` to it as well (house rule 4). Lists
//! move with the arrows, `Ctrl-P` and `Ctrl-N`, and `k` and `j` where
//! nothing is typed; characters are bound as typed (see
//! [`KeyMatcher`](super::KeyMatcher)). A host table builds on one with
//! [`include()`](super::include()) rather than copying its rows.

use ratatui::crossterm::event::KeyCode;

use super::matcher::{alt, alt_key, ch, ctrl, ctrl_key, key};
use super::role::{KeyAction, KeyRole};
use super::table::{KeyRow, alias, shown};
use crate::glyphs::marks;

/// What a menu's keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuKey {
    /// Dismiss the menu (`Ctrl-C`, `q`).
    Abort,
    /// Move the cursor to the previous entry.
    Up,
    /// Move the cursor to the next entry.
    Down,
    /// Activate the entry under the cursor.
    Pick,
    /// Close the menu without picking.
    Close,
}

/// A menu: `↑/↓ move · Enter pick · Esc close`. `q` dismisses it too, as
/// it does every overlay where nothing is typed.
pub const MENU_KEYS: &[KeyRow<MenuKey>] = &[
    alias(ctrl('c'), MenuKey::Abort),
    alias(ch('q'), MenuKey::Abort),
    shown(key(KeyCode::Up), MenuKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), MenuKey::Up),
    alias(ctrl('p'), MenuKey::Up),
    shown(key(KeyCode::Down), MenuKey::Down, marks::ARROW_DOWN, "move"),
    alias(ch('j'), MenuKey::Down),
    alias(ctrl('n'), MenuKey::Down),
    shown(key(KeyCode::Enter), MenuKey::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), MenuKey::Close, "Esc", "close"),
];

impl KeyAction for MenuKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::Pick => KeyRole::Apply,
            Self::Close => KeyRole::Leave,
        }
    }
}

/// What a confirmation dialog's keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfirmKey {
    /// Dismiss the dialog and the overlay it is in (`Ctrl-C`, `q`).
    Abort,
    /// Go ahead.
    Yes,
    /// Do not go ahead.
    No,
}

/// A yes-or-no dialog: `Enter/y confirm · Esc/n cancel`. `q` dismisses
/// it, as `Ctrl-C` does.
pub const CONFIRM_KEYS: &[KeyRow<ConfirmKey>] = &[
    alias(ctrl('c'), ConfirmKey::Abort),
    alias(ch('q'), ConfirmKey::Abort),
    shown(key(KeyCode::Enter), ConfirmKey::Yes, "Enter", "confirm"),
    shown(ch('y'), ConfirmKey::Yes, "y", "confirm"),
    shown(key(KeyCode::Esc), ConfirmKey::No, "Esc", "cancel"),
    shown(ch('n'), ConfirmKey::No, "n", "cancel"),
];

impl KeyAction for ConfirmKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Yes => KeyRole::Apply,
            Self::No => KeyRole::Leave,
        }
    }
}

/// What a choice dialog's keys do: one of the host's choices `C`, or
/// none.
///
/// A choice dialog's table is the host's, because its keys and verbs are
/// the choices: `Ctrl-C` first and `q` to [`ChoiceKey::Abort`], a shown
/// row per choice (`m merge & close`), then `Esc` shown as `cancel` to
/// [`ChoiceKey::Back`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChoiceKey<C> {
    /// Dismiss the dialog and the overlay it is in (`Ctrl-C`, `q`).
    Abort,
    /// Take this choice.
    Choose(C),
    /// Leave without choosing.
    Back,
}

impl<C: Copy> KeyAction for ChoiceKey<C> {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Choose(_) => KeyRole::Apply,
            Self::Back => KeyRole::Leave,
        }
    }
}

/// What a pager's keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PagerKey {
    /// Dismiss the pager (`Ctrl-C`).
    Abort,
    /// Leave the pager.
    Close,
    /// Scroll down a row.
    Down,
    /// Scroll up a row.
    Up,
    /// Scroll down a page.
    PageDown,
    /// Scroll up a page.
    PageUp,
    /// Go to the first row.
    Top,
    /// Go to the last row.
    Bottom,
    /// Copy the content (a pager whose table binds it).
    Copy,
}

/// A pager: `j/k/PgDn/PgUp scroll · Esc/q close`, with `Space` paging
/// down, `g`/`Home` and `G`/`End` going to the ends, and the arrows
/// scrolling. A pager opened by a key adds that key as another close.
pub const PAGER_KEYS: &[KeyRow<PagerKey>] = &[
    alias(ctrl('c'), PagerKey::Abort),
    shown(ch('j'), PagerKey::Down, "j", "scroll"),
    alias(key(KeyCode::Down), PagerKey::Down),
    shown(ch('k'), PagerKey::Up, "k", "scroll"),
    alias(key(KeyCode::Up), PagerKey::Up),
    shown(key(KeyCode::PageDown), PagerKey::PageDown, "PgDn", "scroll"),
    alias(ch(' '), PagerKey::PageDown),
    shown(key(KeyCode::PageUp), PagerKey::PageUp, "PgUp", "scroll"),
    alias(ch('g'), PagerKey::Top),
    alias(key(KeyCode::Home), PagerKey::Top),
    alias(ch('G'), PagerKey::Bottom),
    alias(key(KeyCode::End), PagerKey::Bottom),
    shown(key(KeyCode::Esc), PagerKey::Close, "Esc", "close"),
    shown(ch('q'), PagerKey::Close, "q", "close"),
];

impl KeyAction for PagerKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Close => KeyRole::Leave,
            Self::Down | Self::Up | Self::Top | Self::Bottom => KeyRole::Move,
            Self::PageDown | Self::PageUp => KeyRole::Page,
            Self::Copy => KeyRole::Other,
        }
    }
}

/// What a single-line text prompt's keys do. Keys no row binds edit the
/// text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextInputKey {
    /// Dismiss the prompt (`Ctrl-C`).
    Abort,
    /// Take the text.
    Confirm,
    /// Leave without taking the text.
    Cancel,
    /// Swallowed, so a tab never reaches the text.
    Tab,
    /// Move to the start of the word before the cursor.
    WordLeft,
    /// Move to the start of the word after the cursor.
    WordRight,
}

/// A single-line text prompt: `Enter confirm · Esc cancel`.
///
/// `Alt-b` and `Alt-f` move by words: a terminal sends Alt as an Esc
/// prefix, which arrives as the Alt modifier, so the word motions are
/// bound here rather than left to the edit engine.
pub const TEXT_INPUT_KEYS: &[KeyRow<TextInputKey>] = &[
    alias(ctrl('c'), TextInputKey::Abort),
    shown(
        key(KeyCode::Enter),
        TextInputKey::Confirm,
        "Enter",
        "confirm",
    ),
    shown(key(KeyCode::Esc), TextInputKey::Cancel, "Esc", "cancel"),
    alias(key(KeyCode::Tab), TextInputKey::Tab),
    alias(alt('b'), TextInputKey::WordLeft),
    alias(alt('f'), TextInputKey::WordRight),
];

impl KeyAction for TextInputKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Confirm => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
            Self::Tab => KeyRole::Other,
            Self::WordLeft | Self::WordRight => KeyRole::Move,
        }
    }
}

/// What an editing key does in a text field. [`LINE_EDIT_KEYS`] binds
/// every variant but [`Insert`](Self::Insert), which is the character a key
/// types; the line editor
/// ([`LineEdit`](crate::text_edit::LineEdit)) applies them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditKey {
    /// Type a character at the cursor.
    Insert(char),
    /// Move one character (one grapheme) left.
    Left,
    /// Move one character right.
    Right,
    /// Move to the start of the word before the cursor.
    WordLeft,
    /// Move to the start of the word after the cursor, or to the end.
    WordRight,
    /// Move to the start of the line.
    Home,
    /// Move to the end of the line.
    End,
    /// Delete the character before the cursor.
    DeleteBefore,
    /// Delete the character under the cursor.
    DeleteAfter,
    /// Delete back to the start of the word before the cursor.
    DeleteWordBefore,
    /// Delete forward to the start of the next word.
    DeleteWordAfter,
    /// Delete the whole line.
    DeleteLine,
    /// Delete from the cursor to the end of the line.
    DeleteToEnd,
    /// Type what the last deletion of a word, the rest of the line, or
    /// the line removed.
    Yank,
}

/// The editing keys of every text field: readline's keys, with word
/// motion on both the Alt letters and the Control and Alt arrows.
///
/// A terminal sends Alt as an Esc prefix, which arrives as the Alt
/// modifier, so `Alt-b`, `Alt-f`, `Alt-d`, and `Alt-Backspace` are bound
/// to Alt here (the edit engine underneath binds them to Meta, which a
/// terminal does not report). Chords come before the plain keys they
/// share a code with, because a key that is not a character matches with
/// any modifiers: a stray Shift on `←` still moves. Every row is an
/// alias: a field's footer names the keys of its host (`Enter`, `Esc`),
/// not the editing keys every field shares. `Enter`, `Esc`, and `Tab`
/// are not here; what they mean is the host's.
pub const LINE_EDIT_KEYS: &[KeyRow<EditKey>] = &[
    alias(ctrl_key(KeyCode::Left), EditKey::WordLeft),
    alias(alt_key(KeyCode::Left), EditKey::WordLeft),
    alias(alt('b'), EditKey::WordLeft),
    alias(ctrl_key(KeyCode::Right), EditKey::WordRight),
    alias(alt_key(KeyCode::Right), EditKey::WordRight),
    alias(alt('f'), EditKey::WordRight),
    alias(key(KeyCode::Left), EditKey::Left),
    alias(ctrl('b'), EditKey::Left),
    alias(key(KeyCode::Right), EditKey::Right),
    alias(ctrl('f'), EditKey::Right),
    alias(key(KeyCode::Home), EditKey::Home),
    alias(ctrl('a'), EditKey::Home),
    alias(key(KeyCode::End), EditKey::End),
    alias(ctrl('e'), EditKey::End),
    alias(alt_key(KeyCode::Backspace), EditKey::DeleteWordBefore),
    alias(key(KeyCode::Backspace), EditKey::DeleteBefore),
    alias(ctrl('h'), EditKey::DeleteBefore),
    alias(ctrl_key(KeyCode::Delete), EditKey::DeleteWordAfter),
    alias(key(KeyCode::Delete), EditKey::DeleteAfter),
    alias(ctrl('w'), EditKey::DeleteWordBefore),
    alias(alt('d'), EditKey::DeleteWordAfter),
    alias(ctrl('u'), EditKey::DeleteLine),
    alias(ctrl('k'), EditKey::DeleteToEnd),
    alias(ctrl('y'), EditKey::Yank),
];

impl KeyAction for EditKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Left
            | Self::Right
            | Self::WordLeft
            | Self::WordRight
            | Self::Home
            | Self::End => KeyRole::Move,
            Self::Insert(_)
            | Self::DeleteBefore
            | Self::DeleteAfter
            | Self::DeleteWordBefore
            | Self::DeleteWordAfter
            | Self::DeleteLine
            | Self::DeleteToEnd
            | Self::Yank => KeyRole::Other,
        }
    }
}

/// What a checklist's keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChecklistKey {
    /// Dismiss the checklist (`Ctrl-C`, `q`).
    Abort,
    /// Move the cursor to the previous item.
    Up,
    /// Move the cursor to the next item.
    Down,
    /// Check or uncheck the item under the cursor.
    Toggle,
    /// Take the checked items.
    Confirm,
    /// Leave without taking them.
    Cancel,
}

/// A checklist: `↑/↓ move · Space toggle · Enter confirm · Esc cancel`.
/// `q` dismisses it, as `Ctrl-C` does.
pub const CHECKLIST_KEYS: &[KeyRow<ChecklistKey>] = &[
    alias(ctrl('c'), ChecklistKey::Abort),
    alias(ch('q'), ChecklistKey::Abort),
    shown(key(KeyCode::Up), ChecklistKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), ChecklistKey::Up),
    alias(ctrl('p'), ChecklistKey::Up),
    shown(
        key(KeyCode::Down),
        ChecklistKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ch('j'), ChecklistKey::Down),
    alias(ctrl('n'), ChecklistKey::Down),
    shown(ch(' '), ChecklistKey::Toggle, "Space", "toggle"),
    shown(
        key(KeyCode::Enter),
        ChecklistKey::Confirm,
        "Enter",
        "confirm",
    ),
    shown(key(KeyCode::Esc), ChecklistKey::Cancel, "Esc", "cancel"),
];

impl KeyAction for ChecklistKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::Toggle => KeyRole::Change,
            Self::Confirm => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
        }
    }
}

/// What a form's keys do. Keys no row binds edit the focused field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormKey {
    /// Dismiss the form (`Ctrl-C`).
    Abort,
    /// Leave without submitting.
    Cancel,
    /// Move to the previous field.
    Up,
    /// Move to the next field.
    Down,
    /// Complete the focused path field, or move to the next field.
    Tab,
    /// Move to the previous field.
    Back,
    /// Submit the form.
    Submit,
    /// Step the focused option back.
    OptionPrev,
    /// Step the focused option on, or flip the focused toggle.
    OptionNext,
    /// Drop the focused field's own value and take the inherited one
    /// again.
    Inherit,
}

/// Moving through a form's fields: `↑/↓ move · Tab complete/next ·
/// S-Tab back`, after `Ctrl-C`. A form's table includes it first, then
/// [`FORM_OPTION_KEYS`] if it has option rows, its own rows, its
/// `Enter` row with the form's verb (`Enter save`), and `Esc cancel`.
pub const FORM_KEYS: &[KeyRow<FormKey>] = &[
    alias(ctrl('c'), FormKey::Abort),
    shown(key(KeyCode::Up), FormKey::Up, marks::ARROW_UP, "move"),
    alias(ctrl('p'), FormKey::Up),
    shown(key(KeyCode::Down), FormKey::Down, marks::ARROW_DOWN, "move"),
    alias(ctrl('n'), FormKey::Down),
    shown(key(KeyCode::Tab), FormKey::Tab, "Tab", "complete/next"),
    shown(key(KeyCode::BackTab), FormKey::Back, "S-Tab", "back"),
];

/// Changing a form's option and toggle rows: `←/→/Space change`. On a
/// text field these keys edit the text instead; the form decides.
pub const FORM_OPTION_KEYS: &[KeyRow<FormKey>] = &[
    shown(
        key(KeyCode::Left),
        FormKey::OptionPrev,
        marks::ARROW_LEFT,
        "change",
    ),
    shown(
        key(KeyCode::Right),
        FormKey::OptionNext,
        marks::ARROW_RIGHT,
        "change",
    ),
    shown(ch(' '), FormKey::OptionNext, "Space", "change"),
];

impl KeyAction for FormKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Cancel => KeyRole::Leave,
            Self::Up | Self::Down | Self::Back => KeyRole::Move,
            Self::Tab => KeyRole::Other,
            Self::Submit => KeyRole::Apply,
            Self::OptionPrev | Self::OptionNext | Self::Inherit => KeyRole::Change,
        }
    }
}
