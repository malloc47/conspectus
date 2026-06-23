//! SQLite-backed PRs projection renderer (P10-007 / ADR 0043).
//!
//! Replaces the in-memory `build_pr_rows` path for
//! [`Projection::Pr`]. Emits one row per `forge_pr` node.
//!
//! Query strategy:
//!
//! - Primary query against `node_forge_prs`, ordered by `node_id`.
//! - Three side-lookups feed the cross-row cells:
//!   1. preferred branch per PR — pick_strongest over
//!      `branch_has_forge_pr` candidates whose source is the PR,
//!      target is a branch. Source = PR matches what production
//!      discovery emits (see ADR 0044 §P10-004 notes).
//!   2. checkout roots per branch — every active
//!      `checked_out_branch` candidate's source-checkout root,
//!      grouped by the target branch's structural key.
//!   3. agent sessions with a cwd — driven by `node_agent_sessions`
//!      in iteration order.
//!
//! The `attached` cell composes (1) + (2) + (3): for each PR, find
//! its preferred branch, look up checkout roots that have that
//! branch checked out, and pick agent sessions whose cwd is at or
//! under any of those roots.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use super::render::{
    PRS_COLUMNS, RenderOptions, current_epoch, format_relative_age, header_label,
    node_short_id_from_display, pick_strongest, strip_branch_prefix, unique_prefix_len,
};
use super::table::agent_session_key_for_label;
use crate::filter::{MuxStateKey, SessionMatchInputs};
use crate::model::path_is_ancestor_of;

/// `(repo_common_dir, refname)` identifies a branch structurally.
type BranchKey = (String, String);

#[derive(Debug, Clone)]
struct PrRow {
    node_id_display: String,
    owner: String,
    repo: String,
    number: i64,
    state: Option<String>,
    updated_epoch: Option<i64>,
    is_draft: bool,
}

#[derive(Debug, Clone)]
struct AgentRow {
    /// `agent_session:<harness>:<state_scope>:<session_key>` —
    /// matches the join key used by `fetch_agent_mux_candidate_counts`.
    node_id: String,
    harness_key: String,
    session_key: String,
    cwd: String,
    last_active_epoch: Option<i64>,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_pr_rows_from_conn(
    conn: &Connection,
    columns: &[&'static str],
    options: &RenderOptions,
) -> rusqlite::Result<Vec<Vec<String>>> {
    let prs = fetch_pr_rows(conn)?;
    let preferred_branch = fetch_preferred_branch_per_pr(conn)?;
    let checkout_roots_per_branch = fetch_checkout_roots_per_branch(conn)?;
    let agents = fetch_agents_with_cwd(conn)?;
    let candidate_counts = fetch_agent_mux_candidate_counts(conn)?;

    let body_full_ids: Vec<String> = prs
        .iter()
        .map(|p| node_short_id_from_display(&p.node_id_display))
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(PRS_COLUMNS, key))
            .collect(),
    );

    let filter_active = options.filter.has_narrowing_predicates();
    for (row, full_short) in prs.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let branch = preferred_branch.get(&row.node_id_display);
        let attached = branch
            .and_then(|b| checkout_roots_per_branch.get(b))
            .map(|roots| attached_agents_for_roots(&agents, roots, &candidate_counts, options))
            .unwrap_or_default();
        if filter_active && attached.is_empty() {
            continue;
        }
        let ctx = CellCtx {
            row,
            short_id,
            branch,
            attached: &attached,
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    Ok(rows)
}

fn attached_agents_for_roots(
    agents: &[AgentRow],
    roots: &[String],
    candidate_counts: &HashMap<String, usize>,
    options: &RenderOptions,
) -> Vec<String> {
    let filter_active = options.filter.has_narrowing_predicates();
    let mut labels: Vec<String> = Vec::new();
    for agent in agents {
        let cwd_path = Path::new(&agent.cwd);
        let under_root = roots
            .iter()
            .any(|root| path_is_ancestor_of(Path::new(root), cwd_path));
        if !under_root {
            continue;
        }
        if filter_active {
            let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
            let matches = options.filter.matches_session(&SessionMatchInputs {
                harness_key: &agent.harness_key,
                now_epoch: options.now_epoch,
                last_active_epoch: agent.last_active_epoch,
                mux_state: MuxStateKey::from_candidate_count(candidate_count),
            });
            if !matches {
                continue;
            }
        }
        labels.push(format!(
            "{}:{}",
            agent.harness_key,
            agent_session_key_for_label(&agent.session_key)
        ));
    }
    labels
}

struct CellCtx<'a> {
    row: &'a PrRow,
    short_id: &'a str,
    branch: Option<&'a BranchKey>,
    attached: &'a [String],
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match key {
        "id" => ctx.short_id.to_string(),
        "pr" => {
            let state = ctx.row.state.as_deref().unwrap_or("?");
            let draft = if ctx.row.is_draft { " draft" } else { "" };
            format!(
                "{}/{}#{} ({state}{draft})",
                ctx.row.owner, ctx.row.repo, ctx.row.number
            )
        }
        "state" => ctx.row.state.clone().unwrap_or_else(dash),
        "draft" => {
            if ctx.row.is_draft {
                "draft".to_string()
            } else {
                dash()
            }
        }
        "branch" => ctx
            .branch
            .map(|(_repo, refname)| strip_branch_prefix(refname).to_string())
            .unwrap_or_else(dash),
        "repo" => format!("{}/{}", ctx.row.owner, ctx.row.repo),
        "updated" => ctx
            .row
            .updated_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(dash),
        "attached" => {
            if ctx.attached.is_empty() {
                dash()
            } else {
                ctx.attached.join(", ")
            }
        }
        _ => dash(),
    }
}

pub fn build_pr_rows_from_snapshot(
    snapshot: &crate::model::GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for prs projection");
    build_pr_rows_from_conn(&conn, columns, options)
        .expect("SQLite-backed prs projection should not fail against a freshly loaded snapshot")
}

// -----------------------------------------------------------------------------
// Queries
// -----------------------------------------------------------------------------

fn fetch_pr_rows(conn: &Connection) -> rusqlite::Result<Vec<PrRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, owner, repo, number, state, updated_epoch, is_draft \
         FROM node_forge_prs \
         ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let is_draft: i64 = row.get(6)?;
        Ok(PrRow {
            node_id_display: row.get(0)?,
            owner: row.get(1)?,
            repo: row.get(2)?,
            number: row.get(3)?,
            state: row.get(4)?,
            updated_epoch: row.get(5)?,
            is_draft: is_draft != 0,
        })
    })?;
    rows.collect()
}

/// Per-PR preferred `branch_has_forge_pr` candidate's target branch
/// (structural key). Mirrors `pr_preferred_branch_id` in the
/// in-memory renderer. Pick is by provenance/confidence/link_id
/// ordering, matching `pick_preferred`.
fn fetch_preferred_branch_per_pr(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, BranchKey>> {
    let mut stmt = conn.prepare(
        "SELECT ('forge_pr:' || \
                 json_extract(cl.source, '$.provider') || ':' || \
                 json_extract(cl.source, '$.host') || '/' || \
                 json_extract(cl.source, '$.owner') || '/' || \
                 json_extract(cl.source, '$.repo') || '#' || \
                 json_extract(cl.source, '$.number')) AS pr_node_id, \
                json_extract(cl.target_node, '$.repo.common_dir') AS branch_repo, \
                json_extract(cl.target_node, '$.refname') AS branch_refname, \
                cl.link_id, cl.provenance, cl.confidence \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'forge_pr' \
           AND cl.target_node_kind = 'branch' \
           AND cl.relation = 'branch_has_forge_pr' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via pick_strongest's tiebreak accessor
    struct Raw {
        pr_node_id: String,
        branch_repo: String,
        branch_refname: String,
        link_id: String,
        provenance: String,
        confidence: String,
    }
    let mut per_pr: HashMap<String, Vec<Raw>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok(Raw {
            pr_node_id: row.get(0)?,
            branch_repo: row.get(1)?,
            branch_refname: row.get(2)?,
            link_id: row.get(3)?,
            provenance: row.get(4)?,
            confidence: row.get(5)?,
        })
    })?;
    for entry in rows {
        let raw = entry?;
        per_pr.entry(raw.pr_node_id.clone()).or_default().push(raw);
    }
    let mut out = HashMap::new();
    for (pr_node_id, candidates) in per_pr {
        let Some(best) = pick_strongest(candidates, |r: &Raw| {
            (&r.provenance, &r.confidence, &r.link_id)
        }) else {
            continue;
        };
        out.insert(pr_node_id, (best.branch_repo, best.branch_refname));
    }
    Ok(out)
}

/// All active `checked_out_branch` candidates joined to their source
/// checkout's `root`, grouped by the target branch's structural key.
/// The in-memory walk takes every matching candidate (not just the
/// preferred one per checkout) — same here.
fn fetch_checkout_roots_per_branch(
    conn: &Connection,
) -> rusqlite::Result<HashMap<BranchKey, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(cl.target_node, '$.repo.common_dir') AS branch_repo, \
                json_extract(cl.target_node, '$.refname') AS branch_refname, \
                c.root AS checkout_root \
         FROM candidate_links cl \
         JOIN node_checkouts c \
           ON cl.source_kind = 'checkout' \
           AND ('checkout:repo:' || \
                json_extract(cl.source, '$.repo.common_dir') || '@' || \
                json_extract(cl.source, '$.root')) = c.node_id \
         WHERE cl.relation = 'checked_out_branch' \
           AND cl.state = 'active' \
           AND cl.target_node_kind = 'branch'",
    )?;
    let mut out: HashMap<BranchKey, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let branch_repo: String = row.get(0)?;
        let branch_refname: String = row.get(1)?;
        let checkout_root: String = row.get(2)?;
        Ok(((branch_repo, branch_refname), checkout_root))
    })?;
    for entry in rows {
        let (key, root) = entry?;
        out.entry(key).or_default().push(root);
    }
    Ok(out)
}

/// Agent sessions with a non-NULL cwd, in `BTreeMap<NodeId>`-style
/// order (matches the in-memory `view.agent_sessions.values()`
/// iteration that drives label collection).
fn fetch_agents_with_cwd(conn: &Connection) -> rusqlite::Result<Vec<AgentRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, harness_key, session_key, cwd, last_active_epoch \
         FROM node_agent_sessions \
         WHERE cwd IS NOT NULL \
         ORDER BY harness_key, state_scope, session_key",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AgentRow {
            node_id: row.get(0)?,
            harness_key: row.get(1)?,
            session_key: row.get(2)?,
            cwd: row.get(3)?,
            last_active_epoch: row.get(4)?,
        })
    })?;
    rows.collect()
}

/// Per-agent count of distinct active `linked_to_mux` mux targets —
/// feeds the derived `mux_state` dimension when evaluating
/// `RowFilter` against PR-attached agent sessions. Key form matches
/// `node_agent_sessions.node_id`.
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
