use super::*;

fn state() -> LaunchSpecFormState {
    LaunchSpecFormState::new(LaunchSpecInit {
        harness_value: "codex".to_string(),
        cwd_value: "/tmp".to_string(),
        mux_name_value: "adhoc".to_string(),
        worktree_branch_value: String::new(),
        known_harness_keys: vec!["codex".to_string(), "claude-code".to_string()],
        known_mux_names: vec!["existing".to_string()],
    })
}

#[test]
fn cycle_harness_advances_and_wraps() {
    let mut s = state();
    assert_eq!(s.harness().value(), "codex");
    let next = s.cycle_harness(1).expect("cycled");
    assert_eq!(next, "claude-code");
    assert_eq!(s.harness().value(), "claude-code");
    // Wrap around from last back to first.
    let next = s.cycle_harness(1).expect("cycled");
    assert_eq!(next, "codex");
}

#[test]
fn cycle_harness_returns_none_when_known_set_is_empty() {
    let mut s = LaunchSpecFormState::new(LaunchSpecInit {
        harness_value: "codex".to_string(),
        cwd_value: "/tmp".to_string(),
        mux_name_value: "adhoc".to_string(),
        worktree_branch_value: String::new(),
        known_harness_keys: Vec::new(),
        known_mux_names: Vec::new(),
    });
    assert!(s.cycle_harness(1).is_none());
    assert_eq!(s.harness().value(), "codex");
}

#[test]
fn mux_name_collides_flags_known_names() {
    let mut s = state();
    s.set_mux_name("existing");
    assert!(s.mux_name_collides_with_known());
    assert_eq!(s.matching_known_mux_name(), Some("existing"));
}

#[test]
fn toggle_worktree_flips_the_flag() {
    let mut s = state();
    assert!(!s.worktree_enabled());
    assert!(s.toggle_worktree());
    assert!(s.worktree_enabled());
    assert!(!s.toggle_worktree());
}

#[test]
fn parse_launch_argv_handles_single_quotes() {
    let out = parse_launch_argv("codex --model 'gpt-5 pro'", "test").expect("parse");
    assert_eq!(out, vec!["codex", "--model", "gpt-5 pro"]);
}

#[test]
fn parse_launch_argv_rejects_unclosed_quote() {
    let err = parse_launch_argv("codex --flag 'oops", "test").expect_err("unclosed");
    assert!(err.contains("unclosed"));
}

#[test]
fn parse_launch_argv_empty_input_is_empty_vec() {
    assert!(parse_launch_argv("", "test").unwrap().is_empty());
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| (*p).to_string()).collect()
}

#[test]
fn toggling_skip_permissions_adds_and_removes_the_flag() {
    let mut s = state();
    let default = argv(&["codex"]);
    assert_eq!(s.launch_options().len(), 1);
    s.toggle_launch_option(&default, 1);
    assert_eq!(
        s.launch_argv().value(),
        "codex --dangerously-bypass-approvals-and-sandbox"
    );
    assert!(s.launch_option_selected(s.launch_options()[0], &default));
    s.toggle_launch_option(&default, 1);
    assert_eq!(s.launch_argv().value(), "", "back to following the default");
    assert!(!s.launch_option_selected(s.launch_options()[0], &default));
}

#[test]
fn an_option_toggle_keeps_the_operators_other_tokens() {
    let mut s = state();
    s.set_launch_argv("sandbox run codex");
    s.toggle_launch_option(&argv(&["codex"]), 1);
    assert_eq!(
        s.launch_argv().value(),
        "sandbox run codex --dangerously-bypass-approvals-and-sandbox"
    );
}

#[test]
fn a_harness_without_options_ignores_the_toggle() {
    let mut s = state();
    s.set_harness("opencode");
    assert!(s.launch_options().is_empty());
    s.toggle_launch_option(&argv(&["opencode"]), 1);
    assert_eq!(s.launch_argv().value(), "");
}

#[test]
fn cycling_the_harness_drops_the_old_harnesss_flags() {
    let mut s = state();
    let default_for = |harness: &str| {
        argv(&[if harness == "codex" {
            "codex"
        } else {
            "claude"
        }])
    };
    s.toggle_launch_option(&default_for("codex"), 1);
    s.cycle_harness_keeping_argv(1, &default_for);
    assert_eq!(s.harness().value(), "claude-code");
    assert_eq!(
        s.launch_argv().value(),
        "",
        "an argv that was only the old default and its flag is cleared"
    );
    s.set_launch_argv("claude --model opus");
    s.cycle_harness_keeping_argv(1, &default_for);
    assert_eq!(s.harness().value(), "codex");
    assert_eq!(
        s.launch_argv().value(),
        "claude --model opus",
        "a custom argv stays"
    );
}

#[test]
fn optional_string_trims_and_filters_empty() {
    assert!(optional_string("  ").is_none());
    assert_eq!(optional_string(" foo ").as_deref(), Some("foo"));
}
