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
        .stderr(predicate::str::contains("`--store all` is not valid"));
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

#[test]
fn pin_launch_no_attach_prints_attach_command_when_unbound() {
    // When a real tmux isn't available on the host (CONSPECTUS_DISABLE_TMUX
    // turns discovery off but does not gate the launch path), the
    // `--no-attach` branch still exercises the new_session call. On a
    // host without tmux we expect the launch to fail with the
    // "unavailable" message; on a host with tmux it succeeds. Either
    // outcome confirms the unbound branch is being exercised — the
    // test asserts the message is one of those, not a "no pin" error.
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
        .args(["pin", "launch", "ingest", "--no-attach"])
        .assert();
    // Either tmux is present (spawned + printed attach hint) or
    // missing (graceful "unavailable" failure). What we don't want is
    // a "no pin" failure or an unrelated argv error.
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("spawned `ingest`")
            || combined.contains("tmux is unavailable")
            || combined.contains("a tmux session named")
            || combined.contains("tmux new-session failed"),
        "unexpected launch output: stdout={stdout} stderr={stderr}",
    );
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
