//! The transcript's own keys, beside the pager's.
//!
//! The framework's pager (`tuikit::pager`, over `tuikit::keys::READER_KEYS`)
//! takes every key a reader shares: scrolling, paging, the ends, `/`
//! with `n`/`N`, `?`, `Esc`, `q`, and `Ctrl-C`. The transcript adds the
//! four toggles that show or hide kinds of turn; the pager resolves them
//! after its own table, so they can never take one of its keys. Each is
//! an alias: the toggle's chip in the footer names its key
//! (`tools·off (t)`), and the help page lists all four.

use tuikit::keys::{KeyAction, KeyRole, KeyRow, alias, ch};

/// A toggle of the transcript's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptKey {
    /// Cycle tool detail: hidden, summary, truncated, full (`t`).
    Tools,
    /// Show or hide thinking turns (`T`).
    Thinking,
    /// Show or hide the turns of an exchange the user interrupted (`I`,
    /// for "interrupted").
    Aborted,
    /// Show or hide records the harness's own interface never showed
    /// (`H`, ADR 0122).
    Hidden,
}

impl KeyAction for TranscriptKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Tools | Self::Thinking | Self::Aborted | Self::Hidden => KeyRole::Other,
        }
    }
}

/// The transcript's keys, which the pager resolves after its own.
pub const TRANSCRIPT_KEYS: &[KeyRow<TranscriptKey>] = &[
    alias(ch('t'), TranscriptKey::Tools),
    alias(ch('T'), TranscriptKey::Thinking),
    alias(ch('I'), TranscriptKey::Aborted),
    alias(ch('H'), TranscriptKey::Hidden),
];
