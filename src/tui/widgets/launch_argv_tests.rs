use super::*;
use crate::discovery::harness::launch_options_for;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

fn skip_permissions(harness: &str) -> HarnessLaunchOption {
    launch_options_for(harness)[0]
}

/// Makes `dir` an Atelier workspace, as `atelier` would.
fn make_workspace(dir: &Path) {
    std::fs::write(
        dir.join(crate::discovery::atelier::ATELIER_CONFIG_FILENAME),
        "[workspace]\nname = \"w\"\n",
    )
    .unwrap();
}

#[test]
fn an_empty_field_launches_the_default() {
    assert_eq!(effective(Vec::new(), &argv(&["codex"])), ["codex"]);
    assert_eq!(effective(argv(&["mine"]), &argv(&["codex"])), ["mine"]);
}

#[test]
fn an_option_adds_and_removes_its_flags_and_keeps_the_other_tokens() {
    let option = skip_permissions("codex");
    let on = with_option(argv(&["sandbox", "run", "codex"]), option, true);
    assert_eq!(
        on,
        [
            "sandbox",
            "run",
            "codex",
            "--dangerously-bypass-approvals-and-sandbox"
        ]
    );
    assert_eq!(options_on(launch_options_for("codex"), &on), [true]);
    let off = with_option(on, option, false);
    assert_eq!(off, ["sandbox", "run", "codex"]);
    assert_eq!(options_on(launch_options_for("codex"), &off), [false]);
}

#[test]
fn a_field_that_holds_only_the_default_stays_empty() {
    let default = argv(&["codex"]);
    assert_eq!(field_text(&default, &default), "");
    assert_eq!(field_text(&[], &default), "");
    assert_eq!(
        field_text(&argv(&["codex", "--model", "gpt 5"]), &default),
        "codex --model 'gpt 5'"
    );
}

#[test]
fn an_argv_is_written_so_it_splits_back_to_itself() {
    let written = argv(&["claude", "--model", "opus 4", "", "it's", "a@b,c"]);
    assert_eq!(
        display(&written),
        r"claude --model 'opus 4' '' 'it'\''s' a@b,c"
    );
    assert_eq!(crate::argv::split_command_line(&display(&written)), written);
}

#[test]
fn a_harness_change_takes_the_old_flags_off_and_follows_the_new_default() {
    let claude = argv(&["claude"]);
    let codex = argv(&["codex"]);
    let flagged = with_option(claude.clone(), skip_permissions("claude-code"), true);
    assert_eq!(
        carried(flagged, &claude, &codex),
        codex,
        "an argv that was only the old default and its flag follows the new default"
    );
    assert_eq!(
        carried(argv(&["claude", "--model", "opus"]), &claude, &codex),
        ["claude", "--model", "opus"],
        "an argv of the operator's own stays"
    );
}

#[test]
fn the_default_is_decided_once_until_the_harness_or_the_directory_changes() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().display().to_string();
    let defaults = LaunchDefaults::default().with_atelier(true);
    let mut default = DefaultArgv::resolve(Some(&defaults), "claude-code", &cwd);
    assert_eq!(default.source(), Some(DefaultSource::HarnessDefault));
    assert_eq!(default.argv(), ["claude"]);

    // The directory becomes a workspace; with the same harness and
    // directory the kept answer stands, so nothing reads the disk again.
    make_workspace(dir.path());
    assert!(!default.refresh(Some(&defaults), "claude-code", &cwd));
    assert_eq!(default.source(), Some(DefaultSource::HarnessDefault));

    // A change of directory (here, the same one typed differently) asks
    // again and sees the workspace.
    assert!(default.refresh(Some(&defaults), "claude-code", &format!("{cwd}/")));
    assert_eq!(default.source(), Some(DefaultSource::AtelierExec));
    assert_eq!(default.argv(), ["atelier", "exec", "claude"]);
    assert!(
        default
            .hint()
            .is_some_and(|hint| hint.starts_with("atelier exec claude — from"))
    );

    assert!(default.refresh(Some(&defaults), "codex", &format!("{cwd}/")));
    assert_eq!(default.argv(), ["atelier", "exec", "codex"]);
}

#[test]
fn without_launch_defaults_the_harness_default_has_no_hint() {
    let default = DefaultArgv::resolve(None, "codex", "/tmp");
    assert_eq!(default.argv(), ["codex"]);
    assert_eq!(default.hint(), None);
    let unknown = DefaultArgv::resolve(Some(&LaunchDefaults::default()), "never-registered", "");
    assert!(unknown.argv().is_empty());
    assert_eq!(unknown.hint(), None);
}
