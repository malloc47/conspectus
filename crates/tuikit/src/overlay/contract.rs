//! The one trait every overlay implements.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use super::Outcome;
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
/// - **Full screen.** An overlay that covers the whole screen (a reader
///   with its own header and footer) says so with
///   [`fills_screen`](Self::fills_screen); its stack draws nothing under
///   it.
pub trait Overlay<M, Cx: ?Sized> {
    /// Handles one key press with the host's context, and says what the
    /// host should do next.
    fn handle(&mut self, key: KeyEvent, cx: &Cx) -> Outcome<M>;

    /// Draws the overlay over `area` with the host's context and theme.
    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &Cx, theme: &Theme);

    /// Whether the overlay covers the whole screen, so nothing under it
    /// needs drawing. `false` unless an overlay says otherwise.
    fn fills_screen(&self) -> bool {
        false
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

    fn fills_screen(&self) -> bool {
        (**self).fills_screen()
    }
}
