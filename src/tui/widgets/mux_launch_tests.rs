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

fn profile_defaults(argv: &[&str]) -> crate::launch_default::LaunchDefaults {
    let mut config = crate::config::LaunchConfig::default();
    config.profiles.insert(
        "codex".into(),
        crate::config::LaunchProfile {
            argv: argv.iter().map(|a| (*a).to_string()).collect(),
            origin: crate::config::ProfileOrigin::Project,
        },
    );
    crate::launch_default::LaunchDefaults::new(config).with_atelier(false)
}

#[test]
fn an_empty_argv_shows_the_default_and_where_it_comes_from() {
    let state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    )
    .with_defaults(profile_defaults(&["nix", "develop", "-c", "codex"]));

    assert_eq!(
        state.default_hint().as_deref(),
        Some("nix develop -c codex — from the project launch profile")
    );
}

#[test]
fn a_typed_argv_replaces_the_default_hint() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    )
    .with_defaults(profile_defaults(&["codex", "--flag"]));
    state.spec.set_launch_argv("Launch argv", "codex --mine");

    assert_eq!(state.default_hint(), None);
}

#[test]
fn without_defaults_there_is_no_hint() {
    let state = MuxLaunchFormState::new("codex", "/tmp", "adhoc", Vec::new(), Vec::new());
    assert_eq!(state.default_hint(), None);
}

#[test]
fn the_worktree_form_opens_with_the_toggle_on_and_the_repo_seeded() {
    let state = MuxLaunchFormState::new("codex", "/src/app", "adhoc", Vec::new(), Vec::new())
        .with_worktree("/src/app");

    assert!(state.spec().worktree_enabled());
    assert_eq!(state.worktree_repo_value(), "/src/app");
}

#[test]
fn the_pin_field_cycles_and_rides_the_request() {
    let mut state = MuxLaunchFormState::new("codex", "/tmp", "adhoc", Vec::new(), Vec::new());
    // Walk to the last field, the pin choice.
    for _ in 0..5 {
        state.handle((), key(KeyCode::Tab));
    }
    state.handle((), key(KeyCode::Tab));
    assert_eq!(state.focus, MuxLaunchField::Pin);

    state.handle((), key(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::Pin);
    state.handle((), key(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::PlanOnly);
    state.handle((), key(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::Off);
}

#[test]
fn the_form_shows_the_pin_choice_in_words() {
    use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
    let mut state = MuxLaunchFormState::new("codex", "/tmp", "adhoc", Vec::new(), Vec::new());
    for _ in 0..6 {
        state.handle((), key(KeyCode::Tab));
    }
    state.handle((), key(KeyCode::Right));
    state.handle((), key(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::PlanOnly);

    let theme = crate::tui::Theme::default();
    let area = Rect::new(0, 0, 90, 20);
    let mut buf = Buffer::empty(area);
    MuxLaunchFormWidget::new(&state, &theme).render(area, &mut buf);
    let text: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("pin"), "{text}");
    assert!(
        text.to_lowercase().contains("don't launch")
            || text.to_lowercase().contains("without launching")
            || text.to_lowercase().contains("plan"),
        "the plan-only choice is spelled out:\n{text}"
    );
}
