//! The host-owned stack of open overlays.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Widget};

use super::{Outcome, Overlay};
use crate::theme::Theme;

/// The open overlays, bottom to top, as entries of a type `E` the host
/// chooses: its own enum that implements [`Overlay`] by delegating to
/// each overlay, or `Box<dyn Overlay<M, Cx>>`.
///
/// The host owns the stack and passes its own state as the context of
/// each call, so entries are borrowed mutably while the host's state is
/// borrowed shared. A key goes to the top entry ([`route`](Self::route));
/// the host settles what it returned ([`settle`](Self::settle)) and
/// handles the message. Every entry is drawn, bottom to top
/// ([`draw`](Self::draw)).
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use ratatui::layout::Rect;
/// use tuikit::overlay::{ModalStack, Outcome, Overlay};
/// use tuikit::theme::Theme;
///
/// /// A notice that any key dismisses.
/// struct Notice;
///
/// impl<M, Cx> Overlay<M, Cx> for Notice {
///     fn handle(&mut self, _key: KeyEvent, _cx: &Cx) -> Outcome<M> {
///         Outcome::Close
///     }
///
///     fn render(&mut self, _area: Rect, _buf: &mut Buffer, _cx: &Cx, _theme: &Theme) {}
/// }
///
/// let mut stack: ModalStack<Box<dyn Overlay<(), ()>>> = ModalStack::new();
/// stack.push(Box::new(Notice));
/// stack.push(Box::new(Notice));
/// let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
/// let outcome = stack.route(key, &()).expect("an overlay is open");
/// assert!(stack.settle(outcome).closed.is_some());
/// assert_eq!(stack.len(), 1, "only the top closed");
/// ```
#[derive(Clone, Debug)]
pub struct ModalStack<E> {
    entries: Vec<E>,
}

/// What settling an [`Outcome`] did: the entry it closed, if any, and the
/// message the host handles next, if any.
///
/// The closed entry is handed back so the host can do its own bookkeeping
/// as an overlay goes (remember how it was left); dropping it is fine.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct Settled<E, M> {
    /// The entry the outcome closed: the top one, for `Close` and
    /// `Commit`.
    pub closed: Option<E>,
    /// The message the host handles: from `Commit` (after the entry
    /// closed) or `Stay`.
    pub message: Option<M>,
}

impl<E> Default for ModalStack<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> ModalStack<E> {
    /// An empty stack.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Opens `entry` on top of the others; it takes the keys from now on.
    pub fn push(&mut self, entry: E) {
        self.entries.push(entry);
    }

    /// Closes the top entry and returns it.
    pub fn pop(&mut self) -> Option<E> {
        self.entries.pop()
    }

    /// Closes the top entry if `pred` says so, and returns it: how a host
    /// closes one kind of overlay without closing whatever is over it.
    pub fn pop_if(&mut self, pred: impl FnOnce(&E) -> bool) -> Option<E> {
        if self.entries.last().is_some_and(pred) {
            self.entries.pop()
        } else {
            None
        }
    }

    /// Closes every entry and returns them, top first, the order they
    /// would have closed one at a time.
    pub fn close_all(&mut self) -> Vec<E> {
        let mut closed = std::mem::take(&mut self.entries);
        closed.reverse();
        closed
    }

    /// The entry on top, which takes the keys.
    pub fn top(&self) -> Option<&E> {
        self.entries.last()
    }

    /// The entry on top, mutably.
    pub fn top_mut(&mut self) -> Option<&mut E> {
        self.entries.last_mut()
    }

    /// How many entries are open.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entry is open.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries, bottom to top.
    pub fn iter(&self) -> std::slice::Iter<'_, E> {
        self.entries.iter()
    }

    /// The entries, bottom to top, mutably.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, E> {
        self.entries.iter_mut()
    }

    /// Gives `key` to the top entry with the host's context and returns
    /// what it asked for; `None` when the stack is empty, so the key is
    /// the host's own. The stack is unchanged: [`settle`](Self::settle)
    /// does what the outcome asks of it.
    pub fn route<M, Cx: ?Sized>(&mut self, key: KeyEvent, cx: &Cx) -> Option<Outcome<M>>
    where
        E: Overlay<M, Cx>,
    {
        Some(self.entries.last_mut()?.handle(key, cx))
    }

    /// Does the stack's part of `outcome`: closes the top entry for
    /// [`Outcome::Close`] and [`Outcome::Commit`]. Returns the entry it
    /// closed and the message the host handles next.
    pub fn settle<M>(&mut self, outcome: Outcome<M>) -> Settled<E, M> {
        let closed = if outcome.closes() {
            self.entries.pop()
        } else {
            None
        };
        Settled {
            closed,
            message: outcome.message(),
        }
    }

    /// Draws every entry over `area`, bottom to top, each with the host's
    /// context and the theme. Each entry clears what it covers, so an
    /// entry shows the ones under it around its own frame. Drawing starts
    /// at the topmost entry that [fills the screen](Overlay::fills_screen),
    /// on a cleared area: nothing under it would show.
    pub fn draw<M, Cx: ?Sized>(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme)
    where
        E: Overlay<M, Cx>,
    {
        let start = self
            .entries
            .iter()
            .rposition(|entry| Overlay::<M, Cx>::fills_screen(entry))
            .unwrap_or(0);
        for (index, entry) in self.entries.iter_mut().enumerate().skip(start) {
            if index == start && Overlay::<M, Cx>::fills_screen(entry) {
                Clear.render(area, buf);
            }
            entry.render(area, buf, cx, theme);
        }
    }

    /// Whether an open entry fills the screen, so a host need not draw
    /// its own screen under the stack.
    pub fn fills_screen<M, Cx: ?Sized>(&self) -> bool
    where
        E: Overlay<M, Cx>,
    {
        self.entries
            .iter()
            .any(|entry| Overlay::<M, Cx>::fills_screen(entry))
    }
}
