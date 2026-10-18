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
