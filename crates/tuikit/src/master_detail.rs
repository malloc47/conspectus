//! The stacked master-detail layout: a list zone over a divider over a
//! detail area, in one pane.
//!
//! This is a layer 1 component. A pane that lists the parts of one thing
//! and shows the part under its cursor below them stacks three zones:
//!
//! - **The list** is as tall as its rows wrap to, never less than its
//!   minimum (three rows unless set) and never more than the room a
//!   minimum detail area leaves. Past that it scrolls: its window follows
//!   every row the cursor's line took, and the shared scrollbar gutter
//!   takes its right column.
//! - **The divider** is one row under the list: a line the host hands in,
//!   usually a [`SectionRule`](crate::frame::SectionRule) naming the
//!   detail.
//! - **The detail** gets the rest, at least its minimum (six rows unless
//!   set) while the pane is tall enough. On a shorter pane the list keeps
//!   its minimum and the detail takes what that and the divider leave.
//!
//! The host lays the list's lines out at [`StackedMasterDetail::layout_width`],
//! one column narrower than the pane, so a line as wide as its room (a
//! rule) still fits when the scrollbar takes a column; laid out at the
//! full width it would wrap onto a second row there, and every row under
//! it would land a row lower than the window counts. The layout wraps
//! the lines at the full width while they fit and one column narrower
//! when they overflow, so the rows it counts are the rows it draws.
//!
//! The host keeps what the zones hold: the lines of the list (and any
//! block it puts above them, which moves the cursor's line), the
//! divider's words, and the detail it draws in the area the layout
//! returns. It also keeps the list's [`ScrollWindow`] between frames:
//! [`render`](StackedMasterDetail::render) takes the last frame's window
//! and returns the one it drew.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use tuikit::frame::SectionRule;
//! use tuikit::master_detail::StackedMasterDetail;
//! use tuikit::property_grid::GridLines;
//! use tuikit::scroll::ScrollWindow;
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 30, 16);
//! let layout = StackedMasterDetail::new();
//! assert_eq!(StackedMasterDetail::layout_width(area), 29);
//!
//! // Twenty rows do not fit: the list takes the room six detail rows
//! // leave, and its window follows the cursor down.
//! let lines: Vec<Line> = (1..=20).map(|n| Line::from(format!("item {n}"))).collect();
//! let list = GridLines { lines, cursor_line: Some(14) };
//! let divider = SectionRule::new("Detail").line(usize::from(area.width), &theme);
//! let mut buf = Buffer::empty(area);
//! let drawn = layout.render(area, &mut buf, list, divider, ScrollWindow::default());
//! assert_eq!(drawn.list, Rect::new(0, 0, 30, 9));
//! assert_eq!(drawn.divider, Rect::new(0, 9, 30, 1));
//! assert_eq!(drawn.detail, Rect::new(0, 10, 30, 6));
//! assert_eq!(drawn.window.visible(), 6..15);
//!
//! // The same areas for a list of two rows, without drawing.
//! let (list, divider, detail) = layout.areas(area, 2);
//! assert_eq!((list.height, divider.y, detail.height), (3, 3, 12));
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::property_grid::GridLines;
use crate::scroll::{ScrollGutter, ScrollWindow, WrappedRows};

/// A list zone sized to its rows over a one-row divider over a detail
/// area, in one pane.
///
/// See the [module documentation](self) for the rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StackedMasterDetail {
    min_list: u16,
    min_detail: u16,
}

/// What [`StackedMasterDetail::render`] drew: the list's window, for the
/// host to keep, and the three areas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stacked {
    /// The list's window as drawn. Keep it and hand it to the next
    /// frame's render, and page the list by its
    /// [`page`](ScrollWindow::page).
    pub window: ScrollWindow,
    /// Where the list drew, the scrollbar's gutter included.
    pub list: Rect,
    /// The divider's row.
    pub divider: Rect,
    /// Where the host draws the detail.
    pub detail: Rect,
}

impl Default for StackedMasterDetail {
    fn default() -> Self {
        Self::new()
    }
}

impl StackedMasterDetail {
    /// The list's minimum rows unless [`min_list`](Self::min_list) sets
    /// them.
    pub const MIN_LIST: u16 = 3;
    /// The detail's minimum rows unless [`min_detail`](Self::min_detail)
    /// sets them.
    pub const MIN_DETAIL: u16 = 6;

    /// The layout with the list's and the detail's default minimums.
    pub const fn new() -> Self {
        Self {
            min_list: Self::MIN_LIST,
            min_detail: Self::MIN_DETAIL,
        }
    }

    /// The fewest rows the list takes, however few its rows are and
    /// however short the pane.
    #[must_use]
    pub const fn min_list(mut self, rows: u16) -> Self {
        self.min_list = rows;
        self
    }

    /// The rows the detail keeps however many rows the list has, while
    /// the pane holds them beside the list's minimum and the divider.
    #[must_use]
    pub const fn min_detail(mut self, rows: u16) -> Self {
        self.min_detail = rows;
        self
    }

    /// The width the host lays the list's lines out at: the area's, less
    /// the column the scrollbar takes when the list overflows, so a line
    /// as wide as its room never wraps when the bar appears. At least 1.
    pub fn layout_width(area: Rect) -> usize {
        usize::from(area.width.saturating_sub(1).max(1))
    }

    /// The most rows the list takes in `area`: what the detail's minimum
    /// and the divider leave, or the list's minimum when that is more.
    /// The detail's minimum gives way first on a short pane, to what the
    /// list's minimum and the divider leave.
    fn list_room(&self, area: Rect) -> u16 {
        let detail = self
            .min_detail
            .min(area.height.saturating_sub(self.min_list.saturating_add(1)));
        area.height
            .saturating_sub(detail.saturating_add(1))
            .max(self.min_list)
    }

    /// The list's, the divider's, and the detail's areas in `area` for a
    /// list of `rows` rows (as wrapped), without drawing.
    ///
    /// The list takes its rows, at least its minimum and at most the
    /// room the detail's minimum leaves; the divider takes the row under
    /// it, and the detail the rest. On an area too short for the list's
    /// minimum and the divider, the list keeps its first row, then the
    /// divider its row, then the list what is left, and the detail gets
    /// none.
    pub fn areas(&self, area: Rect, rows: usize) -> (Rect, Rect, Rect) {
        let rows = u16::try_from(rows).unwrap_or(u16::MAX);
        let wanted = rows.min(self.list_room(area)).max(self.min_list);
        let first = wanted.min(1).min(area.height);
        let divider = 1.min(area.height - first);
        let list = wanted.min(area.height - divider);
        let detail = area.height - list - divider;
        let at = |y: u16, height: u16| Rect {
            y: area.y + y,
            height,
            ..area
        };
        (at(0, list), at(list, divider), at(list + divider, detail))
    }

    /// Wrap the list's lines, size the zones, and draw the list and the
    /// divider in `area`; return the list's window and the three areas.
    ///
    /// The lines wrap at the area's width while they fit the list's
    /// room and one column narrower when they overflow it, where the
    /// scrollbar gutter takes that column. `window` is the last frame's:
    /// it is measured again for the list and moved as little as it must
    /// to show every row the cursor's line took (the first row when
    /// there is no cursor), and drawn with the shared scrollbar. The
    /// divider is drawn as it is, one row; lay it out at the area's
    /// width. The detail area is left for the host.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        list: GridLines,
        divider: Line<'_>,
        mut window: ScrollWindow,
    ) -> Stacked {
        let room = self.list_room(area);
        let rows = WrappedRows::fit(&list.lines, usize::from(area.width), usize::from(room));
        let (list_area, divider_area, detail) = self.areas(area, rows.len());
        window.resize(usize::from(list_area.height), rows.len());
        window.follow(list.cursor_line.map_or(0..1, |line| rows.rows_of(line)));
        let gutter = ScrollGutter::layout(list_area, rows.len());
        Paragraph::new(rows.into_rows())
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        Paragraph::new(divider).render(divider_area, buf);
        Stacked {
            window,
            list: list_area,
            divider: divider_area,
            detail,
        }
    }
}

#[cfg(test)]
#[path = "master_detail_tests.rs"]
mod tests;
