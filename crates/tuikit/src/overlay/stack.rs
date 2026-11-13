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
/// An overlay can be several entries: a menu whose pick asks a further
/// question opens the question as a *step* of the menu
/// ([`push_step`](Self::push_step)). The steps and the entry they start
/// from are one flow. `Esc` in a step is a plain [`Outcome::Close`],
/// which closes the step and gives the keys back to the entry under it;
/// an [`Outcome::Commit`] or an [`Outcome::Dismiss`] (`Ctrl-C`) closes
/// the whole flow, because the overlay is done or dismissed, not one
/// level of it. An [`Outcome::Back`] closes the step as `Esc` does and
/// hands the host a message: a step that applies its answer and returns
/// to the entry under it. An entry with a step of its own open over it
/// is drawn with [`Overlay::render_under_step`], since its keys went to
/// that level.
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
/// assert_eq!(stack.settle(outcome).closed.len(), 1);
/// assert_eq!(stack.len(), 1, "only the top closed");
/// ```
#[derive(Clone, Debug)]
pub struct ModalStack<E> {
    entries: Vec<E>,
    /// Beside each entry: whether it was opened as a step of the entry
    /// under it.
    steps: Vec<bool>,
}

/// What settling an [`Outcome`] did: the entries it closed, if any, and
/// the message the host handles next, if any.
///
/// The closed entries are handed back so the host can do its own
/// bookkeeping as an overlay goes (remember how it was left); dropping
/// them is fine.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct Settled<E, M> {
    /// The entries the outcome closed, top first: the top one for
    /// `Close` and `Back`, and for `Commit` and `Dismiss` the top one
    /// with every entry it is a step of. Empty for `Consumed` and `Stay`.
    pub closed: Vec<E>,
    /// The message the host handles: from `Commit` or `Back` (after the
    /// entries closed) or `Stay`.
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
            steps: Vec::new(),
        }
    }

    /// Opens `entry` on top of the others as an overlay of its own; it
    /// takes the keys from now on.
    pub fn push(&mut self, entry: E) {
        self.entries.push(entry);
        self.steps.push(false);
    }

    /// Opens `entry` on top as the next step of the overlay on top: the
    /// question a menu's pick asks, or a prompt for a value it needs. A
    /// `Close` from the step gives the keys back to the entry under it; a
    /// `Commit` or a `Dismiss` closes the step and the flow it belongs
    /// to. On an empty stack the entry has nothing to be a step of and
    /// opens as [`push`](Self::push) opens it.
    pub fn push_step(&mut self, entry: E) {
        let step = !self.entries.is_empty();
        self.entries.push(entry);
        self.steps.push(step);
    }

    /// Closes the top entry and returns it.
    pub fn pop(&mut self) -> Option<E> {
        self.steps.pop();
        self.entries.pop()
    }

    /// Closes the top entry if `pred` says so, and returns it: how a host
    /// closes one kind of overlay without closing whatever is over it.
    pub fn pop_if(&mut self, pred: impl FnOnce(&E) -> bool) -> Option<E> {
        if self.entries.last().is_some_and(pred) {
            self.pop()
        } else {
            None
        }
    }

    /// Closes the top entry and every entry it is a step of, and returns
    /// them, top first: the whole flow the top entry belongs to. Empty
    /// when the stack is.
    pub fn close_flow(&mut self) -> Vec<E> {
        let mut closed = Vec::new();
        while let Some(entry) = self.entries.pop() {
            let step = self.steps.pop().unwrap_or(false);
            closed.push(entry);
            if !step {
                break;
            }
        }
        closed
    }

    /// Closes every entry and returns them, top first, the order they
    /// would have closed one at a time.
    pub fn close_all(&mut self) -> Vec<E> {
        self.steps.clear();
        let mut closed = std::mem::take(&mut self.entries);
        closed.reverse();
        closed
    }

    /// Whether the top entry was opened as a step of the entry under it,
    /// so a `Close` from it gives the keys back to that entry.
    pub fn top_is_step(&self) -> bool {
        self.steps.last().copied().unwrap_or(false)
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
    /// [`Outcome::Close`] and [`Outcome::Back`], and the top entry's whole
    /// flow (it and every entry it is a step of) for [`Outcome::Commit`]
    /// and [`Outcome::Dismiss`]. Returns the entries it closed and the
    /// message the host handles next.
    pub fn settle<M>(&mut self, outcome: Outcome<M>) -> Settled<E, M> {
        let closed = match outcome {
            Outcome::Close | Outcome::Back(_) => self.pop().into_iter().collect(),
            Outcome::Commit(_) | Outcome::Dismiss => self.close_flow(),
            Outcome::Consumed | Outcome::Stay(_) => Vec::new(),
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
    /// on a cleared area: nothing under it would show. An entry with a
    /// step of its own open over it draws through
    /// [`Overlay::render_under_step`].
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
            if self.steps.get(index + 1).copied().unwrap_or(false) {
                entry.render_under_step(area, buf, cx, theme);
            } else {
                entry.render(area, buf, cx, theme);
            }
        }
    }

    /// Whether the top entry has a text field taking keys now, so a paste
    /// goes into it ([`Overlay::takes_text`]). `false` when the stack is
    /// empty.
    pub fn takes_text<M, Cx: ?Sized>(&self) -> bool
    where
        E: Overlay<M, Cx>,
    {
        self.entries
            .last()
            .is_some_and(|entry| Overlay::<M, Cx>::takes_text(entry))
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
