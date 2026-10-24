//! What each viewer message does, as help text.
//!
//! The viewer's key table lives with the other overlay key tables in
//! the TUI, so the help page is built there by pairing each message's
//! description from here with the keys that table binds to it. A new
//! [`ViewerMsg`] has to be described here (the match is exhaustive), so
//! it cannot be left out of the help.

use crate::viewer::input::ViewerMsg;

/// A message's place in the help page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpEntry {
    /// The section heading it is listed under.
    pub section: &'static str,
    /// What it does, in one sentence.
    pub description: &'static str,
}

impl ViewerMsg {
    /// Every message, in the order the help page lists them.
    pub const ALL: [ViewerMsg; 17] = [
        ViewerMsg::ScrollDown,
        ViewerMsg::ScrollUp,
        ViewerMsg::PageDown,
        ViewerMsg::PageUp,
        ViewerMsg::HalfPageDown,
        ViewerMsg::HalfPageUp,
        ViewerMsg::JumpToStart,
        ViewerMsg::JumpToEnd,
        ViewerMsg::CycleToolDetail,
        ViewerMsg::ToggleThinking,
        ViewerMsg::ToggleAborted,
        ViewerMsg::ToggleHidden,
        ViewerMsg::FindOpen,
        ViewerMsg::FindNext,
        ViewerMsg::FindPrev,
        ViewerMsg::ToggleHelp,
        ViewerMsg::Close,
    ];

    /// Where this message is listed in the help, and what it does.
    pub fn help(self) -> HelpEntry {
        let (section, description) = match self {
            ViewerMsg::ScrollDown => ("Moving", "Scroll one line down"),
            ViewerMsg::ScrollUp => ("Moving", "Scroll one line up"),
            ViewerMsg::PageDown => ("Moving", "Page down"),
            ViewerMsg::PageUp => ("Moving", "Page up"),
            ViewerMsg::HalfPageDown => ("Moving", "Half a page down"),
            ViewerMsg::HalfPageUp => ("Moving", "Half a page up"),
            ViewerMsg::JumpToStart => ("Moving", "Jump to the start of the transcript"),
            ViewerMsg::JumpToEnd => ("Moving", "Jump to the end (where the viewer opens)"),
            ViewerMsg::CycleToolDetail => (
                "Showing more or less",
                "Cycle tool detail: hidden, summary, truncated, full",
            ),
            ViewerMsg::ToggleThinking => ("Showing more or less", "Show or hide thinking turns"),
            ViewerMsg::ToggleAborted => (
                "Showing more or less",
                "Show or hide aborted and interrupted turns",
            ),
            ViewerMsg::ToggleHidden => (
                "Showing more or less",
                "Show or hide records the harness's own UI never showed",
            ),
            ViewerMsg::FindOpen => (
                "Finding",
                "Find in transcript; a match in hidden content turns that content on",
            ),
            ViewerMsg::FindNext => ("Finding", "Next match, wrapping at the end"),
            ViewerMsg::FindPrev => ("Finding", "Previous match, wrapping at the start"),
            ViewerMsg::ToggleHelp => ("Viewer", "Show or hide this help"),
            ViewerMsg::Close => ("Viewer", "Close help, then the find, then the viewer"),
        };
        HelpEntry {
            section,
            description,
        }
    }
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
