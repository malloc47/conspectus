//! Key → state-transition reducer. Pure: takes a [`ViewerState`]
//! and a [`ViewerMsg`], returns a new [`ViewerState`] plus an
//! optional [`ViewerEffect`] that signals the surrounding runtime
//! to close the modal. Side effects (terminal IO, exec) live
//! outside.
//!
//! Page-and-half-page navigation reads `state.viewport_height`,
//! which the widget writes during draw. The runtime drives one
//! draw between every key event so the height is always fresh.

use crate::viewer::state::ViewerState;

/// Every state transition the viewer can perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerMsg {
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    JumpToStart,
    JumpToEnd,
    /// Cycle tool detail through Hidden → Summary → Truncated →
    /// Full → Hidden. Bound to `t` in the runtime.
    CycleToolDetail,
    ToggleThinking,
    /// Show / hide turns the parser tagged as aborted. Bound to
    /// capital-`I` (Shift-i, for "interrupted"). Off by default so
    /// the viewer matches what the user saw in the agent's UI at
    /// chat time.
    ToggleAborted,
    ToggleHelp,
    Close,
}

/// What the runtime should do after applying the reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerEffect {
    /// Stay in the viewer; redraw.
    None,
    /// Dismiss the modal and return to the row tree.
    Close,
}

/// Apply one message to the state. Returns the new state and the
/// effect for the runtime.
pub fn reduce(mut state: ViewerState, msg: ViewerMsg) -> (ViewerState, ViewerEffect) {
    // Any explicit nav clears the "open at end" sticky flag. The
    // exception is `JumpToEnd`, which sets it back.
    if !matches!(msg, ViewerMsg::JumpToEnd) {
        state.stick_to_end = false;
    }

    match msg {
        ViewerMsg::ScrollUp => {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
        }
        ViewerMsg::ScrollDown => {
            state.scroll_offset = state
                .scroll_offset
                .saturating_add(1)
                .min(state.max_scroll());
        }
        ViewerMsg::PageUp => {
            let delta = state.viewport_height.max(1) as usize;
            state.scroll_offset = state.scroll_offset.saturating_sub(delta);
        }
        ViewerMsg::PageDown => {
            let delta = state.viewport_height.max(1) as usize;
            state.scroll_offset = state
                .scroll_offset
                .saturating_add(delta)
                .min(state.max_scroll());
        }
        ViewerMsg::HalfPageUp => {
            let delta = (state.viewport_height.max(2) as usize) / 2;
            state.scroll_offset = state.scroll_offset.saturating_sub(delta);
        }
        ViewerMsg::HalfPageDown => {
            let delta = (state.viewport_height.max(2) as usize) / 2;
            state.scroll_offset = state
                .scroll_offset
                .saturating_add(delta)
                .min(state.max_scroll());
        }
        ViewerMsg::JumpToStart => {
            state.scroll_offset = 0;
        }
        ViewerMsg::JumpToEnd => {
            // stick_to_end was already kept by the guard above.
            state.stick_to_end = true;
            state.scroll_offset = state.max_scroll();
        }
        ViewerMsg::CycleToolDetail => {
            state.tool_detail = state.tool_detail.cycle();
            // Layout changed — re-clamp on next draw and drop the
            // cached body lines so the widget recomposes.
            state.scroll_offset = state.scroll_offset.min(state.max_scroll());
            state.invalidate_render_cache();
        }
        ViewerMsg::ToggleThinking => {
            state.show_thinking = !state.show_thinking;
            state.scroll_offset = state.scroll_offset.min(state.max_scroll());
            state.invalidate_render_cache();
        }
        ViewerMsg::ToggleAborted => {
            state.show_aborted = !state.show_aborted;
            state.scroll_offset = state.scroll_offset.min(state.max_scroll());
            state.invalidate_render_cache();
        }
        ViewerMsg::ToggleHelp => {
            state.show_help = !state.show_help;
        }
        ViewerMsg::Close => {
            // Close the help overlay first when it's up; only
            // dismiss the modal when the body is showing.
            if state.show_help {
                state.show_help = false;
                return (state, ViewerEffect::None);
            }
            return (state, ViewerEffect::Close);
        }
    }
    (state, ViewerEffect::None)
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
