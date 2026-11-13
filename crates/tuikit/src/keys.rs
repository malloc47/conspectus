//! Keys: the key-table model every overlay resolves its keys through and
//! generates its footer from, and the scoped keymap a program's own keys
//! live in.
//!
//! This is a layer 2 convention. An overlay's keys are a `const` table
//! of [`KeyRow`]s over the overlay's own action enum `A`. The overlay
//! asks [`resolve`] what a key means and matches on the action
//! exhaustively, so a bound key the overlay ignores fails to compile;
//! its footer is [`footer`] (or [`Footer`]) of the same table, so a
//! footer cannot advertise a key the overlay does not act on.
//!
//! - **Matching.** A row's [`KeyMatcher`] says which key events it binds.
//!   The one rule for characters: [`ch`] binds the character as typed,
//!   alone or with Shift, and never a Control or Alt chord of it; chords
//!   are spelled out ([`ctrl`], [`alt`], [`ctrl_key`]); keys that are not
//!   characters ignore their modifiers ([`key`]), or take none at all
//!   when a chord of them means something else ([`exact`]). So `g` and
//!   `G` jump to a pager's ends and nothing else does.
//! - **Rows.** [`shown`] rows appear in the footer as `label verb`,
//!   [`alias`] rows are bound but left out, [`KeyRow::Note`] adds text
//!   with no key, and [`include()`] splices in another table of the same
//!   actions, which is how a table builds on a standard one. Each bound
//!   row carries whether its action is available now
//!   ([`with_availability`]); a footer and a help page dim one that is
//!   not ([`unavailable_style`]).
//! - **Footers.** One grammar: the keys, a space, and a short imperative
//!   verb with no "to" (`Esc close`, `Enter pick`); labels that share a
//!   verb join with `/`; segments join with [`HINT_SEPARATOR`], the only
//!   place a footer wraps. Labels are the keys as [`key_label`] writes
//!   them.
//! - **Roles.** An action enum implements [`KeyAction`], saying what each
//!   action is for ([`KeyRole`]). [`Convention`] checks a modal's tables
//!   against the modal key convention (`Esc` leaves, `Enter` applies,
//!   `Space` changes the focused item) by role, not by name.
//! - **Checks.** [`mislabeled`], [`unresolved`], [`loose_characters`],
//!   and [`ctrl_c_first`] each return what a table gets wrong, so a host
//!   runs them over every table it has.
//! - **Standard tables** for the standard controls, with the action enums
//!   they bind: [`MENU_KEYS`], [`DIALOG_KEYS`] for every dialog (a
//!   choice's verbs are the dialog's own keys), [`PAGER_KEYS`] (its
//!   scrolling rows are [`PAGER_SCROLL_KEYS`]) with [`READER_KEYS`] for a
//!   pager that finds and has a help page, [`PAGER_HELP_KEYS`] for that
//!   help page, and [`FIND_KEYS`] for its find prompt,
//!   [`TEXT_INPUT_KEYS`], [`CHECKLIST_KEYS`], [`LOG_LIST_KEYS`] for a
//!   log's entries and the selected one's detail, [`PALETTE_KEYS`] with
//!   [`PALETTE_OPTION_KEYS`] for a palette's options pane,
//!   [`SETTINGS_KEYS`] for a settings panel,
//!   [`FORM_KEYS`] with [`FORM_OPTION_KEYS`], [`LINE_EDIT_KEYS`], the
//!   editing keys every text field shares, and [`PATH_FIELD_KEYS`], a path
//!   field's completion key.
//!
//! ```
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use tuikit::keys::{
//!     self, KeyRow, MENU_KEYS, MenuKey, PAGER_KEYS, PagerKey, ch, include, resolve, shown,
//! };
//!
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//! assert_eq!(resolve(MENU_KEYS, press(KeyCode::Char('j'))), Some(MenuKey::Down));
//! assert!(keys::footer(MENU_KEYS).ends_with("Esc close"));
//!
//! // A help page is a pager that `?` also closes.
//! const HELP_KEYS: &[KeyRow<PagerKey>] = &[
//!     include(PAGER_KEYS),
//!     shown(ch('?'), PagerKey::Close, "?", "close"),
//! ];
//! assert!(keys::footer(HELP_KEYS).ends_with("Esc/q/? close"));
//!
//! // Characters bind as typed: Ctrl-G is not `g`.
//! let ctrl_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
//! assert_eq!(resolve(HELP_KEYS, press(KeyCode::Char('g'))), Some(PagerKey::Top));
//! assert_eq!(resolve(HELP_KEYS, ctrl_g), None);
//!
//! // Every key the footer names does what it says.
//! assert!(keys::unresolved(HELP_KEYS).is_empty());
//! assert!(keys::mislabeled(HELP_KEYS).is_empty());
//! assert!(keys::ctrl_c_first(HELP_KEYS));
//! ```
//!
//! # The scoped keymap
//!
//! A program's own keys, the ones no overlay owns, are a [`Keymap`]: one
//! table of [`Bind`]ings in [`Scope`]s (the program's, a pane's while it
//! has focus, an overlay mode's while it is on). Each binding has a
//! stable id, its keys, the action it stands for, the short verb its hint
//! says, the help section and text its help row says, a rank for a status
//! bar, and, if it needs one, a predicate over the host's context saying
//! whether it can act now. A key is looked up in the scopes that have the
//! keys ([`ActiveScopes`]), the most particular first. Building the keymap
//! fails when two keys of one scope can match one key event, so no key
//! shadows another and the table's order is free to be the order a help
//! page and a status bar list the keys in. A pane control's own table
//! becomes a pane's scope through [`KeymapBuilder::table`].
//!
//! ```
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use tuikit::keys::{ActiveScopes, Bind, Keymap, Scope, bind, ch, exact};
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! enum Do {
//!     Open,
//!     Quit,
//! }
//!
//! const KEYS: &[Bind<Do>] = &[
//!     bind("row.open", Scope::Pane("list"), &[exact(KeyCode::Enter)], Do::Open)
//!         .verb("open")
//!         .help("Rows", "Open the selected row"),
//!     bind("quit", Scope::Global, &[ch('q')], Do::Quit).verb("quit").help("Program", "Quit"),
//! ];
//! let keymap = Keymap::builder().rows(KEYS).build().expect("no key shadows another");
//! let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
//! assert_eq!(
//!     keymap.resolve(ActiveScopes::pane("list"), enter).map(|b| b.action()),
//!     Some(Do::Open)
//! );
//! // Another pane has focus: `Enter` is not bound there.
//! assert!(keymap.resolve(ActiveScopes::pane("preview"), enter).is_none());
//! ```
//!
//! # What a host keeps
//!
//! The tables of its own overlays (their action enums, verbs, and notes),
//! what an action does, which actions are available now, and its
//! keymap's bindings, their words, and which scopes have the keys. The
//! help generator ([`help::sections_for`](crate::help::sections_for) and
//! [`help::keymap_sections`](crate::help::keymap_sections)) reads these
//! tables, and the status bar
//! ([`KeyHintBar`](crate::status_bar::KeyHintBar)) names their keys.

mod footer;
mod keymap;
mod matcher;
mod role;
mod standard;
mod table;

pub use footer::{Footer, HINT_SEPARATOR, Segment, footer, unavailable_style};
pub use keymap::{
    ActiveScopes, Bind, Binding, Conflict, Conflicts, Keymap, KeymapBuilder, Scope, bind,
};
pub use matcher::{KeyMatcher, alt, alt_key, ch, ctrl, ctrl_key, exact, key, key_label};
pub use role::{Convention, EnterMeans, KeyAction, KeyRole, SpaceMeans, ctrl_c_first};
pub use standard::{
    CHECKLIST_KEYS, ChecklistKey, DIALOG_KEYS, DialogKey, EditKey, FIND_KEYS, FORM_KEYS,
    FORM_OPTION_KEYS, FindKey, FormKey, LINE_EDIT_KEYS, LOG_LIST_KEYS, LogListKey, MENU_KEYS,
    MenuKey, PAGER_HELP_KEYS, PAGER_KEYS, PAGER_SCROLL_KEYS, PALETTE_KEYS, PALETTE_OPTION_KEYS,
    PATH_FIELD_KEYS, PagerKey, PaletteKey, PaletteOptionKey, PathKey, READER_KEYS, SETTINGS_KEYS,
    SettingsKey, TEXT_INPUT_KEYS, TextInputKey,
};
pub use table::{
    Hint, KeyRow, Rows, alias, include, loose_characters, mislabeled, resolve, rows, shown,
    typed_char, unresolved, with_availability,
};

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
