use super::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn commits_request_when_required_fields_present() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/home/op/repo",
        "adhoc",
        vec!["codex".to_string(), "claude-code".to_string()],
        Vec::new(),
    );
    let outcome = state.handle((), key(KeyCode::Enter));
    match outcome {
        OverlayOutcome::Commit(msg) => match *msg {
            Msg::CommitMuxLaunch(req) => {
                assert_eq!(req.harness, "codex");
                assert_eq!(req.name, "adhoc");
                assert_eq!(req.cwd, "/home/op/repo");
                assert!(req.worktree_branch.is_none());
                assert!(req.launch_argv.is_empty());
            }
            other => panic!("unexpected msg: {other:?}"),
        },
        other => panic!("expected Commit, got {other:?}"),
    }
}

#[test]
fn empty_harness_produces_error() {
    let mut state =
        MuxLaunchFormState::new("", "/tmp", "adhoc", vec!["codex".to_string()], Vec::new());
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(outcome, OverlayOutcome::Consumed);
    assert!(state.error().unwrap().contains("harness"));
}

#[test]
fn name_collision_with_known_mux_produces_error() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "conflict",
        vec!["codex".to_string()],
        vec!["conflict".to_string()],
    );
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(outcome, OverlayOutcome::Consumed);
    assert!(state.error().unwrap().contains("already live"));
}

#[test]
fn worktree_toggle_requires_branch_and_repo() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    );
    state.spec_mut().set_worktree_enabled(true);
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(outcome, OverlayOutcome::Consumed);
    assert!(state.error().unwrap().contains("worktree"));
}

#[test]
fn tab_cycles_focus_through_visible_fields() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    );
    assert_eq!(state.focus(), MuxLaunchField::Harness);
    state.handle((), key(KeyCode::Tab));
    assert_eq!(state.focus(), MuxLaunchField::Cwd);
    state.handle((), key(KeyCode::Tab));
    assert_eq!(state.focus(), MuxLaunchField::MuxName);
}

#[test]
fn launch_argv_parses_shell_quoted() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    );
    state
        .spec_mut()
        .set_launch_argv(LAUNCH_ARGV_LABEL, "codex --model 'gpt-5 pro'".to_string());
    let req = state.build_request().expect("parse ok");
    assert_eq!(req.launch_argv, vec!["codex", "--model", "gpt-5 pro"]);
}
