//! Read-only invariant smoke for the TUI state file
//! (`$XDG_STATE_HOME/conspectus/tui-state.json`).
//!
//! The state file persists the last-active TUI view, and each view's
//! settings, expanded rows, and selected row, across `conspectus tui`
//! restarts. It is **rebuildable cache**, not
//! authoritative state — the worst case is the operator starts in
//! the configured default view instead of their last-active one.
//!
//! That posture means only the interactive TUI itself should
//! create or move the file. Every other surface
//! (`graph` / `table` / `query` / `node show` / `pin *`, and
//! `tui --snapshot`) must leave the file byte-identical: no creation
//! when absent, no content or mtime drift when present. A snapshot
//! also reopens nothing the file remembers, so it stays deterministic.
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
    // Pin every XDG base directory under the temp home: CI runners set
    // XDG_CONFIG_HOME, and any XDG variable inherited from the host would
    // send the CLI's writes to the developer's real config, state, data,
    // or cache.
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_STATE_HOME", home.join(".local").join("state"));
    cmd.env("XDG_DATA_HOME", home.join(".local").join("share"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd.env_remove("CONSPECTUS_PI_STATE");
    cmd.env_remove("PI_CODING_AGENT_DIR");
    cmd.env_remove("PI_CODING_AGENT_SESSION_DIR");
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

/// A state file that remembers the showcase's `project` group expanded
/// and one of its sessions selected. The TUI's own tests check that a
/// live run reopens exactly that (`src/tui/app/restore_tests.rs`).
#[cfg(feature = "snapshot")]
const REMEMBERED_ROWS: &str = r#"{
  "schema_version": 1,
  "row_keys": 1,
  "view_states": {
    "sessions": {
      "expanded": ["group:repo:/fixture/repos/project/.git"],
      "selected": {
        "row": "session:agent_session:claude-code:/fixture/harness/claude:showcase-claude",
        "path": ["group:repo:/fixture/repos/project/.git"]
      }
    }
  }
}"#;

/// The showcase's left pane as `tui --snapshot` draws it, with `home`'s
/// state directory.
#[cfg(feature = "snapshot")]
fn showcase_left_pane(home: &Path) -> String {
    let output = isolated_cmd(home)
        .current_dir(home)
        .args(["tui", "--snapshot", "--snapshot-fixture"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/showcase.json"
        ))
        .args([
            // The showcase's anchor time, so ages read the same in both runs.
            "--snapshot-now",
            "1790769600",
            "--snapshot-pane",
            "left",
            "--snapshot-width",
            "150",
            "--snapshot-height",
            "20",
        ])
        .output()
        .expect("run snapshot");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(feature = "snapshot")]
#[test]
fn snapshot_neither_reopens_nor_rewrites_remembered_rows() {
    let plain = tempfile::TempDir::new().expect("home");
    let expected = showcase_left_pane(plain.path());
    assert!(
        !state_path(plain.path()).exists(),
        "a snapshot creates no state file"
    );

    let home = tempfile::TempDir::new().expect("home");
    fs::create_dir_all(state_dir(home.path())).expect("mkdir state");
    fs::write(state_path(home.path()), REMEMBERED_ROWS).expect("seed tui-state.json");
    let (bytes_before, mtime_before) = fingerprint(&state_path(home.path()));
    std::thread::sleep(std::time::Duration::from_millis(50));

    assert_eq!(
        showcase_left_pane(home.path()),
        expected,
        "the snapshot opens on the default folds and first row"
    );
    let (bytes_after, mtime_after) = fingerprint(&state_path(home.path()));
    assert_eq!(bytes_after, bytes_before);
    assert_eq!(mtime_after, mtime_before);
}
