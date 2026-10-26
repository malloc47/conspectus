//! The content searches the Search view remembers (ADR 0149).
//!
//! Every content search the operator starts becomes the newest entry here
//! and stays until later searches push it out, so a new search does not
//! lose the one before it. Entries live in memory for the life of the
//! process and nowhere else: they hold transcript and scrollback text, so
//! like the search itself they are never saved, snapshotted, or sent to
//! the daemon.
//!
//! An entry is a [`ContentSearch`]: the live search copies itself into
//! its entry as results arrive (the maps inside are shared, not
//! duplicated), and paging to an older entry shows what it held when it
//! ended without scanning again.

use super::content::ContentSearch;
use crate::tui::widgets::search::Reach;

/// Snippets one entry may keep: one per matching session plus every
/// matching pane line.
pub const MAX_SNIPPETS_PER_ENTRY: usize = 2_000;

/// Snippets all entries together may keep.
pub const MAX_SNIPPETS_TOTAL: usize = 20_000;

/// One remembered search.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub search: ContentSearch,
    /// Started from the open search modal and not yet kept: typing more
    /// replaces it instead of adding another entry.
    pub draft: bool,
    /// The search was cut short (the modal closed, or the filter was
    /// cleared, before the scan ended).
    pub stopped: bool,
}

impl HistoryEntry {
    /// What the search was and found, for a list of them: the reach, how
    /// much it found, and how long ago.
    pub fn describe(&self, now: std::time::Instant) -> String {
        let mut parts = vec![
            match self.search.reach {
                Reach::Current => "Current",
                Reach::All => "All",
            }
            .to_string(),
            self.search.status_note(now),
        ];
        if self.stopped {
            parts.push("stopped".to_string());
        }
        parts.join(" · ")
    }
}

/// The remembered searches, oldest first, and the one the Search view
/// shows.
#[derive(Clone, Debug, Default)]
pub struct SearchHistory {
    entries: Vec<HistoryEntry>,
    /// Index of the entry the Search view shows.
    viewed: usize,
}

impl SearchHistory {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// The entry the Search view shows.
    pub fn viewed(&self) -> Option<&HistoryEntry> {
        self.entries.get(self.viewed)
    }

    pub fn viewed_mut(&mut self) -> Option<&mut HistoryEntry> {
        self.entries.get_mut(self.viewed)
    }

    /// Where the shown entry sits: its number from the oldest, and how
    /// many entries there are.
    pub fn position(&self) -> Option<(usize, usize)> {
        (!self.entries.is_empty()).then(|| (self.viewed + 1, self.entries.len()))
    }

    pub fn newest(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    pub fn by_serial(&self, serial: u64) -> Option<&HistoryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.search.serial == serial)
    }

    pub fn by_serial_mut(&mut self, serial: u64) -> Option<&mut HistoryEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.search.serial == serial)
    }

    /// Show the entry `delta` places newer (positive) or older, if there
    /// is one. Returns whether the shown entry changed.
    pub fn page(&mut self, delta: isize) -> bool {
        let next = self
            .viewed
            .checked_add_signed(delta)
            .filter(|next| *next < self.entries.len());
        match next {
            Some(next) if next != self.viewed => {
                self.viewed = next;
                true
            }
            _ => false,
        }
    }

    /// Show the entry at `index`. Returns whether the shown entry changed.
    pub fn show(&mut self, index: usize) -> bool {
        if index < self.entries.len() && index != self.viewed {
            self.viewed = index;
            true
        } else {
            false
        }
    }

    /// Add `search` as the newest entry and show it. A draft at the newest
    /// end is replaced. A search that is not a draft and equals one
    /// already remembered (same text, kinds, and scope, wherever it sits)
    /// takes that entry's place at the newest end instead of adding
    /// another. Older entries beyond `keep` are dropped, and so are the
    /// oldest ones while the snippets kept exceed [`MAX_SNIPPETS_TOTAL`].
    pub fn record(&mut self, search: ContentSearch, draft: bool, keep: usize) {
        if self.entries.last().is_some_and(|newest| newest.draft) {
            self.entries.pop();
        }
        if !draft {
            self.entries
                .retain(|entry| !same_search(&entry.search, &search));
        }
        self.entries.push(HistoryEntry {
            search,
            draft,
            stopped: false,
        });
        let keep = keep.max(1);
        if self.entries.len() > keep {
            self.entries.drain(..self.entries.len() - keep);
        }
        while self.entries.len() > 1 && self.snippets() > MAX_SNIPPETS_TOTAL {
            self.entries.remove(0);
        }
        self.viewed = self.entries.len() - 1;
    }

    /// Replace the entry for `search` with its newer copy, if it has one.
    pub fn update(&mut self, search: ContentSearch) {
        if let Some(entry) = self.by_serial_mut(search.serial) {
            entry.search = search;
        }
    }

    /// Searches started from a modal that has now closed are kept, and
    /// each replaces any older entry for the same search.
    pub fn keep_drafts(&mut self) {
        let kept: Vec<u64> = self
            .entries
            .iter()
            .filter(|entry| entry.draft)
            .map(|entry| entry.search.serial)
            .collect();
        for entry in &mut self.entries {
            entry.draft = false;
        }
        for serial in kept {
            self.drop_older_twins(serial);
        }
    }

    /// The entry with this serial is kept for good: no draft any more, and
    /// it replaces older entries for the same search.
    pub fn keep(&mut self, serial: u64) {
        if let Some(entry) = self.by_serial_mut(serial) {
            entry.draft = false;
            self.drop_older_twins(serial);
        }
    }

    /// Remove the other entries that are the same search as the one with
    /// this serial (drafts excepted: they are still being typed) and keep
    /// showing what was shown, or the survivor if that went.
    fn drop_older_twins(&mut self, serial: u64) {
        let Some(survivor) = self.by_serial(serial).map(|e| e.search.clone()) else {
            return;
        };
        let shown = self.viewed().map(|entry| entry.search.serial);
        self.entries.retain(|entry| {
            entry.search.serial == serial || entry.draft || !same_search(&entry.search, &survivor)
        });
        let follow = shown.unwrap_or(serial);
        self.viewed = self
            .entries
            .iter()
            .position(|entry| entry.search.serial == follow)
            .or_else(|| {
                self.entries
                    .iter()
                    .position(|entry| entry.search.serial == serial)
            })
            .unwrap_or(0);
    }

    /// Forget the draft with this serial (its query was emptied).
    pub fn remove_draft(&mut self, serial: u64) {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !(entry.draft && entry.search.serial == serial));
        if self.entries.len() != before {
            self.viewed = self.entries.len().saturating_sub(1);
        }
    }

    /// Snippets held across every entry.
    pub fn snippets(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.search.snippets())
            .sum()
    }

    /// Drop the oldest entries beyond `keep` (after the limit shrank).
    pub fn trim(&mut self, keep: usize) {
        let keep = keep.max(1);
        if self.entries.len() > keep {
            let drop = self.entries.len() - keep;
            self.entries.drain(..drop);
            self.viewed = self.viewed.saturating_sub(drop);
        }
    }
}

/// Whether two searches are the same one: the query text (trimmed), the
/// kinds searched, and the scope. How results are laid out is not part
/// of it.
fn same_search(a: &ContentSearch, b: &ContentSearch) -> bool {
    a.text.trim() == b.text.trim() && a.kinds == b.kinds && a.reach == b.reach
}
