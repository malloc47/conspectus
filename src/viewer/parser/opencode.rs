//! OpenCode SQLite transcript reader.
//!
//! Reads from `opencode.db` per ADR 0013. Single SQL query returns
//! the session's messages ordered by sequence; rows map to
//! `TranscriptTurn`s. The `session_diff/` directory is *not* read —
//! the SQLite store is authoritative on every observed layout.
//!
//! This is the path that closes the OpenCode coverage gap recall
//! could not (`H-TRANSCRIPT-014`).
//!
//! Filled by `H-VIEWER-NATIVE-005`.

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::SessionLocator;

pub struct OpenCodeParser;

impl HarnessParser for OpenCodeParser {
    fn supports(&self, locator: &SessionLocator) -> bool {
        matches!(locator, SessionLocator::OpenCode { .. })
    }

    fn read(&self, _locator: &SessionLocator) -> ParseResult {
        Err(ParseError::Malformed(
            "opencode parser not implemented (H-VIEWER-NATIVE-005)".to_string(),
        ))
    }
}
