//! SQLite-backed agent projection renderer (P10-004 / ADR 0043).
//!
//! Replaces the in-memory `build_agent_rows` path for
//! [`Projection::Agent`]. Reads everything from a
//! [`rusqlite::Connection`] populated by the loader; consumers
//! never see a `GraphSnapshot` here. Parity with the in-memory
//! renderer is enforced by the existing `output::table` snapshot
//! test corpus.
//!
//! Query strategy:
//!
//! - One **primary** query joins agent sessions to
//!   `v_sessions_with_repo` (for checkout/repo) and to `aliases` for
//!   the title overlay.
//! - **Side-lookups** per cell category fetch the structured candidate-
//!   link data once and index it by session key (or checkout id) in
//!   Rust. Each lookup is small and bounded; rendering walks the
//!   indexed maps in constant time per session. Cells covered:
//!   `branch` (via `fetch_branch_lookup`), `mux` / `mux-conf`,
//!   `lineage`, `workspace`, `fork`, `declared`, and the global
//!   preferred `pr` / `pr-conf`.
//!
//! All JOINs to `node_<kind>` tables use Display-form reconstruction
//! from the endpoint JSON (`'mux_session:' || json_extract(…)`) and
//! compare against `node_<kind>.node_id`, not against the structural
//! columns. The reason: production discovery (e.g. `tmux` adapter)
//! routinely sets `MuxSessionNode.native_id = "<name>"` while the
//! corresponding `MuxSessionId.native_id = "tmux:<name>"`; the
//! structural columns can differ from the typed-ID fields embedded
//! in the JSON. The Display-form match goes through the canonical
//! `node_id` PK that both sides agree on.
//!
//! Cells from ADR 0006:
//! `id` `agent` `cwd` `mux` `mux-conf` `pr` `pr-conf` `lineage`
//! `workspace` `checkout` `branch` `repo` `fork` `declared`
//! `preview` `title` `activity`.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;

use super::render::{
    self, RenderOptions, SESSIONS_COLUMNS, current_epoch, format_relative_age, header_label,
    pick_strongest, strip_branch_prefix,
};
use super::table::{agent_session_key_for_label, short_session_id};
use crate::filter::{MuxStateKey, SessionMatchInputs};

/// Triple identifying an agent session structurally. Matches the
/// `(harness_key, state_scope, session_key)` tuple in
/// `AgentSessionId`. Used as the HashMap key for per-session
/// lookups so cell rendering doesn't have to reconstruct typed
/// `NodeId` values.
type SessionKey = (String, String, String);

fn session_key(harness_key: &str, state_scope: &str, session_key: &str) -> SessionKey {
    (
        harness_key.to_string(),
        state_scope.to_string(),
        session_key.to_string(),
    )
}

/// One row's worth of base session data. Filled in by
/// [`fetch_session_rows`] and consumed by the cell extractors.
#[derive(Debug, Clone)]
struct SessionRow {
    harness_key: String,
    state_scope: String,
    session_key: String,
    cwd: Option<String>,
    title: Option<String>,
    alias_display_name: Option<String>,
    preview: Option<String>,
    last_active_epoch: Option<i64>,
    /// Display-form `NodeId` of the deepest checkout whose root
    /// contains the session's cwd (from `v_sessions_with_repo`).
    /// Used to key the [`branch`] lookup below.
    checkout_node_id: Option<String>,
    checkout_root: Option<String>,
    repo_common_dir: Option<String>,
}

impl SessionRow {
    fn key(&self) -> SessionKey {
        session_key(&self.harness_key, &self.state_scope, &self.session_key)
    }
}

#[derive(Debug, Clone)]
struct MuxInfo {
    backend: String,
    native_id: String,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

#[derive(Debug, Clone)]
struct PrInfo {
    owner: String,
    repo: String,
    number: i64,
    state: Option<String>,
    is_draft: bool,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

#[derive(Debug, Clone)]
struct LineageInfo {
    /// Label to render: either the parent agent_session's session_key
    /// (typically truncated via [`short_session_id`]), or `?<native_id>`
    /// when the preferred parent_session candidate is unresolved.
    label: String,
    /// Set when the parent itself has a preferred `parent_session`
    /// link — triggers the trailing `←` to flag a chain depth > 1.
    has_grandparent: bool,
}

#[derive(Debug, Clone)]
struct DeclaredInfo {
    /// Rendered state label: `"declared"`, `"ignored"`, or
    /// `"overridden"` per `LinkState`.
    state_label: String,
}

#[derive(Debug, Clone)]
struct ForkInfo {
    /// Pre-formatted fork label as `<provider>:<name_or_psk>`.
    label: String,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

/// Build the projection rows for `conn` and `options`. Returned vector
/// is `[header, body…]`, the same shape `render_rows` consumes.
pub fn build_agent_rows_from_conn(
    conn: &Connection,
    columns: &[&'static str],
    options: &RenderOptions,
) -> rusqlite::Result<Vec<Vec<String>>> {
    let sessions = fetch_session_rows(conn)?;
    let mux_lookup = fetch_mux_lookup(conn)?;
    let branch_lookup = fetch_branch_lookup(conn)?;
    let lineage_lookup = fetch_lineage_lookup(conn)?;
    let workspace_lookup = fetch_workspace_lookup(conn)?;
    let fork_lookup = fetch_fork_lookup(conn)?;
    let declared_lookup = fetch_declared_lookup(conn)?;
    let pr_global = fetch_global_pr(conn)?;

    let filtered: Vec<&SessionRow> = sessions
        .iter()
        .filter(|row| row_matches(options, row, &mux_lookup))
        .collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(SESSIONS_COLUMNS, key))
            .collect(),
    );

    for row in filtered {
        let branch_refname = row
            .checkout_node_id
            .as_deref()
            .and_then(|id| branch_lookup.get(id));
        let ctx = CellCtx {
            row,
            mux: mux_lookup.get(&row.key()),
            branch_refname,
            lineage: lineage_lookup.get(&row.key()),
            workspace: workspace_lookup.get(&row.key()),
            fork: fork_lookup.get(&row.key()),
            declared: declared_lookup.get(&row.key()),
            pr: pr_global.as_ref().filter(|_| row.cwd.is_some()),
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    Ok(rows)
}

struct CellCtx<'a> {
    row: &'a SessionRow,
    mux: Option<&'a MuxInfo>,
    /// Preferred `checked_out_branch` candidate's refname for the
    /// session's checkout (already deduped against the session row's
    /// `checkout_node_id`). `None` when the session has no checkout
    /// or its checkout has no active branch link.
    branch_refname: Option<&'a String>,
    lineage: Option<&'a LineageInfo>,
    workspace: Option<&'a String>,
    fork: Option<&'a ForkInfo>,
    declared: Option<&'a DeclaredInfo>,
    /// Global preferred PR (the in-memory renderer is intentionally
    /// loose here per `preferred_pr_for_session`'s in-code TODO; it
    /// applies the first BranchHasForgePr regardless of which
    /// session's checkout the branch belongs to). Set to `None` when
    /// the session has no cwd, matching the existing early-return.
    pr: Option<&'a PrInfo>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    match key {
        "id" => agent_session_key_for_label(&ctx.row.session_key),
        "agent" => format!(
            "{}:{}",
            ctx.row.harness_key,
            agent_session_key_for_label(&ctx.row.session_key)
        ),
        "cwd" => ctx.row.cwd.clone().unwrap_or_else(dash),
        "mux" => match ctx.mux {
            Some(m) => format!("{}:{}", m.backend, m.native_id),
            None => dash(),
        },
        "mux-conf" => match ctx.mux {
            Some(m) => {
                render::indicator_from_tags(&m.provenance, &m.confidence, m.candidate_count > 1)
            }
            None => dash(),
        },
        "pr" => match ctx.pr {
            Some(p) => forge_pr_label(p),
            None => dash(),
        },
        "pr-conf" => match ctx.pr {
            Some(p) => {
                render::indicator_from_tags(&p.provenance, &p.confidence, p.candidate_count > 1)
            }
            None => dash(),
        },
        "lineage" => match ctx.lineage {
            Some(l) if l.has_grandparent => format!("{}←", l.label),
            Some(l) => l.label.clone(),
            None => dash(),
        },
        "workspace" => ctx.workspace.cloned().unwrap_or_else(dash),
        "checkout" => ctx.row.checkout_root.clone().unwrap_or_else(dash),
        "branch" => ctx
            .branch_refname
            .map(|r| strip_branch_prefix(r).to_string())
            .unwrap_or_else(dash),
        "repo" => ctx.row.repo_common_dir.clone().unwrap_or_else(dash),
        "fork" => ctx.fork.map(|f| f.label.clone()).unwrap_or_else(dash),
        "declared" => ctx
            .declared
            .map(|d| d.state_label.clone())
            .unwrap_or_else(dash),
        "preview" => ctx.row.preview.clone().unwrap_or_else(dash),
        "title" => ctx
            .row
            .alias_display_name
            .clone()
            .or_else(|| ctx.row.title.clone())
            .unwrap_or_else(dash),
        "activity" => ctx
            .row
            .last_active_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(dash),
        _ => dash(),
    }
}

fn forge_pr_label(pr: &PrInfo) -> String {
    let state = pr.state.as_deref().unwrap_or("?");
    let draft = if pr.is_draft { " draft" } else { "" };
    format!("{}/{}#{} ({state}{draft})", pr.owner, pr.repo, pr.number)
}

fn row_matches(
    options: &RenderOptions,
    row: &SessionRow,
    mux_lookup: &HashMap<SessionKey, MuxInfo>,
) -> bool {
    if !options.filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = mux_lookup
        .get(&row.key())
        .map(|m| m.candidate_count)
        .unwrap_or(0);
    let inputs = SessionMatchInputs {
        harness_key: &row.harness_key,
        now_epoch: options.now_epoch,
        last_active_epoch: row.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    };
    options.filter.matches_session(&inputs)
}

// -----------------------------------------------------------------------------
// Queries
// -----------------------------------------------------------------------------

fn fetch_session_rows(conn: &Connection) -> rusqlite::Result<Vec<SessionRow>> {
    // Primary query. The aliases JOIN uses the JSON-encoded `node`
    // column (ADR 0044) — we reconstruct a synthetic JSON shape from
    // the structural session columns and match it against the alias
    // table. The shape mirrors `NodeId::AgentSession`'s serde derive
    // exactly: `{"type":"agent_session","harness_key":...,
    // "state_scope":...,"session_key":...}`.
    let mut stmt = conn.prepare(
        "SELECT a.harness_key, a.state_scope, a.session_key, \
                a.cwd, a.title, a.last_message_preview, a.last_active_epoch, \
                s.checkout_node_id, s.checkout_root, s.repo_common_dir, \
                al.display_name AS alias_display_name \
         FROM node_agent_sessions a \
         LEFT JOIN v_sessions_with_repo s ON s.session_node_id = a.node_id \
         LEFT JOIN aliases al \
           ON al.node_kind = 'agent_session' \
           AND json_extract(al.node, '$.harness_key') = a.harness_key \
           AND json_extract(al.node, '$.state_scope') = a.state_scope \
           AND json_extract(al.node, '$.session_key') = a.session_key \
         ORDER BY a.harness_key, a.state_scope, a.session_key",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SessionRow {
            harness_key: row.get(0)?,
            state_scope: row.get(1)?,
            session_key: row.get(2)?,
            cwd: row.get(3)?,
            title: row.get(4)?,
            preview: row.get(5)?,
            last_active_epoch: row.get(6)?,
            checkout_node_id: row.get(7)?,
            checkout_root: row.get(8)?,
            repo_common_dir: row.get(9)?,
            alias_display_name: row.get(10)?,
        })
    })?;
    rows.collect()
}

/// Active `checked_out_branch` candidate links keyed by the source
/// checkout's Display-form `node_id`. Mirrors
/// `session_branch_label`'s "walk by_source_relation for the
/// session's checkout" lookup. The session's checkout id is captured
/// in [`SessionRow::checkout_node_id`].
fn fetch_branch_lookup(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let mut stmt = conn.prepare(
        "SELECT ('checkout:repo:' || \
                 json_extract(cl.source, '$.repo.common_dir') || '@' || \
                 json_extract(cl.source, '$.root')) AS checkout_node_id, \
                json_extract(cl.target_node, '$.refname') AS refname, \
                cl.link_id, cl.provenance, cl.confidence \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'checkout' \
           AND cl.target_node_kind = 'branch' \
           AND cl.relation = 'checked_out_branch' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)]
    struct Raw {
        refname: String,
        link_id: String,
        provenance: String,
        confidence: String,
    }
    let mut per_checkout: HashMap<String, Vec<Raw>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Raw {
                refname: row.get(1)?,
                link_id: row.get(2)?,
                provenance: row.get(3)?,
                confidence: row.get(4)?,
            },
        ))
    })?;
    for entry in rows {
        let (key, raw) = entry?;
        per_checkout.entry(key).or_default().push(raw);
    }
    let mut out = HashMap::new();
    for (key, candidates) in per_checkout {
        let Some(best) = pick_strongest(candidates, |r: &Raw| {
            (&r.provenance, &r.confidence, &r.link_id)
        }) else {
            continue;
        };
        out.insert(key, best.refname);
    }
    Ok(out)
}

/// Active `linked_to_mux` candidate links keyed by source session.
/// For each session we keep the preferred candidate (provenance →
/// confidence → link_id ordering) plus the count of active
/// candidates (for the ambiguity marker in `mux-conf`).
fn fetch_mux_lookup(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, MuxInfo>> {
    // Pull all active linked_to_mux candidates whose source is an
    // agent_session and whose target is a known mux node. Group by
    // session in Rust, run pick_preferred there.
    // The JOIN reconstructs the mux's NodeId Display form
    // (`mux_session:<MuxSessionId.native_id>`) from the link's
    // target JSON. Joining on the structural `m.native_id` column
    // would be wrong: discovery (and test fixtures) routinely sets
    // `MuxSessionNode.native_id` to a value distinct from
    // `MuxSessionId.native_id` (e.g. id is `tmux:editor` while the
    // structural field is just `editor`). Same pattern for the fork
    // and PR joins below.
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
    #[allow(dead_code)] // link_id is read via the pick_strongest tiebreak accessor
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
        let Some(best) = pick_strongest(candidates, |r| (&r.provenance, &r.confidence, &r.link_id))
        else {
            continue;
        };
        out.insert(
            key,
            MuxInfo {
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

/// Mirrors `preferred_pr_for_session`'s intentionally loose behavior:
/// find the first preferred `branch_has_forge_pr` candidate across
/// the snapshot and use its target PR. The in-memory implementation
/// has a TODO calling this out as conservative; we match it for
/// parity.
fn fetch_global_pr(conn: &Connection) -> rusqlite::Result<Option<PrInfo>> {
    // `branch_has_forge_pr` is stored with source=ForgePr,
    // target=Branch in production (see `discovery::forge::github`)
    // and in the in-memory `preferred_pr_for_session`. Despite the
    // relation name reading "branch has forge pr", the link points
    // PR → branch, so we JOIN to `node_forge_prs` via `cl.source`
    // and pull branch keys from `cl.target_node`.
    let mut stmt = conn.prepare(
        "SELECT cl.link_id, cl.provenance, cl.confidence, \
                pr.owner, pr.repo, pr.number, pr.state, pr.is_draft, \
                json_extract(cl.target_node, '$.repo.common_dir') AS branch_repo, \
                json_extract(cl.target_node, '$.refname') AS branch_refname \
         FROM candidate_links cl \
         JOIN node_forge_prs pr \
           ON cl.source_kind = 'forge_pr' \
           AND ('forge_pr:' || \
                json_extract(cl.source, '$.provider') || ':' || \
                json_extract(cl.source, '$.host') || '/' || \
                json_extract(cl.source, '$.owner') || '/' || \
                json_extract(cl.source, '$.repo') || '#' || \
                json_extract(cl.source, '$.number')) = pr.node_id \
         WHERE cl.relation = 'branch_has_forge_pr' \
           AND cl.state = 'active' \
         ORDER BY branch_repo, branch_refname",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via the pick_strongest tiebreak accessor
    struct Raw {
        link_id: String,
        provenance: String,
        confidence: String,
        owner: String,
        repo: String,
        number: i64,
        state: Option<String>,
        is_draft: i64,
        branch_repo: String,
        branch_refname: String,
    }
    let rows = stmt.query_map([], |row| {
        Ok(Raw {
            link_id: row.get(0)?,
            provenance: row.get(1)?,
            confidence: row.get(2)?,
            owner: row.get(3)?,
            repo: row.get(4)?,
            number: row.get(5)?,
            state: row.get(6)?,
            is_draft: row.get(7)?,
            branch_repo: row.get(8)?,
            branch_refname: row.get(9)?,
        })
    })?;
    let all: Vec<Raw> = rows.collect::<rusqlite::Result<_>>()?;
    if all.is_empty() {
        return Ok(None);
    }
    // Group by branch (matches the in-memory walk's per-source
    // grouping in `by_source_relation`); the FIRST branch's preferred
    // candidate wins (BTreeMap iteration order in the in-memory
    // version, source-column lex order here).
    let mut by_branch: BTreeMap<(String, String), Vec<Raw>> = BTreeMap::new();
    for raw in all {
        by_branch
            .entry((raw.branch_repo.clone(), raw.branch_refname.clone()))
            .or_default()
            .push(raw);
    }
    let (_, first_branch_candidates) = by_branch.into_iter().next().expect("non-empty");
    let candidate_count = first_branch_candidates.len();
    let best = pick_strongest(first_branch_candidates, |r: &Raw| {
        (&r.provenance, &r.confidence, &r.link_id)
    })
    .expect("group non-empty");
    Ok(Some(PrInfo {
        owner: best.owner,
        repo: best.repo,
        number: best.number,
        state: best.state,
        is_draft: best.is_draft != 0,
        provenance: best.provenance,
        confidence: best.confidence,
        candidate_count,
    }))
}

/// Active `parent_session` candidate links from each agent session.
/// Builds the `label` + `has_grandparent` flag that the `lineage`
/// cell formats.
fn fetch_lineage_lookup(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, LineageInfo>> {
    // Pull every active parent_session candidate; we'll pick preferred
    // per source and look up grandparents in a second pass.
    let mut stmt = conn.prepare(
        "SELECT json_extract(cl.source, '$.harness_key') AS h, \
                json_extract(cl.source, '$.state_scope') AS s, \
                json_extract(cl.source, '$.session_key') AS k, \
                cl.link_id, cl.provenance, cl.confidence, \
                cl.target_kind, cl.target_node_kind, \
                cl.target_node, cl.target_native_id \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'agent_session' \
           AND cl.relation = 'parent_session' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is read via the pick_strongest tiebreak accessor
    struct Raw {
        link_id: String,
        provenance: String,
        confidence: String,
        target_kind: String,
        target_node_kind: Option<String>,
        target_node: Option<String>,
        target_native_id: Option<String>,
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
                target_kind: row.get(6)?,
                target_node_kind: row.get(7)?,
                target_node: row.get(8)?,
                target_native_id: row.get(9)?,
            },
        ))
    })?;
    for entry in rows {
        let (key, raw) = entry?;
        per_session.entry(key).or_default().push(raw);
    }

    // First pass: pick preferred per source, capture the parent's
    // SessionKey when the target is a known agent_session.
    fn parent_key_of(raw: &Raw) -> Option<SessionKey> {
        if raw.target_kind != "node" || raw.target_node_kind.as_deref() != Some("agent_session") {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(raw.target_node.as_deref()?).ok()?;
        Some(session_key(
            value.get("harness_key")?.as_str()?,
            value.get("state_scope")?.as_str()?,
            value.get("session_key")?.as_str()?,
        ))
    }
    let mut preferred: HashMap<SessionKey, (Raw, Option<SessionKey>)> = HashMap::new();
    for (key, candidates) in per_session {
        let Some(best) = pick_strongest(candidates, |r| (&r.provenance, &r.confidence, &r.link_id))
        else {
            continue;
        };
        let parent_key = parent_key_of(&best);
        preferred.insert(key, (best, parent_key));
    }

    // Second pass: which preferred parents themselves have a
    // preferred parent? That's the `←` flag.
    let mut out = HashMap::new();
    for (key, (raw, parent_key)) in &preferred {
        let label = match raw.target_kind.as_str() {
            "node" if raw.target_node_kind.as_deref() == Some("agent_session") => parent_key
                .as_ref()
                .map(|(_, _, sk)| short_session_id(sk))
                .unwrap_or_else(|| "?".to_string()),
            "node" => {
                // Resolved target but not an agent_session — the
                // in-memory renderer returns `—` in this case (the
                // `_ => return` fallthrough).
                continue;
            }
            "unresolved" => raw
                .target_native_id
                .as_deref()
                .map(|native| format!("?{}", short_session_id(native)))
                .unwrap_or_else(|| "?".to_string()),
            _ => continue,
        };
        let has_grandparent = parent_key
            .as_ref()
            .map(|pk| preferred.contains_key(pk))
            .unwrap_or(false);
        out.insert(
            key.clone(),
            LineageInfo {
                label,
                has_grandparent,
            },
        );
    }
    Ok(out)
}

/// Resolved `associated_with` relationships whose target is a
/// workspace, mirroring `session_workspace_identifier`. For each
/// workspace, if the resolver picked ≥2 distinct `workspace_contains_repo`
/// members, the display becomes a `+`-joined list of member names
/// (basename of each member link's `logical_path`); otherwise the
/// workspace root is used. Multiple workspaces per session join with
/// commas.
fn fetch_workspace_lookup(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, String>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(r.source, '$.harness_key') AS h, \
                json_extract(r.source, '$.state_scope') AS s, \
                json_extract(r.source, '$.session_key') AS k, \
                r.target AS workspace_node, \
                json_extract(r.target, '$.root') AS workspace_root \
         FROM resolved_relationships r \
         WHERE r.source_kind = 'agent_session' \
           AND r.target_kind = 'workspace' \
           AND r.relation = 'associated_with' \
         ORDER BY workspace_root",
    )?;
    let mut per_session: HashMap<SessionKey, Vec<(String, String)>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let h: String = row.get(0)?;
        let s: String = row.get(1)?;
        let k: String = row.get(2)?;
        let workspace_node: String = row.get(3)?;
        let workspace_root: String = row.get(4)?;
        Ok((session_key(&h, &s, &k), workspace_node, workspace_root))
    })?;
    for entry in rows {
        let (key, ws_node, ws_root) = entry?;
        per_session.entry(key).or_default().push((ws_node, ws_root));
    }

    let members = fetch_workspace_member_displays(conn)?;

    let mut out = HashMap::new();
    for (key, mut workspaces) in per_session {
        workspaces.sort();
        workspaces.dedup();
        let displays: Vec<String> = workspaces
            .into_iter()
            .map(|(ws_node, ws_root)| {
                members
                    .get(&ws_node)
                    .filter(|names| names.len() >= 2)
                    .map(|names| names.join("+"))
                    .unwrap_or(ws_root)
            })
            .collect();
        out.insert(key, displays.join(","));
    }
    Ok(out)
}

/// Index workspace member display names by workspace NodeId JSON.
/// Reads the resolver's chosen `workspace_contains_repo` selections
/// and extracts the basename of each link's `logical_path` source
/// field — atelier's `repo.name` directory and generic workspace
/// symlink/dir children both surface that way. Members without a
/// `logical_path` (no provider sets that today, but defensive) are
/// skipped silently; the threshold check in the caller treats a
/// short list as "single-repo workspace" and falls back to the
/// root path.
fn fetch_workspace_member_displays(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT r.source AS workspace_node, cl.source_fields \
         FROM resolved_relationships r \
         JOIN candidate_links cl ON cl.link_id = r.selected_link_id \
         WHERE r.relation = 'workspace_contains_repo' \
           AND r.source_kind = 'workspace' \
         ORDER BY r.source, r.target",
    )?;
    let mut per_workspace: HashMap<String, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let ws_node: String = row.get(0)?;
        let source_fields: String = row.get(1)?;
        Ok((ws_node, source_fields))
    })?;
    for entry in rows {
        let (ws_node, source_fields_json) = entry?;
        if let Some(display) = repo_display_from_fields(&source_fields_json) {
            per_workspace.entry(ws_node).or_default().push(display);
        }
    }
    for displays in per_workspace.values_mut() {
        displays.sort();
        displays.dedup();
    }
    Ok(per_workspace)
}

fn repo_display_from_fields(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let logical_path = value.get("logical_path")?.as_str()?;
    std::path::Path::new(logical_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
}

/// Active `child_session` candidate links from forks to agent
/// sessions; resolves the fork's display label and keys by target
/// session. Mirrors `session_owning_fork_label`.
fn fetch_fork_lookup(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, ForkInfo>> {
    // Same Display-form JOIN pattern: `ForkId.provider_source_key`
    // (in the JSON) may differ from the structural `ForkNode.
    // provider_source_key` column on `node_forks`, so we
    // reconstruct `fork:<psk>` and compare against `f.node_id`.
    let mut stmt = conn.prepare(
        "SELECT json_extract(cl.target_node, '$.harness_key') AS h, \
                json_extract(cl.target_node, '$.state_scope') AS s, \
                json_extract(cl.target_node, '$.session_key') AS k, \
                f.provider_name, f.name, f.provider_source_key, \
                cl.link_id \
         FROM candidate_links cl \
         JOIN node_forks f \
           ON cl.source_kind = 'fork' \
           AND ('fork:' || json_extract(cl.source, '$.provider_source_key')) = f.node_id \
         WHERE cl.target_node_kind = 'agent_session' \
           AND cl.relation = 'child_session' \
           AND cl.state = 'active' \
         ORDER BY cl.link_id",
    )?;
    let mut out: HashMap<SessionKey, ForkInfo> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let h: String = row.get(0)?;
        let s: String = row.get(1)?;
        let k: String = row.get(2)?;
        let provider: String = row.get(3)?;
        let name: Option<String> = row.get(4)?;
        let psk: String = row.get(5)?;
        let display = name.unwrap_or(psk);
        Ok((
            session_key(&h, &s, &k),
            ForkInfo {
                label: format!("{provider}:{display}"),
            },
        ))
    })?;
    for entry in rows {
        let (key, info) = entry?;
        // First-wins matches the in-memory iteration's early return.
        out.entry(key).or_insert(info);
    }
    Ok(out)
}

/// Strongest LocalDeclared or GlobalDeclared candidate per agent
/// session; surfaces its `LinkState` as a `"declared"`/`"ignored"`/
/// `"overridden"` label per the in-memory `session_declared_state`.
fn fetch_declared_lookup(conn: &Connection) -> rusqlite::Result<HashMap<SessionKey, DeclaredInfo>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(cl.source, '$.harness_key') AS h, \
                json_extract(cl.source, '$.state_scope') AS s, \
                json_extract(cl.source, '$.session_key') AS k, \
                cl.link_id, cl.provenance, cl.state \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'agent_session' \
           AND cl.provenance IN ('local_declared', 'global_declared')",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)] // link_id is materialized even though declared selection ignores it
    struct Raw {
        link_id: String,
        provenance: String,
        state: String,
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
                state: row.get(5)?,
            },
        ))
    })?;
    for entry in rows {
        let (key, raw) = entry?;
        per_session.entry(key).or_default().push(raw);
    }
    let mut out = HashMap::new();
    for (key, candidates) in per_session {
        // The in-memory version ranks by provenance precedence only
        // (`local_declared` beats `global_declared`), with no
        // confidence/id tiebreak. Replicate.
        let Some(best) = candidates.into_iter().max_by(|a, b| {
            render::provenance_precedence(&a.provenance)
                .cmp(&render::provenance_precedence(&b.provenance))
        }) else {
            continue;
        };
        let label = match best.state.as_str() {
            "active" => "declared",
            "ignored" => "ignored",
            "overridden" => "overridden",
            _ => continue,
        };
        out.insert(
            key,
            DeclaredInfo {
                state_label: label.to_string(),
            },
        );
    }
    Ok(out)
}

// -----------------------------------------------------------------------------
// Public re-routing
// -----------------------------------------------------------------------------

/// Convenience wrapper used by `output::table::render_with` for
/// [`Projection::Agent`]: materializes the snapshot to an in-memory
/// SQLite connection, runs [`build_agent_rows_from_conn`], and
/// hands the rows back. The materialization step is the bridge until
/// P10-014 demotes `GraphSnapshot` from the public render surface.
pub fn build_agent_rows_from_snapshot(
    snapshot: &crate::model::GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for agent projection");
    build_agent_rows_from_conn(&conn, columns, options)
        .expect("SQLite-backed agent projection should not fail against a freshly loaded snapshot")
}
