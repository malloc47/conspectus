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
mod tests {
    use super::*;
    use crate::viewer::model::{SessionLocator, TranscriptDocument};
    use crate::viewer::state::ToolDetail;
    use std::path::PathBuf;

    fn doc() -> TranscriptDocument {
        TranscriptDocument::unavailable(&SessionLocator {
            harness_key: "claude-code".to_string(),
            session_key: "k".to_string(),
            state_root: PathBuf::from("/x"),
        })
    }

    fn state_with(total: usize, viewport: u16, offset: usize) -> ViewerState {
        let mut s = ViewerState::new(doc());
        s.total_lines = total;
        s.viewport_height = viewport;
        s.scroll_offset = offset;
        s.stick_to_end = false; // these tests run after the first draw
        s
    }

    #[test]
    fn scroll_down_increments_and_clamps_to_max() {
        let s = state_with(50, 10, 38);
        let (s, eff) = reduce(s, ViewerMsg::ScrollDown);
        assert_eq!(s.scroll_offset, 39);
        assert_eq!(eff, ViewerEffect::None);
        let (s, _) = reduce(s, ViewerMsg::ScrollDown);
        assert_eq!(s.scroll_offset, 40, "max_scroll = 50 - 10 = 40 is the cap");
        let (s, _) = reduce(s, ViewerMsg::ScrollDown);
        assert_eq!(s.scroll_offset, 40, "saturates at max_scroll");
    }

    #[test]
    fn scroll_up_decrements_and_saturates_at_zero() {
        let s = state_with(50, 10, 1);
        let (s, _) = reduce(s, ViewerMsg::ScrollUp);
        assert_eq!(s.scroll_offset, 0);
        let (s, _) = reduce(s, ViewerMsg::ScrollUp);
        assert_eq!(s.scroll_offset, 0);
    }

    #[test]
    fn page_down_jumps_by_viewport_height() {
        let s = state_with(100, 24, 0);
        let (s, _) = reduce(s, ViewerMsg::PageDown);
        assert_eq!(s.scroll_offset, 24);
        let (s, _) = reduce(s, ViewerMsg::PageDown);
        assert_eq!(s.scroll_offset, 48);
    }

    #[test]
    fn half_page_jumps_by_half_viewport() {
        let s = state_with(100, 24, 0);
        let (s, _) = reduce(s, ViewerMsg::HalfPageDown);
        assert_eq!(s.scroll_offset, 12);
        let (s, _) = reduce(s, ViewerMsg::HalfPageUp);
        assert_eq!(s.scroll_offset, 0);
    }

    #[test]
    fn jump_to_start_zeros_offset() {
        let s = state_with(100, 24, 50);
        let (s, _) = reduce(s, ViewerMsg::JumpToStart);
        assert_eq!(s.scroll_offset, 0);
        assert!(!s.stick_to_end);
    }

    #[test]
    fn jump_to_end_pins_to_max_and_sticks() {
        let s = state_with(100, 24, 10);
        let (s, _) = reduce(s, ViewerMsg::JumpToEnd);
        assert_eq!(s.scroll_offset, 76);
        assert!(s.stick_to_end, "stick survives so re-layout stays glued");
    }

    #[test]
    fn other_navigation_clears_stick_to_end() {
        let mut s = state_with(100, 24, 76);
        s.stick_to_end = true;
        let (s, _) = reduce(s, ViewerMsg::ScrollUp);
        assert!(!s.stick_to_end);
    }

    #[test]
    fn cycle_tool_detail_walks_four_states_and_reclamps() {
        let s = state_with(100, 24, 80);
        let (s, _) = reduce(s, ViewerMsg::CycleToolDetail);
        assert_eq!(s.tool_detail, ToolDetail::Summary);
        assert!(s.scroll_offset <= s.max_scroll());
        let (s, _) = reduce(s, ViewerMsg::CycleToolDetail);
        assert_eq!(s.tool_detail, ToolDetail::Truncated);
        let (s, _) = reduce(s, ViewerMsg::CycleToolDetail);
        assert_eq!(s.tool_detail, ToolDetail::Full);
        let (s, _) = reduce(s, ViewerMsg::CycleToolDetail);
        assert_eq!(s.tool_detail, ToolDetail::Hidden);
    }

    #[test]
    fn toggle_thinking_flips_flag() {
        let s = state_with(100, 24, 0);
        let (s, _) = reduce(s, ViewerMsg::ToggleThinking);
        assert!(s.show_thinking);
    }

    #[test]
    fn toggle_aborted_flips_flag_and_invalidates_cache() {
        let s = state_with(100, 24, 50);
        assert!(!s.show_aborted, "off by default — matches harness UX");
        let (s, _) = reduce(s, ViewerMsg::ToggleAborted);
        assert!(s.show_aborted);
        assert!(
            s.rendered.is_none(),
            "render cache invalidated so the aborted filter takes effect on next draw"
        );
        let (s, _) = reduce(s, ViewerMsg::ToggleAborted);
        assert!(!s.show_aborted);
    }

    #[test]
    fn close_emits_close_effect() {
        let s = state_with(100, 24, 0);
        let (_s, eff) = reduce(s, ViewerMsg::Close);
        assert_eq!(eff, ViewerEffect::Close);
    }

    #[test]
    fn toggle_help_flips_show_help_flag() {
        let s = state_with(100, 24, 0);
        let (s, eff) = reduce(s, ViewerMsg::ToggleHelp);
        assert!(s.show_help);
        assert_eq!(eff, ViewerEffect::None);
        let (s, _) = reduce(s, ViewerMsg::ToggleHelp);
        assert!(!s.show_help);
    }

    #[test]
    fn close_dismisses_help_overlay_first_then_modal() {
        let mut s = state_with(100, 24, 0);
        s.show_help = true;
        let (s, eff) = reduce(s, ViewerMsg::Close);
        // First close dismisses the help panel and stays in the modal.
        assert!(!s.show_help);
        assert_eq!(eff, ViewerEffect::None);
        // Second close exits the modal.
        let (_s, eff) = reduce(s, ViewerMsg::Close);
        assert_eq!(eff, ViewerEffect::Close);
    }
}
