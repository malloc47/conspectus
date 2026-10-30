//! Palettes: a query line over a ranked list of results, for picking one
//! thing by typing part of its name.
//!
//! This is a layer 1 control (house rules 1, 2, 4, 5, 8, 10, 12, 13, 14,
//! and 15). A filter picker over known candidates, a quick jump to
//! anything by name, a search whose results a slow provider sends, and a
//! command palette are one control: a query, a list that narrows as it is
//! typed, a cursor, keys in a footer, and a pick. Written apart, one draws
//! no caret, one clamps its cursor where the next resets it, one has no
//! footer, and each sizes and scrolls itself:
//!
//! - **The query** is a [`LineEdit`] drawn by
//!   [`TextField`](crate::text_edit::TextField) after its prompt (`/`),
//!   with its caret while the results have the keys. Keys no row binds
//!   edit it, so `q`, `j`, and `k` type.
//! - **The title** names the palette, then its scope strips (a segmented
//!   choice such as `[ Here ] Everywhere`, whose first strip the
//!   [`Scope`](crate::keys::PaletteKey::Scope) key switches) and a count
//!   of the results.
//! - **Ranked results.** The host ranks them: a palette over known
//!   candidates takes a ranker ([`Palette::ranked_by`], with the built-in
//!   [`every_word`] filter for the simple case) and reranks on every edit;
//!   a palette fed by a provider takes each batch of results with
//!   [`Palette::set_rows`]. Every row is a [`PaletteRow`] the host
//!   defines (or the built-in [`Item`]), drawn through
//!   [`selection::row`](crate::selection::row) after the cursor gutter.
//!   The cursor wraps at the ends (house rule 2) and goes to the best
//!   result after every edit; `PgUp`/`PgDn` move by the rows shown.
//! - **Scrolling.** The results scroll in a window the palette keeps,
//!   moving no further than the cursor needs, with the shared scrollbar
//!   when they overflow.
//! - **A status row** under the query, when the host sets one (how far a
//!   scan has got).
//! - **The footer** is generated from the palette's key table
//!   ([`PALETTE_KEYS`], or a host table over
//!   its own actions) and keeps the bottom rows.
//! - **An options pane** ([`Options`]): named settings that change the
//!   results, opened over the list.
//! - **A debounce** ([`Debounce`]) for a slow provider: the palette marks
//!   every edit and the host polls when a search is due; [`Progress`]
//!   puts how far the provider has got in words for the status row.
//! - **Rows of several cells** lay their columns out with the framework's
//!   column schema ([`crate::columns`]), counting the cursor gutter as the
//!   row's lead, so every row lines up and a list outside the palette
//!   drawn from the same schema lines up with it.
//! - **The outcome.** `Enter` asks the host's `on_pick` about the row
//!   under the cursor and reports its answer (a commit of the host's
//!   message); `Enter` with nothing listed closes. `Esc` closes and
//!   `Ctrl-C` dismisses, saying nothing.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::palette::{Item, Palette, every_word};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Open(u32),
//! }
//!
//! let rooms = vec![
//!     Item::new("kitchen", 1).detail("ground floor"),
//!     Item::new("attic", 2).detail("top floor"),
//!     Item::new("cellar", 3).detail("below ground"),
//! ];
//! let mut palette = Palette::new("Go to", rooms, |room: &Item<u32>| {
//!     Outcome::Commit(Msg::Open(room.value))
//! })
//! .ranked_by(every_word);
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // Typing narrows the list to the rows with every word.
//! for c in "ground".chars() {
//!     palette.handle(press(KeyCode::Char(c)), &());
//! }
//! let listed: Vec<&str> = palette.listed().map(|room| room.label.as_str()).collect();
//! assert_eq!(listed, ["kitchen", "cellar"]);
//!
//! // Down moves to the cellar; Enter hands it to the host.
//! palette.handle(press(KeyCode::Down), &());
//! assert_eq!(palette.handle(press(KeyCode::Enter), &()), Outcome::Commit(Msg::Open(3)));
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 70, 16));
//! palette.draw(buf.area, &mut buf, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("Go to  (2/3)"), "{drawn}");
//! assert!(drawn.contains("► cellar  · below ground"), "{drawn}");
//! assert!(drawn.contains("↑/↓ move · Enter pick · Esc close"), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! The candidates or the provider, the ranking, the words of the title,
//! the notes and the status, what each scope and option means, and what a
//! pick commits: `on_pick` gets the row and answers with the host's
//! message. A palette with keys of its own (`Tab` for its scope, `Ctrl-O`
//! for its options, verbs of the host's) takes a table over the host's
//! action enum that implements [`PaletteAction`], and resolves its own
//! actions first ([`Palette::host_action`]) before handing the rest to
//! the palette.
//!
//! # Seams
//!
//! - **A command palette** is an instance over rows built from a key
//!   table's shown rows (the verb as the label, the key as the detail),
//!   filtered with [`every_word`], whose pick commits the row's action;
//!   see the control's page.
//! - **A row preview**: a pane beside the results showing the cursor's
//!   row; the body lays the list out in one place, so a split would go
//!   there, and [`PaletteRow`] would gain a method that says what to show.

mod body;
mod debounce;
mod options;
mod progress;
mod row;

pub use debounce::Debounce;
pub use options::Options;
pub use progress::Progress;
pub use row::{Item, PaletteRow, RowContext, every_word};

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::chip::SegmentedTitle;
use crate::frame::{ModalFrame, ModalSize};
use crate::keys::{
    Footer, KeyAction, KeyRow, PALETTE_KEYS, PALETTE_OPTION_KEYS, PaletteKey, PaletteOptionKey,
    resolve,
};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::{ScrollWindow, clamp_step, wrap_step};
use crate::selection::Ends;
use crate::text_edit::LineEdit;
use crate::theme::Theme;

use body::{Body, View};

/// How large every palette is unless its host says otherwise: the palette
/// frame, 85% of the screen's width and 80% of its height, whatever the
/// results hold, so the frame keeps its size as they narrow.
pub const PALETTE_SIZE: ModalSize = ModalSize::PALETTE;

/// What a palette's empty list says when nothing is typed, unless the
/// host says otherwise.
const EMPTY_NOTE: &str = "(nothing to pick)";
/// What it says when the query matches nothing.
const NO_MATCH_NOTE: &str = "(no matches)";
/// What comes before the query unless the host says otherwise.
const PROMPT: &str = "/";
/// What separates the title's parts: its name, each strip, the count.
const TITLE_GAP: &str = "  ";

/// How a palette's title counts its results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Count {
    /// No count.
    None,
    /// The results listed: `(12)`, for a palette whose rows are the
    /// results a host ranked.
    Listed,
    /// The results listed of every row the palette holds: `(3/7)`, for a
    /// palette that filters its own candidates.
    #[default]
    OfTotal,
}

/// An action a palette's key table binds: the palette's own meaning of
/// it, or none for an action that is the host's.
///
/// [`PaletteKey`] is one, meaning itself. A host whose palette has keys of
/// its own (a verb that keeps the query, one that recalls an earlier one)
/// writes its table over its own enum, implements this to say which of
/// its actions are the palette's, and resolves the rest first with
/// [`Palette::host_action`].
pub trait PaletteAction: KeyAction + Copy + PartialEq + 'static {
    /// What the palette does for this action, or `None` when the host
    /// does it.
    fn palette_key(self) -> Option<PaletteKey>;
}

impl PaletteAction for PaletteKey {
    fn palette_key(self) -> Option<PaletteKey> {
        Some(self)
    }
}

/// The host's decision about a picked row, shared by every copy.
type OnPick<R, M> = Arc<dyn Fn(&R) -> Outcome<M> + Send + Sync>;
/// The host's ranking: the indices of the rows that match a query, best
/// first.
type Ranker<R> = Arc<dyn Fn(&str, &[R]) -> Vec<usize> + Send + Sync>;

/// A segmented choice in the title.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Strip {
    choices: Vec<String>,
    chosen: usize,
}

/// A modal query over a ranked list of results: the query line, the
/// title with its scope strips and count, the results, an optional status
/// row and options pane, and the footer its key table generates.
///
/// `R` is the host's row ([`PaletteRow`]), `M` its message, which
/// `on_pick` builds, and `A` the action enum of its key table
/// ([`PaletteKey`] unless the host has keys of its own). The palette
/// implements [`Overlay<M, Cx>`] for every `Cx` and reads nothing from
/// the context. Drawing keeps the results' scroll window, so the next key
/// moves it no further than the cursor needs. The closures are shared
/// behind an [`Arc`], so a palette clones cheaply.
///
/// A host over known candidates hands them over with a ranker and a pick:
///
/// ```
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use tuikit::overlay::{ModalStack, Outcome};
/// use tuikit::palette::{Item, Palette, every_word};
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum Msg {
///     Play(String),
/// }
///
/// let songs: Vec<Item<String>> = ["Blue Moon", "Moonlight", "Sunrise"]
///     .into_iter()
///     .map(|title| Item::new(title, title.to_lowercase()))
///     .collect();
/// let palette = Palette::new("Play", songs, |song: &Item<String>| {
///     Outcome::Commit(Msg::Play(song.value.clone()))
/// })
/// .ranked_by(every_word)
/// .empty_note("(the library is empty)");
///
/// let mut stack: ModalStack<Palette<Item<String>, Msg>> = ModalStack::new();
/// stack.push(palette);
/// let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
/// for c in "moon".chars() {
///     stack.route(press(KeyCode::Char(c)), &());
/// }
/// // Up wraps from the first result to the last.
/// stack.route(press(KeyCode::Up), &());
/// let done = stack.route(press(KeyCode::Enter), &()).expect("a palette is open");
/// let settled = stack.settle(done);
/// assert_eq!(settled.message, Some(Msg::Play("moonlight".into())));
/// assert!(stack.is_empty(), "the pick closed the palette");
/// ```
#[derive(Clone)]
pub struct Palette<R, M, A: 'static = PaletteKey> {
    title: String,
    prompt: String,
    strips: Vec<Strip>,
    count: Count,
    query: LineEdit,
    rows: Vec<R>,
    ranker: Option<Ranker<R>>,
    /// Indices into `rows` of the results listed, in their order.
    ranked: Vec<usize>,
    /// The position in `ranked` the cursor is on.
    cursor: usize,
    window: ScrollWindow,
    status: Option<String>,
    empty_note: String,
    no_match_note: String,
    keys: &'static [KeyRow<A>],
    size: ModalSize,
    /// Whether the results keep the scrollbar's column while they fit.
    reserve_scrollbar: bool,
    options: Option<Options>,
    debounce: Option<Debounce>,
    on_pick: OnPick<R, M>,
}

impl<R, M> Palette<R, M> {
    /// A palette titled `title` listing `rows` in their order, with the
    /// cursor on the first, keys from [`PALETTE_KEYS`], and the palette
    /// size. `on_pick` gets the row under the cursor when `Enter` is
    /// pressed, and its answer is what the palette reports. Give it a
    /// ranker ([`ranked_by`](Self::ranked_by)) to narrow the rows as the
    /// query is typed; without one the rows are the results, as a host
    /// that ranks them itself hands them over.
    pub fn new(
        title: impl Into<String>,
        rows: Vec<R>,
        on_pick: impl Fn(&R) -> Outcome<M> + Send + Sync + 'static,
    ) -> Self {
        let ranked = (0..rows.len()).collect();
        Self {
            title: title.into(),
            prompt: PROMPT.to_string(),
            strips: Vec::new(),
            count: Count::default(),
            query: LineEdit::default(),
            rows,
            ranker: None,
            ranked,
            cursor: 0,
            window: ScrollWindow::default(),
            status: None,
            empty_note: EMPTY_NOTE.to_string(),
            no_match_note: NO_MATCH_NOTE.to_string(),
            keys: PALETTE_KEYS,
            size: PALETTE_SIZE,
            reserve_scrollbar: false,
            options: None,
            debounce: None,
            on_pick: Arc::new(on_pick),
        }
    }
}

impl<R, M, A: PaletteAction> Palette<R, M, A> {
    /// Resolves keys with `keys`, a table over the host's own actions: a
    /// palette whose `Enter` has a verb of its own, or with a scope, an
    /// options pane, or keys the host handles. The footer is generated
    /// from it.
    #[must_use]
    pub fn keys<B: PaletteAction>(self, keys: &'static [KeyRow<B>]) -> Palette<R, M, B> {
        Palette {
            title: self.title,
            prompt: self.prompt,
            strips: self.strips,
            count: self.count,
            query: self.query,
            rows: self.rows,
            ranker: self.ranker,
            ranked: self.ranked,
            cursor: self.cursor,
            window: self.window,
            status: self.status,
            empty_note: self.empty_note,
            no_match_note: self.no_match_note,
            keys,
            size: self.size,
            reserve_scrollbar: self.reserve_scrollbar,
            options: self.options,
            debounce: self.debounce,
            on_pick: self.on_pick,
        }
    }

    /// Narrows and orders the rows with `rank`, which gets the query and
    /// every row and answers the indices of the rows to list, best first.
    /// The palette reranks on every edit and whenever its rows change.
    #[must_use]
    pub fn ranked_by(
        mut self,
        rank: impl Fn(&str, &[R]) -> Vec<usize> + Send + Sync + 'static,
    ) -> Self {
        self.ranker = Some(Arc::new(rank));
        self.rerank();
        self
    }

    /// Adds a scope strip to the title: `choices`, the first chosen. The
    /// first strip a palette has is its scope, which the
    /// [`Scope`](PaletteKey::Scope) key switches to its next choice; the
    /// host sets the others ([`set_scope`](Self::set_scope)).
    #[must_use]
    pub fn scopes<I, S>(mut self, choices: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.strips.push(Strip {
            choices: choices.into_iter().map(Into::into).collect(),
            chosen: 0,
        });
        self
    }

    /// Sets how the title counts the results ([`Count::OfTotal`] unless
    /// set).
    #[must_use]
    pub fn count(mut self, count: Count) -> Self {
        self.count = count;
        self
    }

    /// Sets what comes before the query (`/` unless set).
    #[must_use]
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Starts with `query` typed, its results ranked.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = LineEdit::new(query);
        self.rerank();
        self
    }

    /// Sets what the empty list says while nothing is typed.
    #[must_use]
    pub fn empty_note(mut self, note: impl Into<String>) -> Self {
        self.empty_note = note.into();
        self
    }

    /// Sets what the empty list says when the query matches nothing.
    #[must_use]
    pub fn no_match_note(mut self, note: impl Into<String>) -> Self {
        self.no_match_note = note.into();
        self
    }

    /// Gives the palette an options pane, which its
    /// [`Options`](PaletteKey::Options) key opens.
    #[must_use]
    pub fn options(mut self, options: Options) -> Self {
        self.options = Some(options);
        self
    }

    /// Makes the palette owe its host a search `delay` after each edit,
    /// scope change, and option change ([`take_due`](Self::take_due)).
    #[must_use]
    pub fn debounced(mut self, delay: Duration) -> Self {
        self.debounce = Some(Debounce::new(delay));
        self
    }

    /// Sizes the palette by `size` instead of [`PALETTE_SIZE`].
    #[must_use]
    pub fn size(mut self, size: ModalSize) -> Self {
        self.size = size;
        self
    }

    /// Keeps the scrollbar's column beside the results while they fit,
    /// blank, so every row has one width whether or not the list
    /// overflows: a list a provider streams into does not shift its
    /// columns when it grows past the frame, and every row ends a cell
    /// before the frame.
    #[must_use]
    pub fn reserve_scrollbar(mut self) -> Self {
        self.reserve_scrollbar = true;
        self
    }

    /// Puts the cursor on row `index` (an index into the rows the palette
    /// was given) when it is listed.
    #[must_use]
    pub fn at(mut self, index: usize) -> Self {
        self.select(index);
        self
    }
}

impl<R, M, A: PaletteAction> Palette<R, M, A> {
    /// The title on the top border, before its strips and count.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The query as typed.
    pub fn query(&self) -> &str {
        self.query.value()
    }

    /// Every row the palette holds, listed or not, in the host's order.
    pub fn rows(&self) -> &[R] {
        &self.rows
    }

    /// How many rows the palette holds.
    pub fn total(&self) -> usize {
        self.rows.len()
    }

    /// The results listed, in their order.
    pub fn listed(&self) -> impl ExactSizeIterator<Item = &R> + '_ {
        self.ranked.iter().map(|&index| &self.rows[index])
    }

    /// The indices of the listed results into [`rows`](Self::rows), in
    /// their order.
    pub fn ranked(&self) -> &[usize] {
        &self.ranked
    }

    /// The position in the listed results the cursor is on.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The row under the cursor, if anything is listed.
    pub fn current(&self) -> Option<&R> {
        self.ranked.get(self.cursor).map(|&index| &self.rows[index])
    }

    /// The choice strip `strip` has chosen; 0 for a strip the palette
    /// lacks.
    pub fn scope(&self, strip: usize) -> usize {
        self.strips.get(strip).map_or(0, |strip| strip.chosen)
    }

    /// The status row, if the host set one.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// The options pane, if the palette has one.
    pub fn options_pane(&self) -> Option<&Options> {
        self.options.as_ref()
    }

    /// Whether the options pane is open over the results.
    pub fn options_open(&self) -> bool {
        self.options.as_ref().is_some_and(Options::is_open)
    }

    /// The debounce timer, if the palette has one.
    pub fn debounce(&self) -> Option<&Debounce> {
        self.debounce.as_ref()
    }

    /// The scroll window of the last draw: which results were on screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// The table the palette resolves its keys with.
    pub fn key_table(&self) -> &'static [KeyRow<A>] {
        self.keys
    }

    /// The footer as it is drawn now: the options pane's while it is
    /// open, and otherwise the palette's table's.
    pub fn footer(&self) -> Footer {
        if self.options_open() {
            Footer::new(PALETTE_OPTION_KEYS)
        } else {
            Footer::new(self.keys)
        }
    }

    /// The action `key` resolves to in the palette's table, unless the
    /// options pane is open (it has its own keys).
    pub fn action(&self, key: KeyEvent) -> Option<A> {
        if self.options_open() {
            return None;
        }
        resolve(self.keys, key)
    }

    /// The action `key` resolves to when it is one of the host's own (its
    /// [`palette_key`](PaletteAction::palette_key) is `None`): a host
    /// with keys of its own handles these before it hands a key to the
    /// palette, which swallows them.
    pub fn host_action(&self, key: KeyEvent) -> Option<A> {
        self.action(key)
            .filter(|action| action.palette_key().is_none())
    }
}

impl<R, M, A: PaletteAction> Palette<R, M, A> {
    /// Replaces the rows: a provider's next batch of results, or another
    /// set of candidates when the scope changes. A palette with a ranker
    /// reranks them; the cursor stays where it was while that is still a
    /// result, and goes to the first otherwise.
    pub fn set_rows(&mut self, rows: Vec<R>) {
        self.rows = rows;
        self.rerank();
    }

    /// Fills the query with `text` (an earlier search recalled), as if it
    /// had been typed.
    pub fn set_query(&mut self, text: impl Into<String>) {
        if self.query.set_value(text).changed {
            self.edited();
        }
    }

    /// Puts the cursor on row `index` (an index into
    /// [`rows`](Self::rows)) when it is listed, and leaves it where it is
    /// otherwise: for a host that keeps a cursor rule of its own across an
    /// edit (the position it had), or lists the same results another way
    /// (back to the first).
    pub fn select(&mut self, index: usize) {
        if let Some(position) = self.ranked.iter().position(|&listed| listed == index) {
            self.cursor = position;
        }
    }

    /// Sets the status row under the query, or removes it.
    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    /// Sets what the empty list says while nothing is typed.
    pub fn set_empty_note(&mut self, note: impl Into<String>) {
        self.empty_note = note.into();
    }

    /// Sets what the empty list says when the query matches nothing.
    pub fn set_no_match_note(&mut self, note: impl Into<String>) {
        self.no_match_note = note.into();
    }

    /// Chooses `chosen` in strip `strip`, as the host switches a strip of
    /// its own. Nothing happens for a strip or a choice the palette
    /// lacks.
    pub fn set_scope(&mut self, strip: usize, chosen: usize) {
        if let Some(strip) = self.strips.get_mut(strip)
            && chosen < strip.choices.len()
        {
            strip.chosen = chosen;
        }
    }

    /// The options pane, to restore or read a setting.
    pub fn options_mut(&mut self) -> Option<&mut Options> {
        self.options.as_mut()
    }

    /// Opens the options pane, if the palette has one.
    pub fn open_options(&mut self) {
        if let Some(options) = self.options.as_mut() {
            options.open();
        }
    }

    /// Owes the host a search for an edit at `at` (a palette opened with
    /// a query to search at once). Does nothing without a debounce.
    pub fn mark_edited(&mut self, at: Instant) {
        if let Some(debounce) = self.debounce.as_mut() {
            debounce.mark(at);
        }
    }

    /// Whether the host should search now: once per edit, when the query
    /// has rested by `now` or when `force` skips the wait. Always `false`
    /// without a debounce.
    pub fn take_due(&mut self, now: Instant, force: bool) -> bool {
        self.debounce
            .as_mut()
            .is_some_and(|debounce| debounce.take_due(now, force))
    }

    /// Ranks the rows against the query, keeping the cursor on the list.
    fn rerank(&mut self) {
        let len = self.rows.len();
        self.ranked = match &self.ranker {
            Some(rank) => rank(self.query.value(), &self.rows)
                .into_iter()
                .filter(|&index| index < len)
                .collect(),
            None => (0..len).collect(),
        };
        if self.cursor >= self.ranked.len() {
            self.cursor = 0;
        }
    }

    /// What the results do after the query, a scope, or an option
    /// changes: they are ranked again, the cursor goes to the best one,
    /// and a slow provider is owed a search.
    fn edited(&mut self) {
        self.cursor = 0;
        self.rerank();
        self.mark_edited(Instant::now());
    }

    /// Moves the cursor by `delta` results, wrapping.
    fn step(&mut self, delta: i32) {
        self.cursor = Ends::Wrap.step(self.cursor, self.ranked.len(), delta);
    }

    /// Moves the cursor by a page of `delta` pages, stopping at the ends.
    fn page(&mut self, delta: i32) {
        let page = i32::try_from(self.window.page().max(1)).unwrap_or(i32::MAX);
        self.cursor = clamp_step(self.cursor, self.ranked.len(), delta.saturating_mul(page));
    }

    /// What the palette does for one of its own actions.
    fn act(&mut self, key: PaletteKey) -> Outcome<M> {
        match key {
            PaletteKey::Abort => return Outcome::Dismiss,
            PaletteKey::Cancel => return Outcome::Close,
            PaletteKey::Pick => {
                return match self.current() {
                    Some(row) => (self.on_pick)(row),
                    None => Outcome::Close,
                };
            }
            PaletteKey::Up => self.step(-1),
            PaletteKey::Down => self.step(1),
            PaletteKey::PageUp => self.page(-1),
            PaletteKey::PageDown => self.page(1),
            PaletteKey::Scope => {
                if let Some(strip) = self.strips.first_mut() {
                    strip.chosen = wrap_step(strip.chosen, strip.choices.len(), 1);
                    self.edited();
                }
            }
            PaletteKey::Options => self.open_options(),
        }
        Outcome::Consumed
    }

    /// What the palette does with `key`, as its overlay's
    /// [`handle`](Overlay::handle) does: for a host that routes keys to
    /// the palette itself, as one whose rows it draws with
    /// [`draw_with`](Self::draw_with) (rows that are no [`PaletteRow`])
    /// does.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<M> {
        if self.options_open() {
            return self.handle_option(key);
        }
        let Some(action) = resolve(self.keys, key) else {
            if self.query.handle(key).changed {
                self.edited();
            }
            return Outcome::Consumed;
        };
        action
            .palette_key()
            .map_or(Outcome::Consumed, |own| self.act(own))
    }

    /// One key while the options pane is open.
    fn handle_option(&mut self, key: KeyEvent) -> Outcome<M> {
        let Some(options) = self.options.as_mut() else {
            return Outcome::Consumed;
        };
        let changed = match resolve(PALETTE_OPTION_KEYS, key) {
            Some(PaletteOptionKey::Abort) => return Outcome::Dismiss,
            Some(PaletteOptionKey::Toggle) => options.toggle(),
            Some(PaletteOptionKey::Back) => {
                options.close();
                false
            }
            Some(PaletteOptionKey::Up) => {
                options.step(-1);
                false
            }
            Some(PaletteOptionKey::Down) => {
                options.step(1);
                false
            }
            None => false,
        };
        if changed {
            self.edited();
        }
        Outcome::Consumed
    }

    /// The title: the name, each strip, and the count, two cells apart.
    fn title_line(&self, theme: &Theme) -> Line<'static> {
        let mut spans = vec![Span::raw(self.title.clone())];
        for strip in &self.strips {
            let segments: Vec<(&str, bool)> = strip
                .choices
                .iter()
                .enumerate()
                .map(|(index, choice)| (choice.as_str(), index == strip.chosen))
                .collect();
            spans.push(Span::raw(TITLE_GAP));
            spans.extend(SegmentedTitle::new(&segments).spans(theme));
        }
        let count = match self.count {
            Count::None => None,
            Count::Listed => Some(format!("({})", self.ranked.len())),
            Count::OfTotal => Some(format!("({}/{})", self.ranked.len(), self.rows.len())),
        };
        if let Some(count) = count {
            spans.push(Span::raw(TITLE_GAP));
            spans.push(Span::raw(count));
        }
        Line::from(spans)
    }
}

impl<R: PaletteRow, M, A: PaletteAction> Palette<R, M, A> {
    /// Draws the palette over `area`: the frame and its title, the query,
    /// the status row, the results (or the options pane), and the footer,
    /// and keeps the results' window. A host whose overlay holds the
    /// palette draws it with this; on a stack, [`Overlay::render`] does.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.draw_with(area, buf, theme, R::cells);
    }
}

impl<R, M, A: PaletteAction> Palette<R, M, A> {
    /// Draws the palette as [`draw`](Self::draw) does, laying each result
    /// on screen out with `cells` instead of its own
    /// [`PaletteRow::cells`]: for rows whose look depends on what the host
    /// holds only while it draws (a word it looks up for each row shown,
    /// too costly to work out for every result, or styles of its own
    /// theme). Only the rows on screen are laid out. Rows drawn only this
    /// way need not be [`PaletteRow`]s; the host routes their keys with
    /// [`handle_key`](Self::handle_key).
    pub fn draw_with(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        cells: impl Fn(&R, &RowContext<'_>) -> Vec<Span<'static>>,
    ) {
        let frame = ModalFrame::new(theme, self.size).title(self.title_line(theme));
        let note = if self.query.is_empty() {
            &self.empty_note
        } else {
            &self.no_match_note
        };
        let body = Body {
            view: View {
                query: &self.query,
                prompt: &self.prompt,
                status: self.status.as_deref(),
                rows: &self.rows,
                ranked: &self.ranked,
                cursor: self.cursor,
                note,
                options: self.options.as_ref().filter(|options| options.is_open()),
                footer: self.footer(),
                theme,
                cells: &cells,
                reserve_scrollbar: self.reserve_scrollbar,
            },
            window: self.window,
        };
        self.window = frame.render(area, buf, body);
    }
}

impl<R, M, A: 'static> fmt::Debug for Palette<R, M, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Palette")
            .field("title", &self.title)
            .field("query", &self.query.value())
            .field("total", &self.rows.len())
            .field("ranked", &self.ranked)
            .field("cursor", &self.cursor)
            .field("strips", &self.strips)
            .field("status", &self.status)
            .field("reserve_scrollbar", &self.reserve_scrollbar)
            .field("options", &self.options)
            .field("debounce", &self.debounce)
            .finish_non_exhaustive()
    }
}

impl<R: PaletteRow, M, A: PaletteAction, Cx: ?Sized> Overlay<M, Cx> for Palette<R, M, A> {
    /// The arrows and `Ctrl-P`/`Ctrl-N` move, wrapping; `PgUp`/`PgDn`
    /// move by the rows shown and stop; `Enter` asks `on_pick` about the
    /// row under the cursor (and closes when nothing is listed); `Esc`
    /// closes (`Close`) and `Ctrl-C` dismisses (`Dismiss`), saying
    /// nothing. The scope and options keys, where the table binds them,
    /// switch the first strip and open the pane. Keys the table does not
    /// bind edit the query; an action that is the host's is swallowed.
    /// While the options pane is open its own table has the keys.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    /// The query takes a paste, except while the options pane is open.
    fn takes_text(&self) -> bool {
        !self.options_open()
    }

    /// The footer the palette draws, the options pane's while it is open
    /// ([`Palette::footer`]).
    fn hints(&self) -> Footer {
        self.footer()
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
