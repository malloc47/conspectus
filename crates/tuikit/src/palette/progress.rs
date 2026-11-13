//! How far a slow provider has got, in the words a status row shows.

use crate::glyphs::marks;

/// How far a provider that reads many things (files, documents, pages)
/// has got: how many it has read of how many there are, how many it
/// could not read, and whether it has finished.
///
/// A palette fed by such a provider says so in its status row; the host
/// keeps the counts, and [`status`](Self::status) puts them in words with
/// the host's noun for what is read. Nothing is said before the provider
/// knows how many there are.
///
/// ```
/// use tuikit::palette::Progress;
///
/// let running = Progress { scanned: 34, total: 77, skipped: 2, done: false };
/// assert_eq!(running.status("files").as_deref(), Some("34/77 files · 2 skipped"));
/// let done = Progress { scanned: 77, total: 77, skipped: 0, done: true };
/// assert_eq!(done.status("files").as_deref(), Some("77 files searched"));
/// assert_eq!(Progress::default().status("files"), None);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Progress {
    /// How many have been read so far.
    pub scanned: usize,
    /// How many there are to read; 0 while that is not known.
    pub total: usize,
    /// How many could not be read.
    pub skipped: usize,
    /// Whether the provider has finished.
    pub done: bool,
}

impl Progress {
    /// The progress in words, `noun` naming what is read: `34/77 files`
    /// while it runs and `77 files searched` once it is done, then
    /// `· 2 skipped` when any could not be read. `None` while the total
    /// is not known.
    pub fn status(&self, noun: &str) -> Option<String> {
        if self.total == 0 {
            return None;
        }
        let mut text = if self.done {
            format!("{} {noun} searched", self.total)
        } else {
            format!("{}/{} {noun}", self.scanned, self.total)
        };
        if self.skipped > 0 {
            text.push_str(marks::SPACED_SEPARATOR);
            text.push_str(&format!("{} skipped", self.skipped));
        }
        Some(text)
    }
}
