//! SQLite-backed union projection renderer (P10-006 / ADR 0043).
//!
//! Replaces the in-memory `build_union_rows` path for
//! [`Projection::Union`]. Emits one row per `agent_session`
//! followed by one row per `mux_session`, with per-row-kind cell
//! rendering driven by a shared column set.
//!
//! The merge happens in SQL via the pre-existing `v_nodes` view —
//! a `UNION ALL` over every `node_<kind>` table — joined left to
//! the per-kind structural tables for cell data. ORDER BY in SQL
//! drives iteration so the renderer doesn't have to concatenate
//! two Rust vectors in a particular order, and the projection
//! reuses the same view future "all nodes" consumers will want.
//!
//! The `relationship` cell on agent rows still needs a per-agent
//! preferred-`linked_to_mux` lookup; that stays a separate small
//! query rather than turning into a correlated subquery on the
//! primary query.

use std::collections::HashMap;

use rusqlite::Connection;

use super::render::{
    self, RenderOptions, UNION_COLUMNS, header_label, node_short_id_from_display, pick_strongest,
    unique_prefix_len,
};
use super::table::agent_session_key_for_label;

type SessionKey = (String, String, String);

fn session_key(harness_key: &str, state_scope: &str, session_key: &str) -> SessionKey {
    (
        harness_key.to_string(),
        state_scope.to_string(),
        session_key.to_string(),
    )
}

/// One row from the union query. Exactly one of the agent-side or
/// mux-side column groups is populated; `node_kind` is the
/// discriminator the cell extractor dispatches on. The schema is
/// wider than either kind in isolation because SQL `UNION ALL`
/// requires compatible column shapes, but every row materializes
/// just one side's data — the other side's columns are `NULL`.
#[derive(Debug, Clone)]
struct UnionRow {
    node_id_display: String,
    /// `agent_session` or `mux_session`; matches the
    /// `v_nodes.node_kind` discriminator.
    node_kind: String,
    // Agent-side fields (populated when node_kind = 'agent_session'):
    harness_key: Option<String>,
    state_scope: Option<String>,
    session_key: Option<String>,
    agent_cwd: Option<String>,
    title: Option<String>,
    preview: Option<String>,
    alias_display_name: Option<String>,
    // Mux-side fields (populated when node_kind = 'mux_session'):
    backend: Option<String>,
    native_id: Option<String>,
    mux_cwd: Option<String>,
}

impl UnionRow {
    /// Session key for agent rows; meaningless on mux rows.
    fn agent_session_key(&self) -> Option<SessionKey> {
        let h = self.harness_key.as_deref()?;
        let s = self.state_scope.as_deref()?;
        let k = self.session_key.as_deref()?;
        Some(session_key(h, s, k))
    }
}

#[derive(Debug, Clone)]
struct PreferredMux {
    backend: String,
    native_id: String,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_union_rows_from_conn(
    conn: &Connection,
    columns: &[&'static str],
    _options: &RenderOptions,
) -> rusqlite::Result<Vec<Vec<String>>> {
    let rows_data = fetch_union_rows(conn)?;
    let preferred_mux = fetch_preferred_mux_per_agent(conn)?;

    let body_full_ids: Vec<String> = rows_data
        .iter()
        .map(|r| node_short_id_from_display(&r.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(UNION_COLUMNS, key))
            .collect(),
    );

    for (row, full_short) in rows_data.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let preferred = row
            .agent_session_key()
            .as_ref()
            .and_then(|k| preferred_mux.get(k));
        let ctx = CellCtx {
            row,
            short_id,
            preferred_mux: preferred,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    Ok(rows)
}

struct CellCtx<'a> {
    row: &'a UnionRow,
    short_id: &'a str,
    preferred_mux: Option<&'a PreferredMux>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match (key, ctx.row.node_kind.as_str()) {
        ("id", _) => ctx.short_id.to_string(),
        ("kind", "agent_session") => "agent".to_string(),
        ("kind", "mux_session") => "mux".to_string(),
        ("label", "agent_session") => format!(
            "{}:{}",
            ctx.row.harness_key.as_deref().unwrap_or(""),
            agent_session_key_for_label(ctx.row.session_key.as_deref().unwrap_or("")),
        ),
        ("label", "mux_session") => format!(
            "{}:{}",
            ctx.row.backend.as_deref().unwrap_or(""),
            ctx.row.native_id.as_deref().unwrap_or(""),
        ),
        ("cwd", "agent_session") => ctx.row.agent_cwd.clone().unwrap_or_else(dash),
        ("cwd", "mux_session") => ctx.row.mux_cwd.clone().unwrap_or_else(dash),
        ("relationship", "agent_session") => match ctx.preferred_mux {
            Some(m) => format!(
                "mux={}:{} [{}]",
                m.backend,
                m.native_id,
                render::indicator_from_tags(&m.provenance, &m.confidence, m.candidate_count > 1)
            ),
            None => "mux=—".to_string(),
        },
        ("preview", "agent_session") => ctx.row.preview.clone().unwrap_or_else(dash),
        ("title", "agent_session") => ctx
            .row
            .alias_display_name
            .clone()
            .or_else(|| ctx.row.title.clone())
            .unwrap_or_else(dash),
        // Mux rows: relationship/preview/title render as `—` per the
        // in-memory `union_cell`.
        _ => dash(),
    }
}

pub fn build_union_rows_from_snapshot(
    snapshot: &crate::model::GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for union projection");
    build_union_rows_from_conn(&conn, columns, options)
        .expect("SQLite-backed union projection should not fail against a freshly loaded snapshot")
}

// -----------------------------------------------------------------------------
// Queries
// -----------------------------------------------------------------------------

/// Drives the union projection's iteration via `v_nodes` (the
/// pre-existing UNION ALL over every `node_<kind>` table). Per-kind
/// structural columns come from LEFT JOINs to the typed node
/// tables; the alias overlay is LEFT-joined on the agent side only.
///
/// `ORDER BY` puts every agent row before every mux row (the
/// in-memory walk's order — agents first, then muxes) and breaks
/// ties within a kind by `node_id` lex order. For typical ASCII
/// structural fields that matches the in-memory
/// `BTreeMap<NodeId>` iteration over `AgentSessionId` /
/// `MuxSessionId`'s derived `Ord`.
fn fetch_union_rows(conn: &Connection) -> rusqlite::Result<Vec<UnionRow>> {
    let mut stmt = conn.prepare(
        "SELECT v.node_id, v.node_kind, \
                a.harness_key, a.state_scope, a.session_key, \
                a.cwd AS agent_cwd, a.title, a.last_message_preview, \
                al.display_name AS alias_display_name, \
                m.backend, m.native_id, m.cwd AS mux_cwd \
         FROM v_nodes v \
         LEFT JOIN node_agent_sessions a \
                ON v.node_kind = 'agent_session' \
               AND a.node_id   = v.node_id \
         LEFT JOIN node_mux_sessions m \
                ON v.node_kind = 'mux_session' \
               AND m.node_id   = v.node_id \
         LEFT JOIN aliases al \
                ON v.node_kind = 'agent_session' \
               AND al.node_kind = 'agent_session' \
               AND json_extract(al.node, '$.harness_key') = a.harness_key \
               AND json_extract(al.node, '$.state_scope') = a.state_scope \
               AND json_extract(al.node, '$.session_key') = a.session_key \
         WHERE v.node_kind IN ('agent_session', 'mux_session') \
         ORDER BY CASE v.node_kind \
                      WHEN 'agent_session' THEN 0 \
                      WHEN 'mux_session'   THEN 1 \
                  END, \
                  v.node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(UnionRow {
            node_id_display: row.get(0)?,
            node_kind: row.get(1)?,
            harness_key: row.get(2)?,
            state_scope: row.get(3)?,
            session_key: row.get(4)?,
            agent_cwd: row.get(5)?,
            title: row.get(6)?,
            preview: row.get(7)?,
            alias_display_name: row.get(8)?,
            backend: row.get(9)?,
            native_id: row.get(10)?,
            mux_cwd: row.get(11)?,
        })
    })?;
    rows.collect()
}

/// Per-agent preferred `linked_to_mux` candidate joined to its
/// mux's structural columns. Same shape as
/// `output::agent::fetch_mux_lookup` and `output::union`'s
/// preferred-mux query — a future story can fold these into a
/// shared helper once the third caller (TUI) lands.
fn fetch_preferred_mux_per_agent(
    conn: &Connection,
) -> rusqlite::Result<HashMap<SessionKey, PreferredMux>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(cl.source, '$.harness_key') AS h, \
                json_extract(cl.source, '$.state_scope') AS s, \
                json_extract(cl.source, '$.session_key') AS k, \
                cl.link_id, cl.provenance, cl.confidence, \
                m.backend, m.native_id \
         FROM candidate_links cl \
         JOIN node_mux_sessions m \
           ON cl.target_node_kind = 'mux_session' \
           AND ('mux_session:' || json_extract(cl.target_node, '$.native_id')) = m.node_id \
         WHERE cl.source_kind = 'agent_session' \
           AND cl.relation = 'linked_to_mux' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via pick_strongest's tiebreak accessor
    struct Raw {
        link_id: String,
        provenance: String,
        confidence: String,
        backend: String,
        native_id: String,
    }
    let mut per_session: HashMap<SessionKey, Vec<Raw>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let h: String = row.get(0)?;
        let s: String = row.get(1)?;
        let k: String = row.get(2)?;
        Ok((
            session_key(&h, &s, &k),
            Raw {
                link_id: row.get(3)?,
                provenance: row.get(4)?,
                confidence: row.get(5)?,
                backend: row.get(6)?,
                native_id: row.get(7)?,
            },
        ))
    })?;
    for entry in rows {
        let (key, raw) = entry?;
        per_session.entry(key).or_default().push(raw);
    }
    let mut out = HashMap::new();
    for (key, candidates) in per_session {
        let candidate_count = candidates.len();
        let Some(best) = pick_strongest(candidates, |r: &Raw| {
            (&r.provenance, &r.confidence, &r.link_id)
        }) else {
            continue;
        };
        out.insert(
            key,
            PreferredMux {
                backend: best.backend,
                native_id: best.native_id,
                provenance: best.provenance,
                confidence: best.confidence,
                candidate_count,
            },
        );
    }
    Ok(out)
}
