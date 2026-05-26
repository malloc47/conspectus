//! `conspectus query <sql>` executor (P9-004).
//!
//! Opens a read-only SQLite connection against the canonical
//! `graph.sqlite` (per ADR 0037) when one exists; otherwise builds a
//! fresh snapshot from cold discovery, loads it into an in-memory
//! database, and locks the connection with `PRAGMA query_only = 1` so
//! the user's SQL cannot mutate it. Executes the SQL and renders the
//! result.
//!
//! Read-only enforcement (no DML, no DDL, no `ATTACH ... AS rw`) comes
//! from the underlying SQLite open flags (`SQLITE_OPEN_READ_ONLY` for
//! file-backed connections) or `PRAGMA query_only = 1` for in-memory
//! fallback connections. Either path makes the engine reject mutations
//! with a clean error message.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};

use crate::model::GraphSnapshot;
use crate::query::{apply_schema, load};

/// Output format for the query result.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Plain-text columnar table with a header row and aligned cells.
    Table,
    /// One JSON object per line, keyed by column name.
    Json,
}

/// Inputs to the executor. `db_path` overrides the default XDG path
/// (used by tests; the CLI passes `None` to use the canonical
/// location).
pub struct QueryInputs<'a> {
    pub sql: &'a str,
    pub format: OutputFormat,
    pub db_path: Option<PathBuf>,
}

/// Discover, load, and execute the user's query. Returns the rendered
/// output as a String. The CLI prints it; tests can assert against it
/// directly.
pub fn run_query(inputs: QueryInputs<'_>) -> Result<String> {
    let conn = open_connection(inputs.db_path.as_deref())?;
    execute(&conn, inputs.sql, inputs.format)
}

/// Same as [`run_query`] but takes a pre-built snapshot. Useful in tests
/// that want to assert query behavior against a known fixture without
/// touching cold discovery or the filesystem.
pub fn run_query_against_snapshot(
    snapshot: &GraphSnapshot,
    sql: &str,
    format: OutputFormat,
) -> Result<String> {
    let mut conn = Connection::open_in_memory().context("open in-memory database")?;
    apply_schema(&conn).context("apply schema")?;
    load(snapshot, &mut conn).context("load snapshot")?;
    lock_read_only(&conn)?;
    apply_query_pragmas(&conn)?;
    execute(&conn, sql, format)
}

fn open_connection(override_path: Option<&std::path::Path>) -> Result<Connection> {
    let path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(graph_db_path);
    if path.exists() {
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| format!("open {} read-only", path.display()))?;
        apply_query_pragmas(&conn)?;
        Ok(conn)
    } else {
        // Cold-discovery fallback. P7-003 will populate the canonical
        // file; until then every invocation pays the discovery cost.
        let mut conn = Connection::open_in_memory().context("open in-memory database")?;
        apply_schema(&conn).context("apply schema")?;
        let cwd = env::current_dir().context("read current working directory")?;
        let snapshot =
            crate::discovery::discover_local_at_roots([cwd]).context("run cold discovery")?;
        let snapshot = crate::resolve::resolve_snapshot(snapshot);
        load(&snapshot, &mut conn).context("load snapshot")?;
        lock_read_only(&conn)?;
        apply_query_pragmas(&conn)?;
        Ok(conn)
    }
}

/// Canonical on-disk location for the graph database, per ADR 0037.
fn graph_db_path() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("conspectus").join("graph.sqlite")
}

fn apply_query_pragmas(conn: &Connection) -> Result<()> {
    // The pragma triplet from ADR 0038. `wal_autocheckpoint` only
    // affects writers; setting it on a read-only connection is a no-op
    // but keeps the policy uniform.
    conn.pragma_update(None, "synchronous", "NORMAL")
        .context("set synchronous=NORMAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .context("set busy_timeout=5000")?;
    conn.pragma_update(None, "wal_autocheckpoint", 1000)
        .context("set wal_autocheckpoint=1000")?;
    Ok(())
}

/// Lock a writable connection so the user's SQL cannot mutate it.
/// `query_only` is a sticky session-level switch in SQLite.
fn lock_read_only(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA query_only = 1")
        .context("set query_only=1")?;
    Ok(())
}

fn execute(conn: &Connection, sql: &str, format: OutputFormat) -> Result<String> {
    let mut stmt = conn
        .prepare(sql)
        .with_context(|| format!("prepare SQL: {sql}"))?;
    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).map(str::to_string))
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("read column names")?;

    let rows: Vec<Vec<Value>> = stmt
        .query_map([], |row| {
            (0..column_count)
                .map(|i| row.get::<_, Value>(i))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .with_context(|| format!("run SQL: {sql}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .with_context(|| format!("fetch rows from: {sql}"))?;

    Ok(match format {
        OutputFormat::Table => render_table(&column_names, &rows),
        OutputFormat::Json => render_json(&column_names, &rows),
    })
}

fn render_table(headers: &[String], rows: &[Vec<Value>]) -> String {
    let widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let header_w = h.chars().count();
            rows.iter()
                .map(|r| value_to_string(&r[i]).chars().count())
                .max()
                .unwrap_or(0)
                .max(header_w)
        })
        .collect();

    let mut out = String::new();
    write_row(&mut out, headers.iter().map(String::as_str), &widths);
    write_separator(&mut out, &widths);
    for row in rows {
        let cells: Vec<String> = row.iter().map(value_to_string).collect();
        write_row(&mut out, cells.iter().map(String::as_str), &widths);
    }
    out
}

fn write_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>, widths: &[usize]) {
    let mut first = true;
    for (cell, width) in cells.zip(widths.iter()) {
        if !first {
            out.push_str("  ");
        }
        first = false;
        let cell_w = cell.chars().count();
        out.push_str(cell);
        if cell_w < *width {
            out.extend(std::iter::repeat_n(' ', *width - cell_w));
        }
    }
    out.push('\n');
}

fn write_separator(out: &mut String, widths: &[usize]) {
    let mut first = true;
    for width in widths {
        if !first {
            out.push_str("  ");
        }
        first = false;
        out.extend(std::iter::repeat_n('-', *width));
    }
    out.push('\n');
}

fn render_json(headers: &[String], rows: &[Vec<Value>]) -> String {
    let mut out = String::new();
    for row in rows {
        let mut obj = serde_json::Map::with_capacity(headers.len());
        for (h, v) in headers.iter().zip(row.iter()) {
            obj.insert(h.clone(), value_to_json(v));
        }
        let line =
            serde_json::to_string(&serde_json::Value::Object(obj)).expect("row JSON serializes");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Integer(n) => n.to_string(),
        Value::Real(f) => format!("{f}"),
        Value::Text(s) => s.clone(),
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}

fn value_to_json(value: &Value) -> serde_json::Value {
    use serde_json::Value as Json;
    match value {
        Value::Null => Json::Null,
        Value::Integer(n) => Json::Number((*n).into()),
        Value::Real(f) => serde_json::Number::from_f64(*f)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::Text(s) => Json::String(s.clone()),
        Value::Blob(b) => Json::String(format!("<blob {} bytes>", b.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, ForkId, ForkNode, Freshness, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, SourceMetadata,
    };

    fn empty_snapshot_run(sql: &str, format: OutputFormat) -> Result<String> {
        run_query_against_snapshot(&GraphSnapshot::empty(), sql, format)
    }

    #[test]
    fn select_count_against_empty_graph() {
        let out = empty_snapshot_run("SELECT COUNT(*) FROM v_nodes", OutputFormat::Table).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "header + separator + one row");
        assert!(lines[0].contains("COUNT(*)"));
        assert!(lines[2].trim_end() == "0");
    }

    #[test]
    fn select_with_json_format_emits_one_object_per_line() {
        let out = empty_snapshot_run("SELECT 1 AS a, 'x' AS b", OutputFormat::Json).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["a"], serde_json::json!(1));
        assert_eq!(parsed["b"], serde_json::json!("x"));
    }

    #[test]
    fn null_values_render_as_empty_in_table_and_null_in_json() {
        let table_out = empty_snapshot_run("SELECT NULL AS x", OutputFormat::Table).unwrap();
        // header + separator + value row
        let value_line = table_out.lines().nth(2).unwrap();
        assert_eq!(value_line.trim_end(), "");

        let json_out = empty_snapshot_run("SELECT NULL AS x", OutputFormat::Json).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(json_out.lines().next().unwrap()).unwrap();
        assert!(parsed["x"].is_null());
    }

    fn read_only_rejects(sql: &str) {
        let err = empty_snapshot_run(sql, OutputFormat::Table).unwrap_err();
        let chain = format!("{err:#}");
        // SQLite returns either "attempt to write a readonly database"
        // or "not authorized" depending on the operation, with the
        // query_only PRAGMA active. Either is acceptable; just ensure
        // it is a refusal, not a silent succeed.
        assert!(
            chain.to_lowercase().contains("readonly")
                || chain.to_lowercase().contains("read-only")
                || chain.to_lowercase().contains("not authorized")
                || chain.to_lowercase().contains("query_only"),
            "expected read-only rejection, got: {chain}"
        );
    }

    #[test]
    fn read_only_rejects_insert() {
        read_only_rejects("INSERT INTO node_repos (node_id, common_dir) VALUES ('x', '/x')");
    }

    #[test]
    fn read_only_rejects_update() {
        read_only_rejects("UPDATE node_repos SET common_dir = '/new'");
    }

    #[test]
    fn read_only_rejects_delete() {
        read_only_rejects("DELETE FROM node_repos");
    }

    #[test]
    fn read_only_rejects_create() {
        read_only_rejects("CREATE TABLE evil (x INTEGER)");
    }

    #[test]
    fn read_only_rejects_drop() {
        read_only_rejects("DROP TABLE node_repos");
    }

    fn snapshot_with_session_and_mux() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        let session_id = AgentSessionId::new("claude-code", "default", "s1");
        let mux_id = MuxSessionId::new("tmux:0");
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: session_id.clone(),
            harness_key: "claude-code".into(),
            cwd: Some("/cwd".into()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
        }));
        snap.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            native_id: "tmux:0".into(),
            backend: "tmux".into(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snap.candidate_links.push(GraphLink {
            id: "L1".into(),
            source: NodeId::AgentSession(session_id),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snap
    }

    #[test]
    fn join_candidate_links_against_node_agent_sessions() {
        let snap = snapshot_with_session_and_mux();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT s.harness_key, l.relation, l.target_node_id \
             FROM node_agent_sessions s \
             JOIN candidate_links l ON l.source_node_id = s.node_id",
            OutputFormat::Json,
        )
        .unwrap();
        let line = out.lines().next().expect("at least one row");
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["harness_key"], "claude-code");
        assert_eq!(parsed["relation"], "linked_to_mux");
        assert!(
            parsed["target_node_id"]
                .as_str()
                .unwrap()
                .starts_with("mux_session:")
        );
    }

    fn snapshot_with_fork_chain() -> GraphSnapshot {
        // Build three Fork nodes a -> b -> c (a is the root, c is the
        // deepest descendant), linked by ParentFork candidate links
        // pointing from child to parent.
        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b", "c"] {
            snap.nodes.push(GraphNode::Fork(ForkNode {
                id: ForkId::new(key),
                provider: "atelier".into(),
                provider_source_key: key.into(),
                name: None,
                scope: None,
                capabilities: vec![],
            }));
        }
        let parent_link = |child: &str, parent: &str, id: &str| GraphLink {
            id: id.into(),
            source: NodeId::Fork(ForkId::new(child)),
            target: LinkEndpoint::Node {
                id: NodeId::Fork(ForkId::new(parent)),
            },
            relation: RelationKind::ParentFork,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        };
        snap.candidate_links.push(parent_link("b", "a", "L_ba"));
        snap.candidate_links.push(parent_link("c", "b", "L_cb"));
        snap
    }

    #[test]
    fn recursive_cte_walks_fork_ancestry() {
        let snap = snapshot_with_fork_chain();
        let out = run_query_against_snapshot(
            &snap,
            "WITH RECURSIVE ancestry(node_id, depth) AS ( \
               SELECT 'fork:c', 0 \
               UNION ALL \
               SELECT cl.target_node_id, ancestry.depth + 1 \
                 FROM candidate_links cl \
                 JOIN ancestry ON cl.source_node_id = ancestry.node_id \
                 WHERE cl.relation = 'parent_fork' \
             ) SELECT node_id, depth FROM ancestry ORDER BY depth",
            OutputFormat::Json,
        )
        .unwrap();
        let parsed: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(parsed.len(), 3, "self + 2 ancestors");
        assert_eq!(parsed[0]["node_id"], "fork:c");
        assert_eq!(parsed[0]["depth"], 0);
        assert_eq!(parsed[1]["node_id"], "fork:b");
        assert_eq!(parsed[1]["depth"], 1);
        assert_eq!(parsed[2]["node_id"], "fork:a");
        assert_eq!(parsed[2]["depth"], 2);
    }

    #[test]
    fn graph_db_path_falls_back_to_xdg_then_home() {
        // Smoke test: the helper never panics and produces a path
        // ending in conspectus/graph.sqlite.
        let path = graph_db_path();
        assert!(path.ends_with("conspectus/graph.sqlite"));
    }
}
