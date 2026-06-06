//! Read-only invariant smoke for the pin-binding sidecar
//! (`$XDG_CACHE_HOME/conspectus/pin-bindings/`).
//!
//! ADR 0058 makes the sidecar a *cache*, not state — discovery cycles
//! write it whenever a pin's binding settles to `Bound`. The cache
//! should NOT, however, be created or perturbed by commands that
//! couldn't produce a bound resolution in the first place:
//!
//! - With no pins configured, no read-only command should create
//!   the cache directory.
//! - With a pin that resolves to `Unbound` (no live mux), no
//!   read-only command should write a sidecar for it.
//! - When an existing sidecar is on disk and the current cycle
//!   would skip-on-unchanged, no read-only command should bump its
//!   mtime.
//!
//! Mirrors `cli_pin_invariants.rs` (H-PIN-019) for the cache surface.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn isolated_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

fn sidecar_dir(home: &Path) -> PathBuf {
    home.join(".cache").join("conspectus").join("pin-bindings")
}

fn sidecar_path(home: &Path, pin_id: &str) -> PathBuf {
    sidecar_dir(home).join(format!("{pin_id}.json"))
}

/// Project config with one unbound pin (no live mux to attribute to).
const PROJECT_CONFIG_UNBOUND: &str = r#"[pins]
schema_version = 1

[[pins.entries]]
id = "ingest"
display_name = "ingest"
harness = "codex"
cwd = "/tmp/conspectus-test"
mux = { backend = "tmux", name = "ingest" }
"#;

fn write_project_config(project: &Path, body: &str) {
    fs::write(project.join(".conspectus.toml"), body).expect("write project config");
}

fn fingerprint(path: &Path) -> (Vec<u8>, SystemTime) {
    let bytes = fs::read(path).expect("read sidecar");
    let mtime = fs::metadata(path)
        .expect("stat sidecar")
        .modified()
        .expect("mtime");
    (bytes, mtime)
}

#[test]
fn pin_list_does_not_create_sidecar_dir_when_no_pins_exist() {
    // No `.conspectus.toml`, no global pins → no Bound resolutions
    // possible. The cache directory must not be conjured into
    // existence.
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list"])
        .assert()
        .success();

    assert!(
        !sidecar_dir(home.path()).exists(),
        "pin list created {} despite no pins existing",
        sidecar_dir(home.path()).display(),
    );
}

#[test]
fn graph_does_not_create_sidecar_dir_when_no_pins_exist() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["graph", "--format", "json"])
        .assert()
        .success();

    assert!(
        !sidecar_dir(home.path()).exists(),
        "graph created sidecar dir without a Bound pin",
    );
}

#[test]
fn pin_show_on_unbound_pin_does_not_write_a_sidecar() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    write_project_config(project.path(), PROJECT_CONFIG_UNBOUND);

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "show", "ingest"])
        .assert()
        .success();

    assert!(
        !sidecar_path(home.path(), "ingest").exists(),
        "pin show wrote a sidecar for an unbound pin",
    );
}

#[test]
fn pin_list_with_unbound_pin_does_not_write_a_sidecar() {
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    write_project_config(project.path(), PROJECT_CONFIG_UNBOUND);

    isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "list"])
        .assert()
        .success();

    assert!(
        !sidecar_path(home.path(), "ingest").exists(),
        "pin list wrote a sidecar for an unbound pin",
    );
}

#[test]
fn read_only_commands_preserve_existing_sidecars_byte_for_byte() {
    // Pre-seed a sidecar (as a prior cycle would have). The pin is
    // unbound in this run (no live mux), so the post-resolve write
    // pass should never fire for it — meaning every read-only
    // command must leave the existing sidecar untouched in content
    // and mtime.
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    write_project_config(project.path(), PROJECT_CONFIG_UNBOUND);

    let dir = sidecar_dir(home.path());
    fs::create_dir_all(&dir).expect("mkdir sidecar dir");
    let path = sidecar_path(home.path(), "ingest");
    let body = r#"{
  "schema_version": 1,
  "pin_id": "ingest",
  "mux_name": "ingest",
  "session_id": "session-a",
  "harness": "codex",
  "observed_epoch": 1738742400
}
"#;
    fs::write(&path, body).expect("seed sidecar");
    // Push the mtime back so an accidental write would visibly bump it.
    std::thread::sleep(std::time::Duration::from_millis(50));

    let before = fingerprint(&path);

    for args in [
        vec!["pin", "list"],
        vec!["pin", "show", "ingest"],
        vec!["graph", "--format", "json"],
        vec!["table", "sessions"],
    ] {
        isolated_cmd(home.path())
            .current_dir(project.path())
            .args(&args)
            .assert()
            .success();

        let after = fingerprint(&path);
        assert_eq!(
            before.0,
            after.0,
            "{:?} mutated sidecar content at {}",
            args,
            path.display(),
        );
        assert_eq!(
            before.1,
            after.1,
            "{:?} bumped sidecar mtime at {}",
            args,
            path.display(),
        );
    }
}

#[test]
fn pin_show_surfaces_last_session_when_sidecar_present() {
    // Positive smoke: with a recorded sidecar, `pin show` for an
    // unbound pin should print the `last_session` line so the
    // operator can see what `pin launch` would resume into.
    let home = tempfile::TempDir::new().expect("home");
    let project = tempfile::TempDir::new().expect("project");
    write_project_config(project.path(), PROJECT_CONFIG_UNBOUND);

    let dir = sidecar_dir(home.path());
    fs::create_dir_all(&dir).expect("mkdir sidecar dir");
    let path = sidecar_path(home.path(), "ingest");
    fs::write(
        &path,
        r#"{
  "schema_version": 1,
  "pin_id": "ingest",
  "mux_name": "ingest",
  "session_id": "session-resumable",
  "harness": "codex",
  "observed_epoch": 1704067200
}
"#,
    )
    .expect("seed sidecar");

    let output = isolated_cmd(home.path())
        .current_dir(project.path())
        .args(["pin", "show", "ingest"])
        .output()
        .expect("invoke pin show");
    assert!(
        output.status.success(),
        "pin show exited non-zero: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("last_session session-resumable"),
        "pin show stdout missing last_session line:\n{stdout}",
    );
    assert!(
        stdout.contains("2024-01-01T00:00:00Z"),
        "pin show stdout missing ISO 8601 timestamp:\n{stdout}",
    );
}
