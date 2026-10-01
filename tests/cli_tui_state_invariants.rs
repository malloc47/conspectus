//! Read-only invariant smoke for the F8-013 TUI state file
//! (`$XDG_STATE_HOME/conspectus/tui-state.json`).
//!
//! The state file persists the last-active TUI view across
//! `conspectus tui` restarts. It is **rebuildable cache**, not
//! authoritative state — the worst case is the operator starts in
//! the configured default view instead of their last-active one.
//!
//! That posture means only the interactive TUI itself should
//! create or move the file. Every other surface
//! (`graph` / `table` / `query` / `node show` / `pin *`) must
//! leave the file byte-identical: no creation when absent, no
//! content or mtime drift when present.
//!
//! Mirrors the read-only invariant pattern in
//! `cli_pin_invariants.rs` and
//! `cli_pin_resume_invariants.rs`.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    // Point XDG_RUNTIME_DIR at a nonexistent path so the CLI cannot
    // find and defer to a real `conspectus serve` socket — otherwise
    // the tests silently pull the operator daemon's snapshot instead
    // of exercising the local discovery + render path.
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("XDG_STATE_HOME", home.join(".local").join("state"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

fn state_dir(home: &Path) -> PathBuf {
    home.join(".local").join("state").join("conspectus")
}

fn state_path(home: &Path) -> PathBuf {
    state_dir(home).join("tui-state.json")
}

fn fingerprint(path: &Path) -> (Vec<u8>, SystemTime) {
    let bytes = fs::read(path).expect("read tui-state");
    let mtime = fs::metadata(path)
        .expect("stat tui-state")
        .modified()
        .expect("mtime");
    (bytes, mtime)
}

/// Build a tui-state.json on disk with a known last_view so the
/// "do not perturb existing file" assertions have something to
/// guard.
fn seed_existing_state(home: &Path) {
    fs::create_dir_all(state_dir(home)).expect("mkdir state");
    fs::write(
        state_path(home),
        r#"{
  "schema_version": 1,
  "last_view": "mux"
}
"#,
    )
    .expect("seed tui-state.json");
}

#[test]
fn graph_does_not_create_state_file_when_absent() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["graph"])
        .assert()
        .success();

    assert!(
        !state_path(home.path()).exists(),
        "graph created the tui-state file out of thin air",
    );
    assert!(
        !state_dir(home.path()).exists(),
        "graph created the state directory just to write a tui-state file",
    );
}

#[test]
fn table_does_not_create_state_file_when_absent() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["table", "sessions"])
        .assert()
        .success();

    assert!(
        !state_path(home.path()).exists(),
        "table sessions created the tui-state file out of thin air",
    );
}

#[test]
fn pin_list_does_not_create_state_file_when_absent() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list"])
        .assert()
        .success();

    assert!(
        !state_path(home.path()).exists(),
        "pin list created the tui-state file out of thin air",
    );
}

#[test]
fn read_only_commands_leave_existing_state_file_byte_identical() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    seed_existing_state(home.path());
    let (bytes_before, mtime_before) = fingerprint(&state_path(home.path()));

    // Sleep a beat so any erroneous mtime bump would actually
    // register on coarse-grained filesystems.
    std::thread::sleep(std::time::Duration::from_millis(50));

    for args in [
        ["graph"].as_slice(),
        ["table", "sessions"].as_slice(),
        ["table", "mux"].as_slice(),
        ["pin", "list"].as_slice(),
    ] {
        isolated_cmd(home.path())
            .current_dir(project.path())
            .args(args)
            .assert()
            .success();

        let (bytes_after, mtime_after) = fingerprint(&state_path(home.path()));
        assert_eq!(
            bytes_after, bytes_before,
            "command `{args:?}` changed the tui-state file payload",
        );
        assert_eq!(
            mtime_after, mtime_before,
            "command `{args:?}` bumped the tui-state file mtime",
        );
    }
}
