//! CLI smoke coverage for the `conspectus pin` command tree.
//!
//! Tests run the assembled binary against an isolated `HOME` so the
//! user-store path is sandboxed and no real harness/tmux state leaks
//! in. Discovery is suppressed via the same env flags the rest of the
//! CLI smoke suite uses.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    // Point XDG_RUNTIME_DIR at a nonexistent path so the CLI cannot
    // find and defer to a real `conspectus serve` socket — otherwise
    // the tests silently pull the operator daemon's snapshot instead
    // of exercising the local discovery + render path.
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

#[test]
fn pin_help_lists_subcommands() {
    let home = tempfile::TempDir::new().expect("home");
    isolated_cmd(home.path())
        .args(["pin", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("rename"))
        .stdout(predicate::str::contains("rm"));
}

#[test]
fn pin_create_writes_project_config_and_list_reads_it_back() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .args(["--mux-name", "ingest-mux"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wrote pin `ingest`"));

    let written =
        fs::read_to_string(project.path().join(".conspectus.toml")).expect("project config exists");
    assert!(written.contains("[pins]"));
    assert!(written.contains(r#"id = "ingest""#));
    assert!(written.contains(r#"name = "ingest-mux""#));
    assert!(written.contains(r#"harness = "codex""#));

    let assert = isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("ingest"), "list missing pin id: {stdout}");
    assert!(
        stdout.contains("tmux:ingest-mux"),
        "list missing mux native id: {stdout}"
    );
    assert!(
        stdout.contains("project"),
        "list missing store column: {stdout}"
    );
    // No live mux exists in this sandboxed run, so the pin should
    // surface as `unbound`.
    assert!(
        stdout.contains("unbound"),
        "expected unbound state: {stdout}"
    );
}

#[test]
fn pin_show_renders_resolver_state() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    let assert = isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "show", "ingest"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(
        stdout.contains("id           ingest"),
        "missing id row: {stdout}"
    );
    assert!(stdout.contains("harness      codex"));
    assert!(stdout.contains("mux          tmux:ingest"));
    assert!(stdout.contains("state        unbound"));
    assert!(
        stdout.contains("diagnostic   unbound"),
        "show should surface PinUnbound diagnostic: {stdout}"
    );
}

#[test]
fn pin_show_fails_on_unknown_id() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "show", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no pin `missing`"));
}

#[test]
fn pin_rename_changes_id_and_display_name() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "old-id", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .args(["--mux-name", "old-mux"])
        .assert()
        .success();

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args([
            "pin",
            "rename",
            "old-id",
            "new-id",
            "--display",
            "Visible Name",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("renamed pin `old-id` → `new-id`"));

    let contents =
        fs::read_to_string(project.path().join(".conspectus.toml")).expect("config exists");
    assert!(contents.contains(r#"id = "new-id""#));
    assert!(contents.contains(r#"display_name = "Visible Name""#));
    assert!(!contents.contains(r#"id = "old-id""#));
}

#[test]
fn pin_rename_with_no_changes_errors() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "rename", "ingest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "requires either a new id, `--display <name>`, or both",
        ));
}

#[test]
fn pin_rm_removes_entry_and_subsequent_show_fails() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "rm", "ingest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed pin `ingest`"));

    // The file should now be gone (single-entry remove unlinks).
    assert!(!project.path().join(".conspectus.toml").exists());

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "show", "ingest"])
        .assert()
        .failure();
}

#[test]
fn pin_rm_unknown_id_errors() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "rm", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no pin `missing`"));
}

#[test]
fn pin_create_rejects_relative_cwd() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args([
            "pin",
            "create",
            "ingest",
            "--harness",
            "codex",
            "--cwd",
            "relative/path",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("absolute path"));
}

#[test]
fn pin_create_with_store_user_writes_user_config() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args([
            "pin",
            "create",
            "global-ingest",
            "--harness",
            "codex",
            "--cwd",
        ])
        .arg(project.path())
        .args(["--store", "user"])
        .assert()
        .success();

    let user_config = home.path().join(".config/conspectus/config.toml");
    assert!(user_config.is_file(), "user config should be created");
    let written = fs::read_to_string(&user_config).expect("user config");
    assert!(written.contains("[pins]"));
    assert!(written.contains(r#"id = "global-ingest""#));

    // Project config should NOT have been touched.
    assert!(!project.path().join(".conspectus.toml").exists());
}

#[test]
fn pin_create_store_all_is_rejected() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .args(["--store", "all"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'all' for '--store"));
}

#[test]
fn pin_launch_help_lists_no_attach_flag() {
    let home = tempfile::TempDir::new().expect("home");
    isolated_cmd(home.path())
        .args(["pin", "launch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--no-attach"));
}

#[test]
fn pin_launch_fails_on_unknown_id() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "launch", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no pin `missing`"));
}

/// A tmux server private to one test. `TMUX` is cleared so tmux cannot
/// reach the operator's server through an inherited client, and
/// `TMUX_TMPDIR` puts the socket in a fresh directory. Dropping the guard
/// kills the server and anything still running in it.
#[cfg(unix)]
struct PrivateTmux {
    dir: tempfile::TempDir,
}

#[cfg(unix)]
impl PrivateTmux {
    fn new() -> Self {
        // Unix socket paths are capped near 100 bytes, so keep the socket
        // directory short rather than under a possibly long `$TMPDIR`.
        let dir = tempfile::Builder::new()
            .prefix("tmux")
            .tempdir_in("/tmp")
            .expect("private tmux dir");
        Self { dir }
    }

    fn apply(&self, cmd: &mut Command) {
        cmd.env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .env("TMUX_TMPDIR", self.dir.path());
    }

    fn available() -> bool {
        std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .is_ok_and(|output| output.status.success())
    }
}

#[cfg(unix)]
impl Drop for PrivateTmux {
    fn drop(&mut self) {
        let _ = std::process::Command::new("tmux")
            .env_remove("TMUX")
            .env("TMUX_TMPDIR", self.dir.path())
            .arg("kill-server")
            .output();
    }
}

#[cfg(unix)]
#[test]
fn pin_launch_no_attach_prints_attach_command_when_unbound() {
    use std::os::unix::fs::PermissionsExt;

    // The launch goes to a private tmux server and runs a stub `codex`
    // that stays alive, so the outcome does not depend on which harnesses
    // the host has installed or on sessions already running there.
    // `CONSPECTUS_DISABLE_TMUX` turns off discovery but not the launch path.
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
    let tmux = PrivateTmux::new();

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    let mut launch = isolated_cmd(home.path());
    tmux.apply(&mut launch);
    // The pane runs the harness through `$SHELL -c`; plain `sh` keeps
    // shell startup files from rewriting `PATH` before the stub is found.
    let launch = launch
        .current_dir(project.path())
        .env("PATH", &path)
        .env("SHELL", "/bin/sh")
        .args(["pin", "launch", "ingest", "--no-attach"])
        .assert();
    if PrivateTmux::available() {
        launch
            .success()
            .stdout(predicate::str::contains("spawned `ingest` (detached)"));
    } else {
        launch
            .failure()
            .stderr(predicate::str::contains("tmux is unavailable"));
    }
}

#[test]
fn pin_bind_fails_on_unknown_pin() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "bind", "missing", "--to", "alpha"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no pin `missing`"));
}

#[test]
fn pin_bind_fails_when_session_not_in_discovery() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    // Sandboxed discovery sees no agent sessions, so any `--to` is
    // unknown and bind should refuse rather than fabricate a session.
    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "bind", "ingest", "--to", "no-such-session"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no `codex` agent session"));
}

#[test]
fn pin_rebind_updates_mux_name_and_socket() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .args(["--mux-name", "old-mux"])
        .assert()
        .success();

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args([
            "pin",
            "rebind",
            "ingest",
            "--mux",
            "new-mux",
            "--mux-socket",
            "scratch",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "rebound pin `ingest` → mux `tmux:scratch:new-mux`",
        ));

    let contents =
        fs::read_to_string(project.path().join(".conspectus.toml")).expect("config exists");
    assert!(contents.contains(r#"name = "new-mux""#));
    assert!(contents.contains(r#"socket_name = "scratch""#));
    assert!(!contents.contains(r#"name = "old-mux""#));
}

#[test]
fn pin_rebind_fails_on_unknown_pin() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "rebind", "missing", "--mux", "anything"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no pin `missing`"));
}

#[test]
fn pin_adopt_fails_when_mux_not_live() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args([
            "pin",
            "adopt",
            "ingest",
            "no-such-mux",
            "--harness",
            "codex",
            "--cwd",
        ])
        .arg(project.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no live mux"));
}

#[test]
fn pin_list_filters_by_state() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "create", "ingest", "--harness", "codex", "--cwd"])
        .arg(project.path())
        .assert()
        .success();

    // Only the `unbound` filter should match this sandboxed pin.
    let bound_output = isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list", "--state", "bound"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(bound_output.is_empty(), "no bound pins expected in sandbox");

    let unbound_output = isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list", "--state", "unbound"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&unbound_output).contains("ingest"));
}
