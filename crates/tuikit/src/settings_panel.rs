//! Settings panels: a program's settings in titled sections of rows, in
//! one modal, where every change applies as it is made.
//!
//! This is a layer 1 control (house rules 1, 2, 4, 5, 8, 10, 12, 13, and
//! 14). A program gathers the settings an operator changes as they work
//! (which view, how it is grouped and sorted, which filters narrow it) in
//! one panel, so they are discoverable without a key for each; every such
//! panel should move, page, apply, and close the same way:
//!
//! - **Rows as data, built from the host's state.** A
//!   [`SettingsPanel<Id, M, Cx>`](SettingsPanel) holds a title and the
//!   host's row model: a closure that builds the [`Section`]s from the
//!   host's context `&Cx`. The panel asks for them on every key and every
//!   draw, so a row always shows the value in force, and keeps only its
//!   cursor (the host's [`Row::id`]) and its scroll window.
//! - **Five kinds of row** ([`Kind`]): a *choice* among several (drawn
//!   bold with `active` when in force), a *checkbox*, an *editor* that
//!   shows a value and opens an editor for it, a *cycle* that steps a
//!   value in place, and a *button*.
//! - **One cursor over the rows.** Headings are not rows: the cursor
//!   steps over them, wrapping from the last row to the first (house rule
//!   2), and rows are drawn through
//!   [`selection::row`](crate::selection::row). `PgUp`/`PgDn` move it by
//!   the rows shown and the ends keys take it to the first or last row,
//!   stopping there (house rule 12).
//! - **Changes apply as they are made.** Acting on a row (`Space`)
//!   reports the row's message at once with
//!   [`Outcome::Stay`]: the host applies
//!   it and the panel stays open, showing the new value. `Enter` on a
//!   choice applies it and closes the panel
//!   ([`Outcome::Commit`]); on a checkbox
//!   or a cycle, whose change `Space` already applied, it closes. `Esc`
//!   closes and keeps everything applied, because there is nothing left
//!   to cancel.
//! - **Editors are steps.** An editor row's message asks the host to open
//!   the row's editor (a [`Checklist`](crate::checklist::Checklist) or a
//!   [`TextPrompt`](crate::text_prompt::TextPrompt)) as a step over the
//!   panel ([`ModalStack::push_step`](crate::overlay::ModalStack::push_step)).
//!   The editor answers with
//!   [`Outcome::Back`], so its value
//!   applies and the panel has the keys again; `Esc` there steps back,
//!   and `Ctrl-C` dismisses both. Under its editor the panel's cursor
//!   row takes the inactive selection look.
//! - **Scrolling with a fixed footer.** The panel is the menu-sized modal
//!   frame, as tall as its rows. On a screen too short for them the
//!   footer [`SETTINGS_KEYS`] generates keeps the bottom rows and the
//!   rows scroll in a window the panel keeps, moving no further than the
//!   cursor needs, with the shared scrollbar beside them.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::settings_panel::{Row, Section, SettingsPanel};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Wrap(bool),
//!     Sort(&'static str),
//! }
//!
//! struct Prefs {
//!     wrap: bool,
//!     sort: &'static str,
//! }
//!
//! let mut panel = SettingsPanel::new("Settings", |prefs: &Prefs| {
//!     let sorts = ["name", "date"].map(|sort| {
//!         let active = prefs.sort == sort;
//!         Row::choice(sort, sort, active, (!active).then_some(Msg::Sort(sort)))
//!     });
//!     vec![
//!         Section::new(
//!             "Display",
//!             vec![Row::checkbox("wrap", "Wrap long lines", prefs.wrap, Msg::Wrap(!prefs.wrap))],
//!         ),
//!         Section::new("Sort", sorts.into()),
//!     ]
//! });
//! let mut prefs = Prefs { wrap: false, sort: "name" };
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // Space applies at once and the panel stays open.
//! let toggled = panel.handle(press(KeyCode::Char(' ')), &prefs);
//! assert_eq!(toggled, Outcome::Stay(Msg::Wrap(true)));
//! prefs.wrap = true;
//!
//! // The cursor steps over the headings; Enter on a choice applies it and
//! // closes the panel.
//! panel.handle(press(KeyCode::Down), &prefs);
//! panel.handle(press(KeyCode::Down), &prefs);
//! assert_eq!(panel.cursor(), Some(&"date"));
//! assert_eq!(panel.handle(press(KeyCode::Enter), &prefs), Outcome::Commit(Msg::Sort("date")));
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 70, 16));
//! panel.draw(buf.area, &mut buf, &prefs, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("  [x] Wrap long lines"), "{drawn}");
//! assert!(drawn.contains("  name  active"), "{drawn}");
//! assert!(drawn.contains("► date"), "{drawn}");
//! assert!(drawn.contains("Enter apply & close · Esc close"), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! Which settings there are, in which sections and order, and which of
//! them a context shows; their labels and the values they show; what
//! acting on each means (the message the row carries, built from the
//! host's state); the editors (which values a checklist offers, how a
//! prompt parses its text) and opening them as steps when their row's
//! message arrives. The panel decides only how the rows look, move,
//! scroll, and close.

mod body;
mod row;

pub use row::{Kind, Row, Section};

use std::fmt;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::frame::{ModalFrame, ModalSize};
use crate::keys::{Footer, SETTINGS_KEYS, SettingsKey, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;
use crate::text;
use crate::theme::Theme;

use body::{Body, View, places};

/// How large every settings panel is: the menu-sized modal frame, as
/// tall as its rows.
pub const SETTINGS_SIZE: ModalSize = ModalSize::MENU;

/// The host's row model: the sections as its context has them now.
type Model<Id, M, Cx> = Arc<dyn Fn(&Cx) -> Vec<Section<Id, M>> + Send + Sync>;

/// A modal panel of settings: a title, the sections the host's row model
/// builds from its context, and the footer the standard settings table
/// generates.
///
/// `Id` is the host's identity for a row, which the cursor keeps across
/// the rebuilds; `M` is the host's message, which acting on a row
/// reports; `Cx` is the host's context, from which the rows are built on
/// every key and every draw. The panel implements [`Overlay<M, Cx>`].
/// Drawing keeps the scroll window it showed, so the next key moves the
/// window no further than the cursor needs. The row model is shared
/// behind an [`Arc`], so a panel clones cheaply.
///
/// An editor row opens its editor as a step over the panel, and the
/// editor steps back with its value:
///
/// ```
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use tuikit::overlay::{ModalStack, Outcome, Overlay};
/// use tuikit::settings_panel::{Row, Section, SettingsPanel};
/// use tuikit::text_prompt::{PromptResult, TextPrompt};
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum Msg {
///     EditName,
///     Rename(String),
/// }
///
/// struct Profile {
///     name: String,
/// }
///
/// let panel = SettingsPanel::new("Profile", |profile: &Profile| {
///     vec![Section::new(
///         "Profile",
///         vec![Row::editor("name", "name", profile.name.clone(), Msg::EditName)],
///     )]
/// });
/// let mut stack: ModalStack<Box<dyn Overlay<Msg, Profile>>> = ModalStack::new();
/// stack.push(Box::new(panel));
/// let mut profile = Profile { name: "ada".into() };
/// let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
///
/// // Enter on the editor row asks the host to open the editor.
/// let open = stack.route(press(KeyCode::Enter), &profile).expect("open");
/// assert_eq!(open, Outcome::Stay(Msg::EditName));
/// let prompt = TextPrompt::new("name", profile.name.clone(), |text: &str| {
///     PromptResult::Back(Msg::Rename(text.trim().to_string()))
/// });
/// stack.push_step(Box::new(prompt));
///
/// // The editor's answer applies, and the panel is there again.
/// stack.route(press(KeyCode::Char('!')), &profile);
/// let answer = stack.route(press(KeyCode::Enter), &profile).expect("open");
/// let settled = stack.settle(answer);
/// assert_eq!(settled.message, Some(Msg::Rename("ada!".into())));
/// profile.name = "ada!".into();
/// assert_eq!(stack.len(), 1, "the editor closed and the panel stayed");
///
/// // Esc closes the panel, keeping what was applied.
/// let close = stack.route(press(KeyCode::Esc), &profile).expect("open");
/// assert_eq!(close, Outcome::Close);
/// ```
pub struct SettingsPanel<Id, M, Cx: ?Sized> {
    title: String,
    model: Model<Id, M, Cx>,
    cursor: Option<Id>,
    window: ScrollWindow,
    name_width: Option<usize>,
}

impl<Id: Clone + PartialEq, M, Cx: ?Sized> SettingsPanel<Id, M, Cx> {
    /// A panel titled `title` whose rows `model` builds from the host's
    /// context, with the cursor on the first row. The panel calls `model`
    /// on every key and every draw.
    pub fn new(
        title: impl Into<String>,
        model: impl Fn(&Cx) -> Vec<Section<Id, M>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            model: Arc::new(model),
            cursor: None,
            window: ScrollWindow::default(),
            name_width: None,
        }
    }

    /// Puts the cursor on the row `id`: where the panel opens. A row the
    /// model does not build leaves the cursor on the first row.
    #[must_use]
    pub fn at(mut self, id: Id) -> Self {
        self.cursor = Some(id);
        self
    }

    /// Sets the width of the name column of editor and cycle rows, in
    /// cells, so their values line up in one column whichever rows the
    /// context shows. By default the column is as wide as the widest
    /// name the rows have now.
    #[must_use]
    pub fn name_width(mut self, cells: usize) -> Self {
        self.name_width = Some(cells);
        self
    }

    /// The title on the panel's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The id of the row the cursor was last put on: by [`at`](Self::at)
    /// or by a key. `None` on a panel opened without `at` until a key
    /// moves the cursor; such a panel's cursor is on its first row.
    pub fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// The scroll window of the last draw: which lines of the panel were
    /// on screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// The sections the host's row model builds from `cx` now.
    pub fn sections(&self, cx: &Cx) -> Vec<Section<Id, M>> {
        (self.model)(cx)
    }

    /// Draws the panel over `area`: the frame, the sections, and the
    /// footer, with the cursor row in the active selection look, and
    /// keeps the window it showed. On a stack, [`Overlay::render`] does
    /// this.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.paint(area, buf, cx, theme, true);
    }

    /// Draws the panel as [`draw`](Self::draw) does, with the cursor row
    /// in the inactive selection look: how it shows under an editor of
    /// its own, which has the keys. On a stack,
    /// [`Overlay::render_under_step`] does this.
    pub fn draw_under_step(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.paint(area, buf, cx, theme, false);
    }

    fn paint(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme, focused: bool) {
        let sections = (self.model)(cx);
        let ids = Self::ids(&sections);
        let cursor = (!ids.is_empty()).then(|| self.index(&ids));
        let name_width = self.name_width.unwrap_or_else(|| widest_name(&sections));
        let frame = ModalFrame::new(theme, SETTINGS_SIZE).title(self.title.as_str());
        let body = Body {
            places: places(&sections),
            view: View {
                sections: &sections,
                cursor,
                focused,
                name_width,
                theme,
            },
            window: self.window,
        };
        self.window = frame.render(area, buf, body);
    }

    /// The ids of every row, in order.
    fn ids(sections: &[Section<Id, M>]) -> Vec<Id> {
        sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.id.clone()))
            .collect()
    }

    /// The index of the cursor's row among `ids`: the row it was put on,
    /// or the first row when the rows have no such row.
    fn index(&self, ids: &[Id]) -> usize {
        self.cursor
            .as_ref()
            .and_then(|cursor| ids.iter().position(|id| id == cursor))
            .unwrap_or(0)
    }
}

/// Where a key moves the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    /// One row by the step, wrapping at the ends.
    Step(i32),
    /// To the row nearest the line a page away, stopping at the ends.
    Page {
        /// Whether the page is down the panel.
        down: bool,
    },
    /// To the first row.
    Top,
    /// To the last row.
    Bottom,
}

impl<Id: Clone + PartialEq, M, Cx: ?Sized> SettingsPanel<Id, M, Cx> {
    /// Moves the cursor as `to` says over the rows the model builds from
    /// `cx` now.
    fn move_to(&mut self, to: Move, cx: &Cx) {
        let sections = (self.model)(cx);
        let ids = Self::ids(&sections);
        let Some(last) = ids.len().checked_sub(1) else {
            return;
        };
        let places = places(&sections);
        let at = self.index(&ids);
        let next = match to {
            Move::Step(delta) => Ends::Wrap.step(at, ids.len(), delta),
            Move::Page { down } => {
                let page = self.window.page();
                let line = places[at].line;
                if down {
                    let target = line.saturating_add(page);
                    places
                        .iter()
                        .position(|place| place.line >= target)
                        .unwrap_or(last)
                } else {
                    let target = line.saturating_sub(page);
                    places
                        .iter()
                        .rposition(|place| place.line <= target)
                        .unwrap_or(0)
                }
            }
            Move::Top => 0,
            Move::Bottom => last,
        };
        self.cursor = Some(ids[next].clone());
    }

    /// What acting on the cursor's row reports, for `Space` (`enter`
    /// false) or `Enter`. With no rows, `Enter` closes and `Space` does
    /// nothing.
    fn act_on(&mut self, cx: &Cx, enter: bool) -> Outcome<M> {
        let mut rows: Vec<Row<Id, M>> = (self.model)(cx)
            .into_iter()
            .flat_map(|section| section.rows)
            .collect();
        if rows.is_empty() {
            return if enter {
                Outcome::Close
            } else {
                Outcome::Consumed
            };
        }
        let ids: Vec<Id> = rows.iter().map(|row| row.id.clone()).collect();
        let at = self.index(&ids);
        self.cursor = Some(ids[at].clone());
        Self::act(rows.swap_remove(at), enter)
    }

    /// What acting on `row` reports: `Space` (`enter` false) or `Enter`.
    fn act(row: Row<Id, M>, enter: bool) -> Outcome<M> {
        let Row {
            kind,
            action,
            leaves,
            ..
        } = row;
        match (kind, enter) {
            // Space already applied a checkbox's or a cycle's change.
            (Kind::Checkbox { .. } | Kind::Cycle { .. }, true) => Outcome::Close,
            // Enter applies a choice and closes; on the one in force it
            // just closes.
            (Kind::Choice { .. }, true) => action.map_or(Outcome::Close, Outcome::Commit),
            _ => match action {
                None => Outcome::Consumed,
                Some(message) if leaves => Outcome::Commit(message),
                Some(message) => Outcome::Stay(message),
            },
        }
    }
}

/// The widest name among the editor and cycle rows of `sections`, in
/// cells.
fn widest_name<Id, M>(sections: &[Section<Id, M>]) -> usize {
    sections
        .iter()
        .flat_map(|section| &section.rows)
        .filter(|row| matches!(row.kind, Kind::Editor { .. } | Kind::Cycle { .. }))
        .map(|row| text::width(&row.label))
        .max()
        .unwrap_or(0)
}

impl<Id: Clone, M, Cx: ?Sized> Clone for SettingsPanel<Id, M, Cx> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            model: Arc::clone(&self.model),
            cursor: self.cursor.clone(),
            window: self.window,
            name_width: self.name_width,
        }
    }
}

impl<Id: fmt::Debug, M, Cx: ?Sized> fmt::Debug for SettingsPanel<Id, M, Cx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsPanel")
            .field("title", &self.title)
            .field("cursor", &self.cursor)
            .field("window", &self.window)
            .field("name_width", &self.name_width)
            .finish_non_exhaustive()
    }
}

impl<Id: Clone + PartialEq, M, Cx: ?Sized> Overlay<M, Cx> for SettingsPanel<Id, M, Cx> {
    /// The arrows and their aliases move over the rows, wrapping;
    /// `PgUp`/`PgDn` and the ends keys move and stop; `Space` acts on the
    /// cursor's row and `Enter` applies it (see [`Kind`] for each kind);
    /// `Esc` closes the panel (`Close`) and `q` and `Ctrl-C` dismiss it
    /// (`Dismiss`), saying nothing and keeping every change applied.
    /// Every other key is swallowed.
    fn handle(&mut self, key: KeyEvent, cx: &Cx) -> Outcome<M> {
        let Some(action) = resolve(SETTINGS_KEYS, key) else {
            return Outcome::Consumed;
        };
        let to = match action {
            SettingsKey::Abort => return Outcome::Dismiss,
            SettingsKey::Close => return Outcome::Close,
            SettingsKey::Change => return self.act_on(cx, false),
            SettingsKey::Apply => return self.act_on(cx, true),
            SettingsKey::Up => Move::Step(-1),
            SettingsKey::Down => Move::Step(1),
            SettingsKey::PageUp => Move::Page { down: false },
            SettingsKey::PageDown => Move::Page { down: true },
            SettingsKey::Top => Move::Top,
            SettingsKey::Bottom => Move::Bottom,
        };
        self.move_to(to, cx);
        Outcome::Consumed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.draw(area, buf, cx, theme);
    }

    fn render_under_step(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.draw_under_step(area, buf, cx, theme);
    }

    /// The standard settings table's footer.
    fn hints(&self) -> Footer {
        Footer::leaving_last(SETTINGS_KEYS)
    }
}

#[cfg(test)]
#[path = "settings_panel_tests.rs"]
mod tests;
