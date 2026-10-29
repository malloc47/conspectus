//! The timer a palette over a slow provider waits on.

use std::time::{Duration, Instant};

/// When a palette whose results come from a slow provider (a scan, a
/// request) owes its host a new search: once the query has rested for
/// the delay after the last edit, or at once when the host forces it.
///
/// The palette marks it on every edit, scope change, and option change;
/// the host polls [`take_due`](Self::take_due) from its loop, starts the
/// search with what the palette shows (its query, scope, and options)
/// when it answers `true`, and hands the results back with
/// [`Palette::set_rows`](super::Palette::set_rows). Typing a word asks
/// for one search, not one per letter. The framework keeps only the
/// timer: what runs the search, and on which thread, is the host's.
///
/// ```
/// use std::time::{Duration, Instant};
/// use tuikit::palette::Debounce;
///
/// let mut debounce = Debounce::new(Duration::from_millis(200));
/// let edited = Instant::now();
/// debounce.mark(edited);
/// assert!(!debounce.take_due(edited, false), "still resting");
/// let later = edited + Duration::from_millis(250);
/// assert!(debounce.take_due(later, false));
/// assert!(!debounce.take_due(later, false), "asked once");
/// debounce.mark(later);
/// assert!(debounce.take_due(later, true), "forced");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Debounce {
    delay: Duration,
    edited_at: Option<Instant>,
}

impl Debounce {
    /// A timer that waits `delay` after the last edit, owing nothing yet.
    pub const fn new(delay: Duration) -> Self {
        Self {
            delay,
            edited_at: None,
        }
    }

    /// How long the query rests before a search is due.
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// Owe a search for an edit made at `at`; a later edit starts the wait
    /// again.
    pub fn mark(&mut self, at: Instant) {
        self.edited_at = Some(at);
    }

    /// Whether a search is owed, due or not.
    pub const fn is_pending(&self) -> bool {
        self.edited_at.is_some()
    }

    /// Whether a search is owed and the query has rested for the delay by
    /// `now`.
    pub fn is_due(&self, now: Instant) -> bool {
        self.edited_at
            .is_some_and(|at| now.saturating_duration_since(at) >= self.delay)
    }

    /// Whether the host should search now: `true` once per owed search,
    /// when it is due by `now` or when `force` skips the wait. Answering
    /// `true` pays the debt.
    pub fn take_due(&mut self, now: Instant, force: bool) -> bool {
        if self.edited_at.is_none() || !(force || self.is_due(now)) {
            return false;
        }
        self.edited_at = None;
        true
    }

    /// Owe nothing, as when the palette closes.
    pub fn cancel(&mut self) {
        self.edited_at = None;
    }
}
