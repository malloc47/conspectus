//! What the viewer's keys do, as help text.
//!
//! The viewer's help page is the framework pager's, generated from the
//! pager's key table and the transcript's own ([`TRANSCRIPT_KEYS`]). The
//! toggles say what they do here, in a section of their own (a new
//! [`TranscriptKey`] has to be described: the match is exhaustive), and
//! [`describe`] gives the viewer's words for the pager keys whose
//! meaning a transcript sharpens.
//!
//! [`TRANSCRIPT_KEYS`]: crate::viewer::keys::TRANSCRIPT_KEYS

use tuikit::help::{HelpAction, HelpEntry};
use tuikit::keys::PagerKey;

use crate::viewer::keys::TranscriptKey;

/// The section the toggles are listed in.
const SHOWING: &str = "Showing more or less";

impl HelpAction for TranscriptKey {
    const LISTED: &'static [Self] = &[
        TranscriptKey::Tools,
        TranscriptKey::Thinking,
        TranscriptKey::Aborted,
        TranscriptKey::Hidden,
    ];

    /// What each toggle does; they share a section of their own.
    fn help(self) -> HelpEntry {
        let description = match self {
            TranscriptKey::Tools => "Cycle tool detail: hidden, summary, truncated, full",
            TranscriptKey::Thinking => "Show or hide thinking turns",
            TranscriptKey::Aborted => "Show or hide aborted and interrupted turns",
            TranscriptKey::Hidden => "Show or hide records the harness's own UI never showed",
        };
        HelpEntry::new(SHOWING, description)
    }
}

/// The viewer's words for what a pager key does, where they say more
/// than the pager's: the ends of a transcript, and finding in one.
pub fn describe(key: PagerKey) -> Option<&'static str> {
    match key {
        PagerKey::Top => Some("Jump to the start of the transcript"),
        PagerKey::Bottom => Some("Jump to the end, where the viewer opens, and stay there"),
        PagerKey::Find => {
            Some("Find in transcript; a match in hidden content turns that content on")
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
