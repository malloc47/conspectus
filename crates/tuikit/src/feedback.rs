//! Feedback: telling the user what happened, at the right volume.
//!
//! This is a layer 2 convention (house rule 11). A program reports what
//! its actions did through three channels, and every report has a
//! [`Severity`] (info, success, warning, error) that each channel styles
//! from the theme, so a failure never looks like a success:
//!
//! - **The status line** holds one message until the host clears it
//!   (usually on the next move). [`Feedback::span`] draws it in its
//!   severity's color.
//! - **Toasts** float at the bottom of the screen for a while and then go.
//!   [`Toasts`] keeps them, each with its own lifetime ([`Feedback::ttl`]:
//!   errors stay longest), and draws each in a bordered box whose border
//!   takes its severity's color.
//! - **The log** keeps what happened for later reading: a bounded,
//!   newest-last [`Log`] of [`Entry`]s that collapses a run of identical
//!   entries into one with a repeat count and counts the warnings and
//!   errors the user has not looked at yet. An entry carries a payload of
//!   the host's type (what it was about, what ran), so the log's
//!   machinery is the framework's and its meaning the host's.
//!
//! One [`Feedback`] (a severity, a text, and a lifetime) is what the
//! status line and a toast show and what a log entry is summarized by.
//! Which channel a report goes to is the host's choice; the convention is
//! a hint or a refusal on the status line, a short confirmation in a
//! toast, and the outcome of an operation in the log (with its summary on
//! the status line).
//!
//! The [`clipboard`] module is the last piece of feedback plumbing: it
//! writes an OSC 52 copy sequence to a sink the host supplies, since the
//! framework writes to nothing of its own.
//!
//! ```
//! use std::time::{Duration, Instant};
//!
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use tuikit::feedback::{Entry, Feedback, Log, Severity, Toasts};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//!
//! // The status line: the text in its severity's color.
//! let status = Feedback::warning("nothing to undo");
//! assert_eq!(status.span(&theme).style.fg, Some(theme.warning));
//!
//! // A toast: an error stays longer than a confirmation.
//! assert!(Severity::Error.ttl() > Severity::Success.ttl());
//! let now = Instant::now();
//! let mut toasts = Toasts::new();
//! toasts.show(Feedback::error("copy failed"), now);
//! let area = Rect::new(0, 0, 60, 12);
//! let mut buf = Buffer::empty(area);
//! toasts.render(area, &mut buf, &theme);
//! toasts.tick(now + Severity::Error.ttl());
//! assert!(toasts.is_empty(), "it went when its time was up");
//!
//! // The log: the host's payload rides along, repeats collapse, and the
//! // failures wait to be seen.
//! let mut log: Log<Option<&str>> = Log::new(100);
//! log.push(Entry::new(1_000, Severity::Success, "saved", Some("notes")));
//! log.push(Entry::new(1_010, Severity::Error, "refresh failed", None));
//! log.push(Entry::new(1_020, Severity::Error, "refresh failed", None));
//! assert_eq!(log.len(), 2);
//! assert_eq!(log.unseen(), 1);
//! let newest = log.newest_first().next().unwrap();
//! assert_eq!((newest.repeats, newest.at), (2, 1_020));
//! ```
//!
//! # What a host keeps
//!
//! Which severity each report has and which channel it takes; the words;
//! the payload type of its log and how an entry's detail reads; when the
//! status line clears; and the sink the clipboard writes to (usually the
//! terminal's output).

pub mod clipboard;
mod log;
mod toasts;

use std::time::Duration;

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::chip::Tone;
use crate::glyphs::marks;
use crate::theme::Theme;

pub use log::{Entry, Log};
pub use toasts::Toasts;

/// How much a report matters, from a note to a failure. The order runs
/// from least to most urgent, so `severity >= Severity::Warning` asks
/// whether it needs attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Worth noting, neither good nor bad news: a hint, or what a key
    /// would do.
    Info,
    /// Something worked: a confirmation.
    Success,
    /// Something needs attention but nothing failed: a refusal, or a
    /// fallback that was taken.
    Warning,
    /// Something failed.
    Error,
}

impl Severity {
    /// Every severity, least urgent first.
    pub const ALL: [Severity; 4] = [
        Severity::Info,
        Severity::Success,
        Severity::Warning,
        Severity::Error,
    ];

    /// The theme's color for this severity: the `info`, `success`,
    /// `warning`, or `error` token.
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Info => theme.info,
            Self::Success => theme.success,
            Self::Warning => theme.warning,
            Self::Error => theme.error,
        }
    }

    /// Text in this severity's color.
    pub fn style(self, theme: &Theme) -> Style {
        Style::default().fg(self.color(theme))
    }

    /// The chip tone of the same color, so a status chip and a toast of
    /// one severity agree.
    pub fn tone(self) -> Tone {
        match self {
            Self::Info => Tone::Info,
            Self::Success => Tone::Success,
            Self::Warning => Tone::Warning,
            Self::Error => Tone::Error,
        }
    }

    /// The mark a list of reports leads each row with: a bullet for a
    /// note, a check for a success, the warning triangle, and a cross for
    /// a failure. Each is one cell, so the text after it lines up.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Info => marks::BULLET,
            Self::Success => marks::OK,
            Self::Warning => marks::WARNING,
            Self::Error => marks::FAIL,
        }
    }

    /// Whether the user should be told about a report until they have
    /// looked at it: warnings and errors.
    pub fn needs_attention(self) -> bool {
        self >= Self::Warning
    }

    /// How long a toast of this severity stays on screen unless its
    /// feedback says otherwise: 1.5 seconds for a note or a confirmation,
    /// 3 for a warning, and 5 for an error, which is the one the user
    /// most needs to read in full.
    pub const fn ttl(self) -> Duration {
        match self {
            Self::Info | Self::Success => Duration::from_millis(1500),
            Self::Warning => Duration::from_secs(3),
            Self::Error => Duration::from_secs(5),
        }
    }
}

/// One report: how much it matters, what it says, and how long a toast
/// of it stays on screen.
///
/// The status line shows a feedback until the host clears it and ignores
/// `ttl`; a toast goes after `ttl`; a log entry is summarized by its
/// severity and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    /// How much it matters.
    pub severity: Severity,
    /// What it says, as one line; a toast wraps it.
    pub text: String,
    /// How long a toast of it stays on screen. The constructors take the
    /// severity's [`Severity::ttl`].
    pub ttl: Duration,
}

impl Feedback {
    /// A report of `severity` saying `text`, with the severity's lifetime.
    pub fn new(severity: Severity, text: impl Into<String>) -> Self {
        Self {
            severity,
            text: text.into(),
            ttl: severity.ttl(),
        }
    }

    /// A note: [`Severity::Info`].
    pub fn info(text: impl Into<String>) -> Self {
        Self::new(Severity::Info, text)
    }

    /// A confirmation: [`Severity::Success`].
    pub fn success(text: impl Into<String>) -> Self {
        Self::new(Severity::Success, text)
    }

    /// A refusal or a fallback: [`Severity::Warning`].
    pub fn warning(text: impl Into<String>) -> Self {
        Self::new(Severity::Warning, text)
    }

    /// A failure: [`Severity::Error`].
    pub fn error(text: impl Into<String>) -> Self {
        Self::new(Severity::Error, text)
    }

    /// The same report with a toast lifetime of its own.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// The text in its severity's color, as a status line draws it.
    pub fn span(&self, theme: &Theme) -> Span<'static> {
        Span::styled(self.text.clone(), self.severity.style(theme))
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
