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
/// The gutter fades on fit: rows that fit the area, or an area narrower
/// than two columns, get no gutter and keep every column. Carving the
/// gutter out of the body, instead of drawing the bar over its last
/// column, keeps the two apart, so a highlighted row never runs under
/// the bar. Lay the rows out at [`content`](Self::content)'s width;
/// [`WrappedRows::fit`](super::WrappedRows::fit) wraps them to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollGutter {
    content: Rect,
    bar: Option<Rect>,
}

impl ScrollGutter {
    /// Split `area` for a body of `rows` rows: all of it when they fit,
    /// and otherwise the area less its right column, with that column as
    /// the gutter.
    pub fn layout(area: Rect, rows: usize) -> Self {
        if rows == 0 || rows <= usize::from(area.height) || area.width < 2 {
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
            vertical: 1,
            horizontal: 0,
        });
        scrollbar.render(track, buf, &mut state);
    }
}

#[cfg(test)]
#[path = "gutter_tests.rs"]
mod tests;
