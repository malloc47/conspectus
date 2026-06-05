//! Mux pane-capture cache + orchestration for the right-panel
//! preview.
//!
//! v1 (P8-009 slice) keeps the cache in-process and refreshes on
//! selection change only. Throttling on a refresh interval, async
//! polling, and per-mux freshness markers come with `T8-007`'s
//! background data adapter.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::discovery::tmux::{TmuxCaptureOutcome, TmuxRunner};
use crate::model::MuxSessionId;

/// In-memory cache of recent capture-pane results keyed by mux id.
/// Plain data — no I/O, no locks — so the reducer can carry it
/// directly on [`crate::tui::app::App`].
#[derive(Debug, Default, Clone)]
pub struct PreviewStore {
    entries: BTreeMap<MuxSessionId, PreviewEntry>,
}

#[derive(Debug, Clone)]
pub struct PreviewEntry {
    pub content: PreviewContent,
    pub captured_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewContent {
    /// Plain text body (from `tmux capture-pane -p -J`).
    Text(String),
    /// The runner reported the target doesn't exist.
    NoTarget,
    /// The runner is unavailable (binary missing, no server).
    Unavailable(String),
    /// The runner returned an error.
    Failed(String),
    /// The runner doesn't support capture (e.g. test fakes).
    Unsupported,
}

impl PreviewStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, mux: &MuxSessionId) -> Option<&PreviewEntry> {
        self.entries.get(mux)
    }

    pub fn insert(&mut self, mux: MuxSessionId, content: PreviewContent) {
        self.entries.insert(
            mux,
            PreviewEntry {
                content,
                captured_at: Some(Instant::now()),
            },
        );
    }

    /// Test seam: insert with an explicit timestamp so deterministic
    /// reads in unit tests don't need a real `Instant`.
    #[cfg(test)]
    pub fn insert_at(&mut self, mux: MuxSessionId, content: PreviewContent, captured_at: Instant) {
        self.entries.insert(
            mux,
            PreviewEntry {
                content,
                captured_at: Some(captured_at),
            },
        );
    }
}

/// Run a single capture against the runner and translate it into a
/// [`PreviewContent`]. Pure (modulo the runner call); callers
/// supply the runner so tests inject [`crate::discovery::tmux::FakeTmux`].
pub fn capture_via(runner: &dyn TmuxRunner, native_id: &str) -> PreviewContent {
    // Today's preview path always queries the default socket;
    // non-default-socket previews are part of the deferred
    // H-PIN-F-001 discovery story.
    match runner.capture_pane(None, native_id) {
        Ok(TmuxCaptureOutcome::Captured(text)) => PreviewContent::Text(text),
        Ok(TmuxCaptureOutcome::NoTarget) => PreviewContent::NoTarget,
        Ok(TmuxCaptureOutcome::Unavailable(reason)) => {
            PreviewContent::Unavailable(reason.as_str().to_string())
        }
        Ok(TmuxCaptureOutcome::Failed { code, message }) => {
            let code_label = code.map(|c| format!(" (code {c})")).unwrap_or_default();
            PreviewContent::Failed(format!("tmux capture-pane failed{code_label}: {message}"))
        }
        Ok(TmuxCaptureOutcome::Unsupported) => PreviewContent::Unsupported,
        Err(err) => PreviewContent::Failed(format!("tmux capture-pane error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome, UnavailableReason};

    #[test]
    fn capture_via_text_passes_content_through() {
        let runner = FakeTmux::with_sessions("")
            .with_capture("editor", TmuxCaptureOutcome::Captured("hello".to_string()));
        assert_eq!(
            capture_via(&runner, "editor"),
            PreviewContent::Text("hello".to_string())
        );
    }

    #[test]
    fn capture_via_no_target_propagates() {
        let runner =
            FakeTmux::with_sessions("").with_capture("missing", TmuxCaptureOutcome::NoTarget);
        assert_eq!(capture_via(&runner, "missing"), PreviewContent::NoTarget);
    }

    #[test]
    fn capture_via_unsupported_when_runner_has_no_capture() {
        let runner = FakeTmux::with_sessions("");
        assert_eq!(
            capture_via(&runner, "anything"),
            PreviewContent::Unsupported
        );
    }

    #[test]
    fn capture_via_unavailable_includes_reason() {
        let runner = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::Unavailable(UnavailableReason::BinaryNotFound),
        );
        match capture_via(&runner, "editor") {
            PreviewContent::Unavailable(reason) => assert!(reason.contains("binary not found")),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn capture_via_failed_includes_code_and_message() {
        let runner = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::Failed {
                code: Some(2),
                message: "denied".to_string(),
            },
        );
        match capture_via(&runner, "editor") {
            PreviewContent::Failed(text) => {
                assert!(text.contains("code 2"));
                assert!(text.contains("denied"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn preview_store_round_trips() {
        use crate::model::MuxSessionId;
        let mut store = PreviewStore::new();
        let id = MuxSessionId::new("editor");
        assert!(store.get(&id).is_none());
        store.insert(id.clone(), PreviewContent::Text("hi".to_string()));
        let entry = store.get(&id).expect("present after insert");
        assert_eq!(entry.content, PreviewContent::Text("hi".to_string()));
        assert!(entry.captured_at.is_some());
    }
}
