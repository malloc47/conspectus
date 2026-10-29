use super::*;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::keys::press;

#[test]
fn commits_request_when_required_fields_present() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/home/op/repo",
        "adhoc",
        vec!["codex".to_string(), "claude-code".to_string()],
        Vec::new(),
    );
    let outcome = state.handle_key(press(KeyCode::Enter));
    match outcome {
        Outcome::Commit(msg) => match msg {
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
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
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
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
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
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
    assert!(state.error().unwrap().contains("worktree"));
}

#[test]
fn down_moves_focus_through_visible_fields() {
    let mut state = MuxLaunchFormState::new(
        "codex",
        "/tmp",
        "adhoc",
        vec!["codex".to_string()],
        Vec::new(),
    );
    assert_eq!(
        state.focus(),
        MuxLaunchField::MuxName,
        "the name comes first"
    );
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::Harness);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(
        state.focus(),
        MuxLaunchField::LaunchOptions,
        "codex offers options"
    );
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::Cwd);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::LaunchArgv);
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), MuxLaunchField::Cwd);
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
        .set_launch_argv("codex --model 'gpt-5 pro'".to_string());
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
    state.spec.set_launch_argv("codex --mine");

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
    // Up from the first field wraps to the last, the pin choice.
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus, MuxLaunchField::Pin);

    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::Pin);
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::PlanOnly);
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.build_request().unwrap().pin, PinChoice::Off);
}

#[test]
fn the_form_shows_the_pin_choice_in_words() {
    use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
    let mut state = MuxLaunchFormState::new("codex", "/tmp", "adhoc", Vec::new(), Vec::new());
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Right));
    state.handle_key(press(KeyCode::Right));
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

fn claude_form() -> MuxLaunchFormState {
    MuxLaunchFormState::new(
        "claude-code",
        "/tmp",
        "adhoc",
        vec![
            "claude-code".to_string(),
            "codex".to_string(),
            "opencode".to_string(),
        ],
        Vec::new(),
    )
}

#[test]
fn up_from_the_first_field_wraps_to_the_last() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), MuxLaunchField::Pin);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::MuxName);
}

#[test]
fn shift_tab_moves_back() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::Harness);
    state.handle_key(press(KeyCode::BackTab));
    assert_eq!(
        state.focus(),
        MuxLaunchField::MuxSocketToggle,
        "Tab stops include the socket checkbox"
    );
    state.handle_key(press(KeyCode::BackTab));
    assert_eq!(state.focus(), MuxLaunchField::MuxName);
}

#[test]
fn the_options_row_exists_only_for_harnesses_that_offer_options() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::LaunchOptions);

    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Right));
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.spec().harness().value(), "opencode");
    state.handle_key(press(KeyCode::Down));
    assert_eq!(
        state.focus(),
        MuxLaunchField::Cwd,
        "opencode has no options"
    );
}

#[test]
fn the_skip_permissions_toggle_reaches_the_launch_argv() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(
        state.spec().launch_argv().value(),
        "claude --dangerously-skip-permissions"
    );
    let request = state.build_request().unwrap();
    assert_eq!(
        request.launch_argv,
        ["claude", "--dangerously-skip-permissions"]
    );
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.spec().launch_argv().value(), "");
    assert!(state.build_request().unwrap().launch_argv.is_empty());
}

#[test]
fn cycling_the_harness_takes_the_old_harnesss_flags_with_it() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.spec().harness().value(), "codex");
    assert_eq!(
        state.spec().launch_argv().value(),
        "",
        "no leftover claude flag"
    );
}

#[test]
fn space_and_arrows_type_in_text_fields() {
    let mut state = claude_form();
    assert_eq!(state.focus(), MuxLaunchField::MuxName);
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Char('x')));
    assert_eq!(state.spec().mux_name().value(), "adhoc x");
}

#[test]
fn tab_completes_a_directory_then_moves_on_when_nothing_is_left_to_complete() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("only-dir")).unwrap();
    let typed = format!("{}/on", dir.path().display());
    let mut state = MuxLaunchFormState::new(
        "claude-code",
        typed,
        "adhoc",
        vec!["claude-code".to_string()],
        Vec::new(),
    );
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::Cwd);
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), MuxLaunchField::Cwd);
    assert_eq!(
        state.spec().cwd().value(),
        format!("{}/only-dir", dir.path().display())
    );
    // Nothing further to complete in an empty directory: Tab moves on.
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), MuxLaunchField::LaunchArgv);
}

#[test]
fn candidates_complete_directories_the_filesystem_does_not_list() {
    let mut state = MuxLaunchFormState::new(
        "claude-code",
        "/srv/wor",
        "adhoc",
        vec!["claude-code".to_string()],
        Vec::new(),
    )
    .with_cwd_candidates(vec![crate::tui::widgets::path_omnibox::PathCandidate::new(
        "/srv/work/known-repo",
        "repo",
        300,
    )]);
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.spec().cwd().value(), "/srv/work/known-repo");
}

fn rendered(state: &MuxLaunchFormState) -> String {
    let area = ratatui::layout::Rect::new(0, 0, 100, 32);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    MuxLaunchFormWidget::new(state, &crate::tui::Theme::default()).render(area, &mut buf);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_form_shows_harness_chips_options_the_cwd_mark_and_the_command() {
    let mut state = claude_form();
    let frame = rendered(&state);
    for expected in [
        "[claude-code]",
        "[codex]",
        "[opencode]",
        "[ ] skip permissions",
        "√",
        "└ command",
        "claude",
        "↑/↓ move",
    ] {
        assert!(frame.contains(expected), "`{expected}` missing:\n{frame}");
    }
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Char(' ')));
    let frame = rendered(&state);
    assert!(frame.contains("[x] skip permissions"), "{frame}");
    assert!(
        frame.contains("claude --dangerously-skip-permissions"),
        "{frame}"
    );
    assert!(!frame.contains("read only"), "{frame}");
}

#[test]
fn a_missing_cwd_is_flagged_in_the_form() {
    let state = MuxLaunchFormState::new(
        "claude-code",
        "/no/such/dir",
        "adhoc",
        vec!["claude-code".to_string()],
        Vec::new(),
    );
    assert!(rendered(&state).contains('!'));
}

fn name_row(frame: &str) -> String {
    frame
        .lines()
        .find(|line| line.contains("mux name"))
        .unwrap_or_default()
        .to_string()
}

#[test]
fn the_name_is_the_first_field_with_the_socket_hidden_behind_a_checkbox() {
    let state = claude_form();
    assert_eq!(state.focus(), MuxLaunchField::MuxName);
    let frame = rendered(&state);
    let lines: Vec<&str> = frame.lines().collect();
    let name = lines.iter().position(|l| l.contains("mux name")).unwrap();
    let harness = lines.iter().position(|l| l.contains("harness  ")).unwrap();
    assert!(
        name < harness,
        "the name row comes before harness:\n{frame}"
    );
    assert!(name_row(&frame).contains("[ ] socket"), "{frame}");
    assert!(
        !frame.contains("mux socket"),
        "the field is hidden:\n{frame}"
    );
}

#[test]
fn the_checkbox_reveals_the_socket_field_under_the_name() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), MuxLaunchField::MuxSocketToggle);
    state.handle_key(press(KeyCode::Char(' ')));
    let frame = rendered(&state);
    assert!(name_row(&frame).contains("[x] socket"), "{frame}");
    let lines: Vec<&str> = frame.lines().collect();
    let name = lines.iter().position(|l| l.contains("mux name")).unwrap();
    assert!(
        lines[name + 1].contains("mux socket"),
        "the socket row is directly beneath:\n{frame}"
    );
    // Down from the checkbox moves to the next row, the socket field.
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::MuxSocket);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), MuxLaunchField::Harness);
}

#[test]
fn the_checkbox_keeps_its_place_and_the_row_keeps_its_width_when_toggled() {
    let mut state = claude_form();
    let off = name_row(&rendered(&state));
    state.handle_key(press(KeyCode::Tab));
    let focused = name_row(&rendered(&state));
    state.handle_key(press(KeyCode::Right));
    let on = name_row(&rendered(&state));
    assert_eq!(off.find("[ ] socket"), focused.find("[ ] socket"));
    assert_eq!(off.find("[ ] socket"), on.find("[x] socket"));
    assert!(
        focused.contains(&format!("{} mux name", tuikit::glyphs::marks::POINTER)),
        "the row is marked: {focused}"
    );
}

#[test]
fn the_socket_is_passed_only_while_the_checkbox_is_on_and_is_remembered() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Tab));
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Down));
    for c in "work".chars() {
        state.handle_key(press(KeyCode::Char(c)));
    }
    assert_eq!(
        state.build_request().unwrap().mux_socket.as_deref(),
        Some("work")
    );

    // Off: the default server, but the typed text is still there.
    state.handle_key(press(KeyCode::BackTab));
    assert_eq!(state.focus(), MuxLaunchField::MuxSocketToggle);
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.build_request().unwrap().mux_socket, None);
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(
        state.build_request().unwrap().mux_socket.as_deref(),
        Some("work")
    );

    // On and empty behaves as the default server.
    state.handle_key(press(KeyCode::Down));
    for _ in 0..4 {
        state.handle_key(press(KeyCode::Backspace));
    }
    assert_eq!(state.build_request().unwrap().mux_socket, None);
}

fn frame_at(state: &MuxLaunchFormState, width: u16, height: u16) -> String {
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    MuxLaunchFormWidget::new(state, &crate::tui::Theme::default()).render(area, &mut buf);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

#[test]
fn the_form_at_120_columns() {
    insta::assert_snapshot!(frame_at(&claude_form(), 120, 20));
}

#[test]
fn the_form_at_80_columns() {
    insta::assert_snapshot!(frame_at(&claude_form(), 80, 20));
}

#[test]
fn the_form_with_the_socket_field_shown() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Tab));
    state.handle_key(press(KeyCode::Char(' ')));
    insta::assert_snapshot!(frame_at(&state, 100, 20));
}
