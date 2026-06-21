//! CLI-level tests for the P7-003 warm-start persistence path. The
//! writer side currently runs after every successful `conspectus
//! table` invocation; the assertions here pin down that behavior
//! plus the `--no-cache` opt-out so a regression that drops the
//! `persist_snapshot` call (or accidentally moves it past the
//! error-path) surfaces in CI.

use assert_cmd::Command;
use std::path::Path;

/// Build a command isolated from the host environment plus an
/// explicit `XDG_DATA_HOME` so the persisted `graph.sqlite` lands in
/// a predictable spot the test can stat.
fn isolated_cmd(home: &Path, data_home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary exists");
    cmd.env("HOME", home);
    cmd.env("XDG_DATA_HOME", data_home);
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
    cmd
}

#[test]
fn table_sessions_persists_graph_sqlite_at_the_canonical_path() {
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let expected = data.path().join("conspectus").join("graph.sqlite");
    assert!(
        expected.exists(),
        "writer should populate {} after `table sessions` runs",
        expected.display()
    );

    // The file should be a real SQLite database, not a zero-byte
    // touch. Open it read-only and read the schema version pragma
    // the writer applies.
    let conn = rusqlite::Connection::open_with_flags(
        &expected,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("open persisted database");
    let user_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user_version");
    assert!(
        user_version > 0,
        "PRAGMA user_version should be set by the writer; got {user_version}"
    );
}

#[test]
fn table_sessions_skips_persistence_when_no_cache_flag_is_passed() {
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

    let expected = data.path().join("conspectus").join("graph.sqlite");
    assert!(
        !expected.exists(),
        "`--no-cache` should suppress the writer; {} should not exist",
        expected.display()
    );
}

#[test]
fn table_sessions_with_refresh_still_persists_the_writer_output() {
    // `--refresh` skips the warm-start *read* but leaves the writer
    // side running so the next invocation can warm-start off this
    // run. The behavioral distinction from `--no-cache` is the
    // entire point of separating the two flags.
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

    let expected = data.path().join("conspectus").join("graph.sqlite");
    assert!(
        expected.exists(),
        "`--refresh` alone should not suppress the writer; {} should exist",
        expected.display()
    );
}

#[test]
fn table_sessions_warm_starts_from_a_prior_run_without_erroring() {
    // P7-003 phase 2 round-trip: run `table sessions` once to
    // populate the cache, then run it again so the second invocation
    // reads `graph.sqlite` via the warm-start backstop, merges it
    // with the live discovery output, re-resolves, and persists
    // back. A regression that breaks the read path (schema drift,
    // bad reader, missing handle) would surface here as either a
    // non-zero exit or a stderr warning the test pins down.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let cache = data.path().join("conspectus").join("graph.sqlite");
    assert!(cache.exists(), "first run should populate the cache");

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
        !stderr.contains("failed to read graph cache"),
        "warm-start read should not surface a warning; got stderr:\n{stderr}"
    );
}
