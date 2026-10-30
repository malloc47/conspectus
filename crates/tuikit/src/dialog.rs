//! Dialogs: a question with something to say, a choice between named
//! verbs, or a notice, in a small modal.
//!
//! This is a layer 1 control (house rules 3, 4, 5, 8, 13, and 14). A
//! program asks before an action that cannot be undone, offers a choice
//! of how to do one, and tells the user something that needs no answer;
//! each such dialog should take the same keys, look the same, and leave
//! the same way, or one confirms on `y` and the next only on `Enter`, one
//! loses its keys on a short screen and the next keeps them:
//!
//! - **Three forms, one control.** A [`Dialog<M>`](Dialog) is a title,
//!   body lines, an optional [`Warning`] list (what the action will
//!   affect), and its [`Kind`]: a confirmation that commits the host's
//!   message, a choice whose [`Verb`]s each commit one, or a notice that
//!   commits nothing.
//! - **One key table.** Every dialog resolves [`DIALOG_KEYS`]: `Enter`
//!   and `y` confirm (and dismiss a notice), `Esc` and `n` cancel,
//!   `Ctrl-C` and `q` dismiss, and the arrows and `k`/`j` scroll a body
//!   taller than the dialog. A choice's verbs are its own keys, resolved
//!   after the table; a choice has no default verb, so `Enter` and `y` do
//!   nothing in one.
//! - **One look.** The menu-sized modal frame ([`DIALOG_SIZE`]) with the
//!   title on its top border; inside, a blank row, the body wrapped, the
//!   warning list under a heading in the theme's `warning`, a blank row,
//!   and the footer, generated from the table (and a choice's verbs) in
//!   the one grammar. On a short screen the blank rows go first, then the
//!   body scrolls under the footer, which keeps the bottom rows, with the
//!   shared scrollbar beside it.
//! - **Silent cancel.** `Esc` and `n` close the dialog (`Close`), which as
//!   a step of another overlay gives that overlay its keys back; `Ctrl-C`
//!   and `q` dismiss it (`Dismiss`), with the overlay it is a step of.
//!   Neither reports anything.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::dialog::{Dialog, Verb};
//! use tuikit::keys::ch;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Delete,
//!     Archive,
//! }
//!
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // A confirmation: `y` or Enter goes ahead, `n` or Esc does not.
//! let mut delete = Dialog::confirm("Delete", Msg::Delete)
//!     .body(["Delete plan.txt?"])
//!     .warning("open in 2 windows:", ["left", "right"]);
//! assert_eq!(delete.handle(press(KeyCode::Char('y')), &()), Outcome::Commit(Msg::Delete));
//! assert_eq!(delete.handle(press(KeyCode::Char('n')), &()), Outcome::Close);
//!
//! // A choice: its verbs are its keys.
//! let mut close = Dialog::choice(
//!     "Close plan.txt",
//!     vec![
//!         Verb::new(ch('a'), "archive", Msg::Archive),
//!         Verb::new(ch('d'), "delete", Msg::Delete),
//!     ],
//! )
//! .body(["Archive it or delete it?"]);
//! assert_eq!(close.handle(press(KeyCode::Char('a')), &()), Outcome::Commit(Msg::Archive));
//! assert_eq!(close.handle(press(KeyCode::Enter), &()), Outcome::Consumed, "no default verb");
//! assert_eq!(close.footer().text(), "a archive · d delete · Esc/n cancel");
//!
//! // A notice: any of the dismiss keys closes it.
//! let mut notice = Dialog::<Msg>::notice("Nothing to do").body(["The list is empty."]);
//! assert_eq!(notice.handle(press(KeyCode::Enter), &()), Outcome::Close);
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
//! delete.draw(buf.area, &mut buf, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("▲ open in 2 windows:"), "{drawn}");
//! assert!(drawn.contains("Enter/y confirm · Esc/n cancel"), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! Every word (the title, the body, the warning heading and its items,
//! each verb), which form a question takes, and the message each answer
//! commits: the host builds the dialog from what it knows when it asks.
//! A dialog that a menu's entry asks opens as a step of the menu
//! ([`ModalStack::push_step`](crate::overlay::ModalStack::push_step)), so
//! `Esc` returns to the menu and a commit or a dismissal closes both.

mod body;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::frame::{ModalFrame, ModalSize};
use crate::keys::{
    DIALOG_KEYS, DialogKey, Footer, Hint, KeyMatcher, KeyRow, Segment, key_label, resolve, rows,
};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::ScrollWindow;
use crate::theme::Theme;

use body::{Body, View};

/// How large every dialog is: the menu-sized modal frame, as tall as its
/// rows. A dialog a menu asks is as wide as the menu, so it covers the
/// menu's rows it overlaps, and the menu shows above and below it only
/// where the menu is taller.
pub const DIALOG_SIZE: ModalSize = ModalSize::MENU;

/// The verb a notice's footer gives the keys that dismiss it: leaving a
/// notice loses nothing.
const NOTICE_VERB: &str = "close";

/// What an action will affect, listed under a heading in the theme's
/// `warning`: the heading after the warning mark, then one item per row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Warning {
    /// What the items are, as a sentence ending in a colon
    /// (`holds 2 open files:`).
    pub heading: String,
    /// The things affected, one per row, wrapped under their dash.
    pub items: Vec<String>,
}

/// One of a choice dialog's answers: the key that takes it, the verb its
/// footer segment names, and the message it commits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verb<M> {
    /// The key that takes the verb. It follows the overlay rule for
    /// characters (`ch('m')` fires on `m` typed plain or with Shift); a
    /// key [`DIALOG_KEYS`] binds never reaches it
    /// ([`Dialog::shadowed_verbs`] finds one).
    pub key: KeyMatcher,
    /// What taking it does, in the footer's grammar: a short imperative
    /// with no "to" (`merge & close`).
    pub verb: String,
    /// What taking it reports: the host's message.
    pub outcome: M,
}

impl<M> Verb<M> {
    /// The verb `verb` on `key`, committing `outcome`.
    pub fn new(key: KeyMatcher, verb: impl Into<String>, outcome: M) -> Self {
        Self {
            key,
            verb: verb.into(),
            outcome,
        }
    }
}

/// The form of a dialog: what its answers are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind<M> {
    /// A yes-or-no question: `Enter` or `y` commits the message, `Esc` or
    /// `n` closes without it.
    Confirm(M),
    /// A choice between named verbs, each on its own key; `Esc` or `n`
    /// closes without choosing. There is no default verb.
    Choice(Vec<Verb<M>>),
    /// Something to read: `Enter`, `y`, `Esc`, and `n` all close it, and
    /// it commits nothing.
    Notice,
}

/// A small modal that asks a question, offers a choice, or tells the user
/// something: a title, body lines, an optional warning list, and the keys
/// its [`Kind`] takes.
///
/// The dialog implements [`Overlay<M, Cx>`] for every `Cx` and reads
/// nothing from the context. Drawing keeps the scroll window it showed,
/// so a body taller than the screen scrolls from where it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialog<M> {
    title: String,
    body: Vec<String>,
    warning: Option<Warning>,
    kind: Kind<M>,
    window: ScrollWindow,
}

impl<M> Dialog<M> {
    /// A yes-or-no question titled `title` that commits `outcome` when
    /// the user goes ahead.
    pub fn confirm(title: impl Into<String>, outcome: M) -> Self {
        Self::of(title, Kind::Confirm(outcome))
    }

    /// A choice titled `title` between `verbs`, in their order.
    pub fn choice(title: impl Into<String>, verbs: Vec<Verb<M>>) -> Self {
        Self::of(title, Kind::Choice(verbs))
    }

    /// A notice titled `title`, which commits nothing.
    pub fn notice(title: impl Into<String>) -> Self {
        Self::of(title, Kind::Notice)
    }

    fn of(title: impl Into<String>, kind: Kind<M>) -> Self {
        Self {
            title: title.into(),
            body: Vec::new(),
            warning: None,
            kind,
            window: ScrollWindow::default(),
        }
    }

    /// Adds `lines` to the body, each wrapped to the dialog's width; an
    /// empty line is a blank row.
    #[must_use]
    pub fn body<I>(mut self, lines: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.body.extend(lines.into_iter().map(Into::into));
        self
    }

    /// Sets the warning list: `heading` after the warning mark, then
    /// `items`, one per row. An empty list draws nothing.
    #[must_use]
    pub fn warning<I>(mut self, heading: impl Into<String>, items: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        let items: Vec<String> = items.into_iter().map(Into::into).collect();
        self.warning = (!items.is_empty()).then(|| Warning {
            heading: heading.into(),
            items,
        });
        self
    }

    /// The title on the dialog's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The body's lines, as given.
    pub fn text(&self) -> &[String] {
        &self.body
    }

    /// The warning list, if the dialog has one.
    pub fn warning_list(&self) -> Option<&Warning> {
        self.warning.as_ref()
    }

    /// The dialog's form and its answers.
    pub fn kind(&self) -> &Kind<M> {
        &self.kind
    }

    /// The scroll window of the last draw: which rows of the body were on
    /// screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// The footer the dialog draws: `Enter/y confirm · Esc/n cancel` for
    /// a confirmation, each verb's key and verb and then `Esc/n cancel`
    /// for a choice, and `Enter/Esc close` for a notice, all from
    /// [`DIALOG_KEYS`] (and the verbs).
    pub fn footer(&self) -> Footer {
        match &self.kind {
            Kind::Confirm(_) => Footer::new(DIALOG_KEYS),
            Kind::Choice(verbs) => {
                let cancel = Footer::only(DIALOG_KEYS, |action| *action == DialogKey::Cancel);
                verbs
                    .iter()
                    .map(|verb| Segment {
                        keys: vec![verb.key],
                        ..Segment::note(format!("{} {}", key_label(&verb.key), verb.verb))
                    })
                    .chain(cancel.segments().iter().cloned())
                    .collect()
            }
            Kind::Notice => {
                let (labels, keys): (Vec<&str>, Vec<KeyMatcher>) =
                    [DialogKey::Confirm, DialogKey::Cancel]
                        .into_iter()
                        .filter_map(first_row)
                        .unzip();
                std::iter::once(Segment {
                    keys,
                    ..Segment::note(format!("{} {NOTICE_VERB}", labels.join("/")))
                })
                .collect()
            }
        }
    }

    /// The verbs no key press can reach, as their key labels: each one a
    /// key [`DIALOG_KEYS`] binds (`y`, `n`, `q`, `j`, `Enter`), or a key
    /// an earlier verb already uses. Empty when every verb works; a host
    /// checks the dialogs it builds with it.
    pub fn shadowed_verbs(&self) -> Vec<String> {
        let Kind::Choice(verbs) = &self.kind else {
            return Vec::new();
        };
        let mut shadowed = Vec::new();
        for (index, verb) in verbs.iter().enumerate() {
            let press = verb.key.example();
            let taken = resolve(DIALOG_KEYS, press).is_some()
                || verbs[..index]
                    .iter()
                    .any(|earlier| earlier.key.matches_event(press));
            if taken {
                shadowed.push(key_label(&verb.key));
            }
        }
        shadowed
    }

    /// Draws the dialog over `area`: the frame, the body, the warning
    /// list, and the footer, and keeps the window it showed. A host whose
    /// overlay holds the dialog as one of its levels draws it with this;
    /// on a stack, [`Overlay::render`] does.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let footer = self.footer();
        let frame = ModalFrame::new(theme, DIALOG_SIZE).title(self.title.as_str());
        let body = Body {
            view: View {
                body: &self.body,
                warning: self.warning.as_ref(),
                footer: &footer,
                theme,
            },
            window: self.window,
            lines: None,
            footer: Vec::new(),
        };
        self.window = frame.render(area, buf, body);
    }
}

/// The label and the key of the first row of [`DIALOG_KEYS`] that shows
/// `action`.
fn first_row(action: DialogKey) -> Option<(&'static str, KeyMatcher)> {
    rows(DIALOG_KEYS).find_map(|row| match row {
        KeyRow::Bind {
            key,
            action: bound,
            hint: Hint::Shown { label, .. },
            ..
        } if *bound == action => Some((*label, *key)),
        _ => None,
    })
}

impl<M: Clone, Cx: ?Sized> Overlay<M, Cx> for Dialog<M> {
    /// `Enter` and `y` commit a confirmation and close a notice; a verb's
    /// key commits it; `Esc` and `n` close the dialog, and `Ctrl-C` and
    /// `q` dismiss it, saying nothing; the arrows and `k`/`j` scroll a
    /// body taller than the dialog. Every other key is swallowed.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        match resolve(DIALOG_KEYS, key) {
            Some(DialogKey::Abort) => Outcome::Dismiss,
            Some(DialogKey::Cancel) => Outcome::Close,
            Some(DialogKey::Up) => {
                self.window.scroll_by(-1);
                Outcome::Consumed
            }
            Some(DialogKey::Down) => {
                self.window.scroll_by(1);
                Outcome::Consumed
            }
            Some(DialogKey::Confirm) => match &self.kind {
                Kind::Confirm(outcome) => Outcome::Commit(outcome.clone()),
                Kind::Notice => Outcome::Close,
                // A choice has no default verb: its own keys choose.
                Kind::Choice(_) => Outcome::Consumed,
            },
            None => match &self.kind {
                Kind::Choice(verbs) => verbs
                    .iter()
                    .find(|verb| verb.key.matches_event(key))
                    .map_or(Outcome::Consumed, |verb| {
                        Outcome::Commit(verb.outcome.clone())
                    }),
                Kind::Confirm(_) | Kind::Notice => Outcome::Consumed,
            },
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    /// The footer of the dialog's form ([`Dialog::footer`]).
    fn hints(&self) -> Footer {
        self.footer()
    }
}

#[cfg(test)]
#[path = "dialog_tests.rs"]
mod tests;
