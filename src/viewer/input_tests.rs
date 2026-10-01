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
