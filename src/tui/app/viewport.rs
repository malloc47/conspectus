//! What a drawn frame measured, kept for the next key and the next frame.
//!
//! A scrolled surface can only know its window once the layout has given
//! it an area: the tree's rows, the explorer's wrapped rows, and the
//! Preview's body all depend on the terminal's size.
//! The renderer works the window out from the App and the area it
//! measured (`tuikit::scroll`), draws it, and returns it in a
//! [`ViewportReport`]. The caller hands the report to
//! [`App::keep_viewports`] after the frame. Nothing is dispatched from
//! inside the draw: keeping a window changes no selection, focus, or arm,
//! so it is not a reducer transition. A scrolled modal (help, the value
//! modal, Messages) keeps the window it drew itself, as it draws.

use tuikit::scroll::{ScrollRegion, ScrollWindow};

use super::App;

/// The windows one frame drew, one per scrolled surface. `None` for a
/// surface the frame did not draw, whose kept window is left as it was.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportReport {
    /// The left pane's tree: one row per visible row.
    pub tree: Option<ScrollWindow>,
    /// The explorer zone of the right pane, over its wrapped rows.
    pub explorer: Option<ScrollWindow>,
    /// The Preview zone's scroll region as its body drew it.
    pub preview: Option<ScrollRegion>,
}

impl App {
    /// Keep what the last frame drew: each scrolled surface's window, so
    /// the next page key moves by the rows on screen and an offset the
    /// frame took back inside its content stays taken back.
    pub fn keep_viewports(&mut self, report: ViewportReport) {
        if let Some(window) = report.tree {
            self.tree_state.set_viewport(window);
        }
        if let Some(window) = report.explorer {
            self.explorer_window = window;
        }
        if let Some(region) = report.preview {
            // The Preview counts its offset from its anchor (the top, a
            // transcript's newest rows, or the search match it opened on);
            // the drawn region has it back inside the rows it showed.
            self.preview = region;
        }
    }

    /// The window the tree drew in the last frame, which the next frame
    /// starts from. Empty before the first frame.
    pub fn left_window(&self) -> ScrollWindow {
        self.tree_state.window()
    }

    /// The first tree row the last frame showed.
    pub fn left_scroll(&self) -> usize {
        self.tree_state.window().offset()
    }

    /// Rows the tree drew in the last frame, which a page key moves the
    /// selection by. Zero before the first frame.
    pub fn left_viewport(&self) -> u16 {
        u16::try_from(self.tree_state.window().viewport()).unwrap_or(u16::MAX)
    }

    /// The window the explorer zone drew in the last frame, over its
    /// wrapped rows. It scrolls apart from the Preview below it.
    pub fn explorer_window(&self) -> ScrollWindow {
        self.explorer_window
    }

    /// The first explorer row the last frame showed.
    pub fn explorer_scroll(&self) -> usize {
        self.explorer_window.offset()
    }
}
