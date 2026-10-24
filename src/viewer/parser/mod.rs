//! Per-harness transcript readers.
//!
//! Each implementor reads one harness's state-of-record into a
//! [`TranscriptDocument`]. The trait is provider-neutral so the
//! viewer (and the future extracted CLI) dispatches on
//! [`SessionLocator`] without harness-specific knowledge in the
//! widget layer.
//!
//! Parser lookup goes through the adapter registry via
//! [`crate::discovery::harness::HarnessAdapter::transcript_parser`],
//! so the caller already knows which parser it wants.

pub mod claude_code;
pub mod codex;
pub mod opencode;
pub mod pi;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::viewer::model::{SessionLocator, TranscriptDocument};

/// Read a session's full transcript from its on-disk shape.
pub trait HarnessParser: Send + Sync {
    /// Read the transcript. Errors degrade to a stub document
    /// with a "transcript unavailable" marker rather than failing
    /// hard, so the widget always has something to render.
    fn read(&self, locator: &SessionLocator) -> ParseResult;

    /// Resolve where every session under `state_root` lives, once, so
    /// a scan of many sessions doesn't re-walk the state root for each
    /// one. `state_root` is what the adapter put in
    /// [`SessionLocator::state_root`]. The default knows nothing, so a
    /// parser without an index is read one session at a time.
    fn index(&self, state_root: &Path) -> Box<dyn TranscriptIndex> {
        let _ = state_root;
        Box::new(MapIndex::default())
    }

    /// Read a transcript whose source [`TranscriptIndex::locate`]
    /// already resolved. The default ignores the source and walks
    /// again, as [`HarnessParser::read`] does.
    fn read_source(&self, source: &TranscriptSource, locator: &SessionLocator) -> ParseResult {
        let _ = source;
        self.read(locator)
    }

    /// Sessions under `state_root` that could contain every one of
    /// `runs` (lowercase ASCII fragments), for harnesses that keep
    /// sessions in a database rather than a file each. `None` means the
    /// parser can't narrow, so every session is a candidate. The answer
    /// may include sessions that don't match, but never omits one that
    /// does.
    fn candidates(
        &self,
        state_root: &Path,
        runs: &[String],
    ) -> Option<std::collections::HashSet<String>> {
        let _ = (state_root, runs);
        None
    }
}

/// Where one session's transcript is stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranscriptSource {
    /// A file holding exactly this session.
    File(PathBuf),
    /// A session inside a SQLite database.
    Sqlite { db: PathBuf, session: String },
}

/// Resolved transcript sources for one state root.
pub trait TranscriptIndex: Send + Sync {
    /// The source of `session_key`, `None` when it isn't there.
    fn locate(&self, session_key: &str) -> Option<TranscriptSource>;
}

/// A [`TranscriptIndex`] backed by a map from session key to source.
#[derive(Default)]
pub struct MapIndex(pub HashMap<String, TranscriptSource>);

impl TranscriptIndex for MapIndex {
    fn locate(&self, session_key: &str) -> Option<TranscriptSource> {
        self.0.get(session_key).cloned()
    }
}

/// An index for a state root that is itself one transcript file: every
/// session key resolves to it.
pub struct SingleFileIndex(pub PathBuf);

impl TranscriptIndex for SingleFileIndex {
    fn locate(&self, _session_key: &str) -> Option<TranscriptSource> {
        Some(TranscriptSource::File(self.0.clone()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("transcript file not found")]
    NotFound,
    #[error("transcript I/O failed: {0}")]
    Io(String),
    #[error("transcript parse failed: {0}")]
    Malformed(String),
}

pub type ParseResult = Result<TranscriptDocument, ParseError>;
