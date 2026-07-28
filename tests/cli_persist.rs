//! CLI-level tests for the post-P11-011a graph.bin persistence
//! path. The legacy graph.sqlite persistence layer is gone (ADR
//! 0082); the assertions here pin the graph.bin writer plus the
//! `--no-cache` / `--refresh` flag semantics so a regression
//! that drops the writer (or accidentally moves it past the
//! error-path) surfaces in CI. The corruption-recovery and
//! backup-rotation assertions from the SQLite era are gone with
//! the machinery they tested.

use assert_cmd::Command;
use std::path::Path;

/// Build a command isolated from the host environment plus an
/// explicit `XDG_DATA_HOME` so the persisted `graph.bin` lands
/// in a predictable spot the test can stat.
///
/// `XDG_RUNTIME_DIR` is pointed at a nonexistent subdirectory of
/// `home` so the CLI's `warm_start_discover_and_resolve` path
/// (src/cli/mod.rs:172) cannot find and defer to a real
/// `conspectus serve` socket. Without this override the tests
/// would silently pull the operator's live daemon snapshot and
/// never exercise the local cold-rebuild + graph.bin writer path
/// they claim to pin. `socket_path()` builds a path under
/// `XDG_RUNTIME_DIR`; when that directory does not exist the
/// connect call fails, `try_daemon_snapshot()` returns `None`,
/// and the CLI falls through to the cold rebuild the tests
/// want to test.
fn isolated_cmd(home: &Path, data_home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("XDG_DATA_HOME", data_home);
    cmd.env("XDG_RUNTIME_DIR", home.join("no-daemon-runtime-dir"));
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

#[test]
fn table_sessions_persists_graph_bin_at_the_canonical_path() {
    // The writer should land a valid `graph.bin` after every
    // successful `table sessions` invocation (assuming no
    // `--no-cache`). The validation pass via `open_mmap`
    // exercises the bytecheck path, so we know the bytes
    // round-trip cleanly back to a `GraphSnapshot`.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let bin = data.path().join("conspectus").join("graph.bin");
    assert!(
        bin.exists(),
        "writer should populate {} after `table sessions` runs",
        bin.display()
    );
    let handle = conspectus::snapshot::open_mmap(&bin).expect("graph.bin must validate");
    assert_eq!(
        handle.header().format_version,
        conspectus::snapshot::FORMAT_VERSION
    );
    let owned = conspectus::snapshot::deserialize_owned(&handle).expect("deserialize");
    assert!(
        owned.nodes.is_empty(),
        "empty cwd should produce empty graph; got {} nodes",
        owned.nodes.len()
    );
}

#[test]
fn table_sessions_with_no_cache_skips_graph_bin() {
    // `--no-cache` short-circuits `cache_resolved_snapshot` so
    // no `graph.bin` lands. A regression that forgets the
    // no_cache branch would create a file the test then
    // detects.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .arg("--no-cache")
        .assert()
        .success();

    let bin = data.path().join("conspectus").join("graph.bin");
    assert!(
        !bin.exists(),
        "--no-cache should suppress graph.bin; got {}",
        bin.display()
    );
}

#[test]
fn table_sessions_with_refresh_still_persists_the_writer_output() {
    // `--refresh` (force cold scan) still runs the writer so
    // the next invocation can warm-start. The behavioral
    // distinction from `--no-cache` is the entire point of
    // separating the two flags.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .arg("--refresh")
        .assert()
        .success();

    let bin = data.path().join("conspectus").join("graph.bin");
    assert!(
        bin.exists(),
        "--refresh alone should not suppress the writer; {} should exist",
        bin.display()
    );
}

#[test]
fn table_sessions_second_run_succeeds_without_warnings() {
    // Round-trip smoke: two successive runs should both
    // succeed cleanly. P11-011a removed the on-disk warm-start
    // prior; both runs are cold rebuilds. The regression net
    // is "no stderr warnings from the cache layer" — a future
    // cache-related regression that surfaces as a warning
    // line lands in CI here.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let bin = data.path().join("conspectus").join("graph.bin");
    assert!(bin.exists(), "first run should populate the cache");

    let output = isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .output()
        .expect("second run");
    assert!(
        output.status.success(),
        "second run should succeed: stderr=\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("warning"),
        "successful runs should not surface a warning; got stderr:\n{stderr}"
    );
}

#[test]
fn legacy_graph_sqlite_artifacts_do_not_block_a_fresh_run() {
    // Operators upgrading from a pre-P11-011a build will have a
    // leftover graph.sqlite in their data dir. The CLI must
    // ignore it (it doesn't read it anymore) and still produce
    // a valid graph.bin on the next run. The daemon cleans up
    // those files on startup; the CLI just lets them sit
    // harmlessly.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    let dir = data.path().join("conspectus");
    std::fs::create_dir_all(&dir).expect("create dir");
    std::fs::write(
        dir.join("graph.sqlite"),
        b"legacy garbage from prior install",
    )
    .expect("write legacy graph.sqlite");
    std::fs::write(dir.join("graph.sqlite-wal"), b"").expect("write legacy wal");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let bin = dir.join("graph.bin");
    assert!(
        bin.exists(),
        "CLI should ignore legacy artifacts and write graph.bin at {}",
        bin.display()
    );
    let handle = conspectus::snapshot::open_mmap(&bin).expect("graph.bin must validate");
    assert_eq!(
        handle.header().format_version,
        conspectus::snapshot::FORMAT_VERSION
    );
}
