use super::*;
use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome, UnavailableReason};

#[test]
fn capture_via_text_passes_content_through() {
    let runner = FakeTmux::with_sessions("")
        .with_capture("editor", TmuxCaptureOutcome::captured("hello".to_string()));
    assert_eq!(
        capture_via(&runner, "editor"),
        PreviewContent::text("hello")
    );
}

#[test]
fn capture_via_no_target_propagates() {
    let runner = FakeTmux::with_sessions("").with_capture("missing", TmuxCaptureOutcome::NoTarget);
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
    let id = PreviewSource::Pane(MuxSessionId::new("editor"));
    assert!(store.get(&id).is_none());
    store.insert(id.clone(), PreviewContent::text("hi"));
    let entry = store.get(&id).expect("present after insert");
    assert_eq!(entry.content, PreviewContent::text("hi"));
    assert!(entry.captured_at.is_some());
}

fn transcript(key: &str) -> PreviewSource {
    PreviewSource::Transcript(crate::model::AgentSessionId::new("codex", "/state", key))
}

#[test]
fn preview_store_tracks_loads_and_stale_marks() {
    let mut store = PreviewStore::new();
    let source = transcript("a");
    assert!(store.start_loading(source.clone()));
    assert!(!store.start_loading(source.clone()), "already in flight");
    store.insert(
        source.clone(),
        PreviewContent::Unavailable("gone".to_string()),
    );
    assert!(!store.is_loading(&source), "landing ends the load");

    store.mark_stale(&source);
    assert!(store.get(&source).expect("kept").stale);
    store.insert(
        source.clone(),
        PreviewContent::Unavailable("gone".to_string()),
    );
    assert!(!store.get(&source).expect("kept").stale);
}

#[test]
fn preview_store_keeps_the_most_recent_transcripts() {
    let mut store = PreviewStore::new();
    let start = Instant::now();
    let pane = PreviewSource::Pane(MuxSessionId::new("editor"));
    store.insert_at(pane.clone(), PreviewContent::text("hi"), start);
    for n in 0..=MAX_CACHED_TRANSCRIPTS {
        store.insert_at(
            transcript(&n.to_string()),
            PreviewContent::Unavailable("gone".to_string()),
            start + std::time::Duration::from_secs(n as u64 + 1),
        );
    }
    assert!(store.get(&transcript("0")).is_none(), "the oldest goes");
    assert!(
        store
            .get(&transcript(&MAX_CACHED_TRANSCRIPTS.to_string()))
            .is_some()
    );
    assert!(store.get(&pane).is_some(), "pane captures don't count");
}
