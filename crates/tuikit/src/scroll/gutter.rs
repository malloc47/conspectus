//! The scrollbar gutter and the one scrollbar look.

use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget};

use super::ScrollWindow;

/// Where a scrolled body draws: the area its rows take and, when the
/// rows overflow, the 1-column gutter on the right edge for the
/// scrollbar.
///
/// The gutter fades where it would show nothing: rows that fit the area,
/// an area narrower than two columns, or one shorter than
/// [`MIN_HEIGHT`](Self::MIN_HEIGHT), where the bar has no room for its
/// track, get no gutter and keep every column. Carving the gutter out of
/// the body, instead of drawing the bar over its last column, keeps the
/// two apart, so a highlighted row never runs under the bar. Lay the rows
/// out at [`content`](Self::content)'s width;
/// [`WrappedRows::fit`](super::WrappedRows::fit) wraps them to it, and
/// [`carves`](Self::carves) says beforehand whether the column goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollGutter {
    content: Rect,
    bar: Option<Rect>,
}

/// The rows the track keeps clear above it and below it.
const MARGIN: u16 = 1;
/// The rows the bar's two arrows take, at the track's ends.
const ARROWS: u16 = 2;

impl ScrollGutter {
    /// The fewest rows a gutter is carved for: the clear row above and
    /// below the track, its two arrows, and one cell of track between
    /// them, where the thumb shows. A shorter body overflows with no bar
    /// (ratatui draws none without a track cell), so it keeps its column.
    pub const MIN_HEIGHT: u16 = 2 * MARGIN + ARROWS + 1;

    /// Whether a body of `rows` rows in an area `width` cells wide and
    /// `height` rows tall gives its right column to the scrollbar: the
    /// rows overflow and the area is at least two columns wide and
    /// [`MIN_HEIGHT`](Self::MIN_HEIGHT) rows tall. A body that lays its
    /// rows out before it draws (a footer beside the bar, rows wrapped to
    /// the width left) asks this, so it agrees with
    /// [`layout`](Self::layout).
    pub const fn carves(width: u16, height: u16, rows: usize) -> bool {
        rows > height as usize && width >= 2 && height >= Self::MIN_HEIGHT
    }

    /// Split `area` for a body of `rows` rows: the area less its right
    /// column, with that column as the gutter, when the body
    /// [`carves`](Self::carves) one, and otherwise all of it.
    pub fn layout(area: Rect, rows: usize) -> Self {
        if !Self::carves(area.width, area.height, rows) {
            return Self {
                content: area,
                bar: None,
            };
        }
        let content = Rect {
            width: area.width - 1,
            ..area
        };
        let bar = Rect {
            x: area.x + area.width - 1,
            width: 1,
            ..area
        };
        Self {
            content,
            bar: Some(bar),
        }
    }

    /// The area the rows draw in.
    pub const fn content(&self) -> Rect {
        self.content
    }

    /// The gutter column, when the rows overflow.
    pub const fn bar(&self) -> Option<Rect> {
        self.bar
    }

    /// Draw the scrollbar for `window` in the gutter. Draws nothing when
    /// there is no gutter or the window's content fits it.
    ///
    /// The track sits inside the gutter with a 1-row margin at the top
    /// and the bottom, so it never draws over a border or a rule above
    /// or below the body. The thumb reaches the bottom of the track
    /// exactly when the window is at its last offset, and its length is
    /// the window's share of the content. The bar removes the reversed
    /// modifier from its cells, so a selection highlight painted there
    /// earlier does not show through.
    pub fn render(&self, window: &ScrollWindow, buf: &mut Buffer) {
        let Some(bar) = self.bar else {
            return;
        };
        if window.viewport() == 0 || !window.overflows() {
            return;
        }
        // The scrollbar counts positions from the first row to the one
        // the thumb stops at. The window's last offset is the last
        // position, so the thumb ends at the bottom of the track there,
        // not halfway down.
        let mut state = ScrollbarState::new(window.last_offset() + 1)
            .position(window.offset())
            .viewport_content_length(window.viewport());
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().remove_modifier(Modifier::REVERSED));
        let track = bar.inner(Margin {
            vertical: MARGIN,
            horizontal: 0,
        });
        scrollbar.render(track, buf, &mut state);
    }
}

#[cfg(test)]
#[path = "gutter_tests.rs"]
mod tests;
