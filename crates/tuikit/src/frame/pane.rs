//! The pane frame: the bordered box a pane of the screen draws in, with
//! the focus cue on its border and title.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};

use super::title::trimmed;
use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// The bordered box a pane of the screen draws in: a border on all four
/// sides, a title on the top border, and the focus cue on both.
///
/// - **Border.** Plain lines on all four sides: in the theme's
///   `panel_focus_accent` while the pane has focus, in the terminal's own
///   look while it does not, so the focused pane is the one framed in
///   color (house rule 9). A modal's border is always the accent; a
///   pane's is only while it has the keys.
/// - **Lead.** The title starts with a space, the focus marker, and a
///   space: [`marks::POINTER`] in `panel_focus_accent` while the pane has
///   focus, a blank cell while it does not. The lead is
///   [`LEAD`](Self::LEAD) cells either way, so a title never moves as
///   focus does.
/// - **Title.** The host's title after the lead, in its own looks: the
///   border's color does not reach it. Spaces it has at its ends are
///   dropped, as in a modal's title, and the frame closes it with one
///   space when the border has a cell left for it. A title wider than
///   the border is cut at its corner, so a host lays its title out for
///   [`title_width`](Self::title_width).
/// - **No title.** A focused pane still shows the lead with its marker;
///   an unfocused pane with no title draws an unbroken top border.
///
/// The frame does not clear its area or paint a background: a pane is
/// the screen itself, not a box over it.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::text::Line;
/// use tuikit::frame::PaneFrame;
/// use tuikit::testkit::buffer::rows;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let area = Rect::new(0, 0, 20, 3);
/// let mut buf = Buffer::empty(area);
/// let inner = PaneFrame::new(&theme)
///     .title(Line::from("items"))
///     .focused(true)
///     .render(area, &mut buf);
/// assert_eq!(rows(&buf)[0], "┌ ► items ─────────┐");
/// assert_eq!(buf[(0, 0)].fg, theme.panel_focus_accent, "the border");
/// assert_eq!(buf[(2, 0)].fg, theme.panel_focus_accent, "the marker");
/// assert_eq!(inner, Rect::new(1, 1, 18, 1));
///
/// // The title's room in a pane this wide: between the corners, less
/// // the lead.
/// assert_eq!(PaneFrame::title_width(20), 15);
/// ```
#[derive(Clone, Debug)]
pub struct PaneFrame<'a> {
    theme: &'a Theme,
    title: Line<'a>,
    focused: bool,
}

impl<'a> PaneFrame<'a> {
    /// The cells the lead takes before the title: a space, the focus
    /// marker's cell, and a space.
    pub const LEAD: usize = 3;

    /// An unfocused frame with no title.
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            title: Line {
                style: Style::new(),
                alignment: None,
                spans: Vec::new(),
            },
            focused: false,
        }
    }

    /// Sets the title. Its spans keep their styles; the frame drops the
    /// spaces at its ends and adds the lead before it.
    #[must_use]
    pub fn title(mut self, title: impl Into<Line<'a>>) -> Self {
        self.title = title.into();
        self
    }

    /// Whether the pane has focus: the border and the marker take
    /// `panel_focus_accent` with it, and the marker is blank without it.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The cells a title may take in a pane `width` cells wide: the top
    /// border between its corners, less the lead. The closing space is
    /// not counted; it is drawn only when the title leaves a cell for it.
    pub fn title_width(width: u16) -> usize {
        usize::from(width.saturating_sub(2)).saturating_sub(Self::LEAD)
    }

    /// The area inside the border of a pane drawn in `area`.
    pub fn inner(&self, area: Rect) -> Rect {
        Block::bordered().inner(area)
    }

    /// Draws the border and the title in `area` and returns the area
    /// inside the border, where the pane's content goes.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> Rect {
        let area = area.intersection(buf.area);
        let border = if self.focused {
            Style::default().fg(self.theme.panel_focus_accent)
        } else {
            Style::default()
        };
        // A reset under the title keeps the border's color off the title's
        // own spans, which style themselves.
        let mut block = Block::bordered()
            .border_style(border)
            .title_style(Style::reset());
        let room = usize::from(area.width.saturating_sub(2));
        if let Some(title) = self.title_line(room) {
            block = block.title(title);
        }
        let inner = block.inner(area);
        block.render(area, buf);
        inner
    }

    /// The title as the top border draws it in `room` cells: the lead,
    /// the host's title, and the closing space when it fits. `None` for an
    /// unfocused pane with no title.
    fn title_line(self, room: usize) -> Option<Line<'a>> {
        let title = trimmed(self.title);
        if title.spans.is_empty() && !self.focused {
            return None;
        }
        // The blank keeps the marker's style, so as focus moves the lead's
        // cells change only their symbol.
        let mark = if self.focused { marks::POINTER } else { " " };
        let marker = Style::default().fg(self.theme.panel_focus_accent);
        let has_text = !title.spans.is_empty();
        let mut spans = Vec::with_capacity(title.spans.len() + 3);
        spans.push(Span::raw(" "));
        spans.push(Span::styled(format!("{mark} "), marker));
        spans.extend(title.spans);
        if has_text && text::spans_width(&spans) < room {
            spans.push(Span::raw(" "));
        }
        Some(Line {
            style: title.style,
            alignment: title.alignment,
            spans,
        })
    }
}

#[cfg(test)]
#[path = "pane_tests.rs"]
mod tests;
