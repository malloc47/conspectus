//! Every command runs from a launch directory that was deleted after
//! the shell entered it (ADR 0111). A child can't be spawned into a
//! missing directory, so each command goes through `sh`, which enters a
//! fresh directory, removes it, and execs `conspectus`.
#![cfg(unix)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

/// `conspectus <args>` isolated from the host like the other CLI tests,
/// started from a directory that no longer exists.
fn from_deleted_dir(home: &Path) -> Command {
    let gone = tempfile::TempDir::new().expect("launch dir").keep();
    let mut cmd = Command::new("sh");
    cmd.args(["-c", r#"cd "$1" && rmdir "$1" && shift && exec "$@""#, "sh"])
        .arg(&gone)
        .arg(assert_cmd::cargo::cargo_bin("conspectus"));
    cmd.env("HOME", home)
        .env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local").join("state"))
        .env("XDG_DATA_HOME", home.join(".local").join("share"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("CONSPECTUS_DISABLE_TMUX", "1")
        .env("CONSPECTUS_DISABLE_FORGE", "1")
        .env_remove("CONSPECTUS_CODEX_STATE")
        .env_remove("CONSPECTUS_CLAUDE_CODE_STATE")
        .env_remove("CONSPECTUS_OPENCODE_STATE")
        .env_remove("CONSPECTUS_PI_STATE")
        .env_remove("PI_CODING_AGENT_DIR")
        .env_remove("PI_CODING_AGENT_SESSION_DIR")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE");
    cmd
}

/// Kills the private tmux server under `TMUX_TMPDIR` when dropped.
struct KillServer<'a>(&'a Path);

impl Drop for KillServer<'_> {
    fn drop(&mut self) {
        let _ = std::process::Command::new("tmux")
            .env_remove("TMUX")
            .env("TMUX_TMPDIR", self.0)
            .arg("kill-server")
            .output();
    }
}

/// A planned mux slot anchored on `project`: nothing is running, so
/// the commands under test only read the slot store.
fn pin_planned(home: &Path, project: &Path) {
    from_deleted_dir(home)
        .args([
            "pin",
            "add",
            "--planned",
            "--kind",
            "mux",
            "--name",
            "ingest",
            "--set",
            "harness=codex",
            "--set",
        ])
        .arg(format!("cwd={}", project.display()))
        .assert()
        .success();
}

#[test]
fn survey_commands_succeed_from_a_deleted_directory() {
    let home = tempfile::TempDir::new().expect("home");
    for args in [
        &["graph", "--format", "json"][..],
        &["table", "sessions"],
        &["pin", "list"],
        &["history", "list"],
        &["alias", "list"],
        &["declared", "list"],
        &["worktree", "list"],
        &["refresh"],
    ] {
        from_deleted_dir(home.path())
            .args(args)
            .assert()
            .success()
            .stderr(predicate::str::contains("os error 2").not());
    }
}

#[test]
fn slots_stay_visible_from_a_deleted_directory() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    pin_planned(home.path(), project.path());

    from_deleted_dir(home.path())
        .args(["pin", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ingest"));
    from_deleted_dir(home.path())
        .args(["pin", "show", "ingest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ingest"));
}

#[test]
fn rebuild_succeeds_from_a_deleted_directory() {
    use std::os::unix::fs::PermissionsExt;

    let tmux_available = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success());
    if !tmux_available {
        return;
    }
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    let bin = tempfile::TempDir::new().expect("stub bin");
    let stub = bin.path().join("codex");
    fs::write(&stub, "#!/bin/sh\nexec sleep 600\n").expect("write stub codex");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod stub codex");
    let path = std::env::join_paths(std::iter::once(bin.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("PATH");
    // A private tmux server, killed when the test ends.
    let tmux_dir = tempfile::Builder::new()
        .prefix("tmux")
        .tempdir_in("/tmp")
        .expect("private tmux dir");
    let _server = KillServer(tmux_dir.path());

    // The TUI runs `conspectus rebuild <slot>` from its own, possibly
    // deleted, directory.
    pin_planned(home.path(), project.path());
    from_deleted_dir(home.path())
        .env("TMUX_TMPDIR", tmux_dir.path())
        .env_remove("CONSPECTUS_DISABLE_TMUX")
        .env("PATH", &path)
        .env("SHELL", "/bin/sh")
        .args(["rebuild", "ingest", "--no-attach"])
        .assert()
        .success();
    let has_session = std::process::Command::new("tmux")
        .env_remove("TMUX")
        .env("TMUX_TMPDIR", tmux_dir.path())
        .args(["has-session", "-t", "ingest"])
        .status()
        .expect("tmux");
    assert!(has_session.success(), "the rebuild started the mux");
}

#[test]
fn commands_defaulting_to_the_cwd_name_the_flag_to_pass() {
    let home = tempfile::TempDir::new().expect("home");
    for (args, flag) in [
        (&["mux", "new", "scratch", "--no-attach"][..], "--cwd"),
        (&["worktree", "new", "feature"], "--repo"),
        (
            &["hook", "init", "claude-code", "--scope", "project"],
            "--scope user",
        ),
    ] {
        from_deleted_dir(home.path())
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("no longer available"))
            .stderr(predicate::str::contains(flag));
    }
}
