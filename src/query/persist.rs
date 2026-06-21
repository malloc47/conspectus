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
use super::schema::apply_schema;

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
