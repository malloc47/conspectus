//! The seam between content sources and everything that shows their
//! results.
//!
//! A [`ContentSource`] searches one kind of unit (a session transcript, a
//! pane's scrollback) and reports [`ContentResult`]s that say where the
//! match was found. The ranking here is shared, so a session and a pane
//! compare by the same rule wherever they are listed.

use std::cmp::Ordering;
use std::ops::Range;

use super::{CancelToken, Progress, ScanOptions, SessionKey};
use crate::viewer::search::{ContentQuery, SessionHits};

/// At most this many hits are kept per pane. The count on [`PaneHits`] is
/// always the true number of matching lines.
pub const MAX_PANE_HITS_PER_ENTRY: usize = 200;

/// Where a result was found.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// A session's transcript.
    Session,
    /// A mux pane's scrollback.
    Pane,
}

impl SourceKind {
    /// The word a result row and the JSON output carry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Pane => "pane",
        }
    }
}

/// A mux: one tmux session on one server.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MuxRef {
    pub backend: String,
    /// The server's socket name; `None` for the default server.
    pub socket: Option<String>,
    pub session: String,
}

impl MuxRef {
    /// The id the graph gives this mux (`MuxSessionId::native_id`):
    /// `tmux:work`, or `tmux:scratch:work` on a named socket.
    pub fn native_id(&self) -> String {
        match self.socket.as_deref() {
            Some(socket) => format!("{}:{socket}:{}", self.backend, self.session),
            None => format!("{}:{}", self.backend, self.session),
        }
    }
}

/// One pane of a mux.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PaneRef {
    pub mux: MuxRef,
    /// The server-wide pane id, such as `%3`.
    pub pane_id: String,
    pub window_index: String,
    pub pane_index: String,
    /// What the pane was running when it was listed.
    pub command: Option<String>,
}

/// One matching line of a pane's scrollback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneHit {
    /// The line's position in the captured scrollback, from 1.
    pub line: usize,
    /// The line, normalized and cropped around the match.
    pub snippet: String,
    /// The match within `snippet`, as bytes.
    pub highlight: Range<usize>,
}

/// What a scan found in one pane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneHits {
    pub pane: PaneRef,
    /// Matching lines after deduplication, before the keep cap.
    pub count: usize,
    /// The newest matches (lowest in the scrollback), at most
    /// [`MAX_PANE_HITS_PER_ENTRY`], in scrollback order.
    pub hits: Vec<PaneHit>,
    /// Matching lines dropped because the pane's session says the same
    /// thing.
    pub deduplicated: usize,
    /// When the pane was captured, in epoch seconds. A pane has no
    /// message times, so this is the time its hits sort by.
    pub captured_at: i64,
    /// The capture hit the line or byte cap, so older scrollback was
    /// not searched.
    pub capture_capped: bool,
}

/// One search result, from any source.
#[derive(Clone, Debug)]
pub enum ContentResult {
    Session { key: SessionKey, hits: SessionHits },
    Pane(PaneHits),
}

impl ContentResult {
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::Session { .. } => SourceKind::Session,
            Self::Pane(_) => SourceKind::Pane,
        }
    }

    /// How many matches this result holds.
    pub fn count(&self) -> usize {
        match self {
            Self::Session { hits, .. } => hits.count,
            Self::Pane(pane) => pane.count,
        }
    }

    /// When the newest match happened, in epoch seconds: a session's
    /// latest message time, or the time a pane was captured.
    pub fn time(&self) -> Option<i64> {
        match self {
            Self::Session { hits, .. } => hits.latest.map(|t| t.timestamp()),
            Self::Pane(pane) => Some(pane.captured_at),
        }
    }

    /// A string that names the origin and orders results with equal rank.
    pub fn origin(&self) -> String {
        match self {
            Self::Session { key, .. } => format!("{}\u{0}{}", key.harness_key, key.session_key),
            Self::Pane(pane) => format!("{}\u{0}{}", pane.pane.mux.native_id(), pane.pane.pane_id),
        }
    }
}

/// Most relevant first: more matches, then the newer, then sessions
/// before panes, then by origin.
pub fn cmp_results(a: &ContentResult, b: &ContentResult) -> Ordering {
    b.count()
        .cmp(&a.count())
        .then_with(|| b.time().cmp(&a.time()))
        .then_with(|| a.kind().cmp(&b.kind()))
        .then_with(|| a.origin().cmp(&b.origin()))
}

/// Newest first: the newer match, then more matches, then sessions
/// before panes, then by origin.
pub fn cmp_results_by_recency(a: &ContentResult, b: &ContentResult) -> Ordering {
    b.time()
        .cmp(&a.time())
        .then_with(|| b.count().cmp(&a.count()))
        .then_with(|| a.kind().cmp(&b.kind()))
        .then_with(|| a.origin().cmp(&b.origin()))
}

/// What a source reports while it runs.
#[derive(Clone, Debug)]
pub enum SourceEvent {
    /// Counters moved.
    Progress(Progress),
    /// A unit matched.
    Result(ContentResult),
}

/// Something that can be searched for content: it lists its own units,
/// reads each one, and reports matches. Later sources (PR bodies, commit
/// messages) implement the same trait.
pub trait ContentSource: Send + Sync {
    fn kind(&self) -> SourceKind;

    /// Search every unit, emitting progress and results, until done or
    /// `cancel` is set. Returns after the last event. Must not panic on
    /// bad data: a unit that cannot be read counts as skipped.
    fn run(
        &self,
        query: &ContentQuery,
        options: &ScanOptions,
        cancel: &CancelToken,
        emit: &(dyn Fn(SourceEvent) + Sync),
    );
}
