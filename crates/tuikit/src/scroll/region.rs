//! A window counted from an anchor: the top, the bottom, or a marked row.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget};

use super::{ScrollGutter, ScrollWindow, WrappedRows};

/// Where a [`ScrollRegion`] counts its offset from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// The first row: the region opens at the top, and scrolling down moves
    /// away from it. The default.
    #[default]
    Top,
    /// The last row: the region opens on the newest rows, and scrolling up
    /// moves away from them. The region keeps its distance from the end, so
    /// it stays on the newest rows as rows arrive until it is scrolled.
    Bottom,
    /// A row the content marks, such as a match the region opened on: the
    /// region opens with that row at its top and scrolls either way from
    /// it. The row is given with [`ScrollRegion::marked`] as the content
    /// is laid out.
    Mark,
}

/// A window onto rows whose offset counts from an [`Anchor`]: the top, the
/// bottom, or a row the content marks.
///
/// A body that shows the newest rows of something (a log, a terminal's
/// output) or opens on a match keeps its place relative to that anchor,
/// not to the first row: scrolled up three rows from the end, it stays
/// three rows from the end when the next frame lays out more rows. The
/// region holds the anchor, the rows it has been moved from it, and the
/// window the last placement showed. Like [`ScrollWindow`], it is a plain
/// `Copy` value the host keeps between frames: a draw places it and
/// returns the region as drawn, which the host stores.
///
/// - **Keys.** [`scroll_by`](Self::scroll_by) moves it, down when
///   positive. A step toward the anchor's own end stops there at once (no
///   higher than the first row counting from the top, no lower than the
///   last counting from the bottom); a step the other way past the far
///   end is taken back by the next placement, so the next step moves.
/// - **Anchors.** [`set_anchor`](Self::set_anchor) says what the region
///   counts from; a different anchor starts the region at it.
///   [`reset`](Self::reset) goes back to the anchor.
/// - **Placing.** [`place`](Self::place) works out the window over the
///   rows a frame laid out, clamped inside them.
/// - **Drawing.** [`draw_lines`](Self::draw_lines) wraps lines to an area
///   and draws the region's window of them, a leading banner scrolling
///   with them; [`draw_cut`](Self::draw_cut) draws rows the host cut at
///   the placed window under a banner that stays put. Both draw the
///   shared scrollbar when the rows overflow. [`rows_under`] is the room a
///   banner leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScrollRegion {
    anchor: Anchor,
    /// Rows moved from the anchor, down when positive: never negative
    /// from the top, never positive from the bottom.
    moved: i64,
    /// The row a [`Anchor::Mark`] counts from.
    mark: usize,
    /// The window the last placement showed.
    window: ScrollWindow,
}

impl ScrollRegion {
    /// A region at `anchor`, not yet placed.
    pub const fn new(anchor: Anchor) -> Self {
        Self {
            anchor,
            moved: 0,
            mark: 0,
            window: ScrollWindow::new(0, 0),
        }
    }

    /// What the region counts from.
    pub const fn anchor(&self) -> Anchor {
        self.anchor
    }

    /// Counts from `anchor` from now on. A region that counted from another
    /// anchor starts at this one; one that already counts from it keeps
    /// its place.
    pub fn set_anchor(&mut self, anchor: Anchor) {
        if self.anchor != anchor {
            self.anchor = anchor;
            self.moved = 0;
        }
    }

    /// This region with `row` as the row an [`Anchor::Mark`] counts from,
    /// in the rows the next placement lays out (a match's first row, less
    /// any context above it). Its offset from the mark is kept, so a mark
    /// that moves as the content is laid out again carries the window with
    /// it.
    #[must_use]
    pub const fn marked(self, row: usize) -> Self {
        Self { mark: row, ..self }
    }

    /// The row an [`Anchor::Mark`] counts from.
    pub const fn mark(&self) -> usize {
        self.mark
    }

    /// Scrolls `delta` rows, down when positive. A step toward the
    /// anchor's own end stops there at once; a step past the far end is
    /// taken back by the next [`place`](Self::place).
    pub fn scroll_by(&mut self, delta: i32) {
        let moved = self.moved.saturating_add(i64::from(delta));
        self.moved = match self.anchor {
            Anchor::Top => moved.max(0),
            Anchor::Bottom => moved.min(0),
            Anchor::Mark => moved,
        };
    }

    /// Goes back to the anchor.
    pub fn reset(&mut self) {
        self.moved = 0;
    }

    /// Rows moved from the anchor, down when positive.
    pub const fn moved(&self) -> i64 {
        self.moved
    }

    /// Rows moved from the anchor, either way: down from the top, up from
    /// the bottom.
    pub const fn away(&self) -> u64 {
        self.moved.unsigned_abs()
    }

    /// The window the last placement showed. Empty before the first.
    pub const fn window(&self) -> ScrollWindow {
        self.window
    }

    /// This region placed over `content` rows in a window of `viewport`
    /// rows, counting from `anchor`: the window it shows, with the rows it
    /// has moved brought inside the content. A region that counted from
    /// another anchor is placed at `anchor`. Keep the result: it is what
    /// the next key moves from.
    ///
    /// ```
    /// use tuikit::scroll::{Anchor, ScrollRegion};
    ///
    /// // Counting from the bottom: three rows up from the end of 20, in 5.
    /// let mut region = ScrollRegion::new(Anchor::Bottom);
    /// region.scroll_by(-3);
    /// let placed = region.place(Anchor::Bottom, 5, 20);
    /// assert_eq!(placed.window().visible(), 12..17);
    ///
    /// // Ten more rows arrive; the region stays three rows from the end.
    /// assert_eq!(placed.place(Anchor::Bottom, 5, 30).window().visible(), 22..27);
    ///
    /// // A step past the top is taken back by the next placement.
    /// let mut far = placed;
    /// far.scroll_by(-100);
    /// let far = far.place(Anchor::Bottom, 5, 20);
    /// assert_eq!(far.window().offset(), 0);
    /// assert_eq!(far.away(), 15);
    /// ```
    #[must_use]
    pub fn place(self, anchor: Anchor, viewport: usize, content: usize) -> Self {
        let moved = if anchor == self.anchor { self.moved } else { 0 };
        let window = ScrollWindow::new(viewport, content);
        let last = window.last_offset();
        let from = rows(match anchor {
            Anchor::Top => 0,
            Anchor::Bottom => last,
            Anchor::Mark => self.mark,
        });
        let at = usize::try_from(from.saturating_add(moved).max(0)).unwrap_or(usize::MAX);
        let offset = at.min(last);
        Self {
            anchor,
            moved: rows(offset) - from,
            mark: self.mark,
            window: window.at(offset),
        }
    }

    /// Draws `text` in `area`, its lines wrapped to the area (a cell
    /// narrower beside the scrollbar when they overflow), placed from
    /// `anchor`, with the scrollbar. A banner leading `text` is its first
    /// lines and scrolls away with the rest. For [`Anchor::Mark`], the mark
    /// is a line of `text` and the window counts from its first row.
    /// Returns the region as drawn.
    pub fn draw_lines(
        self,
        anchor: Anchor,
        area: Rect,
        buf: &mut Buffer,
        text: Text<'static>,
    ) -> Self {
        let height = usize::from(area.height);
        let rows = WrappedRows::fit(&text.lines, usize::from(area.width), height);
        let gutter = ScrollGutter::layout(area, rows.len());
        let mark_row = rows.rows_of(self.mark).start;
        let placed = self
            .marked(mark_row)
            .place(anchor, height, rows.len())
            .marked(self.mark);
        let text = Text {
            lines: rows.into_rows(),
            style: text.style,
            alignment: text.alignment,
        };
        Paragraph::new(text)
            .scroll(placed.window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&placed.window, buf);
        placed
    }

    /// Draws `text` in `area` as it is: a banner, if any, and then the rows
    /// of this region's [`window`](Self::window), which the host cut from
    /// rows it laid out after [`place`](Self::place). Lay those rows out a
    /// cell narrower than `area`, the scrollbar's column; the banner wraps
    /// beside it. The banner stays put above the rows and the scrollbar
    /// tracks the window, not the banner. Returns the region.
    pub fn draw_cut(self, area: Rect, buf: &mut Buffer, text: Text<'static>) -> Self {
        let gutter = ScrollGutter::layout(area, self.window.content().max(text.lines.len()));
        let rows = WrappedRows::new(&text.lines, usize::from(gutter.content().width));
        Paragraph::new(Text {
            lines: rows.into_rows(),
            style: text.style,
            alignment: text.alignment,
        })
        .render(gutter.content(), buf);
        gutter.render(&self.window, buf);
        self
    }
}

/// The rows a body gets under `banner` in an area `width` cells wide and
/// `height` rows tall: what the banner's wrapped rows leave, at least one.
///
/// ```
/// use ratatui::text::Line;
/// use tuikit::scroll::rows_under;
///
/// let banner = [Line::raw("a banner that wraps onto a second row")];
/// assert_eq!(rows_under(&banner, 20, 10), 8);
/// assert_eq!(rows_under(&banner, 20, 2), 1);
/// ```
pub fn rows_under(banner: &[Line<'_>], width: u16, height: usize) -> usize {
    height
        .saturating_sub(WrappedRows::new(banner, usize::from(width)).len())
        .max(1)
}

/// A row count as a signed offset, saturating.
fn rows(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
