//! The form body: a band of rows that scrolls with the cursor between
//! fixed bands above and below it, the bottom one holding the error line
//! and the footer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::frame::ModalBody;
use crate::scroll::ScrollWindow;

/// A form's body, as a [`ModalBody`]: fixed lines at the top, the rows,
/// and at the bottom fixed notes, the error line, and the footer.
///
/// Only the rows scroll. However short the screen, the fixed bands keep
/// their places and the rows scroll to keep the cursor's row in view, so
/// the footer and the error line never scroll away. When the frame has
/// fewer rows than the body asks for, they go in this order: the cursor's
/// row, the error line, the footer (its first lines), the notes (their
/// first lines), the top band (its first lines), and then the other rows.
/// So a refusal is never hidden, and nor is the row it is about.
///
/// The bottom bands are drawn against the bottom of the area and the top
/// band against its top; the rows fill what is between.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::text::Line;
/// use tuikit::form::FormBody;
/// use tuikit::frame::ModalBody;
/// use tuikit::testkit::buffer::rows;
///
/// let fields: Vec<Line> = (0..6).map(|n| Line::from(format!("field {n}"))).collect();
/// let body = FormBody::new(fields, 5)
///     .notes(vec![Line::default()])
///     .error(Some(Line::from("a name is required")))
///     .footer(vec![Line::from("Enter save · Esc cancel")]);
/// let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
/// let window = body.render(buf.area, &mut buf);
/// assert_eq!(window.visible(), 4..6, "the rows scroll to the cursor");
/// let drawn = rows(&buf);
/// assert_eq!(drawn[1].trim_end(), "field 5");
/// assert_eq!(drawn[3].trim_end(), "a name is required");
/// assert_eq!(drawn[4].trim_end(), "Enter save · Esc cancel");
/// ```
#[derive(Clone, Debug, Default)]
pub struct FormBody<'a> {
    top: Vec<Line<'a>>,
    rows: Vec<Line<'a>>,
    cursor: usize,
    notes: Vec<Line<'a>>,
    error: Option<Line<'a>>,
    footer: Vec<Line<'a>>,
}

impl<'a> FormBody<'a> {
    /// A body of `rows`, scrolled to keep row `cursor` in view, with no
    /// fixed bands.
    pub fn new(rows: Vec<Line<'a>>, cursor: usize) -> Self {
        Self {
            rows,
            cursor,
            ..Self::default()
        }
    }

    /// Lines fixed above the rows: a summary or an explanation that
    /// stays in view while the rows scroll.
    #[must_use]
    pub fn top(mut self, lines: Vec<Line<'a>>) -> Self {
        self.top = lines;
        self
    }

    /// Lines fixed under the rows, before the error line: the blank line
    /// that separates the rows from the footer, or details about the
    /// focused row.
    #[must_use]
    pub fn notes(mut self, lines: Vec<Line<'a>>) -> Self {
        self.notes = lines;
        self
    }

    /// The error line, in the footer slot above the footer; `None` while
    /// there is no error.
    #[must_use]
    pub fn error(mut self, line: Option<Line<'a>>) -> Self {
        self.error = line;
        self
    }

    /// The footer, fixed at the bottom.
    #[must_use]
    pub fn footer(mut self, lines: Vec<Line<'a>>) -> Self {
        self.footer = lines;
        self
    }

    /// How many rows each band gets in a body `height` rows tall.
    fn bands(&self, height: usize) -> Bands {
        let mut room = height;
        let mut take = |wanted: usize| {
            let got = wanted.min(room);
            room -= got;
            got
        };
        let cursor = take(self.rows.len().min(1));
        let error = take(usize::from(self.error.is_some()));
        let footer = take(self.footer.len());
        let notes = take(self.notes.len());
        let top = take(self.top.len());
        let rows = cursor + room;
        Bands {
            top,
            rows,
            notes,
            error,
            footer,
        }
    }
}

/// The rows each band of a [`FormBody`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bands {
    top: usize,
    rows: usize,
    notes: usize,
    error: usize,
    footer: usize,
}

impl ModalBody for FormBody<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, _width: u16) -> u16 {
        let rows = self.top.len()
            + self.rows.len()
            + self.notes.len()
            + usize::from(self.error.is_some())
            + self.footer.len();
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let bands = self.bands(usize::from(area.height));
        let window =
            ScrollWindow::new(bands.rows, self.rows.len()).following(self.cursor..self.cursor + 1);
        let line_at = |y: usize| {
            Rect::new(
                area.x,
                area.y + u16::try_from(y).unwrap_or(u16::MAX),
                area.width,
                1,
            )
        };
        let mut y = 0;
        let mut draw = |line: Line<'_>, y: &mut usize| {
            Paragraph::new(line).render(line_at(*y), buf);
            *y += 1;
        };
        for line in self.top.into_iter().take(bands.top) {
            draw(line, &mut y);
        }
        for line in self.rows.into_iter().skip(window.offset()).take(bands.rows) {
            draw(line, &mut y);
        }
        // The bottom bands sit against the bottom of the area.
        y = usize::from(area.height) - bands.notes - bands.error - bands.footer;
        for line in self.notes.into_iter().take(bands.notes) {
            draw(line, &mut y);
        }
        if bands.error > 0
            && let Some(line) = self.error
        {
            draw(line, &mut y);
        }
        for line in self.footer.into_iter().take(bands.footer) {
            draw(line, &mut y);
        }
        window
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
