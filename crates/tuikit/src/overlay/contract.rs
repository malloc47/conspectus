//! The one trait every overlay implements.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use super::Outcome;
use crate::keys::Footer;
use crate::theme::Theme;

/// The contract every overlay keeps: it handles a key and it draws
/// itself, both against the host's context.
///
/// `M` is the host's message type, which a commit carries; `Cx` is the
/// host's context, borrowed for each call. An overlay that never
/// commits, or that needs no host data, implements the trait for every
/// `M` or every `Cx`, so any host can stack it.
///
/// - **One key.** [`handle`](Self::handle) gets every key press while the
///   overlay is on top of its stack; keys never reach what is under it.
///   It resolves the key through the overlay's key table and returns an
///   [`Outcome`]; it performs no effect.
/// - **One draw.** [`render`](Self::render) draws the overlay over the
///   screen's `area`. It clears what it covers (a framed modal draws
///   through [`ModalFrame`](crate::frame::ModalFrame), which clears its
///   rect), so the overlays below show only around it. It takes `&mut
///   self` so a scrolled overlay keeps the window it drew, and the next
///   key moves by the rows that were on screen.
/// - **Under a step.** While a step of the overlay's own is open over it,
///   the stack draws it with [`render_under_step`](Self::render_under_step)
///   instead, so a list whose keys went to that step shows its cursor as
///   a list without focus does.
/// - **Full screen.** An overlay that covers the whole screen (a reader
///   with its own header and footer) says so with
///   [`fills_screen`](Self::fills_screen); its stack draws nothing under
///   it.
/// - **Text.** An overlay whose text field has the keys says so with
///   [`takes_text`](Self::takes_text), so a host types a paste into it
///   and nowhere else.
/// - **Hints.** An overlay names the keys it takes now with
///   [`hints`](Self::hints), the footer of the table it resolves them
///   with, so a status bar shows them while it is on top instead of keys
///   that no longer reach the screen under it.
pub trait Overlay<M, Cx: ?Sized> {
    /// Handles one key press with the host's context, and says what the
    /// host should do next.
    fn handle(&mut self, key: KeyEvent, cx: &Cx) -> Outcome<M>;

    /// Draws the overlay over `area` with the host's context and theme.
    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme);

    /// Draws the overlay while a step of its own is open over it (an
    /// editor a settings panel opened): a level of the overlay has the
    /// keys, so a list in it draws its cursor row with the inactive
    /// selection pair (house rule 10). The stack draws such an entry with
    /// this instead of [`render`](Self::render); by default it is
    /// `render`, so an overlay that does not override it looks the same
    /// under its steps.
    fn render_under_step(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        self.render(area, buf, cx, theme);
    }

    /// Whether the overlay covers the whole screen, so nothing under it
    /// needs drawing. `false` unless an overlay says otherwise.
    fn fills_screen(&self) -> bool {
        false
    }

    /// Whether a text field in the overlay has the keys now (a prompt, a
    /// form's text row, a query, a find prompt), so text pasted into the
    /// overlay goes there (house rule 15). `false` unless an overlay says
    /// otherwise, so a paste never acts as the keys of a menu, a list, or
    /// a choice.
    fn takes_text(&self) -> bool {
        false
    }

    /// The keys the overlay takes now, as the footer of the table it
    /// resolves them with (its footer at its current level), for a
    /// [`StatusBar`](crate::status_bar::StatusBar) to show while it is on
    /// top. Empty unless an overlay says otherwise.
    fn hints(&self) -> Footer {
        Footer::default()
    }
}

/// A boxed overlay is an overlay, so a stack of boxed trait objects
/// works as well as a host's own enum of overlays.
impl<M, Cx: ?Sized, T: Overlay<M, Cx> + ?Sized> Overlay<M, Cx> for Box<T> {
    fn handle(&mut self, key: KeyEvent, cx: &Cx) -> Outcome<M> {
        (**self).handle(key, cx)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        (**self).render(area, buf, cx, theme);
    }

    fn render_under_step(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme) {
        (**self).render_under_step(area, buf, cx, theme);
    }

    fn fills_screen(&self) -> bool {
        (**self).fills_screen()
    }

    fn takes_text(&self) -> bool {
        (**self).takes_text()
    }

    fn hints(&self) -> Footer {
        (**self).hints()
    }
}
