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

fn pin_create(home: &Path, project: &Path, extra: &[&str]) {
    from_deleted_dir(home)
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project)
        .args(extra)
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
fn user_store_pins_stay_visible_from_a_deleted_directory() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    pin_create(home.path(), project.path(), &["--store", "user"]);

    from_deleted_dir(home.path())
        .args(["pin", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ingest"));
}

#[test]
fn project_pin_is_found_through_its_scan_root_from_a_deleted_directory() {
    // The TUI launches a pin by running `conspectus pin launch <id>
    // --scan-root <pin cwd>` from its own, possibly deleted, directory.
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    pin_create(home.path(), project.path(), &["--store", "project"]);
    assert!(project.path().join(".conspectus.toml").is_file());

    from_deleted_dir(home.path())
        .args(["pin", "show", "ingest", "--scan-root"])
        .arg(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("ingest"));
}

#[test]
fn pin_launch_succeeds_from_a_deleted_directory() {
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

    // A project pin, launched the way the TUI does it.
    pin_create(home.path(), project.path(), &["--store", "project"]);
    from_deleted_dir(home.path())
        .env("TMUX_TMPDIR", tmux_dir.path())
        .env("PATH", &path)
        .env("SHELL", "/bin/sh")
        .args(["pin", "launch", "ingest", "--no-attach", "--scan-root"])
        .arg(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("spawned `ingest` (detached)"));
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
