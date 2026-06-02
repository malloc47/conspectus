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

use crate::model::AgentSessionId;
use crate::viewer::model::{SessionLocator, TranscriptDocument};
use crate::viewer::parser::HarnessParser;
use crate::viewer::parser::claude_code::ClaudeCodeParser;
use crate::viewer::parser::codex::CodexParser;
use crate::viewer::parser::opencode::OpenCodeParser;
use crate::viewer::state::ViewerState;

/// Map a graph-flavored [`AgentSessionId`] to a viewer-flavored
/// [`SessionLocator`], or `None` when the harness key has no
/// registered native parser in v1.
pub fn locator_for_session(session: &AgentSessionId) -> Option<SessionLocator> {
    match session.harness_key.as_str() {
        "claude-code" => Some(SessionLocator::ClaudeCode {
            state_root: session.state_scope.clone().into(),
            session_key: session.session_key.clone(),
        }),
        "codex" => Some(SessionLocator::Codex {
            state_root: session.state_scope.clone().into(),
            session_key: session.session_key.clone(),
        }),
        "opencode" => {
            // OpenCode's `state_scope` from the discovery layer is
            // the parent directory containing `opencode.db`
            // (ADR 0013). Append the canonical file name to derive
            // the SQLite path.
            let mut db_path = std::path::PathBuf::from(session.state_scope.clone());
            // If state_scope already names the .db file, keep it;
            // otherwise treat it as the parent directory.
            if db_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("db"))
                != Some(true)
            {
                db_path.push("opencode.db");
            }
            Some(SessionLocator::OpenCode {
                db_path,
                session_id: session.session_key.clone(),
            })
        }
        _ => None,
    }
}

/// Resolve a session into a [`ViewerState`] ready for the modal.
/// Parser failures degrade to a `TranscriptDocument::unavailable`
/// document so the widget can always render a coherent banner
/// instead of bubbling the error up.
pub fn build_viewer_state(session: &AgentSessionId) -> Option<ViewerState> {
    let locator = locator_for_session(session)?;
    let document = read_with_appropriate_parser(&locator)
        .unwrap_or_else(|| TranscriptDocument::unavailable(&locator));
    Some(ViewerState::new(document))
}

fn read_with_appropriate_parser(locator: &SessionLocator) -> Option<TranscriptDocument> {
    // Backends in fixed order; the first one whose `supports()`
    // returns true tries to read. Mirrors the dispatch shape the
    // extracted crate's CLI will use.
    let backends: [&dyn HarnessParser; 3] = [&ClaudeCodeParser, &CodexParser, &OpenCodeParser];
    for backend in backends {
        if backend.supports(locator) {
            return backend.read(locator).ok();
        }
    }
    None
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
        match locator_for_session(&id) {
            Some(SessionLocator::ClaudeCode {
                state_root,
                session_key,
            }) => {
                assert_eq!(state_root, std::path::PathBuf::from("/u/.claude"));
                assert_eq!(session_key, "abc");
            }
            other => panic!("expected ClaudeCode, got {other:?}"),
        }
    }

    #[test]
    fn codex_session_maps_to_codex_locator() {
        let id = session("codex", "/u/.codex", "abc");
        match locator_for_session(&id) {
            Some(SessionLocator::Codex { .. }) => {}
            other => panic!("expected Codex, got {other:?}"),
        }
    }

    #[test]
    fn opencode_session_treats_state_scope_as_parent_dir() {
        let id = session("opencode", "/u/.local/share/opencode", "ses_x");
        match locator_for_session(&id) {
            Some(SessionLocator::OpenCode {
                db_path,
                session_id,
            }) => {
                assert_eq!(
                    db_path,
                    std::path::PathBuf::from("/u/.local/share/opencode/opencode.db")
                );
                assert_eq!(session_id, "ses_x");
            }
            other => panic!("expected OpenCode, got {other:?}"),
        }
    }

    #[test]
    fn opencode_session_with_db_filename_in_state_scope_uses_it_directly() {
        let id = session("opencode", "/some/path/opencode.db", "ses_x");
        match locator_for_session(&id) {
            Some(SessionLocator::OpenCode { db_path, .. }) => {
                assert_eq!(db_path, std::path::PathBuf::from("/some/path/opencode.db"));
            }
            other => panic!("expected OpenCode, got {other:?}"),
        }
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
}
