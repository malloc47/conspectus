//! Log lists: a log's entries, newest first, over the selected entry's
//! detail, for reading what happened after the fact.
//!
//! This is a layer 1 control (house rules 1, 2, 4, 5, 8, 11, 12, 13, 14,
//! and 18). A program that keeps a [`Log`] of what its operations did
//! (what worked, what was refused, what failed, with the output it
//! printed) lets the user read it back in a log list, and every such list
//! should select, scroll, page its detail, and close the same way:
//!
//! - **The log is the host's.** A [`LogList<R, M, Cx>`](LogList) reads
//!   the host's [`Log<R>`] from its context `&Cx` through a closure, on
//!   every key and every draw, so an entry logged while the list is open
//!   shows at once; the time its ages are read against comes from the
//!   context too, so the framework reads no clock. The list keeps only its
//!   selection and its two scroll windows.
//! - **One row per entry**, newest at the top: the severity's mark in its
//!   theme color, the age (`3m ago`), and the summary with its count of
//!   repeats ([`Entry::summary_with_repeats`]), drawn through
//!   [`selection::row`](crate::selection::row) and cut with `…` to fit.
//!   The selection stops at the ends, since a log is scrollable content
//!   (house rule 2); `g` and `G` select the newest and the oldest.
//! - **Scrolling.** The rows take up to two fifths of the frame (at least
//!   three while the log has three) and scroll in a window the list
//!   keeps, moving no further than the selection needs, with the shared
//!   scrollbar.
//! - **The detail** of the selected entry, the host's text, is an
//!   embedded [`Pager`]: it wraps, scrolls a row
//!   with `J`/`K` and a page (the rows it shows) with `PgDn`, `Space`,
//!   and `PgUp`, stops at its end, shows the shared scrollbar, and goes
//!   back to its top when the selection moves.
//! - **The footer** is generated from the table: the keys that select and
//!   scroll, the list's other keys (copy), then the keys that leave. On a
//!   narrow frame it wraps between segments; on a frame too short for it,
//!   its last rows stay and its first rows scroll after the detail's last
//!   row, the framed pager's rule.
//! - **Leaving.** `Esc` closes the list, and `q` and `Ctrl-C` dismiss it,
//!   saying nothing. A host adds the key that opened it as another close.
//! - **Copy.** A table that binds [`LogListKey::Copy`] asks the host's
//!   copy hook for the selected entry, which answers with the host's
//!   message; the clipboard write is the host's.
//! - **Unseen entries.** Opening the list is looking at the log:
//!   [`LogList::mark_seen`] clears the log's count of unseen warnings and
//!   errors, which a host shows elsewhere (a status chip).
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::KeyCode;
//! use ratatui::layout::Rect;
//! use tuikit::feedback::{Entry, Log, Severity};
//! use tuikit::keys::{KeyRow, LOG_LIST_KEYS, LogListKey, ch, include, shown};
//! use tuikit::log_list::LogList;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::testkit::keys::press;
//! use tuikit::theme::Theme;
//!
//! /// The program's state: its log, each entry carrying what it printed.
//! struct State {
//!     log: Log<String>,
//!     now: i64,
//! }
//!
//! #[derive(Debug, PartialEq)]
//! enum Msg {
//!     Copy(String),
//! }
//!
//! const KEYS: &[KeyRow<LogListKey>] = &[
//!     include(LOG_LIST_KEYS),
//!     shown(ch('y'), LogListKey::Copy, "y", "copy"),
//! ];
//!
//! let mut state = State { log: Log::new(100), now: 1_060 };
//! state.log.push(Entry::new(1_000, Severity::Error, "export failed", "disk full".into()));
//! state.log.push(Entry::new(1_030, Severity::Success, "saved", "3 files".into()));
//!
//! let mut list = LogList::new("Events", |state: &State| &state.log, |state: &State| state.now)
//!     .keys(KEYS)
//!     .detail(|entry: &Entry<String>| format!("{}\n{}", entry.summary, entry.item))
//!     .copy(|entry: &Entry<String>| Some(Msg::Copy(entry.item.clone())))
//!     .mark_seen(&mut state.log);
//! assert_eq!(state.log.unseen(), 0, "opening the list saw the failure");
//!
//! // `j` selects the older entry; `y` asks the host to copy it.
//! assert_eq!(list.handle(press(KeyCode::Char('j')), &state), Outcome::Consumed);
//! assert_eq!(
//!     list.handle(press(KeyCode::Char('y')), &state),
//!     Outcome::Stay(Msg::Copy("disk full".into()))
//! );
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
//! list.render(buf.area, &mut buf, &state, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("Events · 2"), "{drawn}");
//! assert!(drawn.contains("► ×   1m ago  export failed"), "{drawn}");
//! assert!(drawn.contains("disk full"), "the detail shows the selection\n{drawn}");
//! assert!(drawn.contains("j/k select · J/K scroll detail · y copy · Esc close"), "{drawn}");
//!
//! // `Esc` closes it.
//! assert_eq!(list.handle(press(KeyCode::Esc), &state), Outcome::Close);
//! ```
//!
//! # What a host keeps
//!
//! The log and its payload type `R` (what each entry was about, what ran,
//! what it printed), the clock the ages are read against, the title, how
//! an entry's detail reads, what a copy copies and the message that asks
//! for it, the words the empty list says, and the table when it adds keys
//! (`y` to copy, the key that opened the list).

mod body;
mod detail;

use std::fmt;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::feedback::{Entry, Log};
use crate::frame::{ModalFrame, ModalSize};
use crate::glyphs::marks;
use crate::keys::{Footer, KeyAction, KeyRole, KeyRow, LOG_LIST_KEYS, LogListKey, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::pager::{Pager, PagerMode};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;
use crate::theme::Theme;

use body::Body;
use detail::Detail;

/// How large every log list is: the reader's frame, the screen less its
/// margins, whatever the log holds.
pub const LOG_LIST_SIZE: ModalSize = ModalSize::WIDE;

/// What an empty log list says unless its host says otherwise.
const EMPTY_NOTE: &str = "nothing logged yet";

/// How the list reaches the host's log in its context.
type Source<R, Cx> = Arc<dyn Fn(&Cx) -> &Log<R> + Send + Sync>;
/// The host's clock in its context, in Unix seconds.
type Clock<Cx> = Arc<dyn Fn(&Cx) -> i64 + Send + Sync>;
/// How an entry's detail reads.
type DetailOf<R> = Arc<dyn Fn(&Entry<R>) -> String + Send + Sync>;
/// The host's message that copies an entry.
type CopyOf<R, M> = Arc<dyn Fn(&Entry<R>) -> Option<M> + Send + Sync>;

/// A modal list of a log's entries, newest first, over the selected
/// entry's detail and the footer its key table generates.
///
/// `R` is the payload of the host's log entries, `M` the host's message
/// (which the copy hook builds), and `Cx` the host's context, from which
/// the list reads the log and the time on every key and every draw. The
/// list implements [`Overlay<M, Cx>`] for that context. Drawing keeps the
/// list's scroll window, so the next key moves it no further than the
/// selection needs, and the detail's, so a page key moves by the rows that
/// were on screen. The closures are shared behind an [`Arc`], so a list
/// clones cheaply.
///
/// A host builds one when the user opens the log:
///
/// ```
/// use ratatui::crossterm::event::KeyCode;
/// use tuikit::feedback::{Entry, Log, Severity};
/// use tuikit::log_list::LogList;
/// use tuikit::overlay::{ModalStack, Outcome};
/// use tuikit::testkit::keys::press;
///
/// struct App {
///     log: Log<()>,
///     clock: i64,
/// }
///
/// let mut app = App { log: Log::default(), clock: 50 };
/// for n in 0..3 {
///     app.log.push(Entry::new(n, Severity::Warning, format!("retry {n}"), ()));
/// }
/// let list: LogList<(), (), App> =
///     LogList::new("History", |app: &App| &app.log, |app: &App| app.clock)
///         .mark_seen(&mut app.log);
///
/// let mut stack = ModalStack::new();
/// stack.push(list);
/// // `G` selects the oldest entry; `j` past it stays there.
/// stack.route(press(KeyCode::Char('G')), &app);
/// stack.route(press(KeyCode::Char('j')), &app);
/// let selected = stack.top().and_then(|list| list.selected_entry(&app.log));
/// assert_eq!(selected.map(|entry| entry.summary.as_str()), Some("retry 0"));
/// // `q` dismisses the list.
/// let done = stack.route(press(KeyCode::Char('q')), &app).expect("a list is open");
/// assert_eq!(done, Outcome::Dismiss);
/// ```
pub struct LogList<R, M, Cx: ?Sized> {
    title: String,
    empty: String,
    keys: &'static [KeyRow<LogListKey>],
    source: Source<R, Cx>,
    clock: Clock<Cx>,
    detail_of: DetailOf<R>,
    copy: Option<CopyOf<R, M>>,
    /// The position of the selected entry, newest first.
    selected: usize,
    /// The entries' window at the last draw.
    window: ScrollWindow,
    /// The selected entry's detail.
    detail: Pager<Detail>,
}

impl<R, M, Cx: ?Sized> LogList<R, M, Cx> {
    /// A list titled `title` over the log `source` finds in the host's
    /// context, with ages read against the time `clock` reads there (Unix
    /// seconds, the clock the entries were logged by). It selects the
    /// newest entry, resolves [`LOG_LIST_KEYS`], and shows an entry's
    /// summary as its detail until [`detail`](Self::detail) says
    /// otherwise.
    pub fn new(
        title: impl Into<String>,
        source: impl Fn(&Cx) -> &Log<R> + Send + Sync + 'static,
        clock: impl Fn(&Cx) -> i64 + Send + Sync + 'static,
    ) -> Self
    where
        R: 'static,
    {
        Self {
            title: title.into(),
            empty: EMPTY_NOTE.to_string(),
            keys: LOG_LIST_KEYS,
            source: Arc::new(source),
            clock: Arc::new(clock),
            detail_of: Arc::new(Entry::summary_with_repeats),
            copy: None,
            selected: 0,
            window: ScrollWindow::default(),
            detail: Pager::new(Detail::default()).mode(PagerMode::Embedded),
        }
    }

    /// Resolves keys with `table`, which also generates the footer: a
    /// host's table includes [`LOG_LIST_KEYS`] and adds its own rows (the
    /// key that opened the list as another
    /// [`Close`](LogListKey::Close), a key bound to
    /// [`Copy`](LogListKey::Copy)). The table starts with `Ctrl-C`.
    #[must_use]
    pub fn keys(mut self, table: &'static [KeyRow<LogListKey>]) -> Self {
        self.keys = table;
        self
    }

    /// Reads an entry's detail with `detail`: the text under the list
    /// while the entry is selected, wrapped at the frame's width, one line
    /// per line of the text.
    #[must_use]
    pub fn detail(mut self, detail: impl Fn(&Entry<R>) -> String + Send + Sync + 'static) -> Self {
        self.detail_of = Arc::new(detail);
        self
    }

    /// The copy hook: the host message that copies an entry, or `None`
    /// when there is nothing to copy. A table that binds
    /// [`Copy`](LogListKey::Copy) asks it for the selected entry, and the
    /// list reports its message with [`Outcome::Stay`].
    #[must_use]
    pub fn copy(mut self, copy: impl Fn(&Entry<R>) -> Option<M> + Send + Sync + 'static) -> Self {
        self.copy = Some(Arc::new(copy));
        self
    }

    /// Sets what the list says while the log is empty: what will be
    /// logged there. Each line of `note` is wrapped at the frame's width.
    #[must_use]
    pub fn empty(mut self, note: impl Into<String>) -> Self {
        self.empty = note.into();
        self
    }

    /// Marks every entry of `log` seen ([`Log::mark_seen`]) as the list
    /// opens over it, so the count of unseen warnings and errors a host
    /// shows elsewhere clears once the user looks.
    #[must_use]
    pub fn mark_seen(self, log: &mut Log<R>) -> Self {
        log.mark_seen();
        self
    }

    /// The title on the top border, before the count of entries.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The table the list resolves its keys with.
    pub fn table(&self) -> &'static [KeyRow<LogListKey>] {
        self.keys
    }

    /// The position of the selected entry, newest first: 0 is the newest.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The selected entry of `log`, if it has any.
    pub fn selected_entry<'a>(&self, log: &'a Log<R>) -> Option<&'a Entry<R>> {
        log.newest_first()
            .nth(self.selected.min(log.len().saturating_sub(1)))
    }

    /// The entries' scroll window at the last draw: which rows of the list
    /// were on screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// The detail's scroll window at the last draw: which of its rows were
    /// on screen.
    pub fn detail_window(&self) -> ScrollWindow {
        self.detail.window()
    }

    /// The footer, generated from the table: the keys that select and
    /// scroll, the list's other keys, then the keys that leave.
    pub fn footer(&self) -> Footer {
        let role = |action: &LogListKey| action.role();
        [
            Footer::only(self.keys, |action| {
                matches!(role(action), KeyRole::Move | KeyRole::Page)
            }),
            Footer::only(self.keys, |action| {
                matches!(
                    role(action),
                    KeyRole::Other | KeyRole::Change | KeyRole::Apply
                )
            }),
            Footer::only(self.keys, |action| {
                matches!(role(action), KeyRole::Leave | KeyRole::Abort)
            }),
        ]
        .iter()
        .flat_map(|group| group.segments().iter().cloned())
        .collect()
    }

    /// Draws the list over `area`: the frame titled with the count of
    /// entries, the entries, the selected entry's detail, and the footer,
    /// reading the log and the time from `cx`. It keeps the windows it
    /// showed. On a stack, [`Overlay::render`] does this.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        let log = (self.source)(cx);
        let now = (self.clock)(cx);
        let title = format!("{}{}{}", self.title, marks::SPACED_SEPARATOR, log.len());
        ModalFrame::new(theme, LOG_LIST_SIZE).title(title).render(
            area,
            buf,
            Body {
                list: self,
                log,
                now,
                theme,
            },
        );
    }

    /// Selects the entry at `index`, newest first, within the `len`
    /// entries; the detail goes back to its top when the selection moves.
    fn select(&mut self, index: usize, len: usize) {
        let index = index.min(len.saturating_sub(1));
        if index != self.selected {
            self.selected = index;
            self.detail.to_top();
        }
    }
}

impl<R, M, Cx: ?Sized> Clone for LogList<R, M, Cx> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            empty: self.empty.clone(),
            keys: self.keys,
            source: Arc::clone(&self.source),
            clock: Arc::clone(&self.clock),
            detail_of: Arc::clone(&self.detail_of),
            copy: self.copy.clone(),
            selected: self.selected,
            window: self.window,
            detail: self.detail.clone(),
        }
    }
}

impl<R, M, Cx: ?Sized> fmt::Debug for LogList<R, M, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogList")
            .field("title", &self.title)
            .field("selected", &self.selected)
            .field("window", &self.window)
            .field("detail", &self.detail.window())
            .finish_non_exhaustive()
    }
}

impl<R, M, Cx: ?Sized> Overlay<M, Cx> for LogList<R, M, Cx> {
    /// `j`/`k` and their aliases move the selection and stop at the ends;
    /// `g`/`G` select the newest and the oldest; `J`/`K` scroll the detail
    /// a row and `PgDn`/`Space`/`PgUp` a page; a key bound to
    /// [`Copy`](LogListKey::Copy) reports the copy hook's message with
    /// `Stay`; `Esc` closes the list (`Close`) and `q` and `Ctrl-C`
    /// dismiss it (`Dismiss`), saying nothing. Every other key is
    /// swallowed.
    fn handle(&mut self, key: KeyEvent, cx: &Cx) -> Outcome<M> {
        let Some(action) = resolve(self.keys, key) else {
            return Outcome::Consumed;
        };
        let log = (self.source)(cx);
        let len = log.len();
        match action {
            LogListKey::Abort => return Outcome::Dismiss,
            LogListKey::Close => return Outcome::Close,
            LogListKey::Copy => {
                let copy = self.copy.as_ref();
                let entry = self.selected_entry(log);
                if let Some(message) = copy.zip(entry).and_then(|(copy, entry)| copy(entry)) {
                    return Outcome::Stay(message);
                }
            }
            LogListKey::Down => self.select(Ends::Clamp.step(self.selected, len, 1), len),
            LogListKey::Up => self.select(Ends::Clamp.step(self.selected, len, -1), len),
            LogListKey::Newest => self.select(0, len),
            LogListKey::Oldest => self.select(len.saturating_sub(1), len),
            LogListKey::DetailDown => self.detail.scroll_by(1),
            LogListKey::DetailUp => self.detail.scroll_by(-1),
            LogListKey::DetailPageDown => self.detail.page_down(),
            LogListKey::DetailPageUp => self.detail.page_up(),
        }
        Outcome::Consumed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.draw(area, buf, cx, theme);
    }

    /// The footer the list draws ([`LogList::footer`]).
    fn hints(&self) -> Footer {
        self.footer()
    }
}

#[cfg(test)]
#[path = "log_list_tests.rs"]
mod tests;
