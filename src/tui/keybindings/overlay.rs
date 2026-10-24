//! Overlay keymaps (ADR 0123).
//!
//! Every overlay resolves its keys through a `const` table of
//! [`KeyRow`]s and renders its footer hint from the same table, so the
//! footer cannot advertise a key the handler does not act on. Each
//! overlay has its own action enum and matches on it exhaustively: a
//! row whose action the handler ignores does not compile.
//!
//! Keys no row binds fall through to the overlay's text field, if it
//! has one. A key that means different things in different fields
//! (Space types in a text field but toggles an option field) resolves
//! to an action, and the handler decides what that action does where
//! the cursor is.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::KeyMatcher;

/// How a bound key appears in its overlay's footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    /// Shown as `label` under `verb`. Rows that share a verb share one
    /// footer segment, their labels joined with `/`: `↑/↓ move`.
    Shown {
        label: &'static str,
        verb: &'static str,
    },
    /// Bound but left out of the footer on purpose: vim-style
    /// aliases (`j`/`k`) and the universal Ctrl-C.
    Alias,
}

/// One row of an overlay keymap.
#[derive(Debug)]
pub enum KeyRow<A: 'static> {
    Bind {
        key: KeyMatcher,
        action: A,
        hint: Hint,
    },
    /// Footer text with no key of its own, such as "type to edit".
    Note(&'static str),
}

/// The action the first row matching `event` binds, if any.
pub fn resolve<A: Copy>(rows: &[KeyRow<A>], event: KeyEvent) -> Option<A> {
    rows.iter().find_map(|row| match row {
        KeyRow::Bind { key, action, .. } if key.matches(event.modifiers, event.code) => {
            Some(*action)
        }
        _ => None,
    })
}

/// The footer hint for `rows`: one segment per verb or note, in the
/// order each first appears, joined with ` · `.
pub fn footer<A>(rows: &[KeyRow<A>]) -> String {
    enum Segment {
        Keys {
            verb: &'static str,
            labels: Vec<&'static str>,
        },
        Note(&'static str),
    }
    let mut segments: Vec<Segment> = Vec::new();
    for row in rows {
        match row {
            KeyRow::Bind {
                hint: Hint::Shown { label, verb },
                ..
            } => {
                let existing = segments.iter_mut().find_map(|segment| match segment {
                    Segment::Keys { verb: v, labels } if v == verb => Some(labels),
                    _ => None,
                });
                match existing {
                    Some(labels) => labels.push(label),
                    None => segments.push(Segment::Keys {
                        verb,
                        labels: vec![label],
                    }),
                }
            }
            KeyRow::Bind {
                hint: Hint::Alias, ..
            } => {}
            KeyRow::Note(text) => segments.push(Segment::Note(text)),
        }
    }
    segments
        .iter()
        .map(|segment| match segment {
            Segment::Keys { verb, labels } => format!("{} {verb}", labels.join("/")),
            Segment::Note(text) => (*text).to_string(),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Rows whose footer label does not name the key they bind, as
/// `(label, bound key)` pairs. Empty when the table is consistent.
pub fn mislabeled<A>(rows: &[KeyRow<A>]) -> Vec<(&'static str, String)> {
    rows.iter()
        .filter_map(|row| match row {
            KeyRow::Bind {
                key,
                hint: Hint::Shown { label, .. },
                ..
            } if !label_names_key(label, key) => Some((*label, super::key_label(key))),
            _ => None,
        })
        .collect()
}

fn label_names_key(label: &str, key: &KeyMatcher) -> bool {
    let code = match key {
        KeyMatcher::Exact { code, .. } | KeyMatcher::AnyMod(code) => *code,
        KeyMatcher::UpperChar(c) | KeyMatcher::AnyModExceptCtrl(c) => KeyCode::Char(*c),
    };
    let ctrl = matches!(key, KeyMatcher::Exact { modifiers, .. } if modifiers.contains(KeyModifiers::CONTROL));
    let named = match code {
        KeyCode::Up => &["↑", "Up"][..],
        KeyCode::Down => &["↓", "Down"],
        KeyCode::Left => &["←", "Left"],
        KeyCode::Right => &["→", "Right"],
        KeyCode::BackTab => &["S-Tab", "Shift-Tab"],
        KeyCode::Char(' ') => &["Space"],
        KeyCode::PageUp => &["PgUp"],
        KeyCode::PageDown => &["PgDn"],
        _ => &[],
    };
    if named.contains(&label) {
        return true;
    }
    let base = super::key_label(&KeyMatcher::AnyMod(code));
    if ctrl {
        label.eq_ignore_ascii_case(&format!("Ctrl-{base}"))
    } else {
        label == base
    }
}

// ---------------------------------------------------------------------
// Row builders. Code-only matches (`AnyMod`) keep the long-standing
// behavior of ignoring modifiers; Ctrl-C is the one chord every
// overlay binds, always first and always to its `Abort` action.
// ---------------------------------------------------------------------

const fn key(code: KeyCode) -> KeyMatcher {
    KeyMatcher::AnyMod(code)
}

const fn ch(c: char) -> KeyMatcher {
    KeyMatcher::AnyMod(KeyCode::Char(c))
}

const fn ctrl(c: char) -> KeyMatcher {
    KeyMatcher::Exact {
        modifiers: KeyModifiers::CONTROL,
        code: KeyCode::Char(c),
    }
}

const fn shown<A>(
    key: KeyMatcher,
    action: A,
    label: &'static str,
    verb: &'static str,
) -> KeyRow<A> {
    KeyRow::Bind {
        key,
        action,
        hint: Hint::Shown { label, verb },
    }
}

const fn alias<A>(key: KeyMatcher, action: A) -> KeyRow<A> {
    KeyRow::Bind {
        key,
        action,
        hint: Hint::Alias,
    }
}

// ---------------------------------------------------------------------
// Menus: Controls, the pins menu, the mux menu.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Abort,
    Up,
    Down,
    Pick,
    Close,
}

pub const MENU_KEYS: &[KeyRow<MenuKey>] = &[
    alias(ctrl('c'), MenuKey::Abort),
    shown(key(KeyCode::Up), MenuKey::Up, "↑", "move"),
    alias(ch('k'), MenuKey::Up),
    shown(key(KeyCode::Down), MenuKey::Down, "↓", "move"),
    alias(ch('j'), MenuKey::Down),
    shown(key(KeyCode::Enter), MenuKey::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), MenuKey::Close, "Esc", "close"),
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
    Up,
    Down,
    /// Select the focused choice, toggle the focused checkbox, or open
    /// the focused row's editor.
    Toggle,
    /// Apply the focused choice (if it is one) and close.
    Done,
    Close,
}

pub const CONTROLS_KEYS: &[KeyRow<ControlsKey>] = &[
    alias(ctrl('c'), ControlsKey::Abort),
    shown(key(KeyCode::Up), ControlsKey::Up, "↑", "move"),
    alias(ch('k'), ControlsKey::Up),
    shown(key(KeyCode::Down), ControlsKey::Down, "↓", "move"),
    alias(ch('j'), ControlsKey::Down),
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
// Multi-select sub-editor (Controls filters).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiSelectKey {
    Abort,
    Up,
    Down,
    Toggle,
    Confirm,
    Cancel,
}

pub const MULTI_SELECT_KEYS: &[KeyRow<MultiSelectKey>] = &[
    alias(ctrl('c'), MultiSelectKey::Abort),
    alias(key(KeyCode::Up), MultiSelectKey::Up),
    alias(ch('k'), MultiSelectKey::Up),
    alias(key(KeyCode::Down), MultiSelectKey::Down),
    alias(ch('j'), MultiSelectKey::Down),
    shown(ch(' '), MultiSelectKey::Toggle, "Space", "toggle"),
    shown(
        key(KeyCode::Enter),
        MultiSelectKey::Confirm,
        "Enter",
        "confirm",
    ),
    shown(key(KeyCode::Esc), MultiSelectKey::Cancel, "Esc", "cancel"),
];

// ---------------------------------------------------------------------
// Single-line text input: rename, the max-age editor, form fields.
// Unbound keys edit the text.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputKey {
    Abort,
    Confirm,
    Cancel,
    /// Swallowed so it never reaches the line editor.
    Tab,
}

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

// ---------------------------------------------------------------------
// `n` new bare mux form. Unbound keys edit the focused field.
// ---------------------------------------------------------------------

pub const NEW_MUX_KEYS: &[KeyRow<FormKey>] = &[
    alias(ctrl('c'), FormKey::Abort),
    shown(key(KeyCode::Up), FormKey::Up, "↑", "move"),
    shown(key(KeyCode::Down), FormKey::Down, "↓", "move"),
    shown(key(KeyCode::Tab), FormKey::Tab, "Tab", "complete/next"),
    shown(key(KeyCode::BackTab), FormKey::Back, "S-Tab", "back"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "next/commit"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

// ---------------------------------------------------------------------
// `/` search. Unbound keys, `j`/`k` included, edit the query.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKey {
    Abort,
    Cancel,
    Pick,
    Down,
    Up,
    /// Switch between searching the rows and searching contents.
    Scope,
    /// Contents scope: one result per session, or per matching turn.
    Results,
    /// Contents scope: include tool calls and output.
    Tools,
    /// Contents scope: include thinking.
    Thinking,
    /// Contents scope: keep the query as the filter on the lists.
    ApplyFilter,
    /// Contents scope: keep the query and open the Search view.
    ApplyView,
}

/// The picker shares the list-and-query keys but has no scopes.
pub const PICKER_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    alias(key(KeyCode::Esc), SearchKey::Cancel),
    alias(key(KeyCode::Enter), SearchKey::Pick),
    alias(key(KeyCode::Down), SearchKey::Down),
    alias(key(KeyCode::Up), SearchKey::Up),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
];

/// `/` searching the rows.
pub const SEARCH_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    shown(key(KeyCode::Up), SearchKey::Up, "↑", "move"),
    shown(key(KeyCode::Down), SearchKey::Down, "↓", "move"),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
    shown(
        key(KeyCode::Tab),
        SearchKey::Scope,
        "Tab",
        "search contents",
    ),
];

/// `/` searching what was said in the sessions.
pub const SEARCH_CONTENT_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    shown(key(KeyCode::Up), SearchKey::Up, "↑", "move"),
    shown(key(KeyCode::Down), SearchKey::Down, "↓", "move"),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
    shown(key(KeyCode::Enter), SearchKey::Pick, "Enter", "open hit"),
    shown(key(KeyCode::Esc), SearchKey::Cancel, "Esc", "close"),
    shown(key(KeyCode::Tab), SearchKey::Scope, "Tab", "search rows"),
    shown(ctrl('r'), SearchKey::Results, "Ctrl-R", "results"),
    shown(ctrl('t'), SearchKey::Tools, "Ctrl-T", "tools"),
    shown(ctrl('y'), SearchKey::Thinking, "Ctrl-Y", "thinking"),
    shown(ctrl('f'), SearchKey::ApplyFilter, "Ctrl-F", "filter lists"),
    shown(
        ctrl('o'),
        SearchKey::ApplyView,
        "Ctrl-O",
        "open Search view",
    ),
];

// ---------------------------------------------------------------------
// Scrolling pagers: the `?` help overlay and the `o` value modal.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerKey {
    Abort,
    Close,
    Down,
    Up,
    PageDown,
    PageUp,
    Top,
    Bottom,
    /// Copy the modal's text (a value modal only).
    Copy,
}

pub const HELP_KEYS: &[KeyRow<PagerKey>] = &[
    alias(ctrl('c'), PagerKey::Abort),
    shown(ch('j'), PagerKey::Down, "j", "to scroll"),
    alias(key(KeyCode::Down), PagerKey::Down),
    shown(ch('k'), PagerKey::Up, "k", "to scroll"),
    alias(key(KeyCode::Up), PagerKey::Up),
    shown(
        key(KeyCode::PageDown),
        PagerKey::PageDown,
        "PgDn",
        "to scroll",
    ),
    alias(ch(' '), PagerKey::PageDown),
    shown(key(KeyCode::PageUp), PagerKey::PageUp, "PgUp", "to scroll"),
    alias(ch('g'), PagerKey::Top),
    alias(key(KeyCode::Home), PagerKey::Top),
    alias(ch('G'), PagerKey::Bottom),
    alias(key(KeyCode::End), PagerKey::Bottom),
    shown(key(KeyCode::Esc), PagerKey::Close, "Esc", "to close"),
    shown(ch('q'), PagerKey::Close, "q", "to close"),
    shown(ch('?'), PagerKey::Close, "?", "to close"),
];

pub const VALUE_MODAL_KEYS: &[KeyRow<PagerKey>] = &[
    alias(ctrl('c'), PagerKey::Abort),
    shown(ch('y'), PagerKey::Copy, "y", "to copy"),
    shown(key(KeyCode::Esc), PagerKey::Close, "Esc", "to close"),
    shown(ch('q'), PagerKey::Close, "q", "to close"),
    shown(ch('o'), PagerKey::Close, "o", "to close"),
    shown(ch('j'), PagerKey::Down, "j", "to scroll"),
    alias(key(KeyCode::Down), PagerKey::Down),
    shown(ch('k'), PagerKey::Up, "k", "to scroll"),
    alias(key(KeyCode::Up), PagerKey::Up),
    alias(key(KeyCode::PageDown), PagerKey::PageDown),
    alias(ch(' '), PagerKey::PageDown),
    alias(key(KeyCode::PageUp), PagerKey::PageUp),
    alias(ch('g'), PagerKey::Top),
    alias(key(KeyCode::Home), PagerKey::Top),
    alias(ch('G'), PagerKey::Bottom),
    alias(key(KeyCode::End), PagerKey::Bottom),
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
    /// Copy the selected entry. The runtime performs it, because the
    /// clipboard write is a side effect the widget does not own.
    Copy,
}

pub const MESSAGES_KEYS: &[KeyRow<MessagesKey>] = &[
    alias(ctrl('c'), MessagesKey::Abort),
    shown(ch('j'), MessagesKey::Next, "j", "select"),
    alias(key(KeyCode::Down), MessagesKey::Next),
    shown(ch('k'), MessagesKey::Prev, "k", "select"),
    alias(key(KeyCode::Up), MessagesKey::Prev),
    alias(ch('g'), MessagesKey::First),
    alias(key(KeyCode::Home), MessagesKey::First),
    alias(ch('G'), MessagesKey::Last),
    alias(key(KeyCode::End), MessagesKey::Last),
    shown(ch('J'), MessagesKey::DetailDown, "J", "scroll detail"),
    shown(ch('K'), MessagesKey::DetailUp, "K", "scroll detail"),
    alias(key(KeyCode::PageDown), MessagesKey::DetailPageDown),
    alias(ch(' '), MessagesKey::DetailPageDown),
    alias(key(KeyCode::PageUp), MessagesKey::DetailPageUp),
    shown(
        KeyMatcher::AnyModExceptCtrl('y'),
        MessagesKey::Copy,
        "y",
        "copy",
    ),
    shown(key(KeyCode::Esc), MessagesKey::Close, "Esc", "close"),
    alias(ch('q'), MessagesKey::Close),
    alias(ch('!'), MessagesKey::Close),
];

// ---------------------------------------------------------------------
// Yes/no confirmations (worktree remove, merge, prune).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKey {
    Abort,
    Yes,
    No,
}

pub const CONFIRM_KEYS: &[KeyRow<ConfirmKey>] = &[
    alias(ctrl('c'), ConfirmKey::Abort),
    shown(key(KeyCode::Enter), ConfirmKey::Yes, "Enter", "to confirm"),
    shown(ch('y'), ConfirmKey::Yes, "y", "to confirm"),
    shown(key(KeyCode::Esc), ConfirmKey::No, "Esc", "to cancel"),
    shown(ch('n'), ConfirmKey::No, "n", "to cancel"),
];

// ---------------------------------------------------------------------
// Worktree close-down choice: merge or discard, then tear down.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDownKey {
    Abort,
    Merge,
    Discard,
    Back,
}

pub const CLOSE_DOWN_KEYS: &[KeyRow<CloseDownKey>] = &[
    alias(ctrl('c'), CloseDownKey::Abort),
    shown(ch('m'), CloseDownKey::Merge, "m", "= merge & close"),
    shown(ch('d'), CloseDownKey::Discard, "d", "= discard & close"),
    shown(key(KeyCode::Esc), CloseDownKey::Back, "Esc", "cancel"),
    alias(ch('q'), CloseDownKey::Back),
];

// ---------------------------------------------------------------------
// Multi-field forms: the mux launch form, the new mux form, and the pin
// edit form. Up and Down always move between fields; Tab completes in a
// path field and moves on elsewhere. Option keys act on the choice and
// toggle rows; on a text field they edit the text.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormKey {
    Abort,
    Cancel,
    Up,
    Down,
    Tab,
    Back,
    Submit,
    OptionPrev,
    OptionNext,
    /// Drop the focused field's override and inherit its value again.
    Inherit,
}

pub const MUX_LAUNCH_KEYS: &[KeyRow<FormKey>] = &[
    alias(ctrl('c'), FormKey::Abort),
    shown(key(KeyCode::Up), FormKey::Up, "↑", "move"),
    shown(key(KeyCode::Down), FormKey::Down, "↓", "move"),
    shown(key(KeyCode::Tab), FormKey::Tab, "Tab", "complete/next"),
    shown(key(KeyCode::BackTab), FormKey::Back, "S-Tab", "back"),
    shown(key(KeyCode::Left), FormKey::OptionPrev, "←", "option"),
    shown(key(KeyCode::Right), FormKey::OptionNext, "→", "option"),
    shown(ch(' '), FormKey::OptionNext, "Space", "option"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "launch"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

pub const PIN_EDIT_KEYS: &[KeyRow<FormKey>] = &[
    alias(ctrl('c'), FormKey::Abort),
    shown(key(KeyCode::Up), FormKey::Up, "↑", "move"),
    shown(key(KeyCode::Down), FormKey::Down, "↓", "move"),
    shown(key(KeyCode::Tab), FormKey::Tab, "Tab", "complete/next"),
    shown(key(KeyCode::BackTab), FormKey::Back, "S-Tab", "back"),
    shown(key(KeyCode::Left), FormKey::OptionPrev, "←", "option"),
    shown(key(KeyCode::Right), FormKey::OptionNext, "→", "option"),
    shown(ch(' '), FormKey::OptionNext, "Space", "option"),
    shown(ctrl('r'), FormKey::Inherit, "Ctrl-R", "follow again"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

// ---------------------------------------------------------------------
// Path omnibox field. Unbound keys edit the path.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmniboxKey {
    Complete,
}

pub const OMNIBOX_KEYS: &[KeyRow<OmniboxKey>] = &[alias(key(KeyCode::Tab), OmniboxKey::Complete)];

// ---------------------------------------------------------------------
// Transcript viewer. The viewer draws its own footer from its toggle
// state, behind the ADR 0052 extraction boundary, so these rows are
// all aliases; the keys still resolve here.
// ---------------------------------------------------------------------

use crate::viewer::find::ViewerFindMsg;
use crate::viewer::input::ViewerMsg;

pub const VIEWER_KEYS: &[KeyRow<ViewerMsg>] = &[
    alias(ctrl('c'), ViewerMsg::Close),
    alias(ctrl('d'), ViewerMsg::HalfPageDown),
    alias(ctrl('u'), ViewerMsg::HalfPageUp),
    alias(key(KeyCode::Esc), ViewerMsg::Close),
    alias(ch('q'), ViewerMsg::Close),
    alias(ch('j'), ViewerMsg::ScrollDown),
    alias(key(KeyCode::Down), ViewerMsg::ScrollDown),
    alias(ch('k'), ViewerMsg::ScrollUp),
    alias(key(KeyCode::Up), ViewerMsg::ScrollUp),
    alias(key(KeyCode::PageDown), ViewerMsg::PageDown),
    alias(ch(' '), ViewerMsg::PageDown),
    alias(key(KeyCode::PageUp), ViewerMsg::PageUp),
    alias(ch('g'), ViewerMsg::JumpToStart),
    alias(key(KeyCode::Home), ViewerMsg::JumpToStart),
    alias(KeyMatcher::UpperChar('G'), ViewerMsg::JumpToEnd),
    alias(key(KeyCode::End), ViewerMsg::JumpToEnd),
    alias(ch('t'), ViewerMsg::CycleToolDetail),
    alias(KeyMatcher::UpperChar('T'), ViewerMsg::ToggleThinking),
    alias(KeyMatcher::UpperChar('I'), ViewerMsg::ToggleAborted),
    alias(KeyMatcher::UpperChar('H'), ViewerMsg::ToggleHidden),
    alias(ch('/'), ViewerMsg::FindOpen),
    alias(ch('n'), ViewerMsg::FindNext),
    alias(KeyMatcher::UpperChar('N'), ViewerMsg::FindPrev),
    alias(ch('?'), ViewerMsg::ToggleHelp),
];

/// The find prompt inside the viewer. Keys no row binds type into the
/// prompt (see [`typed_char`]).
pub const VIEWER_FIND_KEYS: &[KeyRow<ViewerFindMsg>] = &[
    alias(ctrl('c'), ViewerFindMsg::Cancel),
    shown(key(KeyCode::Enter), ViewerFindMsg::Confirm, "Enter", "find"),
    shown(key(KeyCode::Esc), ViewerFindMsg::Cancel, "Esc", "cancel"),
    alias(key(KeyCode::Backspace), ViewerFindMsg::Backspace),
    alias(key(KeyCode::Delete), ViewerFindMsg::Delete),
    alias(key(KeyCode::Left), ViewerFindMsg::Left),
    alias(key(KeyCode::Right), ViewerFindMsg::Right),
    alias(key(KeyCode::Home), ViewerFindMsg::Home),
    alias(key(KeyCode::End), ViewerFindMsg::End),
    alias(ctrl('u'), ViewerFindMsg::ClearLine),
];

/// The character a key types into a text prompt, if it types one:
/// a plain or shifted character with no Control or Alt.
pub fn typed_char(event: KeyEvent) -> Option<char> {
    match event.code {
        KeyCode::Char(c)
            if !event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Some(c)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// The modal key convention. One set of meanings for Esc, Enter, and
// Space across every modal; a modal that departs from it says why here,
// and a consistency test checks each modal's real key tables against
// this table.
//
//   Esc    leaves the modal without applying anything that is still
//          pending. (Controls applies as you go, so what it already
//          applied stays.)
//   Enter  applies or submits the modal's primary action; in a list it
//          activates the focused item.
//   Space  changes the focused item: toggles a checkbox, steps an
//          option, selects a choice. In a text field it types a space.
// ---------------------------------------------------------------------

/// What Enter does in a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterMeans {
    /// Applies or submits the whole modal.
    Submits,
    /// Activates the focused item (a menu entry, a search hit, a choice).
    Activates,
    /// Does nothing special: a viewer.
    Nothing,
}

/// What Space does in a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceMeans {
    /// Toggles, steps, or selects the focused item.
    Toggles,
    /// Pages down, as in any pager.
    Pages,
    /// Not bound: it types into the focused text field, or does nothing
    /// in a modal with none.
    Types,
}

/// One modal's row of the convention.
#[derive(Debug, Clone, Copy)]
pub struct ModalConvention {
    pub modal: &'static str,
    pub enter: EnterMeans,
    pub space: SpaceMeans,
    /// Why the modal departs from the convention, if it does.
    pub exception: &'static str,
}

const fn conv(
    modal: &'static str,
    enter: EnterMeans,
    space: SpaceMeans,
    exception: &'static str,
) -> ModalConvention {
    ModalConvention {
        modal,
        enter,
        space,
        exception,
    }
}

/// Every modal against the convention. Esc leaves every one of them.
pub const MODAL_CONVENTION: &[ModalConvention] = &[
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
