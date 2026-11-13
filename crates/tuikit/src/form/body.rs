//! The form body: a band of rows that scrolls with the cursor between
//! fixed bands above and below it, the bottom one holding the error line
//! and the footer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::frame::{FooterFit, ModalBody, ModalFooter};
use crate::scroll::ScrollWindow;

/// A form's body, as a [`ModalBody`]: fixed lines at the top, the rows,
/// and at the bottom fixed notes and the footer, whose slot holds the
/// error line above the keys.
///
/// Only the rows scroll. However short the screen, the fixed bands keep
/// their places and the rows scroll to keep the cursor's row in view, so
/// the footer and the error line never scroll away. When the frame has
/// fewer rows than the body asks for, they go in this order: the cursor's
/// row, the error line, the footer as [`ModalFooter::fit`] fits it (its
/// last key rows, then its blank row), the notes (their first lines), the
/// top band (its first lines), and then the other rows. The footer's
/// first key rows that do not fit scroll after the last row. So a refusal
/// is never hidden, nor the row it is about, nor the keys that leave.
///
/// The bottom bands are drawn against the bottom of the area and the top
/// band against its top; the rows fill what is between.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::text::Line;
/// use tuikit::form::FormBody;
/// use tuikit::frame::{ModalBody, ModalFooter};
/// use tuikit::keys::{Footer, TEXT_INPUT_KEYS};
/// use tuikit::testkit::buffer::rows;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let fields: Vec<Line> = (0..6).map(|n| Line::from(format!("field {n}"))).collect();
/// let body = FormBody::new(fields, 5)
///     .error(Some(Line::from("a name is required")))
///     .footer(ModalFooter::new(Footer::new(TEXT_INPUT_KEYS), &theme));
/// let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
/// let window = body.render(buf.area, &mut buf);
/// assert_eq!(window.visible(), 4..6, "the rows scroll to the cursor");
/// let drawn = rows(&buf);
/// assert_eq!(drawn[1].trim_end(), "field 5");
/// assert_eq!(drawn[2].trim_end(), "", "the blank row above the footer");
/// assert_eq!(drawn[3].trim_end(), "a name is required");
/// assert_eq!(drawn[4].trim_end(), "Enter confirm · Esc cancel");
/// ```
#[derive(Clone, Debug, Default)]
pub struct FormBody<'a> {
    top: Vec<Line<'a>>,
    rows: Vec<Line<'a>>,
    cursor: usize,
    notes: Vec<Line<'a>>,
    error: Option<Line<'static>>,
    footer: Option<ModalFooter<'a>>,
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

    /// Lines fixed under the rows, above the footer: details about the
    /// focused row, after a blank line that separates them from the rows.
    #[must_use]
    pub fn notes(mut self, lines: Vec<Line<'a>>) -> Self {
        self.notes = lines;
        self
    }

    /// The error line, in the footer's slot above its keys
    /// ([`ModalFooter::status`]); `None` while there is no error.
    #[must_use]
    pub fn error(mut self, line: Option<Line<'static>>) -> Self {
        self.error = line;
        self
    }

    /// The footer, fixed at the bottom under its blank row.
    #[must_use]
    pub fn footer(mut self, footer: ModalFooter<'a>) -> Self {
        self.footer = Some(footer);
        self
    }

    /// The footer `width` cells wide fitted to `room` rows, with the error
    /// line in its slot; a body with no footer keeps the error line alone.
    fn fit(&self, width: usize, room: usize) -> FooterFit {
        match &self.footer {
            Some(footer) => footer.clone().status(self.error.clone()).fit(width, room),
            None => FooterFit {
                kept: self.error.iter().take(room.min(1)).cloned().collect(),
                spill: Vec::new(),
                width: u16::try_from(width).unwrap_or(u16::MAX),
            },
        }
    }

    /// How many rows each band gets in a body `width` cells wide and
    /// `height` rows tall, and the footer fitted to its share.
    fn bands(&self, width: usize, height: usize) -> (Bands, FooterFit) {
        let cursor = self.rows.len().min(1).min(height);
        let fit = self.fit(width, height - cursor);
        let mut room = height - cursor;
        let mut take = |wanted: usize| {
            let got = wanted.min(room);
            room -= got;
            got
        };
        let footer = take(fit.kept.len());
        let notes = take(self.notes.len());
        let top = take(self.top.len());
        let rows = cursor + room;
        let bands = Bands {
            top,
            rows,
            notes,
            footer,
        };
        (bands, fit)
    }
}

/// The rows each band of a [`FormBody`] gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bands {
    top: usize,
    rows: usize,
    notes: usize,
    footer: usize,
}

impl ModalBody for FormBody<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let footer = match &self.footer {
            Some(footer) => footer
                .clone()
                .status(self.error.clone())
                .height(usize::from(width)),
            None => usize::from(self.error.is_some()),
        };
        let rows = self.top.len() + self.rows.len() + self.notes.len() + footer;
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let (bands, fit) = self.bands(usize::from(area.width), usize::from(area.height));
        let FooterFit { kept, spill, width } = fit;
        // The footer's rows that do not fit follow the last row. They spill
        // only while the rows are down to the cursor's, which the window
        // keeps, so a form shows none of them.
        let mut rows = self.rows;
        rows.extend(spill);
        let window =
            ScrollWindow::new(bands.rows, rows.len()).following(self.cursor..self.cursor + 1);
        let line_at = |y: usize, width: u16| {
            Rect::new(
                area.x,
                area.y + u16::try_from(y).unwrap_or(u16::MAX),
                width,
                1,
            )
        };
        let mut y = 0;
        let mut draw = |line: Line<'_>, y: &mut usize, width: u16| {
            Paragraph::new(line).render(line_at(*y, width), buf);
            *y += 1;
        };
        for line in self.top.into_iter().take(bands.top) {
            draw(line, &mut y, area.width);
        }
        for line in rows.into_iter().skip(window.offset()).take(bands.rows) {
            draw(line, &mut y, area.width);
        }
        // The bottom bands sit against the bottom of the area.
        y = usize::from(area.height) - bands.notes - bands.footer;
        for line in self.notes.into_iter().take(bands.notes) {
            draw(line, &mut y, area.width);
        }
        for line in kept.into_iter().take(bands.footer) {
            draw(line, &mut y, width.min(area.width));
        }
        window
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
