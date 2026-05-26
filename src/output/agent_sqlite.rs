//! Spike: SQLite-backed agent projection renderer (ADR 0043 candidate).
//!
//! Sibling of [`super::table`]'s in-memory renderer that consumes the
//! same `RenderOptions` / column registry surface but pulls its data
//! from a [`rusqlite::Connection`] instead of a [`GraphSnapshot`].
//!
//! Scope: the spike implements a minimum-viable subset of the agent
//! projection's cells (`id`, `agent`, `cwd`, `checkout`, `repo`,
//! `branch`, `title`, `preview`, `activity`) to validate the typed-
//! row layer shape. Cells that need additional joins (`mux`,
//! `mux-conf`, `pr`, `pr-conf`, `lineage`, `workspace`, `fork`,
//! `declared`) render as `—` here so the column ordering still
//! matches the in-memory renderer's output column-for-column.
//!
//! Findings are written up in the conversation that produced this
//! file; this comment is a pointer for whoever reads the diff first.

use rusqlite::Connection;

use super::render::{
    RenderOptions, SESSIONS_COLUMNS, current_epoch, default_columns, format_relative_age,
    header_label, node_short_id_from_display, render_rows, unique_prefix_len,
};
use crate::config::Projection;

/// One typed row produced by the agent-projection query. Field-for-
/// field shape of the join that drives the renderer. Optionals reflect
/// LEFT-joined columns (a session without a resolved checkout still
/// renders).
#[derive(Debug, Clone)]
struct AgentRow {
    session_node_id: String,
    harness_key: String,
    session_key: String,
    cwd: Option<String>,
    title: Option<String>,
    preview: Option<String>,
    last_active_epoch: Option<i64>,
    checkout_root: Option<String>,
    repo_common_dir: Option<String>,
    branch_refname: Option<String>,
}

fn fetch_agent_rows(conn: &Connection) -> rusqlite::Result<Vec<AgentRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.session_node_id, s.harness_key, s.session_key, s.cwd, \
                a.title, a.last_message_preview, s.last_active_epoch, \
                s.checkout_root, s.repo_common_dir, \
                c.current_branch_refname \
         FROM v_sessions_with_repo s \
         LEFT JOIN node_agent_sessions a ON a.node_id = s.session_node_id \
         LEFT JOIN node_checkouts c ON c.node_id = s.checkout_node_id \
         ORDER BY s.session_node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AgentRow {
            session_node_id: row.get(0)?,
            harness_key: row.get(1)?,
            session_key: row.get(2)?,
            cwd: row.get(3)?,
            title: row.get(4)?,
            preview: row.get(5)?,
            last_active_epoch: row.get(6)?,
            checkout_root: row.get(7)?,
            repo_common_dir: row.get(8)?,
            branch_refname: row.get(9)?,
        })
    })?;
    rows.collect()
}

fn strip_refs_heads(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string()
}

fn agent_cell(key: &str, row: &AgentRow, short_id: &str, now: i64) -> String {
    match key {
        "id" => short_id.to_string(),
        "agent" => format!("{}:{}", row.harness_key, row.session_key),
        "cwd" => row.cwd.clone().unwrap_or_else(|| "—".to_string()),
        "checkout" => row.checkout_root.clone().unwrap_or_else(|| "—".to_string()),
        "repo" => row
            .repo_common_dir
            .clone()
            .unwrap_or_else(|| "—".to_string()),
        "branch" => row
            .branch_refname
            .as_deref()
            .map(strip_refs_heads)
            .unwrap_or_else(|| "—".to_string()),
        "title" => row.title.clone().unwrap_or_else(|| "—".to_string()),
        "preview" => row.preview.clone().unwrap_or_else(|| "—".to_string()),
        "activity" => row
            .last_active_epoch
            .map(|t| format_relative_age(t, now))
            .unwrap_or_else(|| "—".to_string()),
        // Cells the spike does not cover yet — see module docs.
        _ => "—".to_string(),
    }
}

/// Render the agent projection from `conn`. Mirrors
/// [`super::table::render_with`] for [`Projection::Agent`] but reads
/// from SQLite. Filter / now-epoch knobs honored where applicable;
/// row-filter predicates beyond `now_epoch` are intentionally not
/// re-implemented here — that's part of the migration plan, not the
/// spike.
pub fn render_agent_sqlite(conn: &Connection, options: &RenderOptions) -> rusqlite::Result<String> {
    let columns: Vec<&'static str> = options
        .columns
        .clone()
        .unwrap_or_else(|| default_columns(Projection::Agent));

    let agent_rows = fetch_agent_rows(conn)?;
    let full_ids: Vec<String> = agent_rows
        .iter()
        .map(|r| node_short_id_from_display(&r.session_node_id))
        .collect();
    let id_len = unique_prefix_len(&full_ids);

    let now = options.now_epoch.unwrap_or_else(current_epoch);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(SESSIONS_COLUMNS, key))
            .collect(),
    );

    for (row, full_short) in agent_rows.iter().zip(full_ids.iter()) {
        let short_id = &full_short[..id_len];
        rows.push(
            columns
                .iter()
                .map(|key| agent_cell(key, row, short_id, now))
                .collect(),
        );
    }

    Ok(render_rows(rows, &columns, options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, GraphNode,
        GraphSnapshot, RepoId, RepoNode,
    };
    use crate::query::loader::load;
    use crate::query::schema::apply_schema;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory open");
        apply_schema(&conn).expect("apply schema");
        conn
    }

    fn loaded(snapshot: &GraphSnapshot) -> Connection {
        let mut conn = fresh_conn();
        load(snapshot, &mut conn).expect("load");
        conn
    }

    /// Build a snapshot with one repo, one checkout under that repo,
    /// and an agent session whose cwd lives inside the checkout root.
    /// This is the minimal shape that exercises the `v_sessions_with_repo`
    /// join the renderer leans on.
    fn snapshot_with_one_session() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/r/.git"), "/r"),
            root: "/r".into(),
            git_dir: Some("/r/.git".into()),
            current_branch: Some(BranchId::new(RepoId::new("/r/.git"), "refs/heads/main")),
        }));
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "default", "abc"),
            harness_key: "claude-code".into(),
            cwd: Some("/r/sub".into()),
            title: Some("a title".into()),
            last_message_preview: Some("hello world".into()),
            last_active_epoch: Some(1_700_000_000),
        }));
        snap
    }

    #[test]
    fn renders_basic_cells_from_v_sessions_with_repo() {
        let conn = loaded(&snapshot_with_one_session());
        let opts = RenderOptions::wide()
            .with_columns(vec![
                "id", "agent", "cwd", "checkout", "repo", "branch", "title", "preview", "activity",
            ])
            .with_now_epoch(Some(1_700_000_060));
        let rendered = render_agent_sqlite(&conn, &opts).expect("render");
        // Header at line 0, separator row at line 1, body at line 2.
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[0].contains("AGENT"));
        assert!(lines[0].contains("CWD"));
        assert!(lines[0].contains("BRANCH"));
        // The single data row contains the joined cells.
        let body = lines[2];
        assert!(
            body.contains("claude-code:abc"),
            "agent label missing: {body}"
        );
        assert!(body.contains("/r/sub"), "cwd missing: {body}");
        assert!(body.contains("/r"), "checkout root missing: {body}");
        assert!(body.contains("/r/.git"), "repo common_dir missing: {body}");
        assert!(
            body.contains("main"),
            "branch should strip refs/heads/: {body}"
        );
        assert!(body.contains("a title"), "title missing: {body}");
        assert!(body.contains("hello world"), "preview missing: {body}");
        assert!(
            body.contains("1m"),
            "activity should render relative age: {body}"
        );
    }

    #[test]
    fn missing_checkout_renders_em_dash() {
        // A session whose cwd has no covering checkout (the LEFT JOIN
        // in v_sessions_with_repo yields NULL checkout columns).
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "default", "session-1"),
            harness_key: "codex".into(),
            cwd: Some("/elsewhere".into()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
        }));
        let conn = loaded(&snap);
        let opts = RenderOptions::wide().with_columns(vec!["id", "agent", "checkout", "repo"]);
        let rendered = render_agent_sqlite(&conn, &opts).expect("render");
        let body = rendered.lines().nth(2).unwrap();
        assert!(body.contains("codex:session-1"), "agent label: {body}");
        // Two em-dashes for the un-joined columns.
        let dash_count = body.matches('—').count();
        assert!(dash_count >= 2, "expected two em-dashes, got: {body}");
    }
}
