//! SQLite-backed mux projection renderer (P10-005 / ADR 0043).
//!
//! Replaces the in-memory `build_mux_rows` path for
//! [`Projection::Mux`]. Reads everything from a
//! [`rusqlite::Connection`] populated by the loader. Parity with the
//! in-memory renderer is enforced by the existing `output::table`
//! snapshot test corpus.
//!
//! Query strategy:
//!
//! - Primary query: `SELECT … FROM node_mux_sessions ORDER BY node_id`
//!   — drives row iteration.
//! - Side-lookups for the `agents` / `preview` / `attached-count`
//!   cells: fetch every active `linked_to_mux` candidate whose
//!   source agent session is present in `node_agent_sessions`,
//!   group by mux `node_id` in Rust, and remember each attached
//!   agent's label, provenance/confidence, and `last_message_preview`.
//! - Side-lookup for the per-agent ambiguity count that feeds the
//!   `*` marker in the per-attachment indicator: count distinct
//!   active mux targets per source session.
//!
//! All JOINs to typed `node_<kind>` tables go through Display-form
//! reconstruction from the endpoint JSON (`'mux_session:' || …`)
//! and compare against `node_<kind>.node_id`, for the reason
//! captured in `output::agent`'s module doc.

use std::collections::HashMap;

use rusqlite::Connection;

use super::render::{
    self, MUX_COLUMNS, RenderOptions, current_epoch, format_relative_age, header_label,
    node_short_id_from_display, pick_strongest, unique_prefix_len,
};
use super::table::agent_session_key_for_label;
use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};

/// `(harness_key, state_scope, session_key)` triple identifying an
/// attached agent session — same shape as in `output::agent`.
type SessionKey = (String, String, String);

fn session_key(harness_key: &str, state_scope: &str, session_key: &str) -> SessionKey {
    (
        harness_key.to_string(),
        state_scope.to_string(),
        session_key.to_string(),
    )
}

#[derive(Debug, Clone)]
struct MuxRow {
    /// `NodeId::Display` of the mux — used for the `id` column hash
    /// and as the join key into the attachments lookup.
    node_id_display: String,
    backend: String,
    native_id: String,
    cwd: Option<String>,
    activity_epoch: Option<i64>,
    created_epoch: Option<i64>,
}

/// One attached-agent entry, captured in iteration order matching
/// the former in-memory mux attachment build.
#[derive(Debug, Clone)]
struct AttachedAgent {
    /// Pre-formatted `<harness>:<session_key>` label.
    label: String,
    /// Provenance / confidence of the preferred `linked_to_mux`
    /// candidate that ties this agent to the mux.
    provenance: String,
    confidence: String,
    /// `last_message_preview` of the agent session, when set.
    preview: Option<String>,
    /// Triple of the source agent session, used to look up the
    /// per-agent ambiguity count (`*` marker in the indicator).
    session_key: SessionKey,
    /// Source agent's last-active epoch (Unix seconds) — feeds
    /// `RowFilter::max_age` evaluation when filtering mux rows.
    last_active_epoch: Option<i64>,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

/// Build the projection rows for `conn` and `options`. Returned vector
/// is `[header, body…]`, the same shape `render_rows` consumes.
pub fn build_mux_rows_from_conn(
    conn: &Connection,
    columns: &[&'static str],
    options: &RenderOptions,
) -> rusqlite::Result<Vec<Vec<String>>> {
    let muxes = fetch_mux_rows(conn)?;
    let attachments = fetch_attachment_lookup(conn)?;
    let ambiguity = fetch_per_agent_ambiguity(conn)?;

    let body_full_ids: Vec<String> = muxes
        .iter()
        .map(|m| node_short_id_from_display(&m.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(MUX_COLUMNS, key))
            .collect(),
    );

    let empty_attached: Vec<AttachedAgent> = Vec::new();
    for (row, full_short) in muxes.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let all_attached = attachments
            .get(&row.node_id_display)
            .unwrap_or(&empty_attached);
        let visible_attached: Vec<AttachedAgent> = all_attached
            .iter()
            .filter(|agent| agent_matches_filter(agent, &ambiguity, options, &options.filter))
            .cloned()
            .collect();
        if !mux_matches_filter(all_attached, &visible_attached, &options.filter) {
            continue;
        }
        let ctx = CellCtx {
            row,
            short_id,
            attached: if visible_attached.is_empty() {
                None
            } else {
                Some(visible_attached.as_slice())
            },
            ambiguity: &ambiguity,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    Ok(rows)
}

fn agent_matches_filter(
    agent: &AttachedAgent,
    ambiguity: &HashMap<SessionKey, usize>,
    options: &RenderOptions,
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = ambiguity.get(&agent.session_key).copied().unwrap_or(0);
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.session_key.0,
        now_epoch: options.now_epoch,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
}

fn mux_matches_filter(
    all_attached: &[AttachedAgent],
    visible_attached: &[AttachedAgent],
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    if !visible_attached.is_empty() {
        return true;
    }
    // No attached agents survive the filter. Mirror the mux row-tree
    // rule in `src/tui/rows/mux.rs:550`: keep the mux only when the
    // operator explicitly asked for `mux_state=unmuxed` (and no other
    // narrowing dimension is active) and the mux truly has no
    // attached agents to begin with.
    let RowFilter {
        harness,
        max_age,
        mux_state,
        float_muxed_sessions_top: _,
        float_attached_muxes_top: _,
    } = filter;
    harness.is_none()
        && max_age.is_none()
        && mux_state
            .as_ref()
            .is_some_and(|mux_state| mux_state.values().contains(&MuxStateKey::Unmuxed))
        && all_attached.is_empty()
}

struct CellCtx<'a> {
    row: &'a MuxRow,
    short_id: &'a str,
    /// `None` when no `linked_to_mux` candidate ties an agent to
    /// this mux; `Some(&[])` is not produced (the lookup skips the
    /// entry).
    attached: Option<&'a [AttachedAgent]>,
    ambiguity: &'a HashMap<SessionKey, usize>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match key {
        "id" => ctx.short_id.to_string(),
        "mux" => format!("{}:{}", ctx.row.backend, ctx.row.native_id),
        "cwd" => ctx.row.cwd.clone().unwrap_or_else(dash),
        "agents" => match ctx.attached {
            Some(entries) if !entries.is_empty() => entries
                .iter()
                .map(|a| {
                    let ambiguous = ctx.ambiguity.get(&a.session_key).copied().unwrap_or(0) > 1;
                    format!(
                        "{} [{}]",
                        a.label,
                        render::indicator_from_tags(&a.provenance, &a.confidence, ambiguous)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            _ => dash(),
        },
        "attached-count" => match ctx.attached {
            Some(entries) if !entries.is_empty() => entries.len().to_string(),
            _ => dash(),
        },
        "activity" => ctx
            .row
            .activity_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(dash),
        "created" => ctx
            .row
            .created_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(dash),
        "preview" => ctx
            .attached
            .and_then(|entries| entries.iter().find_map(|a| a.preview.clone()))
            .unwrap_or_else(dash),
        _ => dash(),
    }
}

/// Snapshot → in-memory-SQLite bridge for callers that still pass a
/// `GraphSnapshot` (CLI today; tests). P10-014 retires the bridge.
pub fn build_mux_rows_from_snapshot(
    snapshot: &crate::model::GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for mux projection");
    build_mux_rows_from_conn(&conn, columns, options)
        .expect("SQLite-backed mux projection should not fail against a freshly loaded snapshot")
}

// -----------------------------------------------------------------------------
// Queries
// -----------------------------------------------------------------------------

fn fetch_mux_rows(conn: &Connection) -> rusqlite::Result<Vec<MuxRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, backend, native_id, cwd, activity_epoch, created_epoch \
         FROM node_mux_sessions \
         ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(MuxRow {
            node_id_display: row.get(0)?,
            backend: row.get(1)?,
            native_id: row.get(2)?,
            cwd: row.get(3)?,
            activity_epoch: row.get(4)?,
            created_epoch: row.get(5)?,
        })
    })?;
    rows.collect()
}

/// Active `linked_to_mux` candidate links joined to their source
/// agent session, grouped by mux `node_id`. Mirrors
/// the former in-memory attachment selector: include only attachments whose
/// source agent session is present in `node_agent_sessions`, keep
/// the iteration order the BTreeMap-keyed in-memory walk produces
/// (sorted by source agent NodeId, i.e. by AgentSessionId tuple),
/// and remember per-entry data the renderer needs for the `agents`,
/// `attached-count`, and `preview` cells.
fn fetch_attachment_lookup(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, Vec<AttachedAgent>>> {
    let mut stmt = conn.prepare(
        "SELECT ('mux_session:' || json_extract(cl.target_node, '$.native_id')) AS mux_node_id, \
                a.harness_key, a.state_scope, a.session_key, \
                cl.provenance, cl.confidence, cl.link_id, \
                a.last_message_preview, a.last_active_epoch \
         FROM candidate_links cl \
         JOIN node_agent_sessions a \
           ON cl.source_kind = 'agent_session' \
           AND ('agent_session:' || \
                json_extract(cl.source, '$.harness_key') || ':' || \
                json_extract(cl.source, '$.state_scope') || ':' || \
                json_extract(cl.source, '$.session_key')) = a.node_id \
         WHERE cl.target_node_kind = 'mux_session' \
           AND cl.relation = 'linked_to_mux' \
           AND cl.state = 'active' \
         ORDER BY a.harness_key, a.state_scope, a.session_key, cl.link_id",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via pick_strongest's tiebreak accessor
    struct Raw {
        mux_node_id: String,
        harness_key: String,
        state_scope: String,
        session_key: String,
        provenance: String,
        confidence: String,
        link_id: String,
        preview: Option<String>,
        last_active_epoch: Option<i64>,
    }
    let rows = stmt.query_map([], |row| {
        Ok(Raw {
            mux_node_id: row.get(0)?,
            harness_key: row.get(1)?,
            state_scope: row.get(2)?,
            session_key: row.get(3)?,
            provenance: row.get(4)?,
            confidence: row.get(5)?,
            link_id: row.get(6)?,
            preview: row.get(7)?,
            last_active_epoch: row.get(8)?,
        })
    })?;

    // First pass: group all candidates per (mux_node_id, source
    // session) so we can pick_strongest per source — the in-memory
    // attached_to_mux stores the preferred candidate per source.
    type Key = (String, SessionKey);
    let mut per_source: HashMap<Key, Vec<Raw>> = HashMap::new();
    let mut order: Vec<Key> = Vec::new();
    for entry in rows {
        let raw = entry?;
        let key = (
            raw.mux_node_id.clone(),
            session_key(&raw.harness_key, &raw.state_scope, &raw.session_key),
        );
        if !per_source.contains_key(&key) {
            order.push(key.clone());
        }
        per_source.entry(key).or_default().push(raw);
    }

    // Second pass: emit one AttachedAgent per (mux, source) in
    // first-seen order (which matches the SQL ORDER BY harness/scope/
    // session order = BTreeMap-by-source-NodeId in the in-memory
    // walk).
    let mut out: HashMap<String, Vec<AttachedAgent>> = HashMap::new();
    for key in order {
        let candidates = per_source.remove(&key).expect("filled in first pass");
        let mux_node_id = key.0.clone();
        let session = key.1.clone();
        let Some(best) = pick_strongest(candidates, |r: &Raw| {
            (&r.provenance, &r.confidence, &r.link_id)
        }) else {
            continue;
        };
        let label = format!(
            "{}:{}",
            best.harness_key,
            agent_session_key_for_label(&best.session_key)
        );
        out.entry(mux_node_id).or_default().push(AttachedAgent {
            label,
            provenance: best.provenance,
            confidence: best.confidence,
            preview: best.preview,
            session_key: session,
            last_active_epoch: best.last_active_epoch,
        });
    }
    Ok(out)
}

/// Per-source counts of distinct active `linked_to_mux` mux targets.
/// Feeds the ambiguity `*` marker on the `agents` cell's
/// per-attachment indicator. Multiple evidence links to the same mux
/// are corroboration, not ambiguity.
fn fetch_per_agent_ambiguity(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, usize>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(source, '$.harness_key') AS h, \
                json_extract(source, '$.state_scope') AS s, \
                json_extract(source, '$.session_key') AS k, \
                COUNT(DISTINCT target_node) AS cnt \
         FROM candidate_links \
         WHERE source_kind = 'agent_session' \
           AND relation = 'linked_to_mux' \
           AND state = 'active' \
         GROUP BY source",
    )?;
    let mut out = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let h: String = row.get(0)?;
        let s: String = row.get(1)?;
        let k: String = row.get(2)?;
        let cnt: i64 = row.get(3)?;
        Ok((session_key(&h, &s, &k), cnt as usize))
    })?;
    for entry in rows {
        let (key, cnt) = entry?;
        out.insert(key, cnt);
    }
    Ok(out)
}
