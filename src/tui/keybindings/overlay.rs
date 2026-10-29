//! The TUI's own overlay key tables (ADR 0123, ADR 0160).
//!
//! The key-table model and the standard tables (menus, dialogs, pagers,
//! text prompts, checklists, palettes, forms) are `tuikit::keys`; the
//! picker is a palette on the standard palette table, and quick select and
//! content search are palettes on tables of their own (their options pane
//! takes the standard palette option keys). This module holds the tables
//! of the overlays that are the TUI's own: search, Controls, the help
//! page, the value modal, Messages, the worktree close-down choice, the
//! three forms (built on the standard form rows), the path field, the
//! transcript and pane viewers, and the viewers' find prompt; and the
//! modal key convention every modal is checked against.
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
    PAGER_KEYS, PagerKey, SpaceMeans, alias, ch, ctrl, include, key, shown,
};

use crate::tui::widgets::pane_viewer::PaneViewerMsg;
use crate::viewer::find::ViewerFindMsg;
use crate::viewer::input::ViewerMsg;

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
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
    shown(
        key(KeyCode::Tab),
        SearchKey::Scope,
        "Tab",
        "search current/all",
    ),
    alias(key(KeyCode::BackTab), SearchKey::Scope),
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
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "open hit"),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
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
];

// ---------------------------------------------------------------------
// Controls: a panel whose changes apply as they are made. Space acts on
// the focused row (selects a choice, toggles a checkbox, opens an
// editor); Enter applies the focused choice and closes; Esc closes with
// everything already applied kept.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsKey {
    Abort,
    /// `q`: dismisses the overlay, except in an editor that takes text,
    /// where it types.
    Quit,
    Up,
    Down,
    /// Select the focused choice, toggle the focused checkbox, or open
    /// the focused row's editor.
    Toggle,
    /// Apply the focused choice (if it is one) and close.
    Done,
    Close,
}

impl KeyAction for ControlsKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort | Self::Quit => KeyRole::Abort,
            Self::Up | Self::Down => KeyRole::Move,
            Self::Toggle => KeyRole::Change,
            Self::Done => KeyRole::Apply,
            Self::Close => KeyRole::Leave,
        }
    }
}

pub const CONTROLS_KEYS: &[KeyRow<ControlsKey>] = &[
    alias(ctrl('c'), ControlsKey::Abort),
    alias(ch('q'), ControlsKey::Quit),
    shown(key(KeyCode::Up), ControlsKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), ControlsKey::Up),
    alias(ctrl('p'), ControlsKey::Up),
    shown(
        key(KeyCode::Down),
        ControlsKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ch('j'), ControlsKey::Down),
    alias(ctrl('n'), ControlsKey::Down),
    shown(ch(' '), ControlsKey::Toggle, "Space", "select/toggle"),
    shown(
        key(KeyCode::Enter),
        ControlsKey::Done,
        "Enter",
        "apply & close",
    ),
    shown(key(KeyCode::Esc), ControlsKey::Close, "Esc", "close"),
    KeyRow::Note("changes apply as you make them"),
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
// `!` messages log.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagesKey {
    Abort,
    Close,
    Next,
    Prev,
    First,
    Last,
    DetailDown,
    DetailUp,
    DetailPageDown,
    DetailPageUp,
    /// Copy the selected entry. The overlay asks for it with a message,
    /// because the clipboard write is the executor's.
    Copy,
}

impl KeyAction for MessagesKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Abort => KeyRole::Abort,
            Self::Close => KeyRole::Leave,
            Self::Next | Self::Prev | Self::First | Self::Last => KeyRole::Move,
            Self::DetailDown | Self::DetailUp => KeyRole::Move,
            Self::DetailPageDown | Self::DetailPageUp => KeyRole::Page,
            Self::Copy => KeyRole::Other,
        }
    }
}

pub const MESSAGES_KEYS: &[KeyRow<MessagesKey>] = &[
    alias(ctrl('c'), MessagesKey::Abort),
    shown(ch('j'), MessagesKey::Next, "j", "select"),
    alias(key(KeyCode::Down), MessagesKey::Next),
    alias(ctrl('n'), MessagesKey::Next),
    shown(ch('k'), MessagesKey::Prev, "k", "select"),
    alias(key(KeyCode::Up), MessagesKey::Prev),
    alias(ctrl('p'), MessagesKey::Prev),
    alias(ch('g'), MessagesKey::First),
    alias(key(KeyCode::Home), MessagesKey::First),
    alias(ch('G'), MessagesKey::Last),
    alias(key(KeyCode::End), MessagesKey::Last),
    shown(ch('J'), MessagesKey::DetailDown, "J", "scroll detail"),
    shown(ch('K'), MessagesKey::DetailUp, "K", "scroll detail"),
    alias(key(KeyCode::PageDown), MessagesKey::DetailPageDown),
    alias(ch(' '), MessagesKey::DetailPageDown),
    alias(key(KeyCode::PageUp), MessagesKey::DetailPageUp),
    shown(ch('y'), MessagesKey::Copy, "y", "copy"),
    shown(key(KeyCode::Esc), MessagesKey::Close, "Esc", "close"),
    alias(ch('q'), MessagesKey::Close),
    alias(ch('!'), MessagesKey::Close),
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
// Transcript and pane viewers. Each draws its own footer from its toggle
// state, behind the ADR 0052 extraction boundary, so these rows are all
// aliases; the keys still resolve here.
// ---------------------------------------------------------------------

impl KeyAction for PaneViewerMsg {
    fn role(self) -> KeyRole {
        match self {
            Self::ScrollUp | Self::ScrollDown | Self::JumpToStart | Self::JumpToEnd => {
                KeyRole::Move
            }
            Self::PageUp | Self::PageDown | Self::HalfPageUp | Self::HalfPageDown => KeyRole::Page,
            Self::FindOpen | Self::FindNext | Self::FindPrev | Self::ToggleHelp | Self::Attach => {
                KeyRole::Other
            }
            Self::Close => KeyRole::Leave,
            Self::Quit => KeyRole::Abort,
        }
    }
}

pub const PANE_VIEWER_KEYS: &[KeyRow<PaneViewerMsg>] = &[
    alias(ctrl('c'), PaneViewerMsg::Quit),
    alias(ctrl('d'), PaneViewerMsg::HalfPageDown),
    alias(ctrl('u'), PaneViewerMsg::HalfPageUp),
    alias(key(KeyCode::Esc), PaneViewerMsg::Close),
    alias(ch('q'), PaneViewerMsg::Quit),
    alias(ch('j'), PaneViewerMsg::ScrollDown),
    alias(key(KeyCode::Down), PaneViewerMsg::ScrollDown),
    alias(ch('k'), PaneViewerMsg::ScrollUp),
    alias(key(KeyCode::Up), PaneViewerMsg::ScrollUp),
    alias(key(KeyCode::PageDown), PaneViewerMsg::PageDown),
    alias(ch(' '), PaneViewerMsg::PageDown),
    alias(key(KeyCode::PageUp), PaneViewerMsg::PageUp),
    alias(ch('g'), PaneViewerMsg::JumpToStart),
    alias(key(KeyCode::Home), PaneViewerMsg::JumpToStart),
    alias(ch('G'), PaneViewerMsg::JumpToEnd),
    alias(key(KeyCode::End), PaneViewerMsg::JumpToEnd),
    alias(ch('/'), PaneViewerMsg::FindOpen),
    alias(ch('n'), PaneViewerMsg::FindNext),
    alias(ch('N'), PaneViewerMsg::FindPrev),
    alias(ch('a'), PaneViewerMsg::Attach),
    alias(ch('?'), PaneViewerMsg::ToggleHelp),
];

impl KeyAction for ViewerMsg {
    fn role(self) -> KeyRole {
        match self {
            Self::ScrollUp | Self::ScrollDown | Self::JumpToStart | Self::JumpToEnd => {
                KeyRole::Move
            }
            Self::PageUp | Self::PageDown | Self::HalfPageUp | Self::HalfPageDown => KeyRole::Page,
            Self::CycleToolDetail
            | Self::ToggleThinking
            | Self::ToggleAborted
            | Self::ToggleHidden
            | Self::FindOpen
            | Self::FindNext
            | Self::FindPrev
            | Self::ToggleHelp => KeyRole::Other,
            Self::Close => KeyRole::Leave,
            Self::Quit => KeyRole::Abort,
        }
    }
}

pub const VIEWER_KEYS: &[KeyRow<ViewerMsg>] = &[
    alias(ctrl('c'), ViewerMsg::Quit),
    alias(ctrl('d'), ViewerMsg::HalfPageDown),
    alias(ctrl('u'), ViewerMsg::HalfPageUp),
    alias(key(KeyCode::Esc), ViewerMsg::Close),
    alias(ch('q'), ViewerMsg::Quit),
    alias(ch('j'), ViewerMsg::ScrollDown),
    alias(key(KeyCode::Down), ViewerMsg::ScrollDown),
    alias(ch('k'), ViewerMsg::ScrollUp),
    alias(key(KeyCode::Up), ViewerMsg::ScrollUp),
    alias(key(KeyCode::PageDown), ViewerMsg::PageDown),
    alias(ch(' '), ViewerMsg::PageDown),
    alias(key(KeyCode::PageUp), ViewerMsg::PageUp),
    alias(ch('g'), ViewerMsg::JumpToStart),
    alias(key(KeyCode::Home), ViewerMsg::JumpToStart),
    alias(ch('G'), ViewerMsg::JumpToEnd),
    alias(key(KeyCode::End), ViewerMsg::JumpToEnd),
    alias(ch('t'), ViewerMsg::CycleToolDetail),
    alias(ch('T'), ViewerMsg::ToggleThinking),
    alias(ch('I'), ViewerMsg::ToggleAborted),
    alias(ch('H'), ViewerMsg::ToggleHidden),
    alias(ch('/'), ViewerMsg::FindOpen),
    alias(ch('n'), ViewerMsg::FindNext),
    alias(ch('N'), ViewerMsg::FindPrev),
    alias(ch('?'), ViewerMsg::ToggleHelp),
];

impl KeyAction for ViewerFindMsg {
    fn role(self) -> KeyRole {
        match self {
            Self::Quit => KeyRole::Abort,
            Self::Cancel => KeyRole::Leave,
            Self::Confirm => KeyRole::Apply,
            Self::Edit(edit) => edit.role(),
        }
    }
}

/// The find prompt inside the viewers. Keys no row binds go to the line
/// editor (`tuikit::text_edit::EditKey::of`), so `q` types and the
/// editing keys are every text field's; `Ctrl-C` leaves the viewer, as
/// it does from anywhere in it.
pub const VIEWER_FIND_KEYS: &[KeyRow<ViewerFindMsg>] = &[
    alias(ctrl('c'), ViewerFindMsg::Quit),
    shown(key(KeyCode::Enter), ViewerFindMsg::Confirm, "Enter", "find"),
    shown(key(KeyCode::Esc), ViewerFindMsg::Cancel, "Esc", "cancel"),
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
