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
    /// Plain-text columnar table with a header row and aligned cells,
    /// truncated with `…` to honor the width budget (ADR 0020).
    Table,
    /// One JSON object per line, keyed by column name.
    Json,
    /// RFC-4180-style CSV: comma-separated, double-quote-wrapped when
    /// the cell contains a delimiter, quote, or newline. CRLF row
    /// terminator.
    Csv,
    /// Tab-separated values. Tabs and newlines inside cells are
    /// replaced with literal `\t` and `\n` escapes; the rendering
    /// stays line-oriented at the cost of a one-way escape.
    Tsv,
}

/// Inputs to the executor. `db_path` overrides the default XDG path
/// (used by tests; the CLI passes `None` to use the canonical
/// location).
pub struct QueryInputs<'a> {
    pub sql: &'a str,
    pub format: OutputFormat,
    pub db_path: Option<PathBuf>,
    /// Target total width in display columns for the [`Table`] format,
    /// per ADR 0020. `None` renders untruncated (the right default for
    /// pipes and machine-readable formats).
    pub width: Option<usize>,
    /// Whether to emit ANSI color escapes when the format supports
    /// them (currently `Table` only). Resolved by the CLI per
    /// ADR 0022; non-`Table` formats ignore this flag.
    pub color: bool,
}

impl<'a> QueryInputs<'a> {
    /// Construct an inputs struct with the format-friendly defaults
    /// (no width truncation, color off). Tests use this; the CLI sets
    /// width/color explicitly.
    pub fn plain(sql: &'a str, format: OutputFormat) -> Self {
        Self {
            sql,
            format,
            db_path: None,
            width: None,
            color: false,
        }
    }
}

/// Discover, load, and execute the user's query. Returns the rendered
/// output as a String. The CLI prints it; tests can assert against it
/// directly.
pub fn run_query(inputs: QueryInputs<'_>) -> Result<String> {
    let conn = open_connection(inputs.db_path.as_deref())?;
    execute(&conn, inputs.sql, inputs.format, inputs.width, inputs.color)
}

/// Same as [`run_query`] but takes a pre-built snapshot. Useful in tests
/// that want to assert query behavior against a known fixture without
/// touching cold discovery or the filesystem.
pub fn run_query_against_snapshot(
    snapshot: &GraphSnapshot,
    sql: &str,
    format: OutputFormat,
) -> Result<String> {
    run_query_against_snapshot_with(snapshot, sql, format, None, false)
}

/// Same as [`run_query_against_snapshot`] but exposes the width and
/// color knobs for snapshot tests over the [`OutputFormat::Table`]
/// renderer.
pub fn run_query_against_snapshot_with(
    snapshot: &GraphSnapshot,
    sql: &str,
    format: OutputFormat,
    width: Option<usize>,
    color: bool,
) -> Result<String> {
    let mut conn = Connection::open_in_memory().context("open in-memory database")?;
    apply_schema(&conn).context("apply schema")?;
    load(snapshot, &mut conn).context("load snapshot")?;
    lock_read_only(&conn)?;
    apply_query_pragmas(&conn)?;
    execute(&conn, sql, format, width, color)
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

fn execute(
    conn: &Connection,
    sql: &str,
    format: OutputFormat,
    width: Option<usize>,
    color: bool,
) -> Result<String> {
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
        OutputFormat::Table => render_table(&column_names, &rows, width, color),
        OutputFormat::Json => render_json(&column_names, &rows),
        OutputFormat::Csv => render_csv(&column_names, &rows),
        OutputFormat::Tsv => render_tsv(&column_names, &rows),
    })
}

/// Render the query result as a width-aware columnar text table per
/// ADR 0020. Reuses the `natural_widths` / `fit_to_width` /
/// `truncate_to_width` primitives from `output::table` so the
/// budget arithmetic and Unicode handling stay byte-identical across
/// `conspectus query` and the existing `conspectus table`.
fn render_table(
    headers: &[String],
    rows: &[Vec<Value>],
    width: Option<usize>,
    color: bool,
) -> String {
    use crate::output::table::{
        COLUMN_GAP, display_width, fit_to_width, header_style, natural_widths, push_styled,
        truncate_to_width,
    };

    let column_count = headers.len();
    if column_count == 0 {
        return String::new();
    }

    // Layer header + body into one Vec<Vec<String>> so the width
    // arithmetic and truncation step apply uniformly.
    let mut display_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len() + 1);
    display_rows.push(headers.to_vec());
    for row in rows {
        display_rows.push(row.iter().map(value_to_string).collect());
    }

    let naturals = natural_widths(&display_rows, column_count);
    let budgets = match width {
        None => naturals,
        Some(target) => fit_to_width(&naturals, &display_rows[0], target),
    };

    let mut out = String::new();
    for (row_idx, row) in display_rows.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx > 0 {
                out.push_str(COLUMN_GAP);
            }
            let budget = budgets[col_idx];
            let truncated = truncate_to_width(cell, budget);
            if row_idx == 0 {
                push_styled(&mut out, &truncated, header_style(), color);
            } else {
                out.push_str(&truncated);
            }
            if col_idx + 1 < column_count {
                let pad = budget.saturating_sub(display_width(&truncated));
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push('\n');
        if row_idx == 0 {
            for (col_idx, w) in budgets.iter().enumerate() {
                if col_idx > 0 {
                    out.push_str(COLUMN_GAP);
                }
                out.extend(std::iter::repeat_n('-', *w));
            }
            out.push('\n');
        }
    }
    out
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

/// RFC 4180 CSV: comma-separated, CRLF row terminator. Fields are
/// wrapped in double quotes when they contain a delimiter, double
/// quote, CR, or LF; embedded quotes are doubled.
fn render_csv(headers: &[String], rows: &[Vec<Value>]) -> String {
    let mut out = String::new();
    write_csv_row(&mut out, headers.iter().map(String::as_str));
    let cell_strings: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(value_to_string).collect())
        .collect();
    for row in &cell_strings {
        write_csv_row(&mut out, row.iter().map(String::as_str));
    }
    out
}

fn write_csv_row<'a, I>(out: &mut String, cells: I)
where
    I: IntoIterator<Item = &'a str>,
{
    let mut first = true;
    for cell in cells {
        if !first {
            out.push(',');
        }
        first = false;
        let needs_quote = cell.chars().any(|c| matches!(c, ',' | '"' | '\n' | '\r'));
        if needs_quote {
            out.push('"');
            for c in cell.chars() {
                if c == '"' {
                    out.push('"');
                }
                out.push(c);
            }
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push_str("\r\n");
}

/// Tab-separated values. Embedded tabs and newlines become literal
/// `\t` / `\n` escapes so the rendering stays line-oriented; embedded
/// backslashes are doubled so the escape is unambiguous.
fn render_tsv(headers: &[String], rows: &[Vec<Value>]) -> String {
    let mut out = String::new();
    write_tsv_row(&mut out, headers.iter().map(String::as_str));
    let cell_strings: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(value_to_string).collect())
        .collect();
    for row in &cell_strings {
        write_tsv_row(&mut out, row.iter().map(String::as_str));
    }
    out
}

fn write_tsv_row<'a, I>(out: &mut String, cells: I)
where
    I: IntoIterator<Item = &'a str>,
{
    let mut first = true;
    for cell in cells {
        if !first {
            out.push('\t');
        }
        first = false;
        for c in cell.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                other => out.push(other),
            }
        }
    }
    out.push('\n');
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

    // -------------------------------------------------------------
    // P9-005: format snapshots, width-aware truncation, color codes,
    // CSV / TSV escaping.
    // -------------------------------------------------------------

    #[test]
    fn table_natural_width_pads_short_cells() {
        let out = empty_snapshot_run(
            "SELECT 'hi' AS greeting, 'longer-value' AS body",
            OutputFormat::Table,
        )
        .unwrap();
        // header + separator + one row. The shorter "hi" cell pads
        // to match its column's natural width ("greeting").
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        // Body row: "hi      " (8 chars to match "greeting") + 2 gap +
        // "longer-value"
        assert_eq!(lines[2], "hi        longer-value");
    }

    #[test]
    fn table_truncates_to_width_budget_with_ellipsis() {
        // Two columns where each natural width is 24. At target width
        // 20 the renderer must shrink at least one column to fit.
        let out = run_query_against_snapshot_with(
            &GraphSnapshot::empty(),
            "SELECT '012345678901234567890123' AS a, 'abcdefghijklmnopqrstuvwx' AS b",
            OutputFormat::Table,
            Some(20),
            false,
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // The header line carries `…` somewhere in at least one cell.
        assert!(
            lines[2].contains('…'),
            "expected truncation ellipsis in: {}",
            lines[2]
        );
        // Total rendered width of the body row should not exceed 20
        // columns. We rely on `display_width` (re-used from
        // `output::table`) for the measurement to match the renderer's
        // own arithmetic.
        let body_width = crate::output::table::display_width(lines[2]);
        assert!(
            body_width <= 20,
            "body row width {body_width} exceeds budget 20"
        );
    }

    #[test]
    fn table_color_wraps_header_in_bold_ansi() {
        let out = run_query_against_snapshot_with(
            &GraphSnapshot::empty(),
            "SELECT 1 AS x",
            OutputFormat::Table,
            None,
            true,
        )
        .unwrap();
        // Bold opens with ESC[1m and closes with the reset sequence.
        // We do not pin the exact bytes (anstyle's reset is a known
        // sequence but we don't want to couple the test to its
        // formatting); just assert both an ESC opener and a reset
        // appear around the header text.
        let header_line = out.lines().next().unwrap();
        assert!(
            header_line.contains("\x1b["),
            "no ANSI opener: {header_line:?}"
        );
        assert!(
            header_line.contains("\x1b[0m") || header_line.contains("\x1b[m"),
            "no ANSI reset: {header_line:?}"
        );
    }

    #[test]
    fn table_color_disabled_emits_no_ansi() {
        let out = empty_snapshot_run("SELECT 1 AS x", OutputFormat::Table).unwrap();
        assert!(!out.contains('\x1b'), "unexpected ANSI in: {out:?}");
    }

    #[test]
    fn csv_emits_crlf_and_quotes_only_when_needed() {
        let out = empty_snapshot_run(
            "SELECT 'plain' AS a, 'with,comma' AS b, 'with\"quote' AS c",
            OutputFormat::Csv,
        )
        .unwrap();
        // Header line ends in CRLF.
        let header_end = out.find("\r\n").unwrap();
        assert_eq!(&out[..header_end], "a,b,c");
        // Body row: plain field unquoted; comma field quoted; quote
        // field quoted with the embedded `"` doubled.
        let rest = &out[header_end + 2..];
        let body_end = rest.find("\r\n").unwrap();
        assert_eq!(&rest[..body_end], "plain,\"with,comma\",\"with\"\"quote\"");
    }

    #[test]
    fn csv_quotes_fields_containing_newlines() {
        let out = empty_snapshot_run(
            "SELECT 'line1' || char(10) || 'line2' AS multi",
            OutputFormat::Csv,
        )
        .unwrap();
        // The body row is wrapped in quotes because of the embedded
        // newline. The newline survives between the quotes.
        let parts: Vec<&str> = out.split("\r\n").collect();
        // parts = [header, body, ""]
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "multi");
        assert_eq!(parts[1], "\"line1\nline2\"");
    }

    #[test]
    fn tsv_escapes_tab_newline_and_backslash() {
        let out = empty_snapshot_run(
            "SELECT 'a' || char(9) || 'b' AS tabbed, \
             'x' || char(10) || 'y' AS newlined, \
             'one\\two' AS slashed",
            OutputFormat::Tsv,
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "tabbed\tnewlined\tslashed");
        assert_eq!(lines[1], "a\\tb\tx\\ny\tone\\\\two");
    }

    #[test]
    fn null_renders_as_empty_in_csv_and_tsv() {
        let csv = empty_snapshot_run("SELECT NULL AS x, 'next' AS y", OutputFormat::Csv).unwrap();
        let body = csv.lines().nth(1).unwrap();
        // CSV body row: empty field, comma, "next", then \r is stripped
        // by .lines(). Leading empty before the comma is the NULL.
        assert!(body.starts_with(",next") || body == ",next");

        let tsv = empty_snapshot_run("SELECT NULL AS x, 'next' AS y", OutputFormat::Tsv).unwrap();
        let body = tsv.lines().nth(1).unwrap();
        // First column empty, tab, then `next`.
        assert_eq!(body, "\tnext");
    }
}
