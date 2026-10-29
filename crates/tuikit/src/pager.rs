//! Pager: the one reader, over content the host lays out.
//!
//! This is a layer 1 control (house rules 2, 4, 5, 8, 12, 14, and 18). A
//! [`Pager<C>`](Pager) scrolls a [`PagerContent`]: rows the content lays
//! out for a width, the units a find matches, a header, and keys of its
//! own. Everything a reader shares is the pager's, so every reader of a
//! program pages, finds, steps back, and explains its keys alike:
//!
//! - **Modes** ([`PagerMode`]). `Screen` fills the screen: a header, a
//!   rule, the body, a rule, and a one-row footer with the `[ n/m ]`
//!   position chip. `Modal` draws in a [`ModalFrame`](crate::frame::ModalFrame)
//!   of a named size, titled with the header, with a scrollbar and its
//!   footer kept on the frame's last rows. `Embedded` is the body and a
//!   scrollbar in an area a host lays out (a preview, a detail pane).
//! - **Scrolling.** One window, clamped on every draw and kept: page keys
//!   move by the rows the last draw showed, `Ctrl-D`/`Ctrl-U` by half of
//!   them, `g`/`G` to the ends. `G` (and [`Pager::at_end`]) makes the
//!   window stick to the end, so it follows the newest rows as the content
//!   grows or is laid out again; a key that moves the window lets go.
//! - **Find** ([`Find`]). `/` opens a prompt on the one line editor;
//!   `Enter` confirms and always scrolls to the first match at or below
//!   the top, one row of context above it; `n` and `N` step, wrapping at
//!   the ends with a note saying so. The match the pager is on draws in
//!   the current-match look.
//! - **Levels.** `Esc` steps back: the help page, then the find prompt,
//!   then the active find, then the pager itself. `q` (where nothing is
//!   typed) and `Ctrl-C` leave at once, whatever is open.
//! - **Help.** `?` opens a [`HelpPage`] generated
//!   from the pager's table and the content's, with the help level's own
//!   footer.
//! - **Footer.** Generated from the tables in the one grammar: what
//!   scrolls, what else the pager does, the content's keys, then what
//!   leaves. A full-screen footer too narrow drops the scroll keys first,
//!   then the pager's other keys, then the content's chips, then its keys;
//!   the keys that leave stay.
//!
//! ```
//! use std::ops::Range;
//!
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use tuikit::keys::KeyRow;
//! use tuikit::overlay::Outcome;
//! use tuikit::pager::{Find, NoKeys, Pager, PagerContent, PagerEffect};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::testkit::keys::{press, typed};
//! use tuikit::theme::Theme;
//!
//! /// A document of lines, each a unit a find matches.
//! struct Document(Vec<String>);
//!
//! impl PagerContent for Document {
//!     type Message = ();
//!     type Action = NoKeys;
//!     const KEYS: &'static [KeyRow<NoKeys>] = &[];
//!
//!     fn layout(&mut self, _width: u16, _theme: &Theme) -> usize {
//!         self.0.len()
//!     }
//!
//!     fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
//!         self.0[range].iter().map(|line| Line::from(line.clone())).collect()
//!     }
//!
//!     fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
//!         Line::from(" notes")
//!     }
//!
//!     fn find(&self, query: &str) -> Vec<usize> {
//!         let lines = self.0.iter().enumerate();
//!         lines.filter(|(_, line)| line.contains(query)).map(|(at, _)| at).collect()
//!     }
//!
//!     fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
//!         for (at, row) in rows.iter_mut().enumerate() {
//!             *row = find.paint(row, 0, find.current_unit() == Some(first + at), theme);
//!         }
//!     }
//!
//!     fn apply(&mut self, action: NoKeys) -> PagerEffect<()> {
//!         match action {}
//!     }
//! }
//!
//! let lines = (1..=40).map(|n| format!("line {n}")).collect();
//! let mut pager = Pager::new(Document(lines));
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 60, 12);
//! let mut buf = Buffer::empty(area);
//! pager.draw(area, &mut buf, &theme);
//!
//! // `/`, a query, and `Enter`: the pager scrolls to the match.
//! pager.handle_key(typed('/'));
//! for c in "line 30".chars() {
//!     pager.handle_key(typed(c));
//! }
//! assert_eq!(pager.handle_key(press(ratatui::crossterm::event::KeyCode::Enter)), Outcome::Consumed);
//! pager.draw(area, &mut buf, &theme);
//! let screen = rows(&buf).join("\n");
//! assert!(screen.contains("find \"line 30\" 1/1"), "{screen}");
//! assert_eq!(pager.window().offset(), 28, "one row of context above the match");
//!
//! // `Esc` clears the find, and then leaves.
//! assert_eq!(pager.handle_key(press(ratatui::crossterm::event::KeyCode::Esc)), Outcome::Consumed);
//! assert_eq!(pager.handle_key(press(ratatui::crossterm::event::KeyCode::Esc)), Outcome::Close);
//! ```
//!
//! # What a host keeps
//!
//! What the rows are and how they wrap, what a unit is and which ones
//! hold a query, the header's words, the content's keys and what they do
//! (a request the host handles: attach, copy), the table the pager uses
//! when it is not the standard reader's (a pager opened by a key closes on
//! it too), and when to open, land, or note.

mod body;
mod content;
mod find;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub use content::{NoKeys, PagerContent, PagerEffect};
pub use find::Find;

use crate::feedback::Feedback;
use crate::frame::ModalSize;
use crate::help::{
    HelpAction, HelpEntry, HelpPage, HelpRow, HelpScroll, HelpSection, sections_for,
};
use crate::keys::{
    FIND_KEYS, FindKey, Footer, KeyAction, KeyRole, KeyRow, PAGER_HELP_KEYS, PagerKey, READER_KEYS,
    key_label, resolve, rows,
};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::ScrollWindow;
use crate::theme::Theme;
use find::Step;

/// Rows of context a find leaves above the match it scrolls to.
pub const CONTEXT_ROWS: usize = 1;

/// How a pager sits on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagerMode {
    /// Fills the screen: header, rule, body, rule, and a one-row footer
    /// with the `[ n/m ]` position chip.
    Screen,
    /// In a modal frame of this size, titled with the header, with a
    /// scrollbar and the footer on the frame's last rows.
    Modal(ModalSize),
    /// The body and a scrollbar in the area a host gives it, with no
    /// chrome: a preview or a detail pane, whose keys the host routes.
    Embedded,
}

/// The one reader: a [`PagerContent`] scrolled, found in, and explained
/// by the framework.
///
/// Build one with [`new`](Self::new) (a full-screen reader with the
/// standard [`READER_KEYS`]) and the options; hand it keys with
/// [`handle_key`](Self::handle_key) and draw it with
/// [`draw`](Self::draw), or put it on a modal stack, since it is an
/// [`Overlay`] for any context. The draw lays the content out, clamps the
/// window, scrolls to a match a find landed on, and keeps the window it
/// showed, so the next page key moves by the rows that were on screen.
#[derive(Clone, Debug)]
pub struct Pager<C> {
    content: C,
    mode: PagerMode,
    keys: &'static [KeyRow<PagerKey>],
    scroll: Scroll,
    find: Find,
    help: Option<HelpScroll>,
    note: Option<Feedback>,
    /// The width the content was last laid out at, while that layout
    /// still holds.
    laid_out: Option<u16>,
}

/// Where a pager is scrolled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Scroll {
    /// The first row asked for; each draw brings it inside the content.
    offset: usize,
    /// Whether the window keeps to the end as the content changes.
    stick: bool,
    /// The window the last draw showed.
    window: ScrollWindow,
}

impl Scroll {
    /// Whether a draw has measured the window yet.
    fn drawn(&self) -> bool {
        self.window.viewport() > 0
    }

    /// The offset a step starts from: where the window is.
    fn from(&self) -> usize {
        if !self.drawn() {
            self.offset
        } else if self.stick {
            self.window.last_offset()
        } else {
            self.offset.min(self.window.last_offset())
        }
    }

    /// Moves to `offset`, letting go of the end, clamped to the window
    /// the last draw measured.
    fn go(&mut self, offset: usize) {
        self.stick = false;
        self.offset = if self.drawn() {
            offset.min(self.window.last_offset())
        } else {
            offset
        };
    }

    fn down(&mut self, rows: usize) {
        self.go(self.from().saturating_add(rows));
    }

    fn up(&mut self, rows: usize) {
        self.go(self.from().saturating_sub(rows));
    }

    fn page(&self) -> usize {
        self.window.page()
    }

    fn half(&self) -> usize {
        (self.page() / 2).max(1)
    }

    fn stick_to_end(&mut self) {
        self.stick = true;
        self.offset = self.window.last_offset();
    }

    /// Takes in the window a draw showed.
    fn keep(&mut self, window: ScrollWindow) {
        self.offset = window.offset();
        self.window = window;
    }
}

impl<C: PagerContent> Pager<C> {
    /// A full-screen reader over `content`, at the top, resolving the
    /// standard [`READER_KEYS`].
    pub fn new(content: C) -> Self {
        Self {
            content,
            mode: PagerMode::Screen,
            keys: READER_KEYS,
            scroll: Scroll::default(),
            find: Find::default(),
            help: None,
            note: None,
            laid_out: None,
        }
    }

    /// Draws in `mode`.
    #[must_use]
    pub fn mode(mut self, mode: PagerMode) -> Self {
        self.mode = mode;
        self
    }

    /// Resolves the pager's keys with `table`, which also generates its
    /// footer and help page: a pager with no find or help page binds no
    /// [`Find`](PagerKey::Find) or [`Help`](PagerKey::Help); one opened by
    /// a key adds it as another [`Close`](PagerKey::Close). The table
    /// starts with `Ctrl-C` and has no notes.
    #[must_use]
    pub fn keys(mut self, table: &'static [KeyRow<PagerKey>]) -> Self {
        self.keys = table;
        self
    }

    /// Opens at the end, keeping to it as the content grows.
    #[must_use]
    pub fn at_end(mut self) -> Self {
        self.scroll.stick_to_end();
        self
    }

    /// Opens with `row` at the top, or as near as the content allows.
    #[must_use]
    pub fn at_row(mut self, row: usize) -> Self {
        self.scroll.go(row);
        self
    }

    /// How the pager sits on the screen.
    pub fn draw_mode(&self) -> PagerMode {
        self.mode
    }

    /// The table the pager resolves its keys with.
    pub fn table(&self) -> &'static [KeyRow<PagerKey>] {
        self.keys
    }

    /// The content.
    pub fn content(&self) -> &C {
        &self.content
    }

    /// The content, to change it; call [`refresh`](Self::refresh) after a
    /// change to its rows.
    pub fn content_mut(&mut self) -> &mut C {
        &mut self.content
    }

    /// Takes in a change to the content's rows: the next draw lays them
    /// out, and the active find runs again, staying on the unit it was on.
    pub fn refresh(&mut self) {
        self.laid_out = None;
        if self.find.is_active() {
            let unit = self.find.current_unit();
            let query = self.find.query().to_string();
            self.find.set(&query, self.content.find(&query));
            self.find.stay_on(unit);
        }
    }

    /// The find.
    pub fn find(&self) -> &Find {
        &self.find
    }

    /// Finds `query` as confirming it in the prompt does: the first match
    /// at or below the top, scrolled to by the next draw; an empty query
    /// clears the find.
    pub fn start_find(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.find.clear();
            return;
        }
        self.find.set(query, self.content.find(query));
        if !self.find.matches().is_empty() {
            let at = self.with_rows(|find, rows_of, top| find.first_from(rows_of, top));
            self.find.go(at);
        }
    }

    /// Finds `query` and goes to the match nearest unit `near`, for a
    /// pager opened at a match found elsewhere (a search result). Returns
    /// whether there was one; with none, the find is active with no match
    /// and the pager stays where it is.
    pub fn land(&mut self, query: &str, near: usize) -> bool {
        self.find.set(query, self.content.find(query.trim()));
        match self.find.nearest(near) {
            Some(at) => {
                self.find.go(at);
                true
            }
            None => false,
        }
    }

    /// The note the footer shows until the next key.
    pub fn note(&self) -> Option<&Feedback> {
        self.note.as_ref()
    }

    /// Shows `note` in the footer until the next key: why a key did
    /// nothing, or what opening the pager changed.
    pub fn set_note(&mut self, note: Feedback) {
        self.note = Some(note);
    }

    /// The window the last draw showed.
    pub fn window(&self) -> ScrollWindow {
        self.scroll.window
    }

    /// Whether the window keeps to the end.
    pub fn follows_end(&self) -> bool {
        self.scroll.stick
    }

    /// Whether the help page is open.
    pub fn help_is_open(&self) -> bool {
        self.help.is_some()
    }

    /// Where the help page is scrolled, while it is open.
    pub fn help_scroll(&self) -> Option<&HelpScroll> {
        self.help.as_ref()
    }

    /// Whether the find prompt has the keys, so a paste goes to it.
    pub fn takes_text(&self) -> bool {
        self.find.prompt().is_some()
    }

    /// Scrolls `delta` rows, down when positive, stopping at the ends.
    pub fn scroll_by(&mut self, delta: i32) {
        let rows = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        if delta >= 0 {
            self.scroll.down(rows);
        } else {
            self.scroll.up(rows);
        }
    }

    /// Scrolls down a page: the rows the last draw showed.
    pub fn page_down(&mut self) {
        self.scroll.down(self.scroll.page());
    }

    /// Scrolls up a page.
    pub fn page_up(&mut self) {
        self.scroll.up(self.scroll.page());
    }

    /// Scrolls down half a page, at least one row.
    pub fn half_page_down(&mut self) {
        self.scroll.down(self.scroll.half());
    }

    /// Scrolls up half a page, at least one row.
    pub fn half_page_up(&mut self) {
        self.scroll.up(self.scroll.half());
    }

    /// Scrolls to the first row.
    pub fn to_top(&mut self) {
        self.scroll.go(0);
    }

    /// Scrolls to the end and keeps to it.
    pub fn to_end(&mut self) {
        self.scroll.stick_to_end();
    }

    /// Handles one key: the find prompt's while it is open, the help
    /// page's while it is open, and otherwise the pager's table, then the
    /// content's. Every key clears the note first. `Ctrl-C`, and `q` where
    /// nothing is typed, answer [`Outcome::Dismiss`]; `Esc` steps back and
    /// answers [`Outcome::Close`] at the pager's first level.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<C::Message> {
        self.note = None;
        if self.find.prompt().is_some() {
            return self.prompt_key(key);
        }
        if let Some(help) = &mut self.help {
            return match resolve(PAGER_HELP_KEYS, key) {
                Some(PagerKey::Abort) => Outcome::Dismiss,
                Some(PagerKey::Close | PagerKey::Help) => {
                    self.help = None;
                    Outcome::Consumed
                }
                Some(other) => {
                    help.apply(other);
                    Outcome::Consumed
                }
                None => Outcome::Consumed,
            };
        }
        if let Some(action) = resolve(self.keys, key) {
            return self.act(action);
        }
        match resolve(C::KEYS, key).map(|action| self.content.apply(action)) {
            Some(PagerEffect::Relayout) => {
                self.refresh();
                Outcome::Consumed
            }
            Some(PagerEffect::Close) => Outcome::Close,
            Some(PagerEffect::Request(message)) => Outcome::Stay(message),
            Some(PagerEffect::None) | None => Outcome::Consumed,
        }
    }

    fn prompt_key(&mut self, key: KeyEvent) -> Outcome<C::Message> {
        match resolve(FIND_KEYS, key) {
            Some(FindKey::Abort) => return Outcome::Dismiss,
            Some(FindKey::Cancel) => self.find.close_prompt(),
            Some(FindKey::Confirm) => {
                let text = self.find.take_prompt().unwrap_or_default();
                self.start_find(&text);
            }
            None => self.find.edit(key),
        }
        Outcome::Consumed
    }

    fn act(&mut self, action: PagerKey) -> Outcome<C::Message> {
        match action {
            PagerKey::Abort => return Outcome::Dismiss,
            PagerKey::Close if self.find.is_active() => self.find.clear(),
            PagerKey::Close => return Outcome::Close,
            PagerKey::Down => self.scroll_by(1),
            PagerKey::Up => self.scroll_by(-1),
            PagerKey::PageDown => self.page_down(),
            PagerKey::PageUp => self.page_up(),
            PagerKey::HalfPageDown => self.half_page_down(),
            PagerKey::HalfPageUp => self.half_page_up(),
            PagerKey::Top => self.to_top(),
            PagerKey::Bottom => self.to_end(),
            PagerKey::Find => self.find.open_prompt(),
            PagerKey::FindNext => self.step(true),
            PagerKey::FindPrev => self.step(false),
            PagerKey::Help => self.help = Some(HelpScroll::new()),
            PagerKey::Copy => {
                if let Some(message) = self.content.copy() {
                    return Outcome::Stay(message);
                }
            }
        }
        Outcome::Consumed
    }

    /// Steps the find, saying so in the note when it wraps or has nothing
    /// to step through.
    fn step(&mut self, forward: bool) {
        let window = self.scroll.window.visible();
        let step = self.with_rows(|find, rows_of, _| find.step(forward, rows_of, window));
        self.note = match step {
            Step::Inactive => Some(Feedback::warning(format!(
                "nothing to find yet: press {} to search",
                self.label_of(PagerKey::Find)
            ))),
            Step::Moved { wrapped: true } => Some(Feedback::info(if forward {
                "wrapped to the first match"
            } else {
                "wrapped to the last match"
            })),
            Step::Moved { wrapped: false } | Step::Nothing => None,
        };
    }

    /// Runs `f` with the find, how to read a unit's rows from the last
    /// layout (`None` while there is none), and the window's top row.
    fn with_rows<R>(
        &mut self,
        f: impl FnOnce(&mut Find, Option<&dyn Fn(usize) -> std::ops::Range<usize>>, usize) -> R,
    ) -> R {
        let top = self.scroll.from();
        let content = &self.content;
        let rows_of = |unit: usize| content.match_rows(unit);
        let laid_out: Option<&dyn Fn(usize) -> std::ops::Range<usize>> =
            self.laid_out.is_some().then_some(&rows_of);
        f(&mut self.find, laid_out, top)
    }

    /// The first key the pager's table binds to `action`, as labels write
    /// it.
    fn label_of(&self, action: PagerKey) -> String {
        rows(self.keys)
            .find_map(|row| match row {
                KeyRow::Bind {
                    key, action: bound, ..
                } if *bound == action => Some(key_label(key)),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The pager's footer, generated from its table and the content's:
    /// the keys that scroll, the pager's other keys, the content's keys,
    /// then the keys that leave.
    pub fn footer(&self) -> Footer {
        self.hint_groups()
            .iter()
            .flat_map(|group| group.segments().iter().cloned())
            .collect()
    }

    /// The footer's four groups, in the order they are drawn: the keys
    /// that scroll, the pager's other keys, the content's, and the ones
    /// that leave.
    fn hint_groups(&self) -> [Footer; 4] {
        let role = |action: &PagerKey| action.role();
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
            Footer::new(C::KEYS),
            Footer::only(self.keys, |action| {
                matches!(role(action), KeyRole::Leave | KeyRole::Abort)
            }),
        ]
    }

    /// The help page's sections: the pager's table's, in the content's
    /// words where it has its own ([`PagerContent::describe`]), with the
    /// content's sections before the pager's last one (the keys that
    /// leave).
    pub fn help_sections(&self) -> Vec<HelpSection> {
        let mut sections = sections_for(self.keys);
        // The table's rows follow the listing, each in the next row of its
        // section, so a key's row is found by counting.
        let mut taken: Vec<usize> = vec![0; sections.len()];
        for &action in PagerKey::LISTED {
            let bound = rows(self.keys)
                .any(|row| matches!(row, KeyRow::Bind { action: a, .. } if *a == action));
            if !bound {
                continue;
            }
            let title = action.help().section;
            let Some(at) = sections.iter().position(|section| section.title == title) else {
                continue;
            };
            let row = taken[at];
            taken[at] += 1;
            if let (Some(words), Some(HelpRow::Keys { description, .. })) =
                (C::describe(action), sections[at].rows.get_mut(row))
            {
                *description = words.into();
            }
        }
        let leaving = PagerKey::Close.help().section;
        let at = sections
            .iter()
            .position(|section| section.title == leaving)
            .unwrap_or(sections.len());
        sections.splice(at..at, sections_for(C::KEYS));
        sections
    }

    /// Draws the pager over `area` in its mode, and its help page over
    /// that while it is open.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        match self.mode {
            PagerMode::Screen => body::screen(self, area, buf, theme),
            PagerMode::Modal(size) => body::modal(self, size, area, buf, theme),
            PagerMode::Embedded => body::embedded(self, area, buf, theme),
        }
        if self.help.is_some() {
            let sections = self.help_sections();
            let page = HelpPage::new(theme, &sections).footer(PAGER_HELP_KEYS);
            if let Some(help) = &mut self.help {
                let window = page.scrolled(help).render(area, buf);
                help.keep(window);
            }
        }
    }
}

/// A pager is an overlay for any context: it reads only its content.
/// It fills the screen in [`PagerMode::Screen`].
impl<C: PagerContent, Cx: ?Sized> Overlay<C::Message, Cx> for Pager<C> {
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<C::Message> {
        self.handle_key(key)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    fn fills_screen(&self) -> bool {
        self.mode == PagerMode::Screen
    }

    fn takes_text(&self) -> bool {
        Pager::takes_text(self)
    }
}

impl HelpAction for PagerKey {
    const LISTED: &'static [Self] = &[
        PagerKey::Down,
        PagerKey::Up,
        PagerKey::PageDown,
        PagerKey::PageUp,
        PagerKey::HalfPageDown,
        PagerKey::HalfPageUp,
        PagerKey::Top,
        PagerKey::Bottom,
        PagerKey::Find,
        PagerKey::FindNext,
        PagerKey::FindPrev,
        PagerKey::Copy,
        PagerKey::Help,
        PagerKey::Close,
        PagerKey::Abort,
    ];

    fn help(self) -> HelpEntry {
        let (section, description) = match self {
            PagerKey::Down => ("Moving", "Scroll one line down"),
            PagerKey::Up => ("Moving", "Scroll one line up"),
            PagerKey::PageDown => ("Moving", "Page down"),
            PagerKey::PageUp => ("Moving", "Page up"),
            PagerKey::HalfPageDown => ("Moving", "Half a page down"),
            PagerKey::HalfPageUp => ("Moving", "Half a page up"),
            PagerKey::Top => ("Moving", "Jump to the start"),
            PagerKey::Bottom => ("Moving", "Jump to the end and stay there"),
            PagerKey::Find => ("Finding", "Find text"),
            PagerKey::FindNext => ("Finding", "Next match, wrapping at the end"),
            PagerKey::FindPrev => ("Finding", "Previous match, wrapping at the start"),
            PagerKey::Copy => ("Viewer", "Copy"),
            PagerKey::Help => ("Viewer", "Show or hide this help"),
            PagerKey::Close => (
                "Viewer",
                "Step back: close this help, then the find, then the viewer",
            ),
            PagerKey::Abort => (
                "Viewer",
                "Leave the viewer at once, even with a find active",
            ),
        };
        HelpEntry::new(section, description)
    }
}

#[cfg(test)]
#[path = "pager_tests.rs"]
mod tests;
