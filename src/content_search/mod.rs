//! Search the contents of every session's transcript.
//!
//! This is the scan behind content search: given the sessions in a
//! snapshot and a `ContentQuery`, read each session's transcript where
//! the harness keeps it, match it with the shared matcher in
//! `crate::viewer::search`, and stream the sessions that match. It reads
//! files and databases and nothing else: it starts no process, opens no
//! socket, and writes nothing. Hits live in memory for as long as the
//! caller keeps them and are never persisted or sent to the daemon.
//!
//! - [`plan::plan_sessions`] pairs each agent session with its harness's
//!   locator and parser.
//! - [`scan::scan`] runs the scan on worker threads and streams
//!   [`ScanEvent`]s; [`scan::scan_blocking`] runs it to completion on the
//!   calling thread for the CLI and for snapshot mode.
//! - [`prefilter`] skips transcripts that can't match without parsing
//!   them. It is used only when the query searches text that is stored
//!   verbatim in the file: messages, not tool, thinking, hidden, or
//!   aborted content.

pub mod dedup;
pub mod panes;
pub mod plan;
pub mod prefilter;
pub mod scan;
pub mod source;
pub mod unified;

use crate::viewer::model::SessionLocator;
use crate::viewer::parser::HarnessParser;
use crate::viewer::search::SessionHits;

pub use panes::{Attribution, PaneSource, attribute_panes, sockets_in};
pub use plan::{Plan, plan_sessions};
pub use scan::{
    CancelToken, Polled, ScanHandle, ScanSummary, SessionDetail, hits_for, scan, scan_blocking,
    session_detail,
};
pub use source::{
    ContentResult, ContentSource, MAX_PANE_HITS_PER_ENTRY, MuxRef, PaneHit, PaneHits, PaneRef,
    SourceEvent, SourceKind, cmp_results, cmp_results_by_recency,
};
pub use unified::{
    ContentEvent, ContentHandle, ContentPolled, ContentSummary, SessionSource, search,
    search_blocking,
};

/// Per-session cap on hits materialized by [`hits_for`]. The count in
/// `SessionHits` is always the true number of matching turns.
pub const MAX_HITS_PER_SESSION: usize = 500;

/// Identifies a session across harnesses.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionKey {
    pub harness_key: String,
    pub session_key: String,
}

/// One session to scan: where its transcript is and who parses it.
#[derive(Clone)]
pub struct ScanSession {
    pub key: SessionKey,
    pub locator: SessionLocator,
    pub parser: &'static dyn HarnessParser,
}

/// Tuning for one scan.
#[derive(Clone, Copy, Debug)]
pub struct ScanOptions {
    /// Number of worker threads; 0 picks from the machine, capped at 8.
    pub workers: usize,
    /// Stamped on every event so a caller can drop stale results.
    pub generation: u64,
    /// Skip transcripts the byte-level prefilter rules out. On by
    /// default; turning it off parses everything, which a test or a
    /// benchmark uses as the reference.
    pub prefilter: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            workers: 0,
            generation: 0,
            prefilter: true,
        }
    }
}

/// Progress counters, reported with every event.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    /// Sessions looked at so far, including ones the prefilter ruled out.
    pub scanned: usize,
    /// Sessions to look at.
    pub total: usize,
    /// Sessions that couldn't be read: missing, unreadable, or malformed.
    pub skipped: usize,
}

/// What a running scan reports.
#[derive(Clone, Debug)]
pub enum ScanEvent {
    /// Counters moved.
    Progress { generation: u64, progress: Progress },
    /// A session matched.
    Session {
        generation: u64,
        key: SessionKey,
        hits: SessionHits,
    },
    /// The scan ended, finished or cancelled.
    Done {
        generation: u64,
        progress: Progress,
        matched: usize,
        cancelled: bool,
    },
}

impl ScanEvent {
    pub fn generation(&self) -> u64 {
        match self {
            Self::Progress { generation, .. }
            | Self::Session { generation, .. }
            | Self::Done { generation, .. } => *generation,
        }
    }
}

#[cfg(test)]
#[path = "dedup_tests.rs"]
mod dedup_tests;

#[cfg(test)]
#[path = "panes_tests.rs"]
mod panes_tests;

#[cfg(test)]
#[path = "unified_tests.rs"]
mod unified_tests;

#[cfg(test)]
#[path = "prefilter_tests.rs"]
mod prefilter_tests;

#[cfg(test)]
#[path = "scan_opencode_tests.rs"]
mod scan_opencode_tests;

#[cfg(test)]
#[path = "scan_robustness_tests.rs"]
mod scan_robustness_tests;

#[cfg(test)]
#[path = "scan_tests.rs"]
mod scan_tests;

#[cfg(test)]
mod test_support;
