//! The conformance suite: one call that holds every modal a host has to
//! the framework's conventions.
//!
//! A host's modals are many, and each was written at a different time;
//! the conventions they share (house rules 1, 4, 5, 8, and 14, and the
//! modal key convention) are easy to state and easy to drift from. The
//! host lists its modals once in a [`Registry`], and
//! [`assert_conventions`] checks each of them the same way:
//!
//! | [`Check`] | What it holds |
//! | --- | --- |
//! | `Drawn` | The screen with the modal open differs from the screen without it, at 100 by 30 and 60 by 20. |
//! | `Contained` | At every one of five widths ([`WIDTHS`]) by two heights ([`HEIGHTS`]) the modal draws a frame, nothing it draws escapes that frame, and no text runs into the frame's sides. |
//! | `Title` | The frame's corners and title are the theme's `accent`, the title has one space on each side, and outside the chips it carries (cells in the theme's `badge` look) it is regular weight. |
//! | `Selection` | Moving the selection (down four times, up once) neither moves nor resizes the frame, and moves no text above the last row it repaints, outside the two-cell cursor gutter. |
//! | `Words` | No row inside the frame, at any of the ten sizes, ends at the frame's edge in the start of a longer word the same modal shows whole on a wide screen; a deliberate cut ends in the ellipsis mark. |
//! | `Keys` | Each level's key tables keep the modal key convention by role (`Esc` leaves, `Enter` applies where the [`Convention`] says, `Space` changes), bind `Ctrl-C` first to dismissing, and pass the table checks (every footer label is its key's label, every key a footer names resolves to its row, characters bind as typed, `Ctrl-C` comes first). |
//! | `Footer` | Every segment of the footer the modal's table generates appears in the modal, so the footer is the generated one and not hand-written. |
//! | `Close` | `Ctrl-C` closes the modal, `Esc` closes it at its first level, and `q` does too unless the level types (house rule 4); closing says nothing (house rule 5). |
//!
//! The host's side is a [`Rig`]: how to draw its whole screen, how to
//! close the modal on top, how to press a key the way its loop does, how
//! many modals are open, and what feedback it is showing. Each [`Case`]
//! opens one modal (a fresh host state with that modal on top) and says
//! what the checks need to know: whether the modal fills the screen,
//! whether its first level types, how its selection moves, its key tables
//! per level, and the table its footer comes from. Domain checks (what a
//! menu offers, which modal a gesture opens) stay in the host; this suite
//! is only what every modal of every host should do.
//!
//! [`check_conventions`] returns the [`Failure`]s instead of panicking,
//! and [`check_conventions_of`] runs some checks only, so a host can run
//! the checks as separate tests.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent};
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use ratatui::widgets::{Paragraph, Widget};
//! use tuikit::frame::{Lines, ModalFrame, ModalSize};
//! use tuikit::keys::{self, Convention, EnterMeans, MENU_KEYS, MenuKey, SpaceMeans};
//! use tuikit::overlay::{ModalStack, Outcome, Overlay};
//! use tuikit::testkit::conformance::{self, Case, Registry, Rig};
//! use tuikit::theme::Theme;
//!
//! /// A one-entry menu built on the standard menu keys.
//! struct Menu;
//!
//! impl Overlay<(), ()> for Menu {
//!     fn handle(&mut self, key: KeyEvent, _: &()) -> Outcome<()> {
//!         match keys::resolve(MENU_KEYS, key) {
//!             Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
//!             Some(MenuKey::Pick) => Outcome::Commit(()),
//!             _ => Outcome::Consumed,
//!         }
//!     }
//!
//!     fn render(&mut self, area: Rect, buf: &mut Buffer, _: &(), theme: &Theme) {
//!         let lines = vec![
//!             Line::from("  Open"),
//!             Line::from(""),
//!             Line::from(keys::footer(MENU_KEYS)),
//!         ];
//!         let frame = ModalFrame::new(theme, ModalSize::MENU).title("Actions");
//!         frame.render(area, buf, Lines::new(lines));
//!     }
//! }
//!
//! /// The host: a screen of text and a stack of modals.
//! struct Host(Theme);
//!
//! impl Rig for Host {
//!     type State = ModalStack<Menu>;
//!
//!     fn render(&self, stack: &mut Self::State, area: Rect) -> Buffer {
//!         let mut buf = Buffer::empty(area);
//!         Paragraph::new("the screen under the modal").render(area, &mut buf);
//!         stack.draw::<(), ()>(area, &mut buf, &(), &self.0);
//!         buf
//!     }
//!
//!     fn close_top(&self, stack: &mut Self::State) {
//!         stack.pop();
//!     }
//!
//!     fn press(&self, stack: &mut Self::State, key: KeyEvent) {
//!         if let Some(outcome) = stack.route::<(), ()>(key, &()) {
//!             let _ = stack.settle(outcome);
//!         }
//!     }
//!
//!     fn depth(&self, stack: &Self::State) -> usize {
//!         stack.len()
//!     }
//!
//!     fn theme(&self) -> &Theme {
//!         &self.0
//!     }
//! }
//!
//! let open = || {
//!     let mut stack = ModalStack::new();
//!     stack.push(Menu);
//!     stack
//! };
//! let convention = Convention::new("Menu", EnterMeans::Activates, SpaceMeans::Types, "");
//! let registry = Registry::new(Host(Theme::default())).case(
//!     Case::new("Menu", open)
//!         .keys(convention, [conformance::table("MENU_KEYS", MENU_KEYS)])
//!         .footer(MENU_KEYS),
//! );
//! conformance::assert_conventions(&registry);
//! ```

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::keys::{
    Convention, Footer, KeyAction, KeyRole, KeyRow, ctrl_c_first, loose_characters, mislabeled,
    resolve, unresolved,
};
use crate::theme::Theme;

mod checks;

pub use checks::same_chrome;

/// The screen widths the `Contained` and `Words` checks draw at.
pub const WIDTHS: [u16; 5] = [60, 80, 120, 160, 220];
/// The screen heights the `Contained` and `Words` checks draw at.
pub const HEIGHTS: [u16; 2] = [20, 40];

/// The host's side of the suite: how to draw and drive its screen.
pub trait Rig {
    /// The host's state: its model, with whatever modals are open.
    type State;

    /// Draws the whole screen, modals included, into a buffer of `area`,
    /// as the host's loop draws a frame.
    fn render(&self, state: &mut Self::State, area: Rect) -> Buffer;

    /// Closes the modal on top, leaving everything else as it was, so the
    /// checks can compare the screen with the modal and without it.
    fn close_top(&self, state: &mut Self::State);

    /// Gives `key` to the modal on top as the host's loop does: route it,
    /// settle what the modal asked for, and handle what it sent.
    fn press(&self, state: &mut Self::State, key: KeyEvent);

    /// How many modals are open.
    fn depth(&self, state: &Self::State) -> usize;

    /// What the host shows as feedback right now (a status line, toasts),
    /// so the `Close` check can tell that closing said nothing. Empty by
    /// default, for a host that shows none.
    fn feedback(&self, state: &Self::State) -> Vec<String> {
        let _ = state;
        Vec::new()
    }

    /// Puts `text` on the host's status line, as a modal's opening hint
    /// would be, so the `Close` check can see closing clear it. Does
    /// nothing by default.
    fn note(&self, state: &mut Self::State, text: &str) {
        let _ = (state, text);
    }

    /// The theme the host draws with.
    fn theme(&self) -> &Theme;
}

/// The host's modals and the rig that draws them.
pub struct Registry<R: Rig> {
    rig: R,
    cases: Vec<Case<R::State>>,
}

impl<R: Rig> Registry<R> {
    /// An empty registry over `rig`.
    pub fn new(rig: R) -> Self {
        Self {
            rig,
            cases: Vec::new(),
        }
    }

    /// The registry with `case` added.
    #[must_use]
    pub fn case(mut self, case: Case<R::State>) -> Self {
        self.cases.push(case);
        self
    }

    /// Adds `case`.
    pub fn push(&mut self, case: Case<R::State>) {
        self.cases.push(case);
    }

    /// The rig.
    pub fn rig(&self) -> &R {
        &self.rig
    }

    /// The cases, in the order they were added.
    pub fn cases(&self) -> &[Case<R::State>] {
        &self.cases
    }

    /// The name of every case, once each, in the order first added: what
    /// a host compares with its own list of modals so none goes unchecked.
    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        for case in &self.cases {
            if !names.contains(&case.name) {
                names.push(case.name);
            }
        }
        names
    }
}

/// How a modal sits on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// In a bordered frame over the screen.
    Framed,
    /// Over the whole screen, with no frame of its own to check.
    FullScreen,
}

/// How a modal's selection is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Checked when the arrow keys repaint something.
    MayMove,
    /// Checked, and the arrow keys must move it.
    Moves,
    /// Not checked, for the reason given (the body scrolls as a whole, or
    /// the arrow keys change the host's state under the modal).
    Unchecked(&'static str),
}

/// One modal, as a fresh host state with it on top, and what the checks
/// need to know about it.
pub struct Case<S> {
    name: &'static str,
    variant: &'static str,
    open: Box<dyn Fn() -> S>,
    look: Look,
    typed: bool,
    selection: Selection,
    levels: Vec<KeyLevel>,
    footer: Option<Footer>,
}

impl<S> Case<S> {
    /// The modal `name`, opened by `open`: a framed modal whose first
    /// level does not type, whose selection is checked if it moves, with
    /// no key tables and no footer until they are given.
    pub fn new(name: &'static str, open: impl Fn() -> S + 'static) -> Self {
        Self {
            name,
            variant: "",
            open: Box::new(open),
            look: Look::Framed,
            typed: false,
            selection: Selection::MayMove,
            levels: Vec::new(),
            footer: None,
        }
    }

    /// Names which state of the modal this case opens, when a modal has
    /// several cases (a search in each of its modes).
    #[must_use]
    pub fn variant(mut self, variant: &'static str) -> Self {
        self.variant = variant;
        self
    }

    /// The modal fills the screen: the frame, title, selection, and word
    /// checks do not apply.
    #[must_use]
    pub fn full_screen(mut self) -> Self {
        self.look = Look::FullScreen;
        self
    }

    /// The modal's first level types (a prompt, a form, a query), so `q`
    /// is a letter there rather than a way out.
    #[must_use]
    pub fn typed(mut self) -> Self {
        self.typed = true;
        self
    }

    /// The arrow keys must move the modal's selection, so a check that
    /// compared nothing fails.
    #[must_use]
    pub fn moves(mut self) -> Self {
        self.selection = Selection::Moves;
        self
    }

    /// The selection check does not apply, for `reason`.
    #[must_use]
    pub fn unchecked_selection(mut self, reason: &'static str) -> Self {
        self.selection = Selection::Unchecked(reason);
        self
    }

    /// One level of the modal's keys: its row of the modal key convention
    /// and the tables the level resolves keys with, in the order it
    /// consults them. A modal with several levels (a menu that opens a
    /// prompt) gives one call per level, or one call with every table.
    #[must_use]
    pub fn keys(
        mut self,
        convention: Convention,
        tables: impl IntoIterator<Item = KeyTable>,
    ) -> Self {
        self.levels.push(KeyLevel {
            convention,
            tables: tables.into_iter().collect(),
        });
        self
    }

    /// The modal draws the footer `table` generates.
    #[must_use]
    pub fn footer<A>(mut self, table: &[KeyRow<A>]) -> Self {
        self.footer = Some(Footer::new(table));
        self
    }

    /// The modal's name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The modal's name with the variant, as failures name the case.
    pub fn label(&self) -> String {
        if self.variant.is_empty() {
            self.name.to_string()
        } else {
            format!("{} ({})", self.name, self.variant)
        }
    }

    /// A fresh host state with the modal on top.
    pub fn open(&self) -> S {
        (self.open)()
    }

    /// How the modal sits on the screen.
    pub fn look(&self) -> Look {
        self.look
    }

    /// The modal's key levels, by their convention rows.
    pub fn conventions(&self) -> Vec<Convention> {
        self.levels.iter().map(|level| level.convention).collect()
    }

    /// A press of every key the modal's tables bind, once each, level by
    /// level and table by table.
    pub fn bound_keys(&self) -> Vec<KeyEvent> {
        let mut keys: Vec<KeyEvent> = Vec::new();
        for table in self.levels.iter().flat_map(|level| &level.tables) {
            for key in table.keys() {
                if !keys.contains(key) {
                    keys.push(*key);
                }
            }
        }
        keys
    }
}

/// One level of a modal's keys.
struct KeyLevel {
    convention: Convention,
    tables: Vec<KeyTable>,
}

/// A key table as the `Keys` check reads it: the role it gives a key, and
/// what the table checks found wrong with it.
pub struct KeyTable {
    name: &'static str,
    role: Box<dyn Fn(KeyEvent) -> Option<KeyRole>>,
    problems: Vec<String>,
    keys: Vec<KeyEvent>,
}

impl KeyTable {
    /// The table's name, as failures give it.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// What the table checks found wrong: empty for a sound table.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// A press of every key the table binds (included tables spliced in),
    /// once each, in table order: what a driver presses to exercise every
    /// key.
    pub fn keys(&self) -> &[KeyEvent] {
        &self.keys
    }
}

/// The key table `rows`, named `name` (its constant's name), for a case's
/// [`Case::keys`].
pub fn table<A: KeyAction + PartialEq + 'static>(
    name: &'static str,
    rows: &'static [KeyRow<A>],
) -> KeyTable {
    let mut problems = Vec::new();
    for (label, key) in mislabeled(rows) {
        problems.push(format!("labels `{key}` as `{label}`"));
    }
    for (label, key) in unresolved(rows) {
        problems.push(format!(
            "its footer names `{label}`, but an earlier row takes `{key}`"
        ));
    }
    for key in loose_characters(rows) {
        problems.push(format!("binds the character `{key}` with its chords too"));
    }
    if !ctrl_c_first(rows) {
        problems.push("does not start with Ctrl-C bound to dismissing".to_string());
    }
    let mut keys: Vec<KeyEvent> = Vec::new();
    for row in crate::keys::rows(rows) {
        if let KeyRow::Bind { key, .. } = row
            && !keys.contains(&key.example())
        {
            keys.push(key.example());
        }
    }
    KeyTable {
        name,
        role: Box::new(move |event| resolve(rows, event).map(KeyAction::role)),
        problems,
        keys,
    }
}

/// One check of the suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    /// The modal is drawn.
    Drawn,
    /// Nothing it draws escapes its frame, and no text runs into the
    /// frame's sides.
    Contained,
    /// Its frame and title take the one look.
    Title,
    /// Moving its selection moves no text.
    Selection,
    /// No word is cut at its edge.
    Words,
    /// Its keys keep the convention and pass the table checks.
    Keys,
    /// Its footer is the one its table generates.
    Footer,
    /// It closes by the one rule, silently.
    Close,
}

impl Check {
    /// Every check, in the order the suite runs them.
    pub const ALL: [Check; 8] = [
        Check::Drawn,
        Check::Contained,
        Check::Title,
        Check::Selection,
        Check::Words,
        Check::Keys,
        Check::Footer,
        Check::Close,
    ];
}

/// One way a modal broke a convention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The case, by [`Case::label`].
    pub case: String,
    /// The check it failed.
    pub check: Check,
    /// What went wrong, with the size or key where it did.
    pub message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{:?}]: {}", self.case, self.check, self.message)
    }
}

/// Every way the registry's modals break the conventions.
pub fn check_conventions<R: Rig>(registry: &Registry<R>) -> Vec<Failure> {
    check_conventions_of(registry, &Check::ALL)
}

/// The failures of the `checks` given only.
pub fn check_conventions_of<R: Rig>(registry: &Registry<R>, checks: &[Check]) -> Vec<Failure> {
    let mut failures = Vec::new();
    let mut tables_seen: Vec<&'static str> = Vec::new();
    for case in &registry.cases {
        for check in checks {
            let found = match check {
                Check::Drawn => checks::drawn(&registry.rig, case),
                Check::Contained => checks::contained(&registry.rig, case),
                Check::Title => checks::title(&registry.rig, case),
                Check::Selection => checks::selection(&registry.rig, case),
                Check::Words => checks::words(&registry.rig, case),
                Check::Keys => checks::keys(case, &mut tables_seen),
                Check::Footer => checks::footer(&registry.rig, case),
                Check::Close => checks::close(&registry.rig, case),
            };
            failures.extend(found.into_iter().map(|message| Failure {
                case: case.label(),
                check: *check,
                message,
            }));
        }
    }
    failures
}

/// Fails the test, listing every failure, when the registry's modals break
/// the conventions.
///
/// # Panics
///
/// When [`check_conventions`] finds anything.
pub fn assert_conventions<R: Rig>(registry: &Registry<R>) {
    assert_conventions_of(registry, &Check::ALL);
}

/// [`assert_conventions`] for the `checks` given only.
///
/// # Panics
///
/// When [`check_conventions_of`] finds anything.
pub fn assert_conventions_of<R: Rig>(registry: &Registry<R>, checks: &[Check]) {
    let failures = check_conventions_of(registry, checks);
    if !failures.is_empty() {
        let report: Vec<String> = failures.iter().map(Failure::to_string).collect();
        panic!(
            "{} modal convention failure(s):\n{}",
            failures.len(),
            report.join("\n")
        );
    }
}

#[cfg(test)]
#[path = "conformance_tests.rs"]
mod tests;
