//! Key → state-transition reducer. Pure: takes a [`ViewerState`]
//! and a [`ViewerMsg`], returns a new [`ViewerState`]. Side effects
//! live in the bridge (`crate::tui::viewer_bridge`).
//!
//! Filled by `H-VIEWER-NATIVE-006`.

use crate::viewer::state::ViewerState;

/// Every state transition the viewer can perform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewerMsg {
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    JumpToStart,
    JumpToEnd,
    OpenSearch,
    SearchInput(char),
    SearchBackspace,
    CommitSearch,
    NextMatch,
    PrevMatch,
    Close,
}

/// Pure reducer. Returns the updated state plus a "close requested"
/// flag the runtime polls to dismiss the modal.
pub fn reduce(state: ViewerState, _msg: ViewerMsg) -> (ViewerState, bool) {
    (state, false)
}
