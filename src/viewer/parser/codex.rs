//! Codex JSONL transcript reader.
//!
//! Reads `<state_root>/sessions/YYYY/MM/DD/rollout-*-<session_key>.jsonl`
//! and produces a [`TranscriptDocument`]. Honors the channel-marker
//! filter from `H-PREVIEW-006` and the codex split state/log layout
//! documented in ADR 0048.
//!
//! Filled by `H-VIEWER-NATIVE-004`.

use super::{HarnessParser, ParseError, ParseResult};
use crate::viewer::model::SessionLocator;

pub struct CodexParser;

impl HarnessParser for CodexParser {
    fn supports(&self, locator: &SessionLocator) -> bool {
        matches!(locator, SessionLocator::Codex { .. })
    }

    fn read(&self, _locator: &SessionLocator) -> ParseResult {
        Err(ParseError::Malformed(
            "codex parser not implemented (H-VIEWER-NATIVE-004)".to_string(),
        ))
    }
}
