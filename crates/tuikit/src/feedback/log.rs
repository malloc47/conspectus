//! The log: what happened, kept for reading later.

use std::collections::VecDeque;

use super::{Feedback, Severity};

/// One thing that happened: when, how much it matters, a one-line
/// summary, and the host's payload of type `T` (what it was about, what
/// ran, what it printed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<T> {
    /// When it happened, in seconds since the Unix epoch by the host's
    /// clock. A repeat moves it to the latest time.
    pub at: i64,
    /// How much it matters.
    pub severity: Severity,
    /// One line: what a list of entries and a status line show.
    pub summary: String,
    /// The host's part of the entry.
    pub item: T,
    /// How many times it was logged back to back; see [`Log::push`].
    pub repeats: u32,
}

impl<T> Entry<T> {
    /// An entry logged once, at `at`.
    pub fn new(at: i64, severity: Severity, summary: impl Into<String>, item: T) -> Self {
        Self {
            at,
            severity,
            summary: summary.into(),
            item,
            repeats: 1,
        }
    }

    /// The entry's severity and summary, as the status line or a toast
    /// would show it.
    pub fn feedback(&self) -> Feedback {
        Feedback::new(self.severity, self.summary.clone())
    }
}

/// A bounded, newest-last log of [`Entry`]s, and a count of the ones that
/// need attention and have not been seen.
///
/// The log keeps its last `capacity` entries, dropping the oldest. An
/// entry equal to the newest one apart from its time (a refresh that
/// keeps failing the same way) does not add a row: it moves the newest
/// one's time forward and counts a repeat, so a failure that recurs
/// every few seconds stays one entry and is one unseen failure. Warnings
/// and errors count as unseen until [`Log::mark_seen`], which a host calls
/// when the user opens the log.
///
/// The log lives in memory only. Whether entries may ever be written
/// anywhere is the host's decision; a payload often holds program output.
#[derive(Clone, Debug)]
pub struct Log<T> {
    entries: VecDeque<Entry<T>>,
    capacity: usize,
    unseen: usize,
}

impl<T> Default for Log<T> {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

impl<T> Log<T> {
    /// How many entries a default log keeps.
    pub const DEFAULT_CAPACITY: usize = 200;

    /// An empty log that keeps the last `capacity` (at least one)
    /// entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
            unseen: 0,
        }
    }

    /// How many entries the log keeps before dropping the oldest.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many entries the log holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log holds no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries, newest first.
    pub fn newest_first(&self) -> impl Iterator<Item = &Entry<T>> {
        self.entries.iter().rev()
    }

    /// The newest entry that `matches`, if any.
    pub fn newest_where(&self, matches: impl Fn(&Entry<T>) -> bool) -> Option<&Entry<T>> {
        self.newest_first().find(|entry| matches(entry))
    }

    /// Warnings and errors logged since [`Log::mark_seen`]. A repeat does
    /// not count again.
    pub fn unseen(&self) -> usize {
        self.unseen
    }

    /// The user has seen the log: nothing in it is unseen any more.
    pub fn mark_seen(&mut self) {
        self.unseen = 0;
    }
}

impl<T: PartialEq> Log<T> {
    /// Append `entry`, or count it as a repeat of the newest entry when
    /// the two are equal apart from their times.
    pub fn push(&mut self, entry: Entry<T>) {
        if let Some(last) = self.entries.back_mut()
            && last.severity == entry.severity
            && last.summary == entry.summary
            && last.item == entry.item
        {
            last.at = entry.at;
            last.repeats = last.repeats.saturating_add(1);
            return;
        }
        if entry.severity.needs_attention() {
            self.unseen += 1;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
