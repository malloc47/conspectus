use super::*;
use std::cell::RefCell;

/// Test seam: records probed binary names and returns membership
/// in a fixed set of "installed" binaries.
struct FakeBinaryProbe {
    installed: Vec<&'static str>,
    probed: RefCell<Vec<String>>,
}

impl FakeBinaryProbe {
    fn new(installed: &[&'static str]) -> Self {
        Self {
            installed: installed.to_vec(),
            probed: RefCell::new(Vec::new()),
        }
    }
}

impl BinaryProbe for FakeBinaryProbe {
    fn on_path(&self, binary: &str) -> bool {
        self.probed.borrow_mut().push(binary.to_string());
        self.installed.contains(&binary)
    }
}

fn session(harness: &str) -> AgentSessionId {
    AgentSessionId::new(harness, "/state", "sess-uuid-abc")
}

/// Lay down a fake `<state>/projects/<proj>/<session_key>.jsonl`
/// so `resolve_viewer_target` can resolve a file path.
/// Returns the temp dir + the session id it created.
fn claude_fixture(session_key: &str) -> (tempfile::TempDir, AgentSessionId) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("projects").join("-home-user-proj");
    std::fs::create_dir_all(&project).expect("project dir");
    std::fs::write(project.join(format!("{session_key}.jsonl")), "{}\n").expect("seed jsonl");
    let id = AgentSessionId::new("claude-code", dir.path().to_string_lossy(), session_key);
    (dir, id)
}

#[test]
fn claude_code_with_claude_history_installed_launches_claude_history() {
    let probe = FakeBinaryProbe::new(&["claude-history"]);
    let (_tmp, id) = claude_fixture("sess-uuid-abc");
    match resolve_viewer_target(&id, &probe) {
        ViewerTarget::Launch(plan) => {
            assert_eq!(plan.program, "claude-history");
            assert_eq!(plan.args.len(), 1, "single positional arg = jsonl path");
            assert!(
                plan.args[0].ends_with("/projects/-home-user-proj/sess-uuid-abc.jsonl"),
                "got {:?}",
                plan.args[0]
            );
        }
        other @ ViewerTarget::Disabled(_) => panic!("expected Launch, got {other:?}"),
    }
}

#[test]
fn claude_history_missing_transcript_reports_transcript_not_found() {
    let probe = FakeBinaryProbe::new(&["claude-history"]);
    match resolve_viewer_target(&session("claude-code"), &probe) {
        ViewerTarget::Disabled(ViewerDisabled::TranscriptNotFound { binary, hint }) => {
            assert_eq!(binary, "claude-history");
            assert!(hint.contains("sess-uuid-abc"), "got {hint:?}");
        }
        other => panic!("expected TranscriptNotFound, got {other:?}"),
    }
}

#[test]
fn aider_is_unsupported() {
    let probe = FakeBinaryProbe::new(&["claude-history"]);
    match resolve_viewer_target(&session("aider"), &probe) {
        ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
            assert_eq!(harness_key, "aider");
        }
        other => panic!("expected UnsupportedHarness, got {other:?}"),
    }
}

#[test]
fn unknown_harness_is_unsupported() {
    let probe = FakeBinaryProbe::new(&["claude-history"]);
    match resolve_viewer_target(&session("mystery"), &probe) {
        ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
            assert_eq!(harness_key, "mystery");
        }
        other => panic!("expected UnsupportedHarness, got {other:?}"),
    }
}

/// Non-claude harnesses have no escape-hatch backend registered.
/// Operators wanting Codex / OpenCode external viewers will
/// configure them via `H-TRANSCRIPT-013`.
#[test]
fn codex_has_no_escape_hatch_backend() {
    let probe = FakeBinaryProbe::new(&["claude-history"]);
    match resolve_viewer_target(&session("codex"), &probe) {
        ViewerTarget::Disabled(ViewerDisabled::UnsupportedHarness { harness_key }) => {
            assert_eq!(harness_key, "codex");
        }
        other => panic!("expected UnsupportedHarness, got {other:?}"),
    }
}

#[test]
fn claude_code_with_no_binary_reports_install_hint() {
    let probe = FakeBinaryProbe::new(&[]);
    match resolve_viewer_target(&session("claude-code"), &probe) {
        ViewerTarget::Disabled(ViewerDisabled::BinaryNotInstalled {
            binaries,
            harness_key,
        }) => {
            assert_eq!(harness_key, "claude-code");
            assert_eq!(binaries, vec!["claude-history"]);
        }
        other => panic!("expected BinaryNotInstalled, got {other:?}"),
    }
}

#[test]
fn disabled_reason_text_covers_every_variant() {
    assert_eq!(
        viewer_disabled_reason(&ViewerDisabled::NoSelection),
        "view: no row selected"
    );
    assert_eq!(
        viewer_disabled_reason(&ViewerDisabled::UnsupportedRow),
        "view: select an agent session row"
    );
    assert_eq!(
        viewer_disabled_reason(&ViewerDisabled::UnsupportedHarness {
            harness_key: "aider".to_string()
        }),
        "view: no viewer registered for aider"
    );
    assert_eq!(
        viewer_disabled_reason(&ViewerDisabled::BinaryNotInstalled {
            binaries: vec!["claude-history".to_string()],
            harness_key: "claude-code".to_string(),
        }),
        "view: install claude-history to view claude-code sessions"
    );
    assert_eq!(
        viewer_disabled_reason(&ViewerDisabled::TranscriptNotFound {
            binary: "claude-history".to_string(),
            hint: "no transcript under /state/projects/*/sess.jsonl".to_string(),
        }),
        "view: claude-history cannot resolve transcript — no transcript under /state/projects/*/sess.jsonl"
    );
}
