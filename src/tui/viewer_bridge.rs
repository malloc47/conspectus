//! Bridge between the conspectus TUI (graph-aware) and the
//! extraction-ready native viewer (`src/viewer/`, graph-agnostic).
//!
//! This file lives **outside** the `src/viewer/` subtree on purpose
//! (ADR 0052 §"Extraction-ready boundary" rule 2): it's the only
//! place in conspectus that knows both the graph-flavored
//! `AgentSessionId` and the viewer-flavored `SessionLocator`. When
//! the viewer is extracted to a standalone crate, this file is the
//! glue that stays behind — extracted-crate users wire up their own
//! `SessionLocator` from clap arguments without ever touching
//! conspectus graph types.
//!
//! Harness dispatch goes through the adapter registry.
//! `locator_for_session` iterates registered adapters and asks each
//! for a `transcript_source`; the first non-`None` answer wins.
//! `build_viewer_state` grabs the same adapter's `transcript_parser`
//! and delegates the read. Aider (and every future harness that
//! declines `transcript_source`) drops through both functions with
//! `None` so the escape-hatch external viewer takes over.

use crate::discovery::harness::registered_adapters;
use crate::model::AgentSessionId;
use crate::viewer::model::{SessionLocator, TranscriptDocument};
use crate::viewer::parser::ParseError;
use crate::viewer::state::ViewerState;

/// Map a graph-flavored [`AgentSessionId`] to a viewer-flavored
/// `SessionLocator`, or `None` when the harness key has no
/// registered native transcript source.
pub fn locator_for_session(session: &AgentSessionId) -> Option<SessionLocator> {
    registered_adapters()
        .find(|a| a.harness_key() == session.harness_key)
        .and_then(|a| a.transcript_source(session))
}

/// Resolve a session into a `ViewerState` ready for the modal.
/// Parser failures degrade to a `TranscriptDocument::unavailable`
/// document so the widget can always render a coherent banner
/// instead of bubbling the error up.
pub fn build_viewer_state(session: &AgentSessionId) -> Option<ViewerState> {
    let adapter = registered_adapters().find(|a| a.harness_key() == session.harness_key)?;
    let locator = adapter.transcript_source(session)?;
    let document = adapter
        .transcript_parser()
        .and_then(|parser| parser.read(&locator).ok())
        .unwrap_or_else(|| TranscriptDocument::unavailable(&locator));
    Some(ViewerState::new(document))
}

/// Why a session's transcript has nothing to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptMissing {
    /// There is nothing to read: the viewer has no parser for the
    /// harness, or the transcript file is gone.
    Unavailable(String),
    /// The transcript is there, but reading or parsing it failed.
    Failed(String),
}

/// Read a session's transcript for the inline preview (ADR 0112).
/// Unlike [`build_viewer_state`], which degrades every failure to an
/// empty document for the modal's banner, this keeps the reason so the
/// preview can say why there is nothing to show.
pub fn load_transcript(session: &AgentSessionId) -> Result<TranscriptDocument, TranscriptMissing> {
    let unsupported = || {
        TranscriptMissing::Unavailable(format!(
            "the viewer can't read {} sessions",
            session.harness_key
        ))
    };
    let adapter = registered_adapters()
        .find(|a| a.harness_key() == session.harness_key)
        .ok_or_else(unsupported)?;
    let locator = adapter.transcript_source(session).ok_or_else(unsupported)?;
    let parser = adapter.transcript_parser().ok_or_else(unsupported)?;
    parser.read(&locator).map_err(|err| match err {
        ParseError::NotFound => TranscriptMissing::Unavailable(err.to_string()),
        ParseError::Io(_) | ParseError::Malformed(_) => TranscriptMissing::Failed(err.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(harness: &str, state_scope: &str, session_key: &str) -> AgentSessionId {
        AgentSessionId::new(harness, state_scope, session_key)
    }

    #[test]
    fn claude_code_session_maps_to_claude_code_locator() {
        let id = session("claude-code", "/u/.claude", "abc");
        let locator = locator_for_session(&id).expect("claude-code has a locator");
        assert_eq!(locator.harness_key, "claude-code");
        assert_eq!(locator.session_key, "abc");
        assert_eq!(locator.state_root, std::path::PathBuf::from("/u/.claude"));
    }

    #[test]
    fn codex_session_maps_to_codex_locator() {
        let id = session("codex", "/u/.codex", "abc");
        let locator = locator_for_session(&id).expect("codex has a locator");
        assert_eq!(locator.harness_key, "codex");
    }

    #[test]
    fn opencode_session_treats_state_scope_as_parent_dir() {
        let id = session("opencode", "/u/.local/share/opencode", "ses_x");
        let locator = locator_for_session(&id).expect("opencode has a locator");
        assert_eq!(locator.harness_key, "opencode");
        assert_eq!(locator.session_key, "ses_x");
        assert_eq!(
            locator.state_root,
            std::path::PathBuf::from("/u/.local/share/opencode/opencode.db")
        );
    }

    #[test]
    fn opencode_session_with_db_filename_in_state_scope_uses_it_directly() {
        let id = session("opencode", "/some/path/opencode.db", "ses_x");
        let locator = locator_for_session(&id).expect("opencode has a locator");
        assert_eq!(
            locator.state_root,
            std::path::PathBuf::from("/some/path/opencode.db")
        );
    }

    #[test]
    fn pi_session_maps_to_pi_locator() {
        let id = session("pi", "/u/.pi/agent/sessions", "abc");
        let locator = locator_for_session(&id).expect("pi has a locator");
        assert_eq!(locator.harness_key, "pi");
        assert_eq!(locator.session_key, "abc");
        assert_eq!(
            locator.state_root,
            std::path::PathBuf::from("/u/.pi/agent/sessions")
        );
    }

    #[test]
    fn unknown_harness_has_no_locator() {
        let id = session("aider", "/u", "k");
        assert!(locator_for_session(&id).is_none());
    }

    #[test]
    fn build_viewer_state_returns_none_for_unsupported_harness() {
        let id = session("aider", "/u", "k");
        assert!(build_viewer_state(&id).is_none());
    }

    #[test]
    fn build_viewer_state_yields_unavailable_doc_when_file_absent() {
        // Claude harness key + a state root that doesn't exist on
        // disk → parser errors with NotFound → bridge falls back
        // to TranscriptDocument::unavailable rather than None.
        let id = session("claude-code", "/nonexistent/.claude", "abc");
        let state = build_viewer_state(&id).expect("supported harness yields a state");
        assert!(state.document.is_empty(), "fallback doc has no turns");
        assert_eq!(state.document.meta.harness, "claude-code");
        assert_eq!(state.document.meta.session_key, "abc");
    }

    #[test]
    fn load_transcript_says_the_viewer_cannot_read_an_unsupported_harness() {
        let id = session("aider", "/u", "k");
        assert_eq!(
            load_transcript(&id),
            Err(TranscriptMissing::Unavailable(
                "the viewer can't read aider sessions".to_string()
            ))
        );
    }

    #[test]
    fn load_transcript_reports_a_missing_file_as_unavailable() {
        let id = session("claude-code", "/nonexistent/.claude", "abc");
        assert_eq!(
            load_transcript(&id),
            Err(TranscriptMissing::Unavailable(
                "transcript file not found".to_string()
            ))
        );
    }

    #[test]
    fn load_transcript_parses_a_transcript_on_disk() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("projects").join("-work");
        std::fs::create_dir_all(&project).expect("project dir");
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/claude/transcript-basic.jsonl"
            ),
            project.join("abc.jsonl"),
        )
        .expect("copy fixture transcript");
        let id = session("claude-code", &root.path().display().to_string(), "abc");
        let document = load_transcript(&id).expect("transcript parses");
        assert!(!document.is_empty());
    }
}
