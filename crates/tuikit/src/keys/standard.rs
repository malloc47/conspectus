//! The standard key tables and the action enums they bind: one table per
//! standard control, so every menu, dialog, pager, prompt, checklist,
//! palette, settings panel, and form takes the same keys.
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

/// What a dialog's keys do: the one table every
/// [`Dialog`](crate::dialog::Dialog) resolves with, whatever its form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogKey {
    /// Dismiss the dialog and the overlay it is a step of (`Ctrl-C`,
    /// `q`).
    Abort,
    /// Scroll a body taller than the dialog up a row.
    Up,
    /// Scroll a body taller than the dialog down a row.
    Down,
    /// Go ahead: a confirmation's yes, a notice's dismissal.
    Confirm,
    /// Do not go ahead: leave the dialog, one level back.
    Cancel,
}

/// Every dialog: `Enter/y confirm · Esc/n cancel` in a confirmation.
/// `Ctrl-C` and `q` dismiss it; the arrows and `k`/`j` scroll a body
/// taller than the dialog. A choice dialog shows its verbs' own keys and
/// then `Esc/n cancel`; a notice shows `Enter/Esc close`.
pub const DIALOG_KEYS: &[KeyRow<DialogKey>] = &[
    alias(ctrl('c'), DialogKey::Abort),
    alias(ch('q'), DialogKey::Abort),
    alias(key(KeyCode::Up), DialogKey::Up),
    alias(ch('k'), DialogKey::Up),
    alias(key(KeyCode::Down), DialogKey::Down),
    alias(ch('j'), DialogKey::Down),
    shown(key(KeyCode::Enter), DialogKey::Confirm, "Enter", "confirm"),
    shown(ch('y'), DialogKey::Confirm, "y", "confirm"),
    shown(key(KeyCode::Esc), DialogKey::Cancel, "Esc", "cancel"),
    shown(ch('n'), DialogKey::Cancel, "n", "cancel"),
];

impl KeyAction for DialogKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::Confirm => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
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
}

/// A single-line text prompt: `Enter confirm · Esc cancel`.
///
/// The prompt binds only what a prompt means; every other key goes to
/// the line editor, whose own table
/// ([`LINE_EDIT_KEYS`](crate::text_edit::LINE_EDIT_KEYS)) holds the
/// editing keys, word motion included.
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
];

impl KeyAction for TextInputKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Confirm => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
            Self::Tab => KeyRole::Other,
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

/// What a path field's own key does. Keys no row binds edit the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathKey {
    /// Add what the best completion adds to the typed path.
    Complete,
}

/// A path field's own key: `Tab` completes the path
/// ([`PathField::handle`](crate::path_field::PathField::handle)).
///
/// A part of a field, not an overlay's whole table: a form resolves its
/// own `Tab` row first and asks the field to complete, moving on to the
/// next field when there is nothing to complete, and the field's other
/// keys are [`LINE_EDIT_KEYS`]. The row is an alias because the form's
/// footer names `Tab` (`Tab complete/next`); the field's hint writes the
/// key from this row.
pub const PATH_FIELD_KEYS: &[KeyRow<PathKey>] = &[alias(key(KeyCode::Tab), PathKey::Complete)];

impl KeyAction for PathKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Complete => KeyRole::Other,
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
    /// Move the cursor up by the rows the list shows.
    PageUp,
    /// Move the cursor down by the rows the list shows.
    PageDown,
    /// Move the cursor to the first item.
    Top,
    /// Move the cursor to the last item.
    Bottom,
    /// Check or uncheck the item under the cursor.
    Toggle,
    /// Check every item that can change.
    SelectAll,
    /// Uncheck every item that can change.
    SelectNone,
    /// Take the checked items.
    Confirm,
    /// Leave without taking them.
    Cancel,
}

/// A checklist: `↑/↓ move · Space toggle · a select all · n select none ·
/// Enter confirm · Esc cancel`. `q` dismisses it, as `Ctrl-C` does;
/// `PgUp`/`PgDn` move by the rows shown, and `g`/`Home` and `G`/`End` go
/// to the ends.
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
    alias(key(KeyCode::PageUp), ChecklistKey::PageUp),
    alias(key(KeyCode::PageDown), ChecklistKey::PageDown),
    alias(ch('g'), ChecklistKey::Top),
    alias(key(KeyCode::Home), ChecklistKey::Top),
    alias(ch('G'), ChecklistKey::Bottom),
    alias(key(KeyCode::End), ChecklistKey::Bottom),
    shown(ch(' '), ChecklistKey::Toggle, "Space", "toggle"),
    shown(ch('a'), ChecklistKey::SelectAll, "a", "select all"),
    shown(ch('n'), ChecklistKey::SelectNone, "n", "select none"),
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
            Self::Up | Self::Down | Self::Top | Self::Bottom => KeyRole::Move,
            Self::PageUp | Self::PageDown => KeyRole::Page,
            Self::Toggle | Self::SelectAll | Self::SelectNone => KeyRole::Change,
            Self::Confirm => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
        }
    }
}

/// What a settings panel's keys do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsKey {
    /// Dismiss the panel and every step of it (`Ctrl-C`, `q`).
    Abort,
    /// Move the cursor to the previous row.
    Up,
    /// Move the cursor to the next row.
    Down,
    /// Move the cursor up by the rows the panel shows.
    PageUp,
    /// Move the cursor down by the rows the panel shows.
    PageDown,
    /// Move the cursor to the first row.
    Top,
    /// Move the cursor to the last row.
    Bottom,
    /// Act on the row under the cursor: select a choice, toggle a
    /// checkbox, step a cycle, open an editor, press a button.
    Change,
    /// Apply the choice under the cursor and close; open an editor or
    /// press a button as `Change` does; close on any other row, whose
    /// change is already applied.
    Apply,
    /// Close the panel, keeping every change already applied.
    Close,
}

/// A settings panel: `↑/↓ move · Space select/toggle · Enter apply &
/// close · Esc close · changes apply as you make them`. Nothing is typed
/// in the panel, so `k`/`j` move and `q` dismisses it, as `Ctrl-C` does;
/// `PgUp`/`PgDn` move by the rows shown, and `g`/`Home` and `G`/`End` go
/// to the ends (aliases).
pub const SETTINGS_KEYS: &[KeyRow<SettingsKey>] = &[
    alias(ctrl('c'), SettingsKey::Abort),
    alias(ch('q'), SettingsKey::Abort),
    shown(key(KeyCode::Up), SettingsKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), SettingsKey::Up),
    alias(ctrl('p'), SettingsKey::Up),
    shown(
        key(KeyCode::Down),
        SettingsKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ch('j'), SettingsKey::Down),
    alias(ctrl('n'), SettingsKey::Down),
    alias(key(KeyCode::PageUp), SettingsKey::PageUp),
    alias(key(KeyCode::PageDown), SettingsKey::PageDown),
    alias(ch('g'), SettingsKey::Top),
    alias(key(KeyCode::Home), SettingsKey::Top),
    alias(ch('G'), SettingsKey::Bottom),
    alias(key(KeyCode::End), SettingsKey::Bottom),
    shown(ch(' '), SettingsKey::Change, "Space", "select/toggle"),
    shown(
        key(KeyCode::Enter),
        SettingsKey::Apply,
        "Enter",
        "apply & close",
    ),
    shown(key(KeyCode::Esc), SettingsKey::Close, "Esc", "close"),
    KeyRow::Note("changes apply as you make them"),
];

impl KeyAction for SettingsKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down | Self::Top | Self::Bottom => KeyRole::Move,
            Self::PageUp | Self::PageDown => KeyRole::Page,
            Self::Change => KeyRole::Change,
            Self::Apply => KeyRole::Apply,
            Self::Close => KeyRole::Leave,
        }
    }
}

/// What a palette's keys do. Keys no row binds edit the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteKey {
    /// Dismiss the palette (`Ctrl-C`).
    Abort,
    /// Move the cursor to the previous result.
    Up,
    /// Move the cursor to the next result.
    Down,
    /// Move the cursor up by the rows the list shows.
    PageUp,
    /// Move the cursor down by the rows the list shows.
    PageDown,
    /// Pick the result under the cursor.
    Pick,
    /// Close the palette without picking.
    Cancel,
    /// Switch the palette's scope strip to its next choice. No standard
    /// table binds it; a palette with scopes binds its own key (`Tab`)
    /// under its own verb.
    Scope,
    /// Open the palette's options pane. No standard table binds it; a
    /// palette with options binds `Ctrl-O` under its own verb.
    Options,
}

/// A palette: `↑/↓ move · Enter pick · Esc close`. `PgUp`/`PgDn` move by
/// the rows shown (aliases). Characters are the query's, so `q`, `j`, and
/// `k` type; `Ctrl-C` dismisses. A palette with scopes or options builds
/// its table on this one with [`include()`](super::include()) and adds
/// its [`Scope`](PaletteKey::Scope) and [`Options`](PaletteKey::Options)
/// rows with its own verbs.
pub const PALETTE_KEYS: &[KeyRow<PaletteKey>] = &[
    alias(ctrl('c'), PaletteKey::Abort),
    shown(key(KeyCode::Up), PaletteKey::Up, marks::ARROW_UP, "move"),
    shown(
        key(KeyCode::Down),
        PaletteKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ctrl('n'), PaletteKey::Down),
    alias(ctrl('p'), PaletteKey::Up),
    alias(key(KeyCode::PageUp), PaletteKey::PageUp),
    alias(key(KeyCode::PageDown), PaletteKey::PageDown),
    shown(key(KeyCode::Enter), PaletteKey::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), PaletteKey::Cancel, "Esc", "close"),
];

impl KeyAction for PaletteKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::PageUp | Self::PageDown => KeyRole::Page,
            Self::Pick => KeyRole::Apply,
            Self::Cancel => KeyRole::Leave,
            Self::Scope | Self::Options => KeyRole::Other,
        }
    }
}

/// What the keys of a palette's options pane do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteOptionKey {
    /// Dismiss the whole palette (`Ctrl-C`, `q`).
    Abort,
    /// Move the cursor to the previous option.
    Up,
    /// Move the cursor to the next option.
    Down,
    /// Turn the option under the cursor on or off.
    Toggle,
    /// Close the pane and go back to the results.
    Back,
}

/// A palette's options pane: `↑/↓ move · Space toggle · Esc back`.
/// Nothing is typed in the pane, so `k`/`j` move and `q` dismisses the
/// palette; `Enter` toggles as `Space` does, and `Ctrl-O`, the key that
/// opens the pane, closes it again.
pub const PALETTE_OPTION_KEYS: &[KeyRow<PaletteOptionKey>] = &[
    alias(ctrl('c'), PaletteOptionKey::Abort),
    shown(
        key(KeyCode::Up),
        PaletteOptionKey::Up,
        marks::ARROW_UP,
        "move",
    ),
    shown(
        key(KeyCode::Down),
        PaletteOptionKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ctrl('n'), PaletteOptionKey::Down),
    alias(ctrl('p'), PaletteOptionKey::Up),
    alias(ch('k'), PaletteOptionKey::Up),
    alias(ch('j'), PaletteOptionKey::Down),
    shown(ch(' '), PaletteOptionKey::Toggle, "Space", "toggle"),
    alias(key(KeyCode::Enter), PaletteOptionKey::Toggle),
    shown(key(KeyCode::Esc), PaletteOptionKey::Back, "Esc", "back"),
    alias(ctrl('o'), PaletteOptionKey::Back),
    alias(ch('q'), PaletteOptionKey::Abort),
];

impl KeyAction for PaletteOptionKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::Toggle => KeyRole::Change,
            Self::Back => KeyRole::Leave,
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
    /// Flip the focused toggle or the option under a check group's
    /// cursor; on any other option row, step it on as
    /// [`OptionNext`](Self::OptionNext) does.
    Toggle,
    /// Drop the focused field's own value and take the inherited one
    /// again: an override row's reset. No standard table binds it; a form
    /// with override rows binds its own key to it.
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
/// text field these keys edit the text instead; the form decides. `Space`
/// is its own action ([`FormKey::Toggle`]) so a row of several checkboxes
/// can move between them with the arrows and flip one with `Space`.
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
    shown(ch(' '), FormKey::Toggle, "Space", "change"),
];

impl KeyAction for FormKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Cancel => KeyRole::Leave,
            Self::Up | Self::Down | Self::Back => KeyRole::Move,
            Self::Tab => KeyRole::Other,
            Self::Submit => KeyRole::Apply,
            Self::OptionPrev | Self::OptionNext | Self::Toggle | Self::Inherit => KeyRole::Change,
        }
    }
}
