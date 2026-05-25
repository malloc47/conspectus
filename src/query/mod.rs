//! Embedded SQLite query engine surface.
//!
//! See ADRs 0036 (engine selection), 0037 (persistence model),
//! 0038 (CLI/server transport under WAL), 0039 (`query` Cargo feature),
//! 0040 (distribution amendment), 0041 (resolver stays in Rust).
//!
//! The module is gated behind the `query` Cargo feature. The shipped
//! `conspectus` binary builds with the feature on by default; library
//! consumers who do not want the engine surface set
//! `default-features = false`.

pub mod loader;
pub mod schema;

pub use loader::load;
pub use schema::{SCHEMA_SQL, SCHEMA_VERSION, apply_schema, read_user_version};

use rusqlite::Connection;

/// Bundled libsqlite3 floor. Below this version, the WAL-reset
/// corruption bug class (affecting 3.7.0 .. 3.51.2 in multi-writer or
/// multi-checkpointer scenarios) is unfixed. Conspectus's continuous
/// server plus one-shot CLI coexistence pattern (ADR 0038) is exactly
/// the workload that bug class targets.
pub const MIN_SQLITE_VERSION: (u32, u32, u32) = (3, 51, 3);

/// Verify the bundled libsqlite3 meets [`MIN_SQLITE_VERSION`].
///
/// Panics with a clear diagnostic if it does not. Called from
/// `bundled_libsqlite3_meets_floor` in the test module; running
/// `cargo test --features query` therefore fails the build if the
/// bundled version regresses below the floor required by ADR 0036.
pub fn assert_min_sqlite_version() {
    let actual = bundled_version();
    if actual < MIN_SQLITE_VERSION {
        let (a, b, c) = actual;
        let (x, y, z) = MIN_SQLITE_VERSION;
        panic!(
            "bundled libsqlite3 {a}.{b}.{c} is below the floor {x}.{y}.{z} \
             required by ADR 0036 (WAL-reset corruption fix landed in 3.51.3)"
        );
    }
}

/// Query the bundled libsqlite3 for its `sqlite_version()` and parse it.
fn bundled_version() -> (u32, u32, u32) {
    let conn = Connection::open_in_memory().expect("open in-memory connection");
    let raw: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .expect("sqlite_version() returns a string");
    parse_version(&raw)
}

fn parse_version(s: &str) -> (u32, u32, u32) {
    let mut iter = s.split('.');
    let major = iter.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = iter.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = iter.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::OpenFlags;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn parse_version_handles_normal_strings() {
        assert_eq!(parse_version("3.51.3"), (3, 51, 3));
        assert_eq!(parse_version("3.51.10"), (3, 51, 10));
        assert_eq!(parse_version("10.0.0"), (10, 0, 0));
    }

    #[test]
    fn parse_version_handles_extra_components_and_garbage() {
        assert_eq!(parse_version("3.51.3.1"), (3, 51, 3));
        assert_eq!(parse_version("bogus"), (0, 0, 0));
        assert_eq!(parse_version("3.51"), (3, 51, 0));
    }

    #[test]
    fn bundled_libsqlite3_meets_floor() {
        assert_min_sqlite_version();
    }

    #[test]
    fn in_memory_connection_runs_a_trivial_query() {
        let conn = Connection::open_in_memory().expect("in-memory open");
        let one: i64 = conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .expect("SELECT 1");
        assert_eq!(one, 1);
    }

    /// WAL concurrency smoke test. Validates that a read-only connection
    /// does not block on an in-flight writer transaction — the property
    /// ADR 0038's CLI/server transport hinges on. We run the writer and
    /// reader on two threads with independent connections; SQLite's WAL
    /// sharing model is host-scoped, so the threaded scenario exercises
    /// the same locking machinery as separate processes would.
    #[test]
    fn wal_reader_does_not_block_on_open_writer_transaction() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let path = temp.path().join("wal_smoke.sqlite");

        // Initialize the database in WAL mode with one preexisting row.
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
            conn.execute("INSERT INTO t(v) VALUES (1)", []).unwrap();
        }

        let barrier = Arc::new(Barrier::new(2));

        let writer_path = path.clone();
        let writer_barrier = Arc::clone(&barrier);
        let writer = thread::spawn(move || {
            let conn = Connection::open(&writer_path).unwrap();
            conn.pragma_update(None, "busy_timeout", 5000).unwrap();
            conn.execute("BEGIN IMMEDIATE", []).unwrap();
            conn.execute("INSERT INTO t(v) VALUES (2)", []).unwrap();
            // Writer now holds the lock with an uncommitted insert.
            // Release the reader and hold the transaction for a beat so
            // any reader contention would manifest as a wait.
            writer_barrier.wait();
            thread::sleep(Duration::from_millis(200));
            conn.execute("COMMIT", []).unwrap();
        });

        let reader_path = path.clone();
        let reader_barrier = Arc::clone(&barrier);
        let reader = thread::spawn(move || {
            reader_barrier.wait();
            let conn = Connection::open_with_flags(&reader_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
            conn.pragma_update(None, "busy_timeout", 5000).unwrap();
            let start = Instant::now();
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
                .unwrap();
            let elapsed = start.elapsed();
            // The reader sees the pre-writer state (one row) — the writer
            // has not committed. Critically, it does not block on the
            // writer's open transaction.
            assert_eq!(count, 1, "reader should see pre-writer snapshot");
            assert!(
                elapsed < Duration::from_millis(150),
                "reader query took {elapsed:?}; should not have blocked on the writer"
            );
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }
}
