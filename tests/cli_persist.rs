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
fn table_sessions_dual_writes_graph_bin_alongside_graph_sqlite() {
    // P11-008 contract: every cold rebuild writes both the
    // legacy graph.sqlite (P7-003) AND the new zero-copy
    // graph.bin (ADR 0083) so a follow-up daemon cycle or a
    // future mmap-fresh consumer can pick the file up
    // immediately. Without this dual-write, the only path
    // populating graph.bin would be conspectus serve — and the
    // backlog plan reserves daemon-only writes for after
    // P11-011's SQLite removal.
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
    let sqlite = data.path().join("conspectus").join("graph.sqlite");
    assert!(
        sqlite.exists(),
        "graph.sqlite should land at {}",
        sqlite.display()
    );
    assert!(
        bin.exists(),
        "graph.bin should land alongside graph.sqlite at {}",
        bin.display()
    );

    // Validate graph.bin end-to-end via the snapshot module's
    // mmap reader (which runs bytecheck) and deserialize. The
    // empty cwd + isolated env produces an empty snapshot;
    // the regression net here is "the writer produced a
    // structurally sound archive," not the content shape.
    let handle = conspectus::snapshot::open_mmap(&bin).expect("graph.bin must validate");
    assert_eq!(handle.header().format_version, 1);
    let owned = conspectus::snapshot::deserialize_owned(&handle).expect("deserialize");
    assert!(
        owned.nodes.is_empty(),
        "empty cwd should produce empty graph"
    );
}

#[test]
fn table_sessions_with_no_cache_skips_graph_bin_too() {
    // Per P11-008, the dual-write helper honors --no-cache:
    // neither graph.sqlite nor graph.bin should land when the
    // operator opts out. The graph.sqlite half is the
    // pre-existing assertion in the next test; this one pins
    // the graph.bin half so a future helper change that
    // forgets the no_cache branch surfaces in CI.
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

#[test]
fn table_sessions_warm_start_carries_fresh_provider_slice_through_to_post_run_cache() {
    // P7-003 phase 3 end-to-end: hand-stamp a `github` node into
    // `graph.sqlite` with a freshness epoch of "right now", then
    // run `table sessions` with no forge runner installed. The
    // freshness gate should classify github as fresh, skip
    // re-running it (vacuous — there is no forge runner anyway),
    // and the backstop merge should carry the node through to
    // the post-run persisted snapshot.
    //
    // A regression that wires the CLI to the old cold-rebuild
    // path (or that breaks the per-provider TTL gate) would
    // surface here as the github node disappearing from the
    // re-written `graph.sqlite`.
    use rusqlite::Connection;

    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    // First run populates the cache from a cold scan so the
    // schema is in place. The empty cwd means an empty graph;
    // that's fine — we'll add the github node by hand below.
    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let cache_path = data.path().join("conspectus").join("graph.sqlite");
    assert!(cache_path.exists());

    // Open the cache and inject a fresh github-attributed repo
    // node. Using the `node_repos` table because it's the
    // simplest shape; the freshness gate only inspects the
    // `discovery_provider` + `discovery_freshness_epoch`
    // columns, not the node kind. Use a freshness epoch within
    // the last 60 seconds so the 5-minute forge TTL keeps the
    // slice fresh.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    {
        let conn = Connection::open(&cache_path).expect("open cache rw");
        conn.execute(
            "INSERT INTO node_repos (\
                node_id, \
                common_dir, \
                source_paths, \
                remotes, \
                discovery_provider, \
                discovery_freshness_epoch\
             ) VALUES (?, ?, '[]', '[]', ?, ?)",
            rusqlite::params![
                r#"{"type":"repo","common_dir":"/warm-test-repo/.git"}"#,
                "/warm-test-repo/.git",
                "github",
                now,
            ],
        )
        .expect("insert fresh github-attributed repo");
    }

    // Second run: warm-start path reads the cache, classifies
    // github as fresh, skips it (no forge runner anyway), and
    // merges the cached node through to the post-run write.
    let output = isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .output()
        .expect("second run");
    assert!(
        output.status.success(),
        "warm-start invocation should succeed: stderr=\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The hand-injected node must still be present in the
    // post-run cache. Stale-gating or a missing wire-up would
    // drop it.
    let conn = Connection::open_with_flags(&cache_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("reopen cache for verification");
    let surviving: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM node_repos WHERE common_dir = '/warm-test-repo/.git'",
            [],
            |row| row.get(0),
        )
        .expect("count surviving repo");
    assert_eq!(
        surviving, 1,
        "fresh-classified github slice must survive the warm-start re-write"
    );
}

#[test]
fn table_sessions_writes_a_backup_on_the_first_cold_run_but_not_on_subsequent_warm_runs() {
    // ADR 0037 rotation contract: cold rebuilds produce a
    // VACUUM INTO backup; warm-start runs do not. The first
    // invocation against an empty cache is a cold rebuild and
    // should leave one backup behind; the second invocation
    // (warm-start hit) should not add a second.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let backups_dir = data.path().join("conspectus").join("backups");
    let after_cold: Vec<_> = std::fs::read_dir(&backups_dir)
        .expect("read backups dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        after_cold.len(),
        1,
        "cold rebuild should leave exactly one backup; got {after_cold:?}"
    );

    // Warm-start run: the cache is populated, the freshness gate
    // sees nothing stale, no providers re-run. Backup count must
    // stay at 1.
    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();
    let after_warm: Vec<_> = std::fs::read_dir(&backups_dir)
        .expect("re-read backups dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        after_warm.len(),
        1,
        "warm-start run must not produce an additional backup; got {after_warm:?}"
    );

    // `--refresh` forces a cold rebuild and should add a fresh
    // backup. Sleep one second so the epoch-named file does not
    // collide with the first backup (rotate skips colliding
    // names by design).
    std::thread::sleep(std::time::Duration::from_secs(1));
    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .arg("--refresh")
        .assert()
        .success();
    let after_refresh: Vec<_> = std::fs::read_dir(&backups_dir)
        .expect("post-refresh read")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        after_refresh.len(),
        2,
        "--refresh cold rebuild should add a second backup; got {after_refresh:?}"
    );
}

#[test]
fn table_sessions_recovers_from_a_truncated_graph_cache() {
    // Hardening: a malformed `graph.sqlite` (truncated bytes,
    // bad header, mid-write crash that left the file unreadable)
    // must not abort the run. The CLI should warn, fall back to
    // a cold rebuild, and heal the file via the post-run write.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    let cache_dir = data.path().join("conspectus");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    let cache_path = cache_dir.join("graph.sqlite");

    // Drop a few bytes of garbage in place of the database. The
    // SQLite open call survives (it lazily validates on first
    // query) but the version pragma read will fail, surfacing
    // the malformed-cache path.
    std::fs::write(&cache_path, b"this is not a sqlite database").expect("write garbage");

    let output = isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .output()
        .expect("run against malformed cache");

    assert!(
        output.status.success(),
        "warm-start must recover from a malformed cache: stderr=\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Post-run write should have replaced the garbage with a
    // real SQLite database carrying the current schema. Open
    // read-only and verify a valid `user_version`.
    let conn = rusqlite::Connection::open_with_flags(
        &cache_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("reopen healed cache");
    let user_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user_version");
    assert!(
        user_version > 0,
        "post-run write should heal the cache with a valid user_version; got {user_version}"
    );
}

#[test]
fn table_sessions_with_refresh_drops_a_fresh_cached_slice() {
    // The counterpart to the test above: with `--refresh`, the
    // warm-start prior is forced to empty, so even a
    // fresh-classified cached slice is left out of the merge and
    // the post-run write loses it. The gate's classification
    // doesn't matter — `--refresh` bypasses the read.
    use rusqlite::Connection;

    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .assert()
        .success();

    let cache_path = data.path().join("conspectus").join("graph.sqlite");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    {
        let conn = Connection::open(&cache_path).expect("open cache rw");
        conn.execute(
            "INSERT INTO node_repos (\
                node_id, \
                common_dir, \
                source_paths, \
                remotes, \
                discovery_provider, \
                discovery_freshness_epoch\
             ) VALUES (?, ?, '[]', '[]', ?, ?)",
            rusqlite::params![
                r#"{"type":"repo","common_dir":"/refresh-drops-me/.git"}"#,
                "/refresh-drops-me/.git",
                "github",
                now,
            ],
        )
        .expect("insert fresh github repo");
    }

    isolated_cmd(home.path(), data.path())
        .current_dir(cwd.path())
        .arg("table")
        .arg("sessions")
        .arg("--refresh")
        .assert()
        .success();

    let conn = Connection::open_with_flags(&cache_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("reopen cache");
    let surviving: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM node_repos WHERE common_dir = '/refresh-drops-me/.git'",
            [],
            |row| row.get(0),
        )
        .expect("count refresh-dropped repo");
    assert_eq!(
        surviving, 0,
        "`--refresh` must skip the warm-start read entirely; cached slice should not survive"
    );
}
