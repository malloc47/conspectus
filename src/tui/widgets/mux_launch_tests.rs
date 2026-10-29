use super::*;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::keys::press;

fn form(harness: &str, cwd: &str, name: &str, known: &[&str], live: &[&str]) -> MuxLaunchForm {
    let strings = |items: &[&str]| items.iter().map(|item| (*item).to_string()).collect();
    MuxLaunchForm::new(harness, cwd, name, strings(known), strings(live))
}

/// The request `Enter` would commit, from a copy of the form.
fn request(state: &MuxLaunchForm) -> MuxLaunchRequest {
    match state.clone().handle_key(press(KeyCode::Enter)) {
        Outcome::Commit(Msg::CommitMuxLaunch(request)) => request,
        other => panic!("expected a launch, got {other:?} ({:?})", state.error()),
    }
}

fn type_str(state: &mut MuxLaunchForm, text: &str) {
    for c in text.chars() {
        state.handle_key(press(KeyCode::Char(c)));
    }
}

/// Moves the focus down the ring to field `id`.
fn focus_on(state: &mut MuxLaunchForm, id: &str) {
    for _ in 0..20 {
        if state.focus() == Some(id) {
            return;
        }
        state.handle_key(press(KeyCode::Down));
    }
    panic!("no `{id}` field");
}

#[test]
fn commits_request_when_required_fields_present() {
    let mut state = form(
        "codex",
        "/home/op/repo",
        "adhoc",
        &["codex", "claude-code"],
        &[],
    );
    match state.handle_key(press(KeyCode::Enter)) {
        Outcome::Commit(Msg::CommitMuxLaunch(req)) => {
            assert_eq!(req.harness, "codex");
            assert_eq!(req.name, "adhoc");
            assert_eq!(req.cwd, "/home/op/repo");
            assert!(req.worktree_branch.is_none());
            assert!(req.launch_argv.is_empty());
            assert_eq!(req.pin, PinChoice::Off);
        }
        other => panic!("expected Commit, got {other:?}"),
    }
}

#[test]
fn empty_harness_produces_error_and_takes_the_focus() {
    let mut state = form("", "/tmp", "adhoc", &["codex"], &[]);
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
    assert_eq!(state.error(), Some("mux launch: harness is required"));
    assert_eq!(state.focus(), Some(HARNESS), "the field to fill in");
}

#[test]
fn name_collision_with_known_mux_produces_error() {
    let mut state = form("codex", "/tmp", "conflict", &["codex"], &["conflict"]);
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
    assert!(state.error().unwrap().contains("already live"));
}

#[test]
fn worktree_toggle_requires_branch_and_repo() {
    let mut state = form("codex", "/tmp", "adhoc", &["codex"], &[]).with_worktree("");
    assert_eq!(state.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(
        state.error(),
        Some("mux launch: worktree branch is required")
    );
    assert_eq!(state.focus(), Some(BRANCH));
    type_str(&mut state, "feature");
    assert_eq!(state.error(), None, "typing clears the error");
    state.handle_key(press(KeyCode::Enter));
    assert_eq!(state.error(), Some("mux launch: worktree repo is required"));
    assert_eq!(state.focus(), Some(REPO));
}

#[test]
fn down_moves_focus_through_visible_fields() {
    let mut state = form("codex", "/tmp", "adhoc", &["codex"], &[]);
    assert_eq!(state.focus(), Some(NAME), "the name comes first");
    let mut seen = Vec::new();
    for _ in 0..5 {
        state.handle_key(press(KeyCode::Down));
        seen.push(state.focus().unwrap());
    }
    assert_eq!(
        seen,
        [SERVER, HARNESS, OPTIONS, CWD, ARGV],
        "codex offers options; the command row is not a stop"
    );
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), Some(CWD));
}

/// One ring: `Tab` and `Down` visit the same stops, as `S-Tab` and `Up`
/// do going back.
#[test]
fn tab_and_down_walk_one_ring() {
    let walk = |code: KeyCode| {
        let mut state = claude_form();
        (0..14)
            .map(|_| {
                state.handle_key(press(code));
                state.focus().unwrap()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(walk(KeyCode::Tab), walk(KeyCode::Down));
    assert_eq!(walk(KeyCode::BackTab), walk(KeyCode::Up));
    assert_eq!(
        walk(KeyCode::Down)[..9],
        [
            SERVER, HARNESS, OPTIONS, CWD, ARGV, WORKTREE, PIN, NAME, SERVER
        ]
    );
}

#[test]
fn launch_argv_parses_shell_quoted() {
    let mut state = form("codex", "/tmp", "adhoc", &["codex"], &[]);
    focus_on(&mut state, ARGV);
    type_str(&mut state, "codex --model 'gpt-5 pro' -p \"a\\b\"");
    assert_eq!(
        request(&state).launch_argv,
        ["codex", "--model", "gpt-5 pro", "-p", r"a\b"]
    );
}

#[test]
fn an_unfinished_argv_is_refused_on_its_field() {
    let mut state = form("codex", "/tmp", "adhoc", &["codex"], &[]);
    focus_on(&mut state, ARGV);
    type_str(&mut state, "codex 'oops");
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(
        state.error(),
        Some("mux launch: launch argv has an unclosed `'` quote")
    );
    assert_eq!(state.focus(), Some(ARGV));
}

fn profile_defaults(argv: &[&str]) -> LaunchDefaults {
    let mut config = crate::config::LaunchConfig::default();
    config.profiles.insert(
        "codex".into(),
        crate::config::LaunchProfile {
            argv: argv.iter().map(|a| (*a).to_string()).collect(),
            origin: crate::config::ProfileOrigin::Project,
        },
    );
    LaunchDefaults::new(config).with_atelier(false)
}

#[test]
fn an_empty_argv_shows_the_default_and_where_it_comes_from() {
    let state = form("codex", "/tmp", "adhoc", &["codex"], &[])
        .with_defaults(profile_defaults(&["nix", "develop", "-c", "codex"]));

    let hint = "nix develop -c codex — from the project launch profile";
    assert_eq!(state.default_hint().as_deref(), Some(hint));
    assert_eq!(state.command(), hint);
}

#[test]
fn a_typed_argv_replaces_the_default_hint() {
    let mut state = form("codex", "/tmp", "adhoc", &["codex"], &[])
        .with_defaults(profile_defaults(&["codex", "--flag"]));
    focus_on(&mut state, ARGV);
    type_str(&mut state, "codex --mine");

    assert_eq!(state.default_hint(), None);
    assert_eq!(state.command(), "codex --mine");
}

#[test]
fn without_defaults_there_is_no_hint() {
    let state = form("codex", "/tmp", "adhoc", &[], &[]);
    assert_eq!(state.default_hint(), None);
    assert_eq!(state.command(), "codex", "the harness default");
}

/// The command row reads the default the form decided when it opened;
/// drawing never looks for an Atelier workspace again. Only a change of
/// the harness or the cwd decides it again.
#[test]
fn drawing_the_form_twice_reads_the_default_it_kept() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().display().to_string();
    let mut state = form("claude-code", &cwd, "adhoc", &["claude-code"], &[])
        .with_defaults(LaunchDefaults::default().with_atelier(true));
    let before = "claude — from the harness default";
    assert_eq!(state.command(), before);

    std::fs::write(
        dir.path()
            .join(crate::discovery::atelier::ATELIER_CONFIG_FILENAME),
        "[workspace]\nname = \"w\"\n",
    )
    .unwrap();
    for _ in 0..2 {
        let frame = rendered(&state);
        assert!(frame.contains(before), "{frame}");
    }
    assert_eq!(state.command(), before);

    focus_on(&mut state, CWD);
    type_str(&mut state, "/");
    assert!(
        state
            .command()
            .starts_with("atelier exec claude — from `atelier exec`"),
        "{}",
        state.command()
    );
}

#[test]
fn the_worktree_form_opens_with_the_toggle_on_and_the_repo_seeded() {
    let state = form("codex", "/src/app", "adhoc", &[], &[]).with_worktree("/src/app");

    assert!(state.values().is_on(WORKTREE));
    assert_eq!(state.values().text(REPO), "/src/app");
}

#[test]
fn the_pin_field_cycles_and_rides_the_request() {
    let mut state = form("codex", "/tmp", "adhoc", &[], &[]);
    // Up from the first field wraps to the last, the pin choice.
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), Some(PIN));

    state.handle_key(press(KeyCode::Right));
    assert_eq!(request(&state).pin, PinChoice::Pin);
    state.handle_key(press(KeyCode::Right));
    assert_eq!(request(&state).pin, PinChoice::PlanOnly);
    state.handle_key(press(KeyCode::Right));
    assert_eq!(request(&state).pin, PinChoice::Off);
}

#[test]
fn the_form_shows_the_pin_choice_in_words() {
    let mut state = form("codex", "/tmp", "adhoc", &[], &[]);
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Right));
    state.handle_key(press(KeyCode::Right));
    assert_eq!(request(&state).pin, PinChoice::PlanOnly);

    let text = frame_at(&state, 90, 20);
    assert!(text.contains("pin"), "{text}");
    assert!(
        text.contains("(•) pin, don't launch"),
        "the plan-only choice is spelled out:\n{text}"
    );
}

fn claude_form() -> MuxLaunchForm {
    form(
        "claude-code",
        "/tmp",
        "adhoc",
        &["claude-code", "codex", "opencode"],
        &[],
    )
}

#[test]
fn up_from_the_first_field_wraps_to_the_last() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), Some(PIN));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(NAME));
}

#[test]
fn shift_tab_moves_back_over_the_server_checkbox() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(HARNESS));
    state.handle_key(press(KeyCode::BackTab));
    assert_eq!(
        state.focus(),
        Some(SERVER),
        "the checkbox is a stop of the ring"
    );
    state.handle_key(press(KeyCode::BackTab));
    assert_eq!(state.focus(), Some(NAME));
}

#[test]
fn the_options_row_exists_only_for_harnesses_that_offer_options() {
    let mut state = claude_form();
    focus_on(&mut state, HARNESS);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(OPTIONS));

    let mut state = claude_form();
    focus_on(&mut state, HARNESS);
    state.handle_key(press(KeyCode::Right));
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.values().text(HARNESS), "opencode");
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(CWD), "opencode has no options");
    assert!(!rendered(&state).contains("options"));
}

#[test]
fn the_skip_permissions_toggle_reaches_the_launch_argv() {
    let mut state = claude_form();
    focus_on(&mut state, OPTIONS);
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(
        state.values().text(ARGV),
        "claude --dangerously-skip-permissions"
    );
    assert_eq!(
        request(&state).launch_argv,
        ["claude", "--dangerously-skip-permissions"]
    );
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.values().text(ARGV), "");
    assert!(request(&state).launch_argv.is_empty());
}

#[test]
fn typing_the_flag_checks_the_option() {
    let mut state = claude_form();
    focus_on(&mut state, ARGV);
    type_str(&mut state, "claude --dangerously-skip-permissions");
    let checked = state.values().check_group(OPTIONS).map(CheckGroup::checked);
    assert_eq!(checked, Some(vec![true]));
}

#[test]
fn cycling_the_harness_takes_the_old_harnesss_flags_with_it() {
    let mut state = claude_form();
    focus_on(&mut state, OPTIONS);
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.values().text(HARNESS), "codex");
    assert_eq!(state.values().text(ARGV), "", "no leftover claude flag");

    focus_on(&mut state, ARGV);
    type_str(&mut state, "codex --model gpt");
    focus_on(&mut state, HARNESS);
    state.handle_key(press(KeyCode::Right));
    assert_eq!(state.values().text(HARNESS), "opencode");
    assert_eq!(
        state.values().text(ARGV),
        "codex --model gpt",
        "a custom argv stays"
    );
}

#[test]
fn typing_a_harness_leaves_the_argv_alone() {
    let mut state = claude_form();
    focus_on(&mut state, ARGV);
    type_str(&mut state, "claude --dangerously-skip-permissions");
    focus_on(&mut state, HARNESS);
    type_str(&mut state, "x");
    assert_eq!(state.values().text(HARNESS), "claude-codex");
    assert_eq!(
        state.values().text(ARGV),
        "claude --dangerously-skip-permissions",
        "only stepping through the known harnesses carries the argv"
    );
}

#[test]
fn space_and_arrows_type_in_text_fields() {
    let mut state = claude_form();
    assert_eq!(state.focus(), Some(NAME));
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Char('x')));
    assert_eq!(state.values().text(NAME), "adhoc x");
}

#[test]
fn tab_completes_a_directory_then_moves_on_when_nothing_is_left_to_complete() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("only-dir")).unwrap();
    let typed = format!("{}/on", dir.path().display());
    let mut state = form("claude-code", &typed, "adhoc", &["claude-code"], &[]);
    focus_on(&mut state, CWD);
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), Some(CWD));
    assert_eq!(
        state.values().text(CWD),
        format!("{}/only-dir", dir.path().display())
    );
    // Nothing further to complete in an empty directory: Tab moves on.
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), Some(ARGV));
}

#[test]
fn candidates_complete_directories_the_filesystem_does_not_list() {
    let mut state =
        form("claude-code", "/srv/wor", "adhoc", &["claude-code"], &[]).with_cwd_candidates(vec![
            tuikit::path_field::Candidate::new("/srv/work/known-repo", 300),
        ]);
    focus_on(&mut state, CWD);
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.values().text(CWD), "/srv/work/known-repo");
}

fn rendered(state: &MuxLaunchForm) -> String {
    let area = Rect::new(0, 0, 100, 32);
    let mut buf = Buffer::empty(area);
    state.draw(area, &mut buf, &crate::tui::Theme::default());
    tuikit::testkit::buffer::rows(&buf).join("\n")
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
    focus_on(&mut state, OPTIONS);
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
    let state = form(
        "claude-code",
        "/no/such/dir",
        "adhoc",
        &["claude-code"],
        &[],
    );
    assert!(rendered(&state).contains(tuikit::glyphs::marks::WARNING));
}

/// Whether `line` is the row labelled `label`: the label follows the
/// frame's border and the cursor gutter.
fn is_labelled(line: &str, label: &str) -> bool {
    line.split_once('│').is_some_and(|(_, inside)| {
        inside
            .chars()
            .skip(2)
            .collect::<String>()
            .starts_with(&format!("{label} "))
    })
}

fn row_of(frame: &str, label: &str) -> String {
    frame
        .lines()
        .find(|line| is_labelled(line, label))
        .unwrap_or_default()
        .to_string()
}

#[test]
fn the_name_is_the_first_field_and_the_socket_hides_behind_the_server_checkbox() {
    let state = claude_form();
    assert_eq!(state.focus(), Some(NAME));
    let frame = rendered(&state);
    let lines: Vec<&str> = frame.lines().collect();
    let name = lines.iter().position(|l| is_labelled(l, NAME)).unwrap();
    assert!(
        is_labelled(lines[name + 1], SERVER),
        "the server checkbox is the row under the name:\n{frame}"
    );
    assert!(row_of(&frame, SERVER).contains("[ ] own socket"), "{frame}");
    assert!(
        !row_of(&frame, NAME).contains("socket"),
        "the name row holds only the name:\n{frame}"
    );
    assert!(
        !lines.iter().any(|line| is_labelled(line, SOCKET)),
        "the socket field is hidden:\n{frame}"
    );
}

#[test]
fn the_labels_are_the_short_forms() {
    let frame = rendered(&claude_form());
    let lines: Vec<&str> = frame.lines().collect();
    for label in [
        "name", "server", "harness", "options", "cwd", "argv", "worktree", "pin",
    ] {
        assert!(
            lines.iter().any(|line| is_labelled(line, label)),
            "no `{label}` row:\n{frame}"
        );
    }
    for long in ["mux name", "launch argv"] {
        assert!(!frame.contains(long), "`{long}` is the long form:\n{frame}");
    }
}

#[test]
fn the_pin_row_offers_one_of_three_not_checkboxes() {
    let frame = rendered(&claude_form());
    assert!(
        row_of(&frame, PIN).contains("(•) don't pin ( ) launch, then pin"),
        "{frame}"
    );
}

#[test]
fn the_server_checkbox_reveals_the_socket_field_under_it() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.focus(), Some(SERVER));
    let focused = row_of(&rendered(&state), SERVER);
    assert!(
        focused.contains(&format!("{} server ", tuikit::glyphs::marks::POINTER)),
        "the checkbox's row carries the cursor: {focused}"
    );
    state.handle_key(press(KeyCode::Char(' ')));
    let frame = rendered(&state);
    assert!(row_of(&frame, SERVER).contains("[x] own socket"), "{frame}");
    let lines: Vec<&str> = frame.lines().collect();
    let server = lines.iter().position(|l| is_labelled(l, SERVER)).unwrap();
    assert!(
        is_labelled(lines[server + 1], SOCKET),
        "the socket row is directly beneath:\n{frame}"
    );
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(SOCKET));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.focus(), Some(HARNESS));
}

#[test]
fn the_socket_is_passed_only_while_the_checkbox_is_on_and_is_remembered() {
    let mut state = claude_form();
    state.handle_key(press(KeyCode::Tab));
    state.handle_key(press(KeyCode::Char(' ')));
    state.handle_key(press(KeyCode::Down));
    type_str(&mut state, "work");
    assert_eq!(request(&state).mux_socket.as_deref(), Some("work"));

    // Off: the default server, but the typed text is still there.
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.focus(), Some(SERVER));
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(request(&state).mux_socket, None);
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(request(&state).mux_socket.as_deref(), Some("work"));

    // On and empty behaves as the default server.
    state.handle_key(press(KeyCode::Down));
    for _ in 0..4 {
        state.handle_key(press(KeyCode::Backspace));
    }
    assert_eq!(request(&state).mux_socket, None);
}

fn frame_at(state: &MuxLaunchForm, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    state.draw(area, &mut buf, &crate::tui::Theme::default());
    tuikit::testkit::buffer::rows(&buf)
        .iter()
        .map(|row| row.trim_end())
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
