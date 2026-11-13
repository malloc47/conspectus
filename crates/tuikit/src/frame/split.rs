//! The split pane: two panes side by side on a wide area, one above the
//! other on a narrow one.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Two panes sharing one area in halves: side by side while the area is
/// at least [`threshold`](Self::threshold) cells wide, the first above
/// the second when it is narrower.
///
/// A host keeps one split for its screen and asks it for the two areas
/// wherever it needs them: to draw the panes, and to find one pane's
/// cells again afterwards (a dev tool that prints one pane, a check that
/// reads one). An odd width gives its odd cell to the first pane, and so
/// does an odd height when the panes stack.
///
/// ```
/// use ratatui::layout::Rect;
/// use tuikit::frame::SplitPane;
///
/// let split = SplitPane::new(100);
/// let [left, right] = split.areas(Rect::new(0, 1, 120, 30));
/// assert_eq!((left, right), (Rect::new(0, 1, 60, 30), Rect::new(60, 1, 60, 30)));
///
/// // Below the threshold the panes stack.
/// let narrow = Rect::new(0, 1, 80, 30);
/// assert!(split.stacks(narrow));
/// let [top, bottom] = split.areas(narrow);
/// assert_eq!((top, bottom), (Rect::new(0, 1, 80, 15), Rect::new(0, 16, 80, 15)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitPane {
    threshold: u16,
}

impl SplitPane {
    /// A split that puts its panes side by side on an area at least
    /// `threshold` cells wide and stacks them on a narrower one.
    pub const fn new(threshold: u16) -> Self {
        Self { threshold }
    }

    /// The narrowest area, in cells, whose panes sit side by side.
    pub const fn threshold(self) -> u16 {
        self.threshold
    }

    /// Whether the panes stack on `area`: it is narrower than the
    /// threshold.
    pub const fn stacks(self, area: Rect) -> bool {
        area.width < self.threshold
    }

    /// The two panes' areas in `area`: the first (left, or top when they
    /// stack) and the second, each half of it.
    pub fn areas(self, area: Rect) -> [Rect; 2] {
        let direction = if self.stacks(area) {
            Direction::Vertical
        } else {
            Direction::Horizontal
        };
        let halves = Layout::default()
            .direction(direction)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        [halves[0], halves[1]]
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
