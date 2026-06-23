//! SQLite-backed forks projection renderer (P10-008 / ADR 0043).
//!
//! Replaces the in-memory `build_fork_rows` path for
//! [`Projection::Fork`]. Emits one row per `fork` node.
//!
//! Query strategy:
//!
//! - Primary query against `node_forks`, ordered by `node_id`.
//! - Side-lookup for the `parent` cell: pick_strongest over
//!   `parent_session` candidates whose source is the fork, then
//!   render either the parent agent session's session_key (short
//!   form) or `?<native_id>` when the candidate target is
//!   unresolved.
//! - Side-lookup for the `children` cell: count active
//!   `child_session` candidates whose source is the fork and whose
//!   target is either an `agent_session` node or an unresolved
//!   endpoint (matching the in-memory `fork_child_session_count`).

use std::collections::HashMap;

use rusqlite::Connection;

use super::render::{
    FORKS_COLUMNS, RenderOptions, header_label, node_short_id_from_display, pick_strongest,
    unique_prefix_len,
};
use super::table::short_session_id;
use crate::filter::{MuxStateKey, SessionMatchInputs};

#[derive(Debug, Clone)]
struct ForkRow {
    /// Display-form `NodeId` of the fork — keys the parent and
    /// children lookups and feeds the short id hash.
    node_id_display: String,
    /// Structural `ForkNode.provider_source_key` (NOT the ID's
    /// psk; the two can diverge). Used by `fork_label` as the
    /// fallback when `name` is `None`, mirroring the in-memory
    /// renderer which reads `fork.provider_source_key`.
    structural_provider_source_key: String,
    provider: String,
    name: Option<String>,
    scope: Option<String>,
    capabilities_json: String,
}

#[derive(Debug, Clone)]
struct ParentInfo {
    /// Pre-formatted label as it should appear in the `parent`
    /// cell (e.g. `abc123` or `?native-id-short`).
    label: String,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_fork_rows_from_conn(
    conn: &Connection,
    columns: &[&'static str],
    options: &RenderOptions,
) -> rusqlite::Result<Vec<Vec<String>>> {
    let forks = fetch_fork_rows(conn)?;
    let parents = fetch_parent_session_per_fork(conn)?;
    let child_counts = fetch_child_session_counts_per_fork(conn)?;

    let body_full_ids: Vec<String> = forks
        .iter()
        .map(|f| node_short_id_from_display(&f.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(FORKS_COLUMNS, key))
            .collect(),
    );

    let filter_active = options.filter.has_narrowing_predicates();
    // Resolved child-agent metadata is only needed when filter is
    // active — without a narrowing predicate the fork row keeps the
    // total `children` count (which includes unresolved-target
    // candidates) and never drops.
    let resolved_children = if filter_active {
        Some(fetch_resolved_child_agents_per_fork(conn)?)
    } else {
        None
    };
    let candidate_counts = if filter_active {
        Some(fetch_agent_mux_candidate_counts(conn)?)
    } else {
        None
    };

    for (row, full_short) in forks.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let child_count = if filter_active {
            let visible = visible_child_count(
                row,
                resolved_children
                    .as_ref()
                    .expect("populated when filter_active"),
                candidate_counts
                    .as_ref()
                    .expect("populated when filter_active"),
                options,
            );
            if visible == 0 {
                continue;
            }
            visible
        } else {
            child_counts.get(&row.node_id_display).copied().unwrap_or(0)
        };
        let ctx = CellCtx {
            row,
            short_id,
            parent: parents.get(&row.node_id_display),
            child_count,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    Ok(rows)
}

fn visible_child_count(
    fork: &ForkRow,
    resolved: &HashMap<String, Vec<ChildAgent>>,
    candidate_counts: &HashMap<String, usize>,
    options: &RenderOptions,
) -> usize {
    let Some(children) = resolved.get(&fork.node_id_display) else {
        return 0;
    };
    children
        .iter()
        .filter(|child| {
            let candidate_count = candidate_counts.get(&child.node_id).copied().unwrap_or(0);
            options.filter.matches_session(&SessionMatchInputs {
                harness_key: &child.harness_key,
                now_epoch: options.now_epoch,
                last_active_epoch: child.last_active_epoch,
                mux_state: MuxStateKey::from_candidate_count(candidate_count),
            })
        })
        .count()
}

#[derive(Debug, Clone)]
struct ChildAgent {
    node_id: String,
    harness_key: String,
    last_active_epoch: Option<i64>,
}

struct CellCtx<'a> {
    row: &'a ForkRow,
    short_id: &'a str,
    parent: Option<&'a ParentInfo>,
    child_count: usize,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match key {
        "id" => ctx.short_id.to_string(),
        "fork" => match &ctx.row.name {
            Some(name) => format!("{}:{}", ctx.row.provider, name),
            None => ctx.row.structural_provider_source_key.clone(),
        },
        "provider" => ctx.row.provider.clone(),
        "scope" => ctx.row.scope.clone().unwrap_or_else(dash),
        "parent" => ctx.parent.map(|p| p.label.clone()).unwrap_or_else(dash),
        "children" => {
            if ctx.child_count == 0 {
                dash()
            } else {
                ctx.child_count.to_string()
            }
        }
        "capabilities" => {
            let caps: Vec<String> =
                serde_json::from_str(&ctx.row.capabilities_json).unwrap_or_default();
            if caps.is_empty() {
                dash()
            } else {
                caps.join(", ")
            }
        }
        _ => dash(),
    }
}

pub fn build_fork_rows_from_snapshot(
    snapshot: &crate::model::GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for forks projection");
    build_fork_rows_from_conn(&conn, columns, options)
        .expect("SQLite-backed forks projection should not fail against a freshly loaded snapshot")
}

// -----------------------------------------------------------------------------
// Queries
// -----------------------------------------------------------------------------

fn fetch_fork_rows(conn: &Connection) -> rusqlite::Result<Vec<ForkRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, provider_source_key, provider_name, name, scope, capabilities \
         FROM node_forks \
         ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ForkRow {
            node_id_display: row.get(0)?,
            structural_provider_source_key: row.get(1)?,
            provider: row.get(2)?,
            name: row.get(3)?,
            scope: row.get(4)?,
            capabilities_json: row.get(5)?,
        })
    })?;
    rows.collect()
}

/// Per-fork preferred `parent_session` candidate's rendered label.
/// Mirrors `fork_parent_session_label` — resolved agent_session
/// targets render as the short session_key; unresolved targets
/// render as `?<short native_id>`; anything else yields no entry.
fn fetch_parent_session_per_fork(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, ParentInfo>> {
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(cl.source, '$.provider_source_key')) AS fork_node_id, \
                cl.link_id, cl.provenance, cl.confidence, \
                cl.target_kind, cl.target_node_kind, \
                cl.target_node, cl.target_native_id \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'fork' \
           AND cl.relation = 'parent_session' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via pick_strongest's tiebreak accessor
    struct Raw {
        fork_node_id: String,
        link_id: String,
        provenance: String,
        confidence: String,
        target_kind: String,
        target_node_kind: Option<String>,
        target_node: Option<String>,
        target_native_id: Option<String>,
    }
    let mut per_fork: HashMap<String, Vec<Raw>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok(Raw {
            fork_node_id: row.get(0)?,
            link_id: row.get(1)?,
            provenance: row.get(2)?,
            confidence: row.get(3)?,
            target_kind: row.get(4)?,
            target_node_kind: row.get(5)?,
            target_node: row.get(6)?,
            target_native_id: row.get(7)?,
        })
    })?;
    for entry in rows {
        let raw = entry?;
        per_fork
            .entry(raw.fork_node_id.clone())
            .or_default()
            .push(raw);
    }
    let mut out = HashMap::new();
    for (fork_node_id, candidates) in per_fork {
        let Some(best) = pick_strongest(candidates, |r: &Raw| {
            (&r.provenance, &r.confidence, &r.link_id)
        }) else {
            continue;
        };
        let agent_label = || -> Option<String> {
            let target_json = best.target_node.as_deref()?;
            let v: serde_json::Value = serde_json::from_str(target_json).ok()?;
            let sk = v.get("session_key").and_then(|x| x.as_str())?;
            Some(short_session_id(sk))
        };
        let label = match best.target_kind.as_str() {
            "node" if best.target_node_kind.as_deref() == Some("agent_session") => agent_label(),
            "unresolved" => Some(
                best.target_native_id
                    .as_deref()
                    .map(|native| format!("?{}", short_session_id(native)))
                    .unwrap_or_else(|| "?".to_string()),
            ),
            _ => None,
        };
        if let Some(label) = label {
            out.insert(fork_node_id, ParentInfo { label });
        }
    }
    Ok(out)
}

/// Per-fork list of resolved child agent sessions — the subset of
/// `child_session` candidates whose target is a known
/// `agent_session` node. Used only when a `RowFilter` is active so
/// the filter has a per-session row to evaluate; unresolved-target
/// children are intentionally excluded (a `RowFilter`'s predicates
/// all need session-level metadata that isn't available for those).
fn fetch_resolved_child_agents_per_fork(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, Vec<ChildAgent>>> {
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(cl.source, '$.provider_source_key')) AS fork_node_id, \
                a.node_id, a.harness_key, a.last_active_epoch \
         FROM candidate_links cl \
         JOIN node_agent_sessions a \
           ON cl.target_node_kind = 'agent_session' \
           AND ('agent_session:' || \
                json_extract(cl.target_node, '$.harness_key') || ':' || \
                json_extract(cl.target_node, '$.state_scope') || ':' || \
                json_extract(cl.target_node, '$.session_key')) = a.node_id \
         WHERE cl.source_kind = 'fork' \
           AND cl.relation = 'child_session' \
           AND cl.state = 'active' \
         ORDER BY fork_node_id, a.node_id",
    )?;
    let mut out: HashMap<String, Vec<ChildAgent>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let fork_node_id: String = row.get(0)?;
        Ok((
            fork_node_id,
            ChildAgent {
                node_id: row.get(1)?,
                harness_key: row.get(2)?,
                last_active_epoch: row.get(3)?,
            },
        ))
    })?;
    for entry in rows {
        let (fork_node_id, child) = entry?;
        out.entry(fork_node_id).or_default().push(child);
    }
    Ok(out)
}

/// Per-agent count of distinct active `linked_to_mux` mux targets.
/// Same shape as `output::prs::fetch_agent_mux_candidate_counts` —
/// the two callers will collapse into a shared helper when a third
/// surface arrives.
fn fetch_agent_mux_candidate_counts(conn: &Connection) -> rusqlite::Result<HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT ('agent_session:' || json_extract(source, '$.harness_key') || ':' || \
                 json_extract(source, '$.state_scope') || ':' || \
                 json_extract(source, '$.session_key')) AS agent_node_id, \
                COUNT(DISTINCT target_node) \
         FROM candidate_links \
         WHERE source_kind = 'agent_session' \
           AND relation = 'linked_to_mux' \
           AND state = 'active' \
         GROUP BY source",
    )?;
    let rows = stmt.query_map([], |row| {
        let count: i64 = row.get(1)?;
        Ok((row.get::<_, String>(0)?, count as usize))
    })?;
    let mut out = HashMap::new();
    for entry in rows {
        let (node_id, count) = entry?;
        out.insert(node_id, count);
    }
    Ok(out)
}

/// Per-fork count of active `child_session` candidates targeting
/// an `agent_session` node OR an unresolved endpoint. Mirrors
/// `fork_child_session_count`'s filter.
fn fetch_child_session_counts_per_fork(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(cl.source, '$.provider_source_key')) AS fork_node_id, \
                COUNT(*) AS cnt \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'fork' \
           AND cl.relation = 'child_session' \
           AND cl.state = 'active' \
           AND (cl.target_node_kind = 'agent_session' OR cl.target_kind = 'unresolved') \
         GROUP BY fork_node_id",
    )?;
    let mut out = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let fork_node_id: String = row.get(0)?;
        let cnt: i64 = row.get(1)?;
        Ok((fork_node_id, cnt as usize))
    })?;
    for entry in rows {
        let (k, v) = entry?;
        out.insert(k, v);
    }
    Ok(out)
}
