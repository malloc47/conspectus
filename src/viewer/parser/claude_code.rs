//! Claude Code JSONL transcript reader.
//!
//! Reads `<state_root>/projects/*/<session_key>.jsonl` and produces
//! a [`TranscriptDocument`]. The H-PREVIEW grammar
//! (`src/discovery/harness/claude_code.rs`) is the reference for
//! what to skip: tool_use, tool_result, thinking, system records,
//! compaction synthetic-summary user rows. The viewer reader
//! retains those records as distinct `TurnKind`s so the operator
//! can opt to show them.
//!
//! Filled by `H-VIEWER-NATIVE-003`.

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::SessionLocator;

pub struct ClaudeCodeParser;

impl HarnessParser for ClaudeCodeParser {
    fn supports(&self, locator: &SessionLocator) -> bool {
        matches!(locator, SessionLocator::ClaudeCode { .. })
    }

    fn read(&self, _locator: &SessionLocator) -> ParseResult {
        Err(ParseError::Malformed(
            "claude_code parser not implemented (H-VIEWER-NATIVE-003)".to_string(),
        ))
    }
}
