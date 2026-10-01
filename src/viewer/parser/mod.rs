//! Per-harness transcript readers.
//!
//! Each implementor reads one harness's state-of-record into a
//! [`TranscriptDocument`]. The trait is provider-neutral so the
//! viewer (and the future extracted CLI) dispatches on
//! [`SessionLocator`] without harness-specific knowledge in the
//! widget layer.
//!
//! Parser lookup now goes through the adapter
//! registry via
//! [`crate::discovery::harness::HarnessAdapter::transcript_parser`],
//! so the pre-H-EXT-006 `supports(&locator) -> bool` fan-out is
//! gone — the caller already knows which parser it wants.

pub mod claude_code;
pub mod codex;
pub mod opencode;

use crate::viewer::model::{SessionLocator, TranscriptDocument};

/// Read a session's full transcript from its on-disk shape.
pub trait HarnessParser: Send + Sync {
    /// Read the transcript. Errors degrade to a stub document
    /// with a "transcript unavailable" marker rather than failing
    /// hard, so the widget always has something to render.
    fn read(&self, locator: &SessionLocator) -> ParseResult;
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
