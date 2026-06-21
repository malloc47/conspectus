//! SQLite snapshot writer (P7-003).
//!
//! Companion to [`super::loader`] (which writes a `GraphSnapshot` into
//! an open SQLite connection) and [`super::runner`] (which reads
//! `graph.sqlite` for `conspectus query`). This module owns the
//! lifecycle bits the rest of the warm-start path needs:
//!
//!   * [`graph_db_path`] — the canonical on-disk location per ADR 0037.
//!     Promoted from a `runner`-private helper so any caller can
//!     decide where the snapshot lives without re-deriving the path.
//!   * [`persist_snapshot`] — open the database read-write, apply the
//!     schema if needed, hand off to the loader, commit. Returns
//!     errors so callers can downgrade to "warn and continue" without
//!     aborting the run — the rendered output is the operator's
//!     primary product; persistence is a side effect for the next
//!     invocation.
//!
//! The TTL comparison + selective re-run plumbing the warm-start path
//! needs lives in the discovery layer, not here; this module is just
//! the writer. P7-005 builds the per-provider eviction primitive on
//! top.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};

use crate::model::GraphSnapshot;

use super::loader::load;
use super::reader::read_snapshot;
use super::schema::{SCHEMA_VERSION, apply_schema, read_user_version};

/// Canonical on-disk location for the persisted graph database, per
/// ADR 0037. Resolves under `$XDG_DATA_HOME/conspectus/` when set,
/// otherwise `$HOME/.local/share/conspectus/`, otherwise the current
/// directory. The `-wal` / `-shm` sidecars live alongside.
pub fn graph_db_path() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("conspectus").join("graph.sqlite")
}

/// Persist `snapshot` to the canonical `graph.sqlite` location (or
/// `override_path` when set; tests use the override). Creates the
/// parent directory and the database file if they don't yet exist,
/// applies the schema, and replaces every row in a single
/// transaction via [`crate::query::loader::load`].
///
/// Per ADR 0037, atomicity comes from SQLite's transaction
/// semantics — there is no temp-file-plus-rename dance. WAL mode is
/// enabled on the connection so concurrent readers (the user's CLI
/// from another shell, the TUI in another terminal) keep seeing the
/// prior consistent snapshot until this transaction commits.
pub fn persist_snapshot(snapshot: &GraphSnapshot, override_path: Option<&Path>) -> Result<()> {
    let path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(graph_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    // If the existing file is not a usable SQLite database (e.g.
    // truncated bytes from a mid-write crash, half-applied
    // migration, manual `echo > graph.sqlite` from the operator),
    // SQLite refuses to open it even with `OPEN_CREATE` because
    // the path is already populated. Detect that case up front
    // and move the unusable file aside so this run can heal the
    // cache by writing a fresh database in its place. The
    // moved-aside file stays around for forensic inspection
    // rather than being silently deleted.
    if path.exists() {
        move_aside_if_unusable(&path);
    }
    let mut conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {} read-write", path.display()))?;
    apply_writer_pragmas(&conn)
        .with_context(|| format!("apply writer pragmas on {}", path.display()))?;
    apply_schema(&conn).with_context(|| format!("apply schema on {}", path.display()))?;
    load(snapshot, &mut conn).with_context(|| format!("load snapshot into {}", path.display()))?;
    Ok(())
}

/// Maximum number of `VACUUM INTO` snapshots [`rotate_backup`]
/// retains under `<graph.sqlite parent>/backups/`. Matches the
/// `N=5` value from ADR 0037's "Rotation and backups" section.
pub const BACKUP_RETENTION: usize = 5;

/// Produce a point-in-time backup of the persisted graph
/// database via `VACUUM INTO` and prune older backups so at most
/// [`BACKUP_RETENTION`] survive, per ADR 0037.
///
/// Triggered by callers that know they just performed a cold
/// rebuild (no warm-start cache hit) — the warm-start path's
/// "always persist" cadence would otherwise produce one backup
/// per CLI invocation, which is wasteful churn. Backups are
/// **debugging artifacts**, not part of the warm-start read
/// path; the warm-start path always reads `graph.sqlite`
/// directly.
///
/// `override_path` overrides the canonical `graph.sqlite`
/// location (tests use this). Backups land under
/// `<override_path parent>/backups/graph-<epoch>.sqlite` —
/// epoch-named so file listing order matches chronological
/// order. Best-effort: I/O failures bubble up as `Err` so the
/// caller can warn + continue without aborting the run.
pub fn rotate_backup(override_path: Option<&Path>) -> Result<()> {
    let graph_path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(graph_db_path);
    if !graph_path.exists() {
        // No primary file means there is nothing to back up.
        // This happens on the very first run when the operator
        // wired `--no-cache` so no persist landed; treat as a
        // no-op rather than a hard error.
        return Ok(());
    }
    let parent = graph_path.parent().ok_or_else(|| {
        anyhow::anyhow!("graph cache path has no parent: {}", graph_path.display())
    })?;
    let backups_dir = parent.join("backups");
    std::fs::create_dir_all(&backups_dir)
        .with_context(|| format!("create {}", backups_dir.display()))?;

    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_path = backups_dir.join(format!("graph-{epoch}.sqlite"));

    // `VACUUM INTO` produces a self-contained copy without
    // blocking other readers/writers, per ADR 0037. The path
    // must not already exist; epoch granularity is one second so
    // a back-to-back rotate within the same wall-clock second
    // would collide. Detect that and skip — losing a redundant
    // backup is fine.
    if backup_path.exists() {
        return Ok(());
    }
    let conn = Connection::open_with_flags(
        &graph_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {} read-only for backup", graph_path.display()))?;
    conn.execute(
        "VACUUM INTO ?",
        rusqlite::params![backup_path.to_string_lossy().as_ref()],
    )
    .with_context(|| format!("VACUUM INTO {}", backup_path.display()))?;

    prune_old_backups(&backups_dir).with_context(|| format!("prune {}", backups_dir.display()))?;
    Ok(())
}

/// Keep the [`BACKUP_RETENTION`] newest `graph-<epoch>.sqlite`
/// files in `dir`; delete the rest. Sorting is by filename which
/// — given the epoch encoding — equals chronological order.
fn prune_old_backups(dir: &Path) -> Result<()> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("graph-") && name.ends_with(".sqlite"))
        })
        .collect();
    backups.sort();
    if backups.len() <= BACKUP_RETENTION {
        return Ok(());
    }
    let drop_count = backups.len() - BACKUP_RETENTION;
    for path in backups.into_iter().take(drop_count) {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

/// Probe whether the file at `path` is a usable SQLite database
/// the writer can open. If not, rename it to a sibling
/// `.corrupt.<epoch>` so the subsequent open-with-CREATE can
/// produce a fresh database in its place. Best-effort: rename
/// failures fall through silently and the next open will surface
/// the underlying error with full context.
fn move_aside_if_unusable(path: &Path) {
    let Ok(conn) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return;
    };
    let probe: rusqlite::Result<u32> = conn.query_row("PRAGMA user_version", [], |row| row.get(0));
    drop(conn);
    if probe.is_ok() {
        return;
    }
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".corrupt.{epoch}"));
    let aside = PathBuf::from(aside);
    if std::fs::rename(path, &aside).is_ok() {
        eprintln!(
            "conspectus: warning: graph cache at {} was unusable; moved aside to {} \
             and rebuilding from cold",
            path.display(),
            aside.display()
        );
    }
}

/// Read the persisted graph from the canonical `graph.sqlite`
/// location (or `override_path` when set). Returns `Ok(None)` when
/// the file does not yet exist — a cold start on a fresh machine,
/// not an error. Any other failure (corrupt schema, unreadable
/// permissions) propagates so callers can warn + fall back to a
/// pure cold rebuild.
///
/// The connection is opened read-only: the warm-start path never
/// mutates the cache, only the writer side of the same invocation
/// does (via [`persist_snapshot`] at the end of the run).
///
/// P7-003 phase 2: this powers the backstop-only warm-start the
/// CLI runs before fresh discovery. Phase 3 will graduate this
/// into per-provider TTL comparison + selective re-run.
pub fn load_cached_snapshot(override_path: Option<&Path>) -> Result<Option<GraphSnapshot>> {
    let path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(graph_db_path);
    if !path.exists() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {} read-only", path.display()))?;
    // Schema-version gate (ADR 0037). A mismatched user_version
    // means either:
    //   * the database is fresh / empty (user_version = 0), or
    //   * a different binary version wrote it.
    // In both cases we treat the warm-start as a cache miss and
    // return `Ok(None)`; the caller falls back to a cold rebuild
    // and the post-run persist overwrites the file with the
    // current schema. A migration chain (forward-only per ADR
    // 0037) lands when schema drift is actually surfacing user
    // pain — until then, "rebuild and overwrite" is the safer
    // default than "try to read across versions."
    // A garbage / non-SQLite file opens successfully (SQLite
    // validates lazily) but the very first query fails with
    // `NotADatabase`. Treat that as a cache miss so the cold
    // rebuild + persist (which moves the bad file aside) can
    // heal the cache. No warning here: the persist side will
    // emit a more useful "moved aside" line and double-warning
    // would just clutter the terminal.
    let observed = match read_user_version(&conn) {
        Ok(v) => v,
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::NotADatabase =>
        {
            return Ok(None);
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("read PRAGMA user_version from {}", path.display()));
        }
    };
    if observed != SCHEMA_VERSION {
        eprintln!(
            "conspectus: warning: graph cache at {} has schema version {observed} \
             but this binary expects {SCHEMA_VERSION}; rebuilding from cold and \
             the next write will overwrite the file with the current schema",
            path.display()
        );
        return Ok(None);
    }
    let snapshot =
        read_snapshot(&conn).with_context(|| format!("read snapshot from {}", path.display()))?;
    Ok(Some(snapshot))
}

/// Pragma triplet from ADR 0038 plus `journal_mode = WAL`. The
/// reader side (`runner::apply_query_pragmas`) sets the same trio
/// minus the journal-mode toggle (a no-op on read-only connections).
/// Keeping the policy uniform here means a writer never reverts WAL
/// mode the reader might have established on a prior run.
fn apply_writer_pragmas(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")
        .context("set journal_mode=WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .context("set synchronous=NORMAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .context("set busy_timeout=5000")?;
    conn.pragma_update(None, "wal_autocheckpoint", 1000)
        .context("set wal_autocheckpoint=1000")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GraphNode, NodeProvenance, RepoId, RepoNode};
    use crate::query::reader::read_snapshot;
    use rusqlite::OpenFlags;

    #[test]
    fn persist_then_reopen_round_trips_a_single_repo() {
        // Build a minimal snapshot with a tagged node so the
        // provenance sidecar gets exercised end-to-end (P7-002 +
        // P7-003 land together for this path: writing a fresh
        // snapshot and reading it back yields the same nodes plus
        // the same per-node `(provider, freshness_epoch)` tuples).
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");

        let mut snap = GraphSnapshot::empty();
        let repo_id = RepoId::new("/r/.git");
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.node_provenance.insert(
            crate::model::NodeId::Repo(repo_id.clone()),
            NodeProvenance {
                provider: "git".to_string(),
                freshness_epoch: Some(1_700_000_500),
            },
        );

        persist_snapshot(&snap, Some(&path)).expect("write");
        assert!(path.exists(), "writer should create {}", path.display());

        let reader = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .expect("open read-only");
        let (provider, epoch): (String, i64) = reader
            .query_row(
                "SELECT discovery_provider, discovery_freshness_epoch FROM node_repos",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query");
        assert_eq!(provider, "git");
        assert_eq!(epoch, 1_700_000_500);

        let reopened = read_snapshot(&reader).expect("read snapshot");
        assert_eq!(reopened.nodes.len(), 1);
        assert!(matches!(reopened.nodes[0], GraphNode::Repo(_)));
        assert_eq!(reopened.node_provenance.len(), 1);
    }

    #[test]
    fn persist_replaces_prior_rows() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");

        let mut snap_a = GraphSnapshot::empty();
        snap_a
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/a"))));
        persist_snapshot(&snap_a, Some(&path)).expect("write a");

        let mut snap_b = GraphSnapshot::empty();
        snap_b
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/b"))));
        persist_snapshot(&snap_b, Some(&path)).expect("write b");

        let reader = Connection::open(&path).expect("open");
        let common_dirs: Vec<String> = {
            let mut stmt = reader.prepare("SELECT common_dir FROM node_repos").unwrap();
            stmt.query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(common_dirs, vec!["/b".to_string()]);
    }

    #[test]
    fn load_cached_snapshot_returns_none_when_file_missing() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let missing = tmp.path().join("never_written.sqlite");
        let result = load_cached_snapshot(Some(&missing)).expect("load");
        assert!(
            result.is_none(),
            "cold-start cache miss must surface as Ok(None), not an error"
        );
    }

    #[test]
    fn load_cached_snapshot_round_trips_a_written_snapshot() {
        // Writer + reader meet here so a future refactor that lets
        // them drift (different schema versions, different column
        // mapping, etc.) trips this test before it ships.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");

        let mut snap = GraphSnapshot::empty();
        let repo_id = RepoId::new("/r/.git");
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.node_provenance.insert(
            crate::model::NodeId::Repo(repo_id.clone()),
            NodeProvenance {
                provider: "git".to_string(),
                freshness_epoch: Some(1_700_000_900),
            },
        );

        persist_snapshot(&snap, Some(&path)).expect("write");
        let loaded = load_cached_snapshot(Some(&path))
            .expect("load")
            .expect("written snapshot reloads as Some");
        assert_eq!(loaded.nodes.len(), 1);
        assert_eq!(loaded.node_provenance.len(), 1);
        let entry = loaded
            .node_provenance
            .get(&crate::model::NodeId::Repo(repo_id))
            .expect("provenance for the written node");
        assert_eq!(entry.provider, "git");
        assert_eq!(entry.freshness_epoch, Some(1_700_000_900));
    }

    #[test]
    fn load_cached_snapshot_falls_back_when_schema_version_mismatches() {
        // Simulates the binary-upgrade case: a previous binary
        // wrote the file at one schema version and the new binary
        // expects another. The warm-start path must treat this as
        // a cache miss rather than an error so the cold rebuild +
        // persist heals the file.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        persist_snapshot(&GraphSnapshot::empty(), Some(&path)).expect("write");
        // Stomp the user_version to something the binary does not
        // expect. Anything other than `SCHEMA_VERSION` should
        // trigger the fallback path; pick a far-future value to
        // also exercise the binary-downgrade case implicitly.
        {
            let conn = Connection::open(&path).expect("open rw");
            conn.execute_batch("PRAGMA user_version = 9999")
                .expect("stomp version");
        }

        let result = load_cached_snapshot(Some(&path)).expect("load");
        assert!(
            result.is_none(),
            "schema-version mismatch must surface as Ok(None), not an error or stale snapshot"
        );
    }

    #[test]
    fn load_cached_snapshot_falls_back_on_zero_user_version() {
        // A bare SQLite file with no schema and no user_version
        // (the default `0`) should also map to a cache miss
        // rather than trying to read tables that don't exist yet.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        {
            let _ = Connection::open(&path).expect("create empty db");
        }

        let result = load_cached_snapshot(Some(&path)).expect("load");
        assert!(
            result.is_none(),
            "an empty database file must round-trip as Ok(None)"
        );
    }

    #[test]
    fn rotate_backup_writes_to_sibling_backups_dir() {
        // Smoke test for the basic VACUUM INTO + naming flow.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        persist_snapshot(&GraphSnapshot::empty(), Some(&path)).expect("write primary");
        rotate_backup(Some(&path)).expect("rotate");

        let backups_dir = tmp.path().join("backups");
        let mut entries: Vec<String> = std::fs::read_dir(&backups_dir)
            .expect("read backups dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(entries.len(), 1, "exactly one backup after one rotate");
        assert!(
            entries[0].starts_with("graph-") && entries[0].ends_with(".sqlite"),
            "backup must be epoch-named; got `{}`",
            entries[0]
        );
    }

    #[test]
    fn rotate_backup_prunes_to_retention_when_exceeded() {
        // Stage BACKUP_RETENTION + 3 fake backups by hand, call
        // rotate once, and confirm only the newest
        // BACKUP_RETENTION survive. We hand-make the files
        // because epoch granularity is one second; calling
        // `rotate_backup` BACKUP_RETENTION+1 times in quick
        // succession would collide on filename and skip writes.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        persist_snapshot(&GraphSnapshot::empty(), Some(&path)).expect("write primary");
        let backups_dir = tmp.path().join("backups");
        std::fs::create_dir_all(&backups_dir).expect("mkdir backups");
        // Stamp eight ascending-epoch fake backups. The retention
        // pass sorts by filename — which equals chronological
        // order under our naming — and keeps the newest N.
        for i in 1..=BACKUP_RETENTION + 3 {
            let fake = backups_dir.join(format!("graph-{i:020}.sqlite"));
            std::fs::write(&fake, b"placeholder").expect("write fake backup");
        }
        assert_eq!(
            std::fs::read_dir(&backups_dir).unwrap().count(),
            BACKUP_RETENTION + 3
        );

        // The rotate call itself also writes a new backup (with a
        // wall-clock epoch that sorts last), so post-rotate the
        // directory holds exactly BACKUP_RETENTION entries.
        rotate_backup(Some(&path)).expect("rotate");

        let surviving: Vec<String> = std::fs::read_dir(&backups_dir)
            .expect("read")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            surviving.len(),
            BACKUP_RETENTION,
            "post-rotate should retain exactly N=BACKUP_RETENTION; got {surviving:?}"
        );
    }

    #[test]
    fn rotate_backup_ignores_non_matching_files_in_backups_dir() {
        // The retention sweep must only touch its own files.
        // An operator-authored note or an unrelated SQLite dump
        // dropped into `backups/` should survive every rotate
        // call, even if it pushes the count above the retention.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        persist_snapshot(&GraphSnapshot::empty(), Some(&path)).expect("write primary");
        let backups_dir = tmp.path().join("backups");
        std::fs::create_dir_all(&backups_dir).expect("mkdir backups");
        let user_note = backups_dir.join("README.txt");
        std::fs::write(&user_note, b"operator note: do not delete").expect("write note");
        for i in 1..=BACKUP_RETENTION + 5 {
            let fake = backups_dir.join(format!("graph-{i:020}.sqlite"));
            std::fs::write(&fake, b"placeholder").expect("write fake backup");
        }

        rotate_backup(Some(&path)).expect("rotate");

        assert!(
            user_note.exists(),
            "unrelated files in backups/ must survive retention sweeps"
        );
    }

    #[test]
    fn rotate_backup_is_a_noop_when_primary_is_missing() {
        // Operator ran with `--no-cache` so no primary file ever
        // landed. Triggering a rotate from a peer command (or
        // from the cold-rebuild path on a fully-suppressed run)
        // must not error.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("graph.sqlite");
        // No persist_snapshot call.
        rotate_backup(Some(&path)).expect("rotate no-op");
        assert!(
            !tmp.path().join("backups").exists()
                || std::fs::read_dir(tmp.path().join("backups"))
                    .map(|d| d.count())
                    .unwrap_or(0)
                    == 0,
            "missing primary should not create or populate backups/"
        );
    }

    #[test]
    fn persist_creates_missing_parent_directory() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let nested = tmp
            .path()
            .join("nested")
            .join("subdir")
            .join("graph.sqlite");
        let snap = GraphSnapshot::empty();
        persist_snapshot(&snap, Some(&nested)).expect("write");
        assert!(nested.exists());
    }
}
