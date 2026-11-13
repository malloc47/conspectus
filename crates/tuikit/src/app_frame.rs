//! The application frame: the layers a program's screen is drawn in, and
//! where its header, body, and footer go.
//!
//! This is a layer 1 chrome control. A full-screen program draws the same
//! layers in the same order on every frame, whatever it shows:
//!
//! 1. **The chrome.** A header row at the top, a footer row at the bottom
//!    (a status line, key hints), and the body between them, which holds
//!    the program's panes. [`AppFrame::areas`] says where each goes, so a
//!    tool that needs one part's cells again (a dev tool that prints one
//!    region, a check that reads one) finds them by the same rule.
//! 2. **The overlays**, drawn over the chrome from the bottom of the
//!    host's [`ModalStack`](crate::overlay::ModalStack) to its top. While
//!    an open overlay fills the screen, the chrome is not drawn at all:
//!    nothing of it would show.
//! 3. **The toasts**, drawn last, over everything, so a report shows over
//!    a full-screen overlay too.
//!
//! The host supplies what each part draws through the [`Screen`] trait,
//! usually on a small adapter over its own state, and the frame calls the
//! parts in that order with the areas it worked out. What the header and
//! footer say and which view fills the body stay the host's. The body's
//! draw returns what it measured (the windows its scrolled panes showed),
//! and [`AppFrame::render`] hands that back to the caller, which keeps it
//! for the next key and frame. The frame sends nothing and keeps nothing.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use ratatui::widgets::{Paragraph, Widget};
//! use tuikit::app_frame::{AppFrame, Screen};
//! use tuikit::frame::{PaneFrame, SplitPane};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! /// A program's state, and what its screen draws in each part.
//! struct Program {
//!     theme: Theme,
//!     left_focused: bool,
//! }
//!
//! impl Screen for Program {
//!     /// The body reports the two panes' areas it drew.
//!     type Report = Option<[Rect; 2]>;
//!
//!     fn header(&mut self, area: Rect, buf: &mut Buffer) {
//!         Paragraph::new("title").render(area, buf);
//!     }
//!
//!     fn body(&mut self, area: Rect, buf: &mut Buffer) -> Self::Report {
//!         let panes = SplitPane::new(30).areas(area);
//!         for (pane, focused) in panes.into_iter().zip([self.left_focused, !self.left_focused]) {
//!             PaneFrame::new(&self.theme).title("pane").focused(focused).render(pane, buf);
//!         }
//!         Some(panes)
//!     }
//!
//!     fn footer(&mut self, area: Rect, buf: &mut Buffer) {
//!         Paragraph::new(Line::from("q quit")).render(area, buf);
//!     }
//! }
//!
//! let mut program = Program { theme: Theme::default(), left_focused: true };
//! let area = Rect::new(0, 0, 40, 6);
//! let mut buf = Buffer::empty(area);
//! let frame = AppFrame::new();
//! let drawn = frame.render(area, &mut buf, &mut program);
//! assert_eq!(drawn, Some([Rect::new(0, 1, 20, 4), Rect::new(20, 1, 20, 4)]));
//! assert_eq!(frame.areas(area).body, Rect::new(0, 1, 40, 4));
//! let text = rows(&buf);
//! assert!(text[0].starts_with("title"));
//! assert_eq!(text[1], "┌ ► pane ──────────┐┌   pane ──────────┐");
//! assert!(text[5].starts_with("q quit"));
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};

/// What a host draws in each part of an [`AppFrame`].
///
/// The frame calls the parts in its layer order, each with the area it
/// worked out, and only [`body`](Self::body) is required. A host
/// usually implements this on a small adapter that borrows its state for
/// the frame, so the parts can each borrow it in turn.
pub trait Screen {
    /// What the body's draw measured and hands back to the caller, such as
    /// the windows its scrolled panes showed. The default value is what
    /// the frame returns when an overlay hid the body.
    type Report: Default;

    /// Draws the header row (or rows). Draws nothing unless implemented.
    fn header(&mut self, area: Rect, buf: &mut Buffer) {
        let _ = (area, buf);
    }

    /// Draws the body, the area between the header and the footer, and
    /// returns what it measured.
    fn body(&mut self, area: Rect, buf: &mut Buffer) -> Self::Report;

    /// Draws the footer row (or rows). Draws nothing unless implemented.
    fn footer(&mut self, area: Rect, buf: &mut Buffer) {
        let _ = (area, buf);
    }

    /// Whether an open overlay fills the whole screen
    /// ([`ModalStack::fills_screen`](crate::overlay::ModalStack::fills_screen)),
    /// so the header, the body, and the footer are not drawn. `false`
    /// unless implemented.
    fn overlays_fill_screen(&self) -> bool {
        false
    }

    /// Draws the open overlays over the whole screen, bottom to top: a
    /// host with a [`ModalStack`](crate::overlay::ModalStack) draws it
    /// here ([`ModalStack::draw`](crate::overlay::ModalStack::draw)).
    /// Draws nothing unless implemented.
    fn overlays(&mut self, area: Rect, buf: &mut Buffer) {
        let _ = (area, buf);
    }

    /// Draws the toasts over everything else: a host with
    /// [`Toasts`](crate::feedback::Toasts) renders them here. Draws
    /// nothing unless implemented.
    fn toasts(&mut self, area: Rect, buf: &mut Buffer) {
        let _ = (area, buf);
    }
}

/// Where the chrome's parts go on a screen: the header rows, the body,
/// and the footer rows, top to bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppAreas {
    /// The header rows at the top.
    pub header: Rect,
    /// The body between the header and the footer.
    pub body: Rect,
    /// The footer rows at the bottom.
    pub footer: Rect,
}

/// The frame a program's screen is drawn in: a header, a body, and a
/// footer, then the overlays, then the toasts (see the
/// [module documentation](self)).
///
/// The header and the footer take one row each unless set. The body
/// takes the rest and, on a screen too short for all three, keeps at
/// least [`MIN_BODY`](Self::MIN_BODY) rows, the header and the footer
/// giving way first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppFrame {
    header: u16,
    footer: u16,
}

impl Default for AppFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl AppFrame {
    /// The rows the body keeps on a short screen before the header and
    /// the footer have theirs.
    pub const MIN_BODY: u16 = 3;

    /// A frame with a one-row header and a one-row footer.
    pub const fn new() -> Self {
        Self {
            header: 1,
            footer: 1,
        }
    }

    /// Gives the header `rows` rows; 0 leaves the screen without one.
    #[must_use]
    pub const fn header_rows(mut self, rows: u16) -> Self {
        self.header = rows;
        self
    }

    /// Gives the footer `rows` rows; 0 leaves the screen without one.
    #[must_use]
    pub const fn footer_rows(mut self, rows: u16) -> Self {
        self.footer = rows;
        self
    }

    /// Where the header, the body, and the footer go in `area`.
    pub fn areas(self, area: Rect) -> AppAreas {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(self.header),
            Constraint::Min(Self::MIN_BODY),
            Constraint::Length(self.footer),
        ])
        .areas(area);
        AppAreas {
            header,
            body,
            footer,
        }
    }

    /// Draws `screen` in `area`: the header, the body, and the footer
    /// (unless an overlay fills the screen), then the overlays over the
    /// whole area, then the toasts. Returns what the body measured, or
    /// the report's default when the body was not drawn.
    pub fn render<S: Screen + ?Sized>(
        self,
        area: Rect,
        buf: &mut Buffer,
        screen: &mut S,
    ) -> S::Report {
        let area = area.intersection(buf.area);
        let mut report = S::Report::default();
        if !screen.overlays_fill_screen() {
            let areas = self.areas(area);
            screen.header(areas.header, buf);
            report = screen.body(areas.body, buf);
            screen.footer(areas.footer, buf);
        }
        screen.overlays(area, buf);
        screen.toasts(area, buf);
        report
    }
}

#[cfg(test)]
#[path = "app_frame_tests.rs"]
mod tests;
