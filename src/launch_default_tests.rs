use std::fs;

use super::*;
use crate::config::{ConfigLoader, LaunchProfile};

fn config(user: Option<&[&str]>, project: Option<&[&str]>) -> LaunchConfig {
    let mut config = LaunchConfig::default();
    for (argv, origin) in [
        (user, ProfileOrigin::User),
        (project, ProfileOrigin::Project),
    ] {
        if let Some(argv) = argv {
            config.profiles.insert(
                "claude-code".into(),
                LaunchProfile {
                    argv: argv.iter().map(|a| (*a).to_string()).collect(),
                    origin,
                },
            );
        }
    }
    config
}

fn strings(argv: &[OsString]) -> Vec<String> {
    argv.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn atelier_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("atelier.toml"),
        "[workspace]\nname = \"w\"\n",
    )
    .unwrap();
    dir
}

type Case<'a> = (
    Option<&'a [&'a str]>,
    Option<&'a [&'a str]>,
    bool,
    DefaultSource,
    &'a str,
);

#[test]
fn layers_apply_in_precedence_order() {
    let ws = atelier_dir();
    let cwd = Some(ws.path());
    let harness = strings(&launch_argv_for("claude-code"));

    // (user, project, atelier on PATH) -> (argv head, source)
    let cases: [Case<'_>; 5] = [
        (
            Some(&["u"]),
            Some(&["p"]),
            true,
            DefaultSource::ProjectProfile,
            "p",
        ),
        (Some(&["u"]), None, true, DefaultSource::UserProfile, "u"),
        (None, None, true, DefaultSource::AtelierExec, "atelier"),
        (
            None,
            None,
            false,
            DefaultSource::HarnessDefault,
            harness[0].as_str(),
        ),
        (
            None,
            Some(&["p"]),
            false,
            DefaultSource::ProjectProfile,
            "p",
        ),
    ];
    for (user, project, atelier, source, head) in cases {
        let defaults = LaunchDefaults::new(config(user, project)).with_atelier(atelier);
        let (argv, got) = defaults.default_argv("claude-code", cwd).unwrap();
        assert_eq!(got, source, "{user:?} {project:?} {atelier}");
        assert_eq!(strings(&argv)[0], head);
    }
}

#[test]
fn atelier_wraps_the_harness_default_only_inside_a_workspace() {
    let ws = atelier_dir();
    let outside = tempfile::tempdir().unwrap();
    let defaults = LaunchDefaults::new(LaunchConfig::default()).with_atelier(true);
    let harness = strings(&launch_argv_for("claude-code"));

    let (argv, source) = defaults
        .default_argv("claude-code", Some(ws.path()))
        .unwrap();
    assert_eq!(source, DefaultSource::AtelierExec);
    assert_eq!(&strings(&argv)[..2], ["atelier", "exec"]);
    assert_eq!(strings(&argv)[2..], harness[..]);

    let (_, source) = defaults
        .default_argv("claude-code", Some(outside.path()))
        .unwrap();
    assert_eq!(source, DefaultSource::HarnessDefault);
    let (_, source) = defaults.default_argv("claude-code", None).unwrap();
    assert_eq!(source, DefaultSource::HarnessDefault);
}

#[test]
fn a_profile_wins_over_atelier_and_may_be_a_wrapper() {
    let ws = atelier_dir();
    let defaults = LaunchDefaults::new(config(Some(&["nix", "develop", "-c", "claude"]), None))
        .with_atelier(true);
    let (argv, source) = defaults
        .default_argv("claude-code", Some(ws.path()))
        .unwrap();
    assert_eq!(source, DefaultSource::UserProfile);
    assert_eq!(strings(&argv), ["nix", "develop", "-c", "claude"]);
}

#[test]
fn an_unknown_harness_has_no_default() {
    let defaults = LaunchDefaults::new(LaunchConfig::default()).with_atelier(true);
    assert!(defaults.default_argv("never-registered", None).is_none());
}

#[test]
fn profiles_load_from_both_files_with_the_project_winning() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let xdg = dir.path().join("xdg");
    let project = dir.path().join("proj");
    fs::create_dir_all(xdg.join("conspectus")).unwrap();
    fs::create_dir_all(&project).unwrap();
    fs::write(
        xdg.join("conspectus/config.toml"),
        "[launch.claude-code]\nargv = [\"claude\", \"--user\"]\n[launch.codex]\nargv = [\"codex\", \"--user\"]\n",
    )
    .unwrap();
    fs::write(
        project.join(".conspectus.toml"),
        "[launch.claude-code]\nargv = [\"claude\", \"--project\"]\n",
    )
    .unwrap();
    let loader = ConfigLoader::new()
        .with_home(&home)
        .with_xdg_config_home(&xdg);

    let outcome = loader.load(Some(&project));

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let profiles = &outcome.config.launch.profiles;
    assert_eq!(profiles["claude-code"].argv, ["claude", "--project"]);
    assert_eq!(profiles["claude-code"].origin, ProfileOrigin::Project);
    assert_eq!(profiles["codex"].origin, ProfileOrigin::User);
}

#[test]
fn bad_profiles_are_diagnosed_and_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    fs::create_dir_all(xdg.join("conspectus")).unwrap();
    fs::write(
        xdg.join("conspectus/config.toml"),
        "[launch.nonsense]\nargv = [\"x\"]\n[launch.codex]\nargv = []\n[launch.claude-code]\nflags = 1\n",
    )
    .unwrap();
    let loader = ConfigLoader::new()
        .with_home(dir.path())
        .with_xdg_config_home(&xdg);

    let outcome = loader.load(None);

    assert!(outcome.config.launch.profiles.is_empty());
    let messages: Vec<_> = outcome
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("launch.nonsense") && m.contains("no known harness"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("launch.codex") && m.contains("non-empty `argv`"))
    );
    assert!(messages.iter().any(|m| m.contains("launch.claude-code")));
}
