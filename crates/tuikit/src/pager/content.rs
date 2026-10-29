//! What a pager shows: the content trait, what acting on the content
//! asks of the pager, and the action set of content with no keys.

use std::borrow::Cow;
use std::ops::Range;

use ratatui::text::{Line, Span};

use super::Find;
use crate::help::{HelpAction, HelpEntry};
use crate::keys::{KeyAction, KeyRole, KeyRow, PagerKey};
use crate::theme::Theme;

/// What a [`Pager`](super::Pager) reads: rows laid out for a width, the
/// units a find matches, a header, and the keys of the content's own.
///
/// The pager owns everything a reader shares (the chrome, the scroll
/// window and its sticky end, paging, the find prompt and stepping, the
/// help page, the footer); the content owns what its rows are. Rows are
/// laid out for the width the pager is drawn at: a content that wraps
/// keeps its layout until the width or its own state changes, so a draw
/// that only scrolls lays nothing out again.
///
/// A find matches *units*, which the content numbers: a line of a
/// document, a paragraph, an entry. [`find`](Self::find) lists the units
/// that hold a query, in order; [`match_rows`](Self::match_rows) says
/// which rows a unit takes in the last layout, so the pager scrolls to
/// it; [`paint`](Self::paint) picks the matches out on the rows drawn.
pub trait PagerContent {
    /// The host's message, which [`PagerEffect::Request`] and the
    /// [`copy`](Self::copy) hook carry; the pager reports it to its host
    /// and stays open.
    type Message;

    /// The content's own key actions (attach, toggles), beside the
    /// pager's. A content with none uses [`NoKeys`].
    type Action: KeyAction + HelpAction;

    /// The content's keys, resolved after the pager's table, so they never
    /// take a key the pager binds. Their footer segments are drawn before
    /// the pager's keys that leave, and their help rows in sections of
    /// their own before the pager's last section. Starts with no `Ctrl-C`:
    /// the pager's table does.
    const KEYS: &'static [KeyRow<Self::Action>];

    /// Lays the rows out `width` cells wide and returns how many there
    /// are. Every draw calls it first; a content that keeps its layout
    /// returns it unchanged while `width` and its own state are the same.
    fn layout(&mut self, width: u16, theme: &Theme) -> usize;

    /// The rows in `range` of the last layout, styled.
    fn rows(&self, range: Range<usize>, theme: &Theme) -> Vec<Line<'static>>;

    /// The header line: drawn above a full-screen pager's body, and as
    /// a framed pager's title. `width` is the cells it has; the pager cuts
    /// a longer line with the ellipsis mark.
    fn header(&self, width: usize, theme: &Theme) -> Line<'static>;

    /// Chips the footer draws after the find chip and the note: the
    /// state of the content's toggles, in order of importance. A footer
    /// too narrow for every chip drops them from the end.
    fn chips(&self, theme: &Theme) -> Vec<Span<'static>> {
        let _ = theme;
        Vec::new()
    }

    /// The units that hold `query` (trimmed, not empty), in the order
    /// the content shows them. Empty for a content that does not find.
    fn find(&self, query: &str) -> Vec<usize> {
        let _ = query;
        Vec::new()
    }

    /// The rows unit `id` takes in the last layout: its first row is where
    /// the pager scrolls a match to.
    fn match_rows(&self, id: usize) -> Range<usize> {
        id..id + 1
    }

    /// Picks the matches of the active `find` out on `rows`, the rows of
    /// the last layout from `first`. The pager calls it while a find is
    /// active; [`Find::paint`] paints one row's matches in the shared
    /// looks, the unit the find is on in the current match's.
    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
        let _ = (rows, first, find, theme);
    }

    /// More that the find chip says when nothing shown matches `query`
    /// (how many matches what is hidden holds), or nothing.
    fn find_detail(&self, query: &str) -> Option<String> {
        let _ = query;
        None
    }

    /// The content's own words for what one of the pager's keys does, on
    /// the help page (`Jump to the oldest line`), or `None` to keep the
    /// pager's. The key keeps its place and its section.
    fn describe(key: PagerKey) -> Option<&'static str> {
        let _ = key;
        None
    }

    /// What the body says when there are no rows.
    fn empty(&self) -> Cow<'_, str> {
        Cow::Borrowed("nothing to show")
    }

    /// Acts on one of the content's keys.
    fn apply(&mut self, action: Self::Action) -> PagerEffect<Self::Message>;

    /// The copy hook: the host message that copies what the content holds
    /// (the clipboard write is the host's effect), or `None` when there is
    /// nothing to copy. A pager whose table binds
    /// [`PagerKey::Copy`](crate::keys::PagerKey::Copy) asks it.
    fn copy(&self) -> Option<Self::Message> {
        None
    }
}

/// What acting on the content asks of its pager.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PagerEffect<M> {
    /// Nothing more: the content took the key.
    None,
    /// The content's rows changed (a toggle showed or hid some): the
    /// pager runs the active find again, staying on the unit it was on,
    /// and the next draw lays the rows out.
    Relayout,
    /// The pager closes, as `Esc` at its first level does.
    Close,
    /// The pager stays open and its host handles `M`.
    Request(M),
}

/// The actions of a content that has no keys of its own. There are none,
/// so a value of this type cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoKeys {}

impl KeyAction for NoKeys {
    fn role(self) -> KeyRole {
        match self {}
    }
}

impl HelpAction for NoKeys {
    const LISTED: &'static [Self] = &[];

    fn help(self) -> HelpEntry {
        match self {}
    }
}
