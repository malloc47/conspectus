//! The TUI's own overlay key tables (ADR 0123, ADR 0160).
//!
//! The key-table model and the standard tables (menus, dialogs, pagers,
//! text prompts, checklists, palettes, settings panels, forms) are
//! `tuikit::keys`; the picker is a palette on the standard palette table,
//! Controls is a settings panel on the standard settings table, and quick
//! select and content search are palettes on tables of their own (their
//! options pane takes the standard palette option keys). This module holds
//! the tables of the overlays that are the TUI's own: search, the help
//! page, the value modal, Messages (the standard log list table with its
//! copy key), the three forms (built on the standard form rows), and the
//! pane viewer's own keys (the framework's pager takes the rest; the
//! transcript viewer's toggles are the viewer's own table); and the modal
//! key convention every modal is checked against.
//!
//! Every overlay resolves its keys through one of these tables or a
//! standard one and renders its footer from the same table, so the
//! footer cannot advertise a key the handler does not act on. Each
//! overlay matches its action enum exhaustively: a row whose action the
//! handler ignores does not compile. Keys no row binds fall through to
//! the overlay's text field, if it has one.

use ratatui::crossterm::event::KeyCode;
use tuikit::glyphs::marks;
use tuikit::keys::{
    Convention, EnterMeans, FORM_KEYS, FORM_OPTION_KEYS, FormKey, KeyAction, KeyRole, KeyRow,
    LOG_LIST_KEYS, LogListKey, PAGER_KEYS, PagerKey, SpaceMeans, alias, ch, ctrl, include, key,
    shown,
};

use crate::tui::widgets::pane_viewer::PaneKey;

// ---------------------------------------------------------------------
// Search: quick select and content search, two palettes; their options
// pane takes `tuikit::keys::PALETTE_OPTION_KEYS`. Unbound keys, `j` and
// `k` included, edit the query.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKey {
    Abort,
    Cancel,
    Pick,
    Down,
    Up,
    /// Move the cursor up by the rows the list shows, stopping at the
    /// first result.
    PageUp,
    /// Move the cursor down by the rows the list shows, stopping at the
    /// last result.
    PageDown,
    /// Quick select: switch between the current view and everything.
    Scope,
    /// Contents scope: list sessions, or the matching messages.
    Granularity,
    /// Contents scope: open the search options.
    Options,
    /// Contents scope: keep the query as the filter on the lists.
    ApplyFilter,
    /// Contents scope: keep the search and open the Search view on it.
    Keep,
    /// Contents scope: fill the field from the previous search string.
    PrevQuery,
    /// Contents scope: fill the field from the next search string, or
    /// empty it past the newest.
    NextQuery,
}

impl KeyAction for SearchKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Cancel => KeyRole::Leave,
            Self::Pick => KeyRole::Apply,
            Self::Down | Self::Up => KeyRole::Move,
            Self::PageUp | Self::PageDown => KeyRole::Page,
            Self::Scope
            | Self::Granularity
            | Self::Options
            | Self::ApplyFilter
            | Self::Keep
            | Self::PrevQuery
            | Self::NextQuery => KeyRole::Other,
        }
    }
}

/// `/` quick select. Tab and Shift-Tab both switch the reach, which is
/// only ever one of two (and the same in the contents modal).
pub const SEARCH_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    shown(key(KeyCode::Up), SearchKey::Up, marks::ARROW_UP, "move"),
    shown(
        key(KeyCode::Down),
        SearchKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
    alias(key(KeyCode::PageUp), SearchKey::PageUp),
    alias(key(KeyCode::PageDown), SearchKey::PageDown),
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "pick"),
    shown(
        key(KeyCode::Tab),
        SearchKey::Scope,
        "Tab",
        "search current/all",
    ),
    alias(key(KeyCode::BackTab), SearchKey::Scope),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
];

/// Searching what was said in the sessions and in mux scrollback.
pub const SEARCH_CONTENT_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    shown(key(KeyCode::Up), SearchKey::Up, marks::ARROW_UP, "move"),
    shown(
        key(KeyCode::Down),
        SearchKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
    alias(key(KeyCode::PageUp), SearchKey::PageUp),
    alias(key(KeyCode::PageDown), SearchKey::PageDown),
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "open hit"),
    shown(
        key(KeyCode::Tab),
        SearchKey::Scope,
        "Tab",
        "search current/all",
    ),
    shown(
        key(KeyCode::BackTab),
        SearchKey::Granularity,
        "S-Tab",
        "list sessions/messages",
    ),
    shown(ctrl('r'), SearchKey::PrevQuery, "Ctrl-R", "recall search"),
    alias(ctrl('t'), SearchKey::NextQuery),
    shown(ctrl('o'), SearchKey::Options, "Ctrl-O", "set options"),
    shown(ctrl('f'), SearchKey::ApplyFilter, "Ctrl-F", "filter lists"),
    shown(ctrl('s'), SearchKey::Keep, "Ctrl-S", "open Search view"),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
];

// ---------------------------------------------------------------------
// Pagers: the `?` help page and the `o` value modal, each the standard
// pager plus the key that opened it; the value modal also copies.
// ---------------------------------------------------------------------

pub const HELP_KEYS: &[KeyRow<PagerKey>] = &[
    include(PAGER_KEYS),
    shown(ch('?'), PagerKey::Close, "?", "close"),
];

pub const VALUE_MODAL_KEYS: &[KeyRow<PagerKey>] = &[
    include(PAGER_KEYS),
    shown(ch('o'), PagerKey::Close, "o", "close"),
    shown(ch('y'), PagerKey::Copy, "y", "copy"),
];

// ---------------------------------------------------------------------
// `!` messages log: the framework's log list on its standard table, `y`
// copying the selected entry, and `!` closing as it opened.
// ---------------------------------------------------------------------

pub const MESSAGES_KEYS: &[KeyRow<LogListKey>] = &[
    include(LOG_LIST_KEYS),
    shown(ch('y'), LogListKey::Copy, "y", "copy"),
    alias(ch('!'), LogListKey::Close),
];

// ---------------------------------------------------------------------
// Multi-field forms: the new bare mux form, the mux launch form, and the
// pin edit form, on the standard form rows. Up and Down always move
// between fields; Tab completes in a path field and moves on elsewhere;
// Enter submits from any field, with the form's own verb. Option keys act
// on the choice and toggle rows; on a text field they edit the text.
// Unbound keys edit the focused field.
// ---------------------------------------------------------------------

pub const NEW_MUX_KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "create"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

pub const MUX_LAUNCH_KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "launch"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

pub const PIN_EDIT_KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(ctrl('r'), FormKey::Inherit, "Ctrl-R", "follow again"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

// ---------------------------------------------------------------------
// The pane viewer is the framework's pager (`tuikit::keys::READER_KEYS`);
// these are its content's own keys, which the pager resolves after its
// table. `a` is shown; the wrap chip in the footer names `w`. The
// transcript viewer is the same pager; its content's keys (the toggles)
// are the viewer's own (`crate::viewer::keys::TRANSCRIPT_KEYS`).
// ---------------------------------------------------------------------

impl KeyAction for PaneKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Attach | Self::Wrap => KeyRole::Other,
        }
    }
}

pub const PANE_KEYS: &[KeyRow<PaneKey>] = &[
    shown(ch('a'), PaneKey::Attach, "a", "attach"),
    alias(ch('w'), PaneKey::Wrap),
];

// ---------------------------------------------------------------------
// The modal key convention. One set of meanings for Esc, Enter, and
// Space across every modal; a modal that departs from it says why here,
// and a consistency test checks each modal's real key tables against
// this table by the role of each action (`tuikit::keys::Convention`).
//
//   Esc    leaves the modal without applying anything that is still
//          pending. (Controls applies as you go, so what it already
//          applied stays.)
//   Enter  applies or submits the modal's primary action; in a list it
//          activates the focused item.
//   Space  changes the focused item: toggles a checkbox, steps an
//          option, selects a choice. In a text field it types a space.
// ---------------------------------------------------------------------

const fn conv(
    modal: &'static str,
    enter: EnterMeans,
    space: SpaceMeans,
    exception: &'static str,
) -> Convention {
    Convention::new(modal, enter, space, exception)
}

/// Every modal against the convention. Esc leaves every one of them.
pub const MODAL_CONVENTION: &[Convention] = &[
    conv(
        "Help",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a pager: Space pages down",
    ),
    conv(
        "Controls",
        EnterMeans::Activates,
        SpaceMeans::Toggles,
        "changes apply as they are made, so Esc keeps them",
    ),
    conv("Rename", EnterMeans::Submits, SpaceMeans::Types, ""),
    conv("Prompt", EnterMeans::Submits, SpaceMeans::Types, ""),
    conv("Checklist", EnterMeans::Submits, SpaceMeans::Toggles, ""),
    conv("Dialog", EnterMeans::Submits, SpaceMeans::Types, ""),
    conv("Search", EnterMeans::Activates, SpaceMeans::Types, ""),
    conv(
        "ValueModal",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a pager: Space pages down",
    ),
    conv(
        "Messages",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a pager: Space pages the detail",
    ),
    conv(
        "Viewer",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a pager: Space pages down",
    ),
    conv(
        "PaneViewer",
        EnterMeans::Nothing,
        SpaceMeans::Pages,
        "a pager: Space pages down",
    ),
    conv("ViewerFind", EnterMeans::Submits, SpaceMeans::Types, ""),
    conv("WorktreeMenu", EnterMeans::Activates, SpaceMeans::Types, ""),
    conv("NewMux", EnterMeans::Submits, SpaceMeans::Types, ""),
    conv("MuxMenu", EnterMeans::Activates, SpaceMeans::Types, ""),
    conv("MuxLaunch", EnterMeans::Submits, SpaceMeans::Toggles, ""),
    conv("PinEdit", EnterMeans::Submits, SpaceMeans::Toggles, ""),
    conv("RowMenu", EnterMeans::Activates, SpaceMeans::Types, ""),
    conv("CleanUp", EnterMeans::Submits, SpaceMeans::Toggles, ""),
    conv("Picker", EnterMeans::Activates, SpaceMeans::Types, ""),
];

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
