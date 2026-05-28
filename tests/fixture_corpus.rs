//! Fixture corpus loader tests (TEST-002).
//!
//! These tests parse the checked-in fixtures under `tests/fixtures/` through
//! the same adapter, hook, and parser paths that discovery uses. The goal is
//! to catch schema drift between what the synthetic builders produce and what
//! real-world provider data looks like.
//!
//! ## Sanitization workflow
//!
//! When capturing a new fixture from live state:
//!
//! 1. Copy the raw file to a scratch location outside the repo.
//! 2. Replace every occurrence of your real home directory with `$HOME` (or
//!    `/home/user` for checked-in fixtures — the tests normalize to that).
//! 3. Replace host-specific UUIDs/git SHAs with deterministic placeholders
//!    using `sed` or a similar tool.  Preserve UUID structure (8-4-4-4-12 hex
//!    chars) so the parsers still see valid shapes.
//! 4. Replace prompt/message content with short, descriptive placeholder text.
//!    Keep enough structure to exercise the parser paths (e.g. tool-use
//!    content blocks, multi-line messages).
//! 5. Strip any tokens, API keys, or secrets entirely — replace with the
//!    literal string `<REDACTED>`.
//! 6. Review the result: does it still match the provider's schema shape?
//!    Are any edge-case fields (null, missing, empty-string, deeply-nested)
//!    preserved?
//! 7. Place the file under `tests/fixtures/<provider>/<name>.<ext>` and add a
//!    loader test below.

use std::fs;

use conspectus::discovery::harness::{ClaudeCodeAdapter, CodexAdapter, HarnessAdapter};
use conspectus::discovery::tmux::parse_list_sessions;
use conspectus::discovery::{DiscoveryContext, GraphFragment};
use conspectus::hook::{
    HookTmuxRecord, claude_code_record_from_payload, codex_record_from_payload,
};
use conspectus::model::GraphNode;
use tempfile::TempDir;

mod support;

#[track_caller]
fn codepath() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(relative: &str) -> std::path::PathBuf {
    codepath().join("tests/fixtures").join(relative)
}

// ── Codex fixtures ──────────────────────────────────────────────────────

#[test]
fn codex_full_rollout_discovers_session_with_cwd() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("codex");
    let sessions = state_root
        .join("sessions")
        .join("2026")
        .join("05")
        .join("26");
    fs::create_dir_all(&sessions).expect("create sessions dir");
    fs::copy(
        fixture_path("codex/rollout-full.jsonl"),
        sessions.join("rollout-aaaaaaaa-1111-2222-3333-444444444444.jsonl"),
    )
    .expect("copy fixture");

    let context = DiscoveryContext::default()
        .with_harness_state_root(CodexAdapter::new().harness_key(), &state_root);
    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 1);
    let session = &sessions[0];
    assert_eq!(
        session.id.session_key, "aaaaaaaa-1111-2222-3333-444444444444",
        "session key extracted from rollout filename"
    );
    assert_eq!(
        session.cwd.as_deref(),
        Some("/home/user/projects/my-project")
    );
    assert_eq!(session.harness_key, "codex");
}

#[test]
fn codex_minimal_rollout_discovers_session_without_cwd() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("codex");
    let sessions = state_root.join("sessions");
    fs::create_dir_all(&sessions).expect("create sessions dir");
    fs::copy(
        fixture_path("codex/rollout-minimal.jsonl"),
        sessions.join("rollout-d0000000-1111-2222-3333-444444444444.jsonl"),
    )
    .expect("copy fixture");

    let context = DiscoveryContext::default()
        .with_harness_state_root(CodexAdapter::new().harness_key(), &state_root);
    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].cwd.is_none(), "minimal rollout has no cwd");
    assert_eq!(
        sessions[0].id.session_key,
        "d0000000-1111-2222-3333-444444444444"
    );
}

#[test]
fn codex_forked_rollout_emits_parent_session_link() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("codex");
    let sessions = state_root.join("sessions");
    fs::create_dir_all(&sessions).expect("create sessions dir");
    fs::copy(
        fixture_path("codex/rollout-full.jsonl"),
        sessions.join("rollout-aaaaaaaa-1111-2222-3333-444444444444.jsonl"),
    )
    .expect("copy parent");
    fs::copy(
        fixture_path("codex/rollout-forked.jsonl"),
        sessions.join("rollout-e0000000-1111-2222-3333-444444444444.jsonl"),
    )
    .expect("copy forked");

    let context = DiscoveryContext::default()
        .with_harness_state_root(CodexAdapter::new().harness_key(), &state_root);
    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(
        sessions.len(),
        2,
        "both parent and forked sessions discovered"
    );

    let parent = fragment.candidate_links.iter().any(|link| {
        link.source_metadata
            .fields
            .get("parent_native_id")
            .is_some_and(|v: &serde_json::Value| {
                v.as_str() == Some("aaaaaaaa-1111-2222-3333-444444444444")
            })
    });
    assert!(parent, "forked rollout should emit a parent session link");
}

#[test]
fn codex_malformed_rollout_does_not_crash() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("codex");
    let sessions = state_root.join("sessions");
    fs::create_dir_all(&sessions).expect("create sessions dir");
    fs::write(sessions.join("rollout-malformed.jsonl"), "not valid json\n")
        .expect("write malformed");

    let context = DiscoveryContext::default()
        .with_harness_state_root(CodexAdapter::new().harness_key(), &state_root);
    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    assert!(
        fragment.nodes.is_empty(),
        "malformed rollout should degrade"
    );
}

// ── Claude Code fixtures ────────────────────────────────────────────────

fn claude_fixture_scenario(name: &str, filename: &str) -> (TempDir, GraphFragment) {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("claude");
    let projects = state_root
        .join("projects")
        .join("home-user-projects-my-project");
    fs::create_dir_all(&projects).expect("create projects dir");
    fs::copy(
        fixture_path(filename),
        projects.join(format!("{name}.jsonl")),
    )
    .expect("copy fixture");

    let context = DiscoveryContext::default()
        .with_harness_state_root(ClaudeCodeAdapter::new().harness_key(), &state_root);
    let fragment = ClaudeCodeAdapter::new()
        .discover(&context)
        .expect("discover");
    (temp, fragment)
}

#[test]
fn claude_basic_transcript_discovers_session() {
    let (_temp, fragment) = claude_fixture_scenario(
        "11111111-aaaa-2222-bbbb-333333333333",
        "claude/transcript-basic.jsonl",
    );

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].id.session_key,
        "11111111-aaaa-2222-bbbb-333333333333"
    );
    assert_eq!(
        sessions[0].cwd.as_deref(),
        Some("/home/user/projects/my-project")
    );
}

#[test]
fn claude_resume_transcript_captures_parent_uuid() {
    let (_temp, fragment) = claude_fixture_scenario(
        "22222222-aaaa-2222-bbbb-333333333333",
        "claude/transcript-resume.jsonl",
    );

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(sessions.len(), 1);

    let parent_link = fragment.candidate_links.iter().find(|link| {
        link.source_metadata.fields.contains_key("harness_key")
            && link.source_metadata.fields.get("parent_uuid").is_some_and(
                |v: &serde_json::Value| v.as_str() == Some("aa000000-1111-2222-3333-444444444444"),
            )
    });
    assert!(
        parent_link.is_some(),
        "resume transcript with parentUuid field should emit parent session link"
    );
}

#[test]
fn claude_fork_transcript_captures_forked_from() {
    // Need both parent and fork sessions present
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path().join("claude");
    let projects = state_root
        .join("projects")
        .join("home-user-projects-my-project");
    fs::create_dir_all(&projects).expect("create projects dir");

    // Parent session (basic)
    fs::copy(
        fixture_path("claude/transcript-basic.jsonl"),
        projects.join("11111111-aaaa-2222-bbbb-333333333333.jsonl"),
    )
    .expect("copy parent");
    // Fork session
    fs::copy(
        fixture_path("claude/transcript-fork.jsonl"),
        projects.join("33333333-aaaa-2222-bbbb-333333333333.jsonl"),
    )
    .expect("copy fork");

    let context = DiscoveryContext::default()
        .with_harness_state_root(ClaudeCodeAdapter::new().harness_key(), &state_root);
    let fragment = ClaudeCodeAdapter::new()
        .discover(&context)
        .expect("discover");

    let fork_link = fragment.candidate_links.iter().find(|link| {
        link.source_metadata
            .fields
            .get("parent_session_id")
            .is_some_and(|v: &serde_json::Value| {
                v.as_str() == Some("11111111-aaaa-2222-bbbb-333333333333")
            })
    });
    assert!(
        fork_link.is_some(),
        "fork transcript with forkedFrom.sessionId should emit parent session link"
    );
}

// ── Hook payload fixtures ───────────────────────────────────────────────

#[test]
fn claude_hook_payload_converts_to_record() {
    let payload: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture_path("hooks/claude-sessionstart.json")).unwrap(),
    )
    .unwrap();

    let record = claude_code_record_from_payload(
        &payload,
        12345,
        12344,
        Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: Some("$0".to_string()),
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        Some("1.0.0".to_string()),
        1_700_000_600,
    )
    .expect("convert claude hook payload");

    assert_eq!(record.harness_key, "claude-code");
    assert_eq!(record.session_key, "cccccccc-1111-2222-3333-444444444444");
    assert_eq!(
        record.transcript_path.as_deref(),
        Some(
            "/home/user/.claude/projects/home-user-projects-my-project/cccccccc-1111-2222-3333-444444444444.jsonl"
        )
    );
    assert_eq!(
        record.cwd.as_deref(),
        Some("/home/user/projects/my-project")
    );
    assert_eq!(record.hook_event_name.as_deref(), Some("SessionStart"));
}

#[test]
fn claude_ephemeral_hook_handles_null_transcript_path() {
    let payload: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture_path("hooks/claude-ephemeral.json")).unwrap(),
    )
    .unwrap();

    let record = claude_code_record_from_payload(&payload, 12345, 12344, None, None, 1_700_000_600)
        .expect("convert ephemeral hook payload");

    assert!(
        record.transcript_path.is_none(),
        "null transcript_path should be None"
    );
    assert_eq!(record.session_key, "dddddddd-1111-2222-3333-444444444444");
}

#[test]
fn codex_hook_payload_converts_to_record() {
    let payload: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture_path("hooks/codex-sessionstart.json")).unwrap(),
    )
    .unwrap();

    let record = codex_record_from_payload(&payload, 12346, 12345, None, None, 1_700_000_650)
        .expect("convert codex hook payload");

    assert_eq!(record.harness_key, "codex");
    assert_eq!(record.session_key, "eeeeeeee-1111-2222-3333-444444444444");
    assert_eq!(
        record.cwd.as_deref(),
        Some("/home/user/projects/another-project")
    );
    assert_eq!(record.hook_event_name.as_deref(), Some("SessionStart"));
}

// ── Tmux row fixtures ───────────────────────────────────────────────────

#[test]
fn tmux_multi_sessions_parse_all_rows() {
    let stdout = fs::read_to_string(fixture_path("tmux/sessions-multi.txt")).unwrap();
    let rows = parse_list_sessions(&stdout);

    assert_eq!(rows.len(), 4, "all four rows parsed");

    let editor = &rows[0];
    assert_eq!(editor.name, "editor");
    assert_eq!(
        editor.path.as_deref(),
        Some("/home/user/projects/my-project")
    );
    assert_eq!(editor.activity_epoch, Some(1717000000));
    assert_eq!(editor.created_epoch, Some(1716900000));
    assert_eq!(editor.active_pane_command.as_deref(), Some("claude"));
    assert_eq!(editor.active_pane_pid, Some(12345));
    assert_eq!(editor.active_pane_start_command.as_deref(), Some("claude"));

    let empty_pane = &rows[3];
    assert_eq!(empty_pane.name, "empty-pane");
    assert!(
        empty_pane.path.is_none() || empty_pane.path.as_deref() == Some(""),
        "empty pane should have no cwd"
    );
}

#[test]
fn tmux_sessions_with_spaces_preserve_paths() {
    let stdout = fs::read_to_string(fixture_path("tmux/sessions-spaces.txt")).unwrap();
    let rows = parse_list_sessions(&stdout);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "project work");
    assert_eq!(
        rows[0].path.as_deref(),
        Some("/home/user/My Projects/active")
    );
    assert_eq!(
        rows[1].path.as_deref(),
        Some("/home/user/.config with spaces/")
    );
}

// ── /proc fd targets fixture ────────────────────────────────────────────

#[test]
fn fd_targets_fixture_contains_all_expected_link_types() {
    let content = fs::read_to_string(fixture_path("proc/fd-targets.txt")).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    assert!(
        lines.iter().any(|l| l.contains(".codex/sessions/")),
        "codex rollout path"
    );
    assert!(
        lines.iter().any(|l| l.contains(".claude/projects/")),
        "claude task path"
    );
    assert!(
        lines.iter().any(|l| l.contains(".local/share/opencode/")),
        "opencode session path"
    );
    assert!(
        lines.iter().any(|l| l.contains("socket:")),
        "socket descriptor"
    );
    assert!(lines.iter().any(|l| l.contains("pipe:")), "pipe descriptor");
    assert!(
        lines.iter().any(|l| l.contains("anon_inode:")),
        "anon inode descriptor"
    );
    assert!(
        lines.iter().any(|l| l.contains("(deleted)")),
        "deleted file marker"
    );
    assert!(lines.iter().any(|l| l.starts_with("/dev/")), "device node");
}
