//! The follow rule and the clamping scroll window.

use std::ops::Range;

/// The offset that shows `rows` in a window of `viewport` rows over
/// `content` rows, moved from `offset` no further than it must
/// ("minimal motion").
///
/// - The offset first comes back inside the content: it never passes
///   `content - viewport`, the offset that puts the last row on the
///   window's last line.
/// - If the rows end below the window, it moves down until their last
///   row is its last line.
/// - If they start above the window, it moves up until their first row is
///   its first line. A span taller than the window therefore shows its
///   start, the row that names the item.
/// - Otherwise the offset stays where it is: moving the cursor inside the
///   window does not scroll it, and nothing centers the cursor.
///
/// `rows` is the span the cursor's item takes, one row for a plain list
/// or every row a wrapped line took; an empty span means the one row at
/// its start. A window of no rows shows nothing, so only the clamp
/// applies. A control that keeps no offset passes 0, which keeps the
/// window at the top until the cursor passes the first screen and then
/// keeps the cursor on the last line.
///
/// ```
/// use tuikit::scroll::follow_offset;
///
/// // Row 12 in a 5-row window over 20 rows: scroll until it is the last line.
/// assert_eq!(follow_offset(0, 12..13, 5, 20), 8);
/// // Row 9 is already inside a window at 8: stay.
/// assert_eq!(follow_offset(8, 9..10, 5, 20), 8);
/// // Back up to row 3: it becomes the first line.
/// assert_eq!(follow_offset(8, 3..4, 5, 20), 3);
/// ```
pub fn follow_offset(offset: usize, rows: Range<usize>, viewport: usize, content: usize) -> usize {
    let last = content.saturating_sub(viewport);
    let mut offset = offset.min(last);
    if viewport == 0 {
        return offset;
    }
    let start = rows.start;
    let end = rows.end.max(start.saturating_add(1));
    if end > offset.saturating_add(viewport) {
        offset = end - viewport;
    }
    if start < offset {
        offset = start;
    }
    offset.min(last)
}

/// A window of `viewport` rows onto `content` rows, scrolled down by an
/// offset.
///
/// The window clamps on every use. Each constructor and each step leaves
/// the offset at most `content - viewport` (and 0 when the content fits),
/// so a window never shows blank rows below its content while rows above
/// it are scrolled away. A window measured again after its content shrank
/// comes back inside it by [`resize`](Self::resize).
///
/// The window holds no cursor: a list moves its cursor with
/// [`clamp_step`](super::clamp_step) or [`wrap_step`](super::wrap_step)
/// and then calls [`follow`](Self::follow) with the cursor's rows. A
/// pager without a cursor scrolls the window itself with
/// [`scroll_by`](Self::scroll_by), the page steps, and the ends.
///
/// The default window is empty: no rows in no viewport, at offset 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ScrollWindow {
    offset: usize,
    viewport: usize,
    content: usize,
}

impl ScrollWindow {
    /// A window of `viewport` rows at the top of `content` rows.
    pub const fn new(viewport: usize, content: usize) -> Self {
        Self {
            offset: 0,
            viewport,
            content,
        }
    }

    /// This window scrolled to `offset`, or to its last offset if that
    /// is nearer.
    #[must_use]
    pub fn at(self, offset: usize) -> Self {
        Self {
            offset: offset.min(self.last_offset()),
            ..self
        }
    }

    /// Measure the window again: `viewport` rows over `content` rows. The
    /// offset stays where it still fits and otherwise comes back to the
    /// last offset.
    pub fn resize(&mut self, viewport: usize, content: usize) {
        *self = Self::new(viewport, content).at(self.offset);
    }

    /// Move the window as little as it must to show `rows`, by the rule
    /// of [`follow_offset`].
    pub fn follow(&mut self, rows: Range<usize>) {
        self.offset = follow_offset(self.offset, rows, self.viewport, self.content);
    }

    /// Scroll `delta` rows, down when positive, stopping at the ends.
    pub fn scroll_by(&mut self, delta: i32) {
        let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        let offset = if delta >= 0 {
            self.offset.saturating_add(step)
        } else {
            self.offset.saturating_sub(step)
        };
        *self = self.at(offset);
    }

    /// Scroll down one [`page`](Self::page).
    pub fn page_down(&mut self) {
        *self = self.at(self.offset.saturating_add(self.page()));
    }

    /// Scroll up one [`page`](Self::page).
    pub fn page_up(&mut self) {
        self.offset = self.offset.saturating_sub(self.page());
    }

    /// Scroll to the first row.
    pub fn to_top(&mut self) {
        self.offset = 0;
    }

    /// Scroll to the last offset, where the last row is the window's last
    /// line.
    pub fn to_bottom(&mut self) {
        self.offset = self.last_offset();
    }

    /// The first row shown.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// How many rows the window shows at once.
    pub const fn viewport(&self) -> usize {
        self.viewport
    }

    /// How many rows there are in all.
    pub const fn content(&self) -> usize {
        self.content
    }

    /// The largest offset: the one that puts the last row on the window's
    /// last line, or 0 when the content fits.
    pub const fn last_offset(&self) -> usize {
        self.content.saturating_sub(self.viewport)
    }

    /// Whether the content is taller than the window, which is when the
    /// scrollbar gutter appears.
    pub const fn overflows(&self) -> bool {
        self.content > self.viewport
    }

    /// The rows the window shows.
    pub fn visible(&self) -> Range<usize> {
        self.offset..self.offset.saturating_add(self.viewport).min(self.content)
    }

    /// How far a page key moves: the rows the window shows, at least one.
    pub const fn page(&self) -> usize {
        if self.viewport == 0 { 1 } else { self.viewport }
    }

    /// The offset as the `(rows, columns)` pair that
    /// [`Paragraph::scroll`](ratatui::widgets::Paragraph::scroll) takes,
    /// saturating at the largest row it can address.
    pub fn paragraph_scroll(&self) -> (u16, u16) {
        (u16::try_from(self.offset).unwrap_or(u16::MAX), 0)
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
