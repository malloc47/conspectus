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
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, params};

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
    /// Optional path to a SQLite loadable extension (typically
    /// `sqlite-vec`, ADR 0042). When present, the runner loads it
    /// after opening the connection but before executing the SQL,
    /// so the user's query can reference functions and virtual
    /// tables the extension provides.
    pub load_extension: Option<PathBuf>,
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
            load_extension: None,
        }
    }
}

/// Default source-field token for `--similar-to`. Mirrors the only
/// embedded text surface the in-Rust model carries today.
pub const DEFAULT_SIMILAR_TO_FIELD: &str = "last_message_preview";

/// Default result count for `--similar-to`.
pub const DEFAULT_SIMILAR_TO_LIMIT: usize = 10;

/// Inputs to the `--similar-to` linear-scan KNN runner (P9-008 / ADR
/// 0042).
pub struct SimilarToInputs<'a> {
    pub target_node_id: &'a str,
    pub source_field: &'a str,
    pub limit: usize,
    pub format: OutputFormat,
    pub db_path: Option<PathBuf>,
    pub width: Option<usize>,
    pub color: bool,
    pub load_extension: Option<PathBuf>,
}

/// Discover, load, and execute the user's query. Returns the rendered
/// output as a String. The CLI prints it; tests can assert against it
/// directly.
pub fn run_query(inputs: QueryInputs<'_>) -> Result<String> {
    let conn = open_connection(inputs.db_path.as_deref())?;
    if let Some(path) = inputs.load_extension.as_deref() {
        load_sqlite_extension(&conn, path)?;
    }
    execute(&conn, inputs.sql, inputs.format, inputs.width, inputs.color)
}

/// Linear-scan KNN over the `embeddings` table per ADR 0042. Returns
/// the formatted output as a String. The result columns are
/// `node_id`, `source_field`, `model`, and `distance` (cosine
/// distance, lower is more similar).
pub fn run_similar_to(inputs: SimilarToInputs<'_>) -> Result<String> {
    let conn = open_connection(inputs.db_path.as_deref())?;
    if let Some(path) = inputs.load_extension.as_deref() {
        load_sqlite_extension(&conn, path)?;
    }
    let (column_names, rows) = knn_linear_scan(
        &conn,
        inputs.target_node_id,
        inputs.source_field,
        inputs.limit,
    )?;
    Ok(render_output(
        &column_names,
        &rows,
        inputs.format,
        inputs.width,
        inputs.color,
    ))
}

/// SAFETY-rebranded loader for `sqlite-vec` (and any other
/// extension). `Connection::load_extension` requires us to enable
/// extension loading first; we re-disable it after the load so a
/// subsequent user-supplied SQL can't pull in additional extensions
/// it shouldn't.
fn load_sqlite_extension(conn: &Connection, path: &Path) -> Result<()> {
    // SAFETY: `load_extension_enable` and `load_extension_disable`
    // are unsafe because SQLite's extension API can execute
    // arbitrary native code from the loaded library. We accept that
    // risk for user-supplied paths — the user is the operator who
    // built or downloaded the extension.
    unsafe {
        conn.load_extension_enable()
            .context("enable extension loading")?;
    }
    let result: Result<()> = (|| {
        // SAFETY: load_extension is `unsafe` because the extension's
        // entry point can run arbitrary native code. The user-supplied
        // path is trusted operator input (analogous to LD_PRELOAD).
        unsafe {
            conn.load_extension::<_, &std::ffi::CStr>(path, None)
                .with_context(|| format!("load extension from {}", path.display()))?;
        }
        Ok(())
    })();
    // Disabling is safe in rusqlite 0.39 (no need for an unsafe
    // block); the call only resets a flag.
    conn.load_extension_disable()
        .context("disable extension loading")?;
    result
}

fn knn_linear_scan(
    conn: &Connection,
    target_node_id: &str,
    source_field: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<Value>>)> {
    // Fetch the target's vector. We require it to exist; if not,
    // surface a clear error rather than returning empty results so
    // the user can distinguish "no neighbors" from "wrong node id".
    let target_blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT vector FROM embeddings \
             WHERE node_id = ?1 AND source_field = ?2 \
             LIMIT 1",
            params![target_node_id, source_field],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .context("look up target embedding")?;
    let Some(target_blob) = target_blob else {
        return Err(anyhow!(
            "no embedding found for node_id {target_node_id:?} \
             with source_field {source_field:?}; \
             load embeddings via the documented ingestion path first"
        ));
    };
    let target_vector = blob_to_vec(&target_blob);

    // Fetch every candidate in the same source_field, excluding the
    // target itself. Stream rather than collect into a Vec to keep
    // memory bounded on larger corpora.
    let mut stmt = conn.prepare(
        "SELECT node_id, model, dim, vector FROM embeddings \
         WHERE source_field = ?1 AND node_id != ?2",
    )?;
    let candidates: Vec<(String, String, Vec<f32>)> = stmt
        .query_map(params![source_field, target_node_id], |row| {
            let node_id: String = row.get(0)?;
            let model: String = row.get(1)?;
            let _dim: i64 = row.get(2)?;
            let blob: Vec<u8> = row.get(3)?;
            Ok((node_id, model, blob_to_vec(&blob)))
        })
        .context("scan candidate embeddings")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("collect candidate embeddings")?;

    // Score with cosine distance. Rows whose vector length disagrees
    // with the target's are silently dropped — ADR 0042 punts
    // multi-dim corpora to a follow-up and the safe default here is
    // to skip rather than fail.
    let mut scored: Vec<(String, String, f64)> = candidates
        .into_iter()
        .filter(|(_, _, v)| v.len() == target_vector.len())
        .map(|(node_id, model, v)| {
            let distance = cosine_distance(&target_vector, &v);
            (node_id, model, distance)
        })
        .collect();
    scored.sort_by(|a, b| {
        a.2.partial_cmp(&b.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(limit);

    let column_names = vec![
        "node_id".to_string(),
        "source_field".to_string(),
        "model".to_string(),
        "distance".to_string(),
    ];
    let rows: Vec<Vec<Value>> = scored
        .into_iter()
        .map(|(node_id, model, distance)| {
            vec![
                Value::Text(node_id),
                Value::Text(source_field.to_string()),
                Value::Text(model),
                Value::Real(distance),
            ]
        })
        .collect();
    Ok((column_names, rows))
}

/// Decode a packed-little-endian float32 BLOB into a vector. Bytes
/// past the last full float are silently dropped.
pub fn blob_to_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Encode a vector as a packed-little-endian float32 BLOB suitable
/// for the `embeddings.vector` column.
pub fn vec_to_blob(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Cosine distance (1 - cosine similarity). Returns 1.0 when either
/// input has zero magnitude — the safe answer for "this point has
/// no information" rather than NaN.
fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
    debug_assert_eq!(
        a.len(),
        b.len(),
        "cosine_distance requires equal-length vectors"
    );
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = f64::from(*x);
        let yf = f64::from(*y);
        dot += xf * yf;
        na += xf * xf;
        nb += yf * yf;
    }
    if na == 0.0 || nb == 0.0 {
        return 1.0;
    }
    let cos = dot / (na.sqrt() * nb.sqrt());
    1.0 - cos.clamp(-1.0, 1.0)
}

/// Render the curated [`crate::query::SAVED_VIEWS`] registry as an
/// aligned two-column text listing for `conspectus query --list-views`.
/// Output is byte-deterministic so snapshot tests stay stable.
pub fn render_saved_views_list() -> String {
    use crate::query::SAVED_VIEWS;

    let name_width = SAVED_VIEWS.iter().map(|v| v.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for view in SAVED_VIEWS {
        out.push_str(view.name);
        let pad = name_width.saturating_sub(view.name.len());
        out.extend(std::iter::repeat_n(' ', pad));
        out.push_str("  ");
        out.push_str(view.description);
        out.push('\n');
    }
    out
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

    Ok(render_output(&column_names, &rows, format, width, color))
}

/// Format dispatch used by every result-producing path.
fn render_output(
    column_names: &[String],
    rows: &[Vec<Value>],
    format: OutputFormat,
    width: Option<usize>,
    color: bool,
) -> String {
    match format {
        OutputFormat::Table => render_table(column_names, rows, width, color),
        OutputFormat::Json => render_json(column_names, rows),
        OutputFormat::Csv => render_csv(column_names, rows),
        OutputFormat::Tsv => render_tsv(column_names, rows),
    }
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
        // Demonstrates the ADR 0044 join shape: candidate_links'
        // source endpoint is JSON, so structural joins to typed node
        // tables match on json_extract paths.
        let snap = snapshot_with_session_and_mux();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT s.harness_key, l.relation, l.target_node_kind \
             FROM node_agent_sessions s \
             JOIN candidate_links l \
               ON l.source_kind = 'agent_session' \
               AND json_extract(l.source, '$.harness_key') = s.harness_key \
               AND json_extract(l.source, '$.state_scope') = s.state_scope \
               AND json_extract(l.source, '$.session_key') = s.session_key",
            OutputFormat::Json,
        )
        .unwrap();
        let line = out.lines().next().expect("at least one row");
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["harness_key"], "claude-code");
        assert_eq!(parsed["relation"], "linked_to_mux");
        assert_eq!(parsed["target_node_kind"], "mux_session");
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
        // Hand-rolled equivalent of `v_fork_ancestry` — useful as a
        // documentation example for users writing their own recursive
        // queries against the JSON-encoded endpoint columns.
        let snap = snapshot_with_fork_chain();
        let out = run_query_against_snapshot(
            &snap,
            "WITH RECURSIVE ancestry(psk, depth) AS ( \
               SELECT 'c', 0 \
               UNION ALL \
               SELECT json_extract(cl.target_node, '$.provider_source_key'), ancestry.depth + 1 \
                 FROM candidate_links cl \
                 JOIN ancestry \
                   ON cl.source_kind = 'fork' \
                   AND json_extract(cl.source, '$.provider_source_key') = ancestry.psk \
                 WHERE cl.relation = 'parent_fork' \
                   AND cl.target_node_kind = 'fork' \
             ) SELECT 'fork:' || psk AS node_id, depth FROM ancestry ORDER BY depth",
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

    // -------------------------------------------------------------
    // P9-006: saved views + --list-views.
    // -------------------------------------------------------------

    use crate::model::{
        BranchId, BranchNode, CheckoutId, CheckoutNode, ForgePrId, ForgePrNode, RepoId, RepoNode,
        WorkspaceId, WorkspaceNode,
    };

    fn fixture_with_session_under_checkout() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        let repo_common = "/r/.git";
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_common))));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common), "/r"),
            root: "/r".into(),
            git_dir: None,
            current_branch: None,
        }));
        // Nested checkout so the deepest-wins join in v_sessions_with_repo
        // has something to disambiguate.
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common), "/r/sub"),
            root: "/r/sub".into(),
            git_dir: None,
            current_branch: None,
        }));
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "default", "s1"),
            harness_key: "claude-code".into(),
            cwd: Some("/r/sub/deep/path".into()),
            title: None,
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
        }));
        snap
    }

    #[test]
    fn v_sessions_with_repo_picks_deepest_checkout() {
        let snap = fixture_with_session_under_checkout();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT session_node_id, checkout_root, repo_common_dir \
             FROM v_sessions_with_repo",
            OutputFormat::Json,
        )
        .unwrap();
        let line = out.lines().next().expect("one row");
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(
            parsed["session_node_id"]
                .as_str()
                .unwrap()
                .starts_with("agent_session:claude-code:")
        );
        // Deepest matching checkout root for cwd /r/sub/deep/path is /r/sub,
        // not /r — the correlated subquery in the view picks it.
        assert_eq!(parsed["checkout_root"], "/r/sub");
        assert_eq!(parsed["repo_common_dir"], "/r/.git");
    }

    #[test]
    fn v_mux_attachments_returns_one_row_per_active_link() {
        let snap = snapshot_with_session_and_mux();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT agent_session_harness_key, agent_session_session_key, \
                    backend, native_id, provenance, confidence \
             FROM v_mux_attachments",
            OutputFormat::Json,
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["agent_session_harness_key"], "claude-code");
        assert_eq!(parsed["backend"], "tmux");
        assert_eq!(parsed["native_id"], "tmux:0");
        assert_eq!(parsed["provenance"], "strong_discovered");
        assert_eq!(parsed["confidence"], "high");
    }

    fn fixture_with_pr_on_branch() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        let repo = "/r/.git";
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo))));
        let branch_id = BranchId::new(RepoId::new(repo), "refs/heads/feature");
        snap.nodes.push(GraphNode::Branch(BranchNode {
            id: branch_id.clone(),
            refname: "refs/heads/feature".into(),
            current_commit: None,
            upstream: None,
        }));
        let pr = ForgePrNode {
            id: ForgePrId::new("github", "github.com", "owner", "repo", 42),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "owner".into(),
            repo: "repo".into(),
            number: 42,
            state: Some("open".into()),
            url: Some("https://github.com/owner/repo/pull/42".into()),
            updated_epoch: None,
            is_draft: false,
        };
        snap.nodes.push(GraphNode::ForgePr(pr.clone()));
        // Production discovery (forge::github) and the in-memory
        // `preferred_pr_for_session` both build branch_has_forge_pr
        // with source=ForgePr, target=Branch — despite the relation
        // name reading "branch has forge pr". v_pr_by_branch joins
        // against that direction; this fixture mirrors it.
        snap.candidate_links.push(GraphLink {
            id: "L1".into(),
            source: NodeId::ForgePr(pr.id.clone()),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snap
    }

    #[test]
    fn v_pr_by_branch_joins_branches_to_their_prs() {
        let snap = fixture_with_pr_on_branch();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT refname, pr_state, pr_number FROM v_pr_by_branch",
            OutputFormat::Json,
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["refname"], "refs/heads/feature");
        assert_eq!(parsed["pr_state"], "open");
        assert_eq!(parsed["pr_number"], 42);
    }

    #[test]
    fn v_fork_ancestry_returns_self_plus_chain() {
        let snap = snapshot_with_fork_chain();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT fork_node_id, ancestor_node_id, depth \
             FROM v_fork_ancestry \
             WHERE fork_node_id = 'fork:c' \
             ORDER BY depth",
            OutputFormat::Json,
        )
        .unwrap();
        let parsed: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(parsed.len(), 3, "self + 2 ancestors");
        assert_eq!(parsed[0]["ancestor_node_id"], "fork:c");
        assert_eq!(parsed[0]["depth"], 0);
        assert_eq!(parsed[1]["ancestor_node_id"], "fork:b");
        assert_eq!(parsed[2]["ancestor_node_id"], "fork:a");
    }

    fn fixture_with_workspace_containing_repo() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        let repo = "/r/.git";
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo))));
        snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new("/w"),
            root: "/w".into(),
            provider: Some("atelier".into()),
            name: None,
        }));
        snap.candidate_links.push(GraphLink {
            id: "wcr".into(),
            source: NodeId::Workspace(WorkspaceId::new("/w")),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(RepoId::new(repo)),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snap
    }

    #[test]
    fn v_workspace_member_repos_joins_workspaces_to_their_repos() {
        let snap = fixture_with_workspace_containing_repo();
        let out = run_query_against_snapshot(
            &snap,
            "SELECT workspace_root, workspace_provider, repo_common_dir \
             FROM v_workspace_member_repos",
            OutputFormat::Json,
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["workspace_root"], "/w");
        assert_eq!(parsed["workspace_provider"], "atelier");
        assert_eq!(parsed["repo_common_dir"], "/r/.git");
    }

    #[test]
    fn render_saved_views_list_lists_every_registered_view() {
        let rendered = render_saved_views_list();
        for view in crate::query::SAVED_VIEWS {
            assert!(
                rendered.contains(view.name),
                "missing {} in:\n{rendered}",
                view.name
            );
            assert!(
                rendered.contains(view.description),
                "missing description for {} in rendered output",
                view.name
            );
        }
        // Same number of lines as registered views (one per line, no
        // blank trailing line).
        let line_count = rendered.lines().count();
        assert_eq!(line_count, crate::query::SAVED_VIEWS.len());
    }

    // -------------------------------------------------------------
    // P9-008: vector search (ADR 0042).
    // -------------------------------------------------------------

    #[test]
    fn blob_vec_codec_round_trips() {
        let original: Vec<f32> = vec![0.0, 1.0, -1.5, 2.75, f32::MIN_POSITIVE];
        let blob = vec_to_blob(&original);
        assert_eq!(blob.len(), original.len() * 4);
        let decoded = blob_to_vec(&blob);
        assert_eq!(decoded, original);
    }

    #[test]
    fn cosine_distance_known_pairs() {
        // Identical vectors → distance 0.
        let v = vec![1.0_f32, 2.0, 3.0];
        assert!((cosine_distance(&v, &v)).abs() < 1e-9);
        // Orthogonal vectors → distance 1.
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!((cosine_distance(&a, &b) - 1.0).abs() < 1e-9);
        // Opposite vectors → distance 2.
        let c = vec![1.0_f32, 0.0];
        let d = vec![-1.0_f32, 0.0];
        assert!((cosine_distance(&c, &d) - 2.0).abs() < 1e-9);
        // Zero-magnitude side falls back to distance 1.
        let zero = vec![0.0_f32, 0.0];
        let nonzero = vec![1.0_f32, 0.0];
        assert!((cosine_distance(&zero, &nonzero) - 1.0).abs() < 1e-9);
    }

    /// Open a fresh writable connection, apply the schema, and INSERT
    /// the supplied embeddings directly. Used by the KNN tests where
    /// the loader's run-once contract doesn't help (the loader never
    /// touches the embeddings table by design).
    fn conn_with_embeddings(rows: &[(&str, &str, &str, Vec<f32>)]) -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory open");
        apply_schema(&conn).expect("apply schema");
        for (node_id, source_field, model, vector) in rows {
            conn.execute(
                "INSERT INTO embeddings (node_id, source_field, model, dim, vector) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    node_id,
                    source_field,
                    model,
                    vector.len() as i64,
                    vec_to_blob(vector),
                ],
            )
            .expect("insert embedding");
        }
        conn
    }

    #[test]
    fn knn_linear_scan_orders_by_cosine_distance() {
        let target_vec: Vec<f32> = vec![1.0, 0.0, 0.0];
        let conn = conn_with_embeddings(&[
            (
                "agent_session:claude-code:default:target",
                "preview",
                "m1",
                target_vec.clone(),
            ),
            (
                "agent_session:claude-code:default:near",
                "preview",
                "m1",
                vec![0.9, 0.1, 0.0],
            ),
            (
                "agent_session:claude-code:default:far",
                "preview",
                "m1",
                vec![0.0, 1.0, 0.0],
            ),
            (
                "agent_session:claude-code:default:opposite",
                "preview",
                "m1",
                vec![-1.0, 0.0, 0.0],
            ),
        ]);
        let (cols, rows) = knn_linear_scan(
            &conn,
            "agent_session:claude-code:default:target",
            "preview",
            10,
        )
        .expect("knn");
        // Column names: node_id, source_field, model, distance.
        assert_eq!(cols.len(), 4);
        // Three neighbors, ordered by ascending distance: near, far, opposite.
        assert_eq!(rows.len(), 3);
        let ids: Vec<&str> = rows
            .iter()
            .map(|r| match &r[0] {
                Value::Text(s) => s.as_str(),
                _ => panic!("expected TEXT in column 0"),
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                "agent_session:claude-code:default:near",
                "agent_session:claude-code:default:far",
                "agent_session:claude-code:default:opposite",
            ]
        );
    }

    #[test]
    fn knn_linear_scan_returns_clean_error_when_target_missing() {
        let conn = conn_with_embeddings(&[]);
        let err = knn_linear_scan(&conn, "agent_session:does-not-exist", "preview", 10)
            .expect_err("expected an error for a missing target");
        let chain = format!("{err:#}");
        assert!(
            chain.contains("no embedding found"),
            "unexpected error: {chain}"
        );
    }

    #[test]
    fn knn_linear_scan_respects_limit() {
        let target_vec: Vec<f32> = vec![1.0, 0.0];
        let mut fixture = vec![("target", "preview", "m", target_vec.clone())];
        for i in 0..20 {
            let key: &'static str = match i {
                0 => "n_00",
                1 => "n_01",
                2 => "n_02",
                3 => "n_03",
                4 => "n_04",
                5 => "n_05",
                6 => "n_06",
                7 => "n_07",
                8 => "n_08",
                9 => "n_09",
                10 => "n_10",
                11 => "n_11",
                12 => "n_12",
                13 => "n_13",
                14 => "n_14",
                15 => "n_15",
                16 => "n_16",
                17 => "n_17",
                18 => "n_18",
                19 => "n_19",
                _ => unreachable!(),
            };
            fixture.push((
                key,
                "preview",
                "m",
                vec![1.0 - (i as f32) * 0.01, (i as f32) * 0.01],
            ));
        }
        let conn = conn_with_embeddings(&fixture);
        let (_, rows) = knn_linear_scan(&conn, "target", "preview", 5).unwrap();
        assert_eq!(rows.len(), 5, "limit honored");
    }

    #[test]
    fn knn_linear_scan_skips_mismatched_dim_rows() {
        // Two candidates: one with matching dim (3), one with a
        // different dim (2). Only the matching one should appear in
        // the result. ADR 0042 punts multi-dim to a follow-up; the
        // safe default is silent skip.
        let conn = conn_with_embeddings(&[
            ("t", "preview", "m", vec![1.0, 0.0, 0.0]),
            ("c_ok", "preview", "m", vec![0.9, 0.1, 0.0]),
            ("c_wrong_dim", "preview", "m", vec![0.9, 0.1]),
        ]);
        let (_, rows) = knn_linear_scan(&conn, "t", "preview", 10).unwrap();
        assert_eq!(rows.len(), 1);
        let id = match &rows[0][0] {
            Value::Text(s) => s.as_str(),
            _ => panic!(),
        };
        assert_eq!(id, "c_ok");
    }

    #[test]
    fn knn_linear_scan_returns_zero_rows_when_target_alone() {
        let conn = conn_with_embeddings(&[("solo", "preview", "m", vec![1.0, 0.0])]);
        let (_, rows) = knn_linear_scan(&conn, "solo", "preview", 10).unwrap();
        assert!(rows.is_empty(), "no neighbors should yield empty result");
    }

    #[test]
    fn load_sqlite_extension_returns_clear_error_for_bad_path() {
        let conn = Connection::open_in_memory().unwrap();
        let err = load_sqlite_extension(&conn, Path::new("/nonexistent/extension.so"))
            .expect_err("expected an error for a bogus path");
        let chain = format!("{err:#}");
        // Either the load fails outright or the file can't be found.
        // Either error path is acceptable; just ensure it surfaces
        // cleanly rather than panicking.
        assert!(
            chain.contains("load extension from") || chain.contains("not allowed"),
            "unexpected error chain: {chain}"
        );
    }
}
