//! Full-screen Ratatui modal for the transcript viewer.
//!
//! The widget consumes [`ViewerState`] and draws header / body /
//! footer regions. Per ADR 0052 the modal opens with the last turn
//! focused (initial scroll lands at the bottom); `g`/`Home` jumps
//! to start, `G`/`End` to end. Body rendering goes through
//! [`crate::viewer::render::render_turn`].
//!
//! Filled by `H-VIEWER-NATIVE-006`.

use crate::viewer::state::ViewerState;

/// Compute the scroll offset that lands the cursor on the last
/// turn, for the open-on-last-turn contract from ADR 0052.
/// Stub — `H-VIEWER-NATIVE-006` computes from the rendered line
/// table and the visible viewport height.
pub fn initial_scroll_for_last_turn(_state: &ViewerState, _viewport_height: u16) -> usize {
    0
}
