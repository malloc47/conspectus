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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewMuxKey {
    Abort,
    SwitchField,
    Commit,
    Cancel,
}

pub const NEW_MUX_KEYS: &[KeyRow<NewMuxKey>] = &[
    alias(ctrl('c'), NewMuxKey::Abort),
    shown(
        key(KeyCode::Tab),
        NewMuxKey::SwitchField,
        "Tab",
        "switch field",
    ),
    shown(
        key(KeyCode::Enter),
        NewMuxKey::Commit,
        "Enter",
        "next/commit",
    ),
    shown(key(KeyCode::Esc), NewMuxKey::Cancel, "Esc", "cancel"),
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
}

pub const SEARCH_KEYS: &[KeyRow<SearchKey>] = &[
    alias(ctrl('c'), SearchKey::Abort),
    alias(key(KeyCode::Esc), SearchKey::Cancel),
    alias(key(KeyCode::Enter), SearchKey::Pick),
    alias(key(KeyCode::Down), SearchKey::Down),
    alias(key(KeyCode::Up), SearchKey::Up),
    alias(ctrl('n'), SearchKey::Down),
    alias(ctrl('p'), SearchKey::Up),
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
    alias(key(KeyCode::Esc), PagerKey::Close),
    alias(ch('q'), PagerKey::Close),
    alias(ch('o'), PagerKey::Close),
    alias(ch('j'), PagerKey::Down),
    alias(key(KeyCode::Down), PagerKey::Down),
    alias(ch('k'), PagerKey::Up),
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

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
