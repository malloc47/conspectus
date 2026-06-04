//! `conspectus node show <id>` (H-OBS-002).
//!
//! Read-only single-node view: prints the node itself plus every candidate
//! link, resolved relationship, source metadata, and diagnostic that touches
//! the node. The accepted forms for `<id>` are:
//!
//! - The short content-addressed prefix from the session-table `ID` column
//!   (H-TBL-002). Any prefix length ≥ 4 hex chars is accepted; ambiguity
//!   errors with the matching candidates listed.
//! - The full `NodeId` `Display` form, e.g.
//!   `agent_session:codex:/state:session-x` or `mux_session:tmux:editor`.
//! - The harness/mux label that appears in the session table's `AGENT`
//!   or `MUX` column, e.g. `codex:session-x` or `tmux:editor`. The label
//!   only resolves when it uniquely identifies one node.
//! - A bare harness-native agent session key from the session table's `ID`
//!   column, when it uniquely identifies one node.
//!
//! H-TBL-005 wires these forms through the CLI command added by H-OBS-002.
//!
//! SQLite-backed renderer (P10-009 / ADR 0043). The existing
//! `resolve_node_id` / `render_node_show` entry points remain as thin
//! bridges that materialize a snapshot to an in-memory SQLite
//! connection and route through the SQL implementations below.
//! P10-014 retires the bridge.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use rusqlite::Connection;

use crate::model::{GraphSnapshot, NodeId};
use crate::output::render::{self, header_style, node_short_id_from_display, push_styled};

/// Outcome of resolving an `<id>` argument to a [`NodeId`].
#[derive(Debug)]
pub enum NodeResolveError {
    /// No node matched the input under any accepted form.
    NotFound { input: String },
    /// The input matched more than one node. `candidates` lists each
    /// matching `NodeId` so the caller can surface them.
    Ambiguous {
        input: String,
        candidates: Vec<NodeId>,
    },
}

impl std::fmt::Display for NodeResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeResolveError::NotFound { input } => {
                write!(f, "no node matches {input:?}")
            }
            NodeResolveError::Ambiguous { input, candidates } => {
                writeln!(f, "{input:?} matches {} nodes:", candidates.len())?;
                for id in candidates {
                    writeln!(f, "  - {id}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for NodeResolveError {}

// -----------------------------------------------------------------------------
// Public bridge entry points (materialize snapshot, delegate to SQL)
// -----------------------------------------------------------------------------

/// Resolve `input` to a single [`NodeId`] in `snapshot` using every
/// accepted form. Materializes `snapshot` to an in-memory SQLite
/// connection and delegates to [`resolve_node_id_from_conn`].
pub fn resolve_node_id(input: &str, snapshot: &GraphSnapshot) -> Result<NodeId, NodeResolveError> {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for node show");
    resolve_node_id_from_conn(&conn, input)
        .expect("node-id resolution should not fail on a freshly loaded snapshot")
}

/// Render the resolved node `id` against `snapshot` as plain text.
/// Materializes `snapshot` to an in-memory SQLite connection and
/// delegates to [`render_node_show_from_conn`].
pub fn render_node_show(snapshot: &GraphSnapshot, id: &NodeId, color: bool) -> String {
    let conn = crate::query::materialize_snapshot(snapshot)
        .expect("materialize GraphSnapshot to in-memory SQLite for node show");
    render_node_show_from_conn(&conn, id, color)
        .expect("node-show rendering should not fail on a freshly loaded snapshot")
}

// -----------------------------------------------------------------------------
// SQL-driven implementations
// -----------------------------------------------------------------------------

/// Resolve `input` to a `NodeId` by querying `conn`. Accepted forms
/// mirror [`resolve_node_id`].
pub fn resolve_node_id_from_conn(
    conn: &Connection,
    input: &str,
) -> rusqlite::Result<Result<NodeId, NodeResolveError>> {
    let trimmed = input.trim();
    let is_hex_prefix = !trimmed.is_empty()
        && trimmed.len() <= 16
        && trimmed.chars().all(|c| c.is_ascii_hexdigit());

    let mut matches: BTreeMap<NodeId, ()> = BTreeMap::new();

    // 1. Hex prefix and Display matches walk every node via v_nodes.
    let mut stmt = conn.prepare("SELECT node_id FROM v_nodes")?;
    let node_ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    for node_id_text in &node_ids {
        let matches_input = (is_hex_prefix
            && node_short_id_from_display(node_id_text).starts_with(trimmed))
            || node_id_text == trimmed;
        if matches_input && let Some(id) = parse_display_via_typed_tables(conn, node_id_text)? {
            matches.insert(id, ());
        }
    }

    // 2. Label matches against agent and mux sessions only.
    //    `<harness>:<session_key>` or `<harness>:<title>` for agents,
    //    bare `<session_key>` for agents when unique,
    //    `<backend>:<native_id>` for muxes — same as the in-memory
    //    `label_matches`.
    let mut agent_stmt = conn
        .prepare("SELECT harness_key, state_scope, session_key, title FROM node_agent_sessions")?;
    let agent_rows: Vec<(String, String, String, Option<String>)> = agent_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (harness, scope, key, title) in agent_rows {
        let key_label = format!("{harness}:{key}");
        let title_label = title.as_deref().map(|t| format!("{harness}:{t}"));
        if key == trimmed || key_label == trimmed || title_label.as_deref() == Some(trimmed) {
            let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
                harness.clone(),
                scope.clone(),
                key.clone(),
            ));
            matches.insert(id, ());
        }
    }
    let mut mux_stmt = conn.prepare("SELECT node_id, backend, native_id FROM node_mux_sessions")?;
    let mux_rows: Vec<(String, String, String)> = mux_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (node_id, backend, native_id) in mux_rows {
        // Label is `<backend>:<structural native_id>` (matches
        // `label_matches`); the typed NodeId is reconstructed from
        // the canonical `node_id` column since `MuxSessionId.native_id`
        // can diverge from the structural column.
        if format!("{backend}:{native_id}") == trimmed
            && let Some(id) = parse_display_via_typed_tables(conn, &node_id)?
        {
            matches.insert(id, ());
        }
    }

    let mut candidates: Vec<NodeId> = matches.into_keys().collect();
    Ok(match candidates.len() {
        0 => Err(NodeResolveError::NotFound {
            input: trimmed.to_string(),
        }),
        1 => Ok(candidates.remove(0)),
        _ => Err(NodeResolveError::Ambiguous {
            input: trimmed.to_string(),
            candidates,
        }),
    })
}

/// Reconstruct a typed [`NodeId`] from its `Display` form by looking
/// up the matching row in the per-kind `node_<kind>` table and
/// rebuilding the `*Id` from its structural columns. Returns `None`
/// when the Display form doesn't match any present node (which can
/// only happen if the caller passed a hex prefix that hashes a
/// nonexistent node — unreachable in normal use).
fn parse_display_via_typed_tables(
    conn: &Connection,
    display: &str,
) -> rusqlite::Result<Option<NodeId>> {
    use crate::model::{
        AgentSessionId, BranchId, CheckoutId, ForgePrId, ForkId, MuxSessionId, RepoId, WorkspaceId,
    };
    // Each lookup is cheap (PK lookup) and we know exactly one will
    // succeed per kind discriminator.
    let kind = display.split_once(':').map(|(k, _)| k).unwrap_or("");
    match kind {
        "repo" => {
            let common_dir: Option<String> = conn
                .query_row(
                    "SELECT common_dir FROM node_repos WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(common_dir.map(|cd| NodeId::Repo(RepoId::new(cd))))
        }
        "workspace" => {
            let root: Option<String> = conn
                .query_row(
                    "SELECT root FROM node_workspaces WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(root.map(|r| NodeId::Workspace(WorkspaceId::new(r))))
        }
        "mux_session" => {
            // The MuxSessionId's native_id is embedded in the
            // Display form as the suffix after `mux_session:`. The
            // structural column may diverge from it; use the
            // canonical id from the display string.
            let row: Option<String> = conn
                .query_row(
                    "SELECT node_id FROM node_mux_sessions WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(row.map(|_| {
                let native_id = display.strip_prefix("mux_session:").unwrap_or(display);
                NodeId::MuxSession(MuxSessionId::new(native_id))
            }))
        }
        "fork" => {
            let row: Option<String> = conn
                .query_row(
                    "SELECT node_id FROM node_forks WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(row.map(|_| {
                let psk = display.strip_prefix("fork:").unwrap_or(display);
                NodeId::Fork(ForkId::new(psk))
            }))
        }
        "agent_session" => {
            let row: Option<(String, String, String)> = conn
                .query_row(
                    "SELECT harness_key, state_scope, session_key FROM node_agent_sessions \
                     WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            Ok(row.map(|(h, s, k)| NodeId::AgentSession(AgentSessionId::new(h, s, k))))
        }
        "checkout" => {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT repo_common_dir, root FROM node_checkouts WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row.map(|(cd, root)| NodeId::Checkout(CheckoutId::new(RepoId::new(cd), root))))
        }
        "branch" => {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT repo_common_dir, refname FROM node_branches WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row.map(|(cd, refname)| NodeId::Branch(BranchId::new(RepoId::new(cd), refname))))
        }
        "forge_pr" => {
            let row: Option<(String, String, String, String, i64)> = conn
                .query_row(
                    "SELECT provider_name, host, owner, repo, number FROM node_forge_prs \
                     WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;
            Ok(row.map(|(provider, host, owner, repo, number)| {
                NodeId::ForgePr(ForgePrId::new(
                    provider,
                    host,
                    owner,
                    repo,
                    u64::try_from(number).unwrap_or(0),
                ))
            }))
        }
        _ => Ok(None),
    }
}

use rusqlite::OptionalExtension;

/// Render the resolved node `id` against `conn` as plain text.
pub fn render_node_show_from_conn(
    conn: &Connection,
    id: &NodeId,
    color: bool,
) -> rusqlite::Result<String> {
    let mut out = String::new();
    if !write_node_summary_from_conn(&mut out, conn, id, color)? {
        return Ok(format!("node {id} not found in snapshot\n"));
    }
    write_candidate_links_from_conn(&mut out, conn, id, color)?;
    write_resolved_from_conn(&mut out, conn, id, color)?;
    write_diagnostics_from_conn(&mut out, conn, id, color)?;
    Ok(out)
}

// -----------------------------------------------------------------------------
// Per-kind summary
// -----------------------------------------------------------------------------

fn write_section_header(out: &mut String, text: &str, color: bool) {
    push_styled(out, text, header_style(), color);
    out.push('\n');
}

fn node_kind_label(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::RuntimeProcess(_) => "runtime_process",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}

fn node_reference_label_from_display(conn: &Connection, display: &str) -> rusqlite::Result<String> {
    let kind = display.split_once(':').map(|(k, _)| k).unwrap_or("");
    match kind {
        "agent_session" => {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT harness_key, session_key FROM node_agent_sessions WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row
                .map(|(harness, key)| format!("{harness}:{key}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "mux_session" => {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT backend, native_id FROM node_mux_sessions WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row
                .map(|(backend, native_id)| format!("{backend}:{native_id}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "runtime_process" => {
            let row: Option<(String, Option<i64>, Option<String>)> = conn
                .query_row(
                    "SELECT observation_key, pid, command FROM node_runtime_processes \
                     WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            Ok(row
                .map(
                    |(observation_key, pid, command)| match (pid, command.as_deref()) {
                        (Some(pid), Some(command)) => format!("pid {pid}: {command}"),
                        (Some(pid), None) => format!("pid {pid}"),
                        (None, Some(command)) => command.to_string(),
                        (None, None) => observation_key,
                    },
                )
                .unwrap_or_else(|| display.to_string()))
        }
        "forge_pr" => {
            let row: Option<(String, String, i64)> = conn
                .query_row(
                    "SELECT owner, repo, number FROM node_forge_prs WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            Ok(row
                .map(|(owner, repo, number)| format!("{owner}/{repo}#{number}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "fork" => {
            let row: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT provider_source_key, name FROM node_forks WHERE node_id = ?1",
                    [display],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row
                .map(|(source_key, name)| name.unwrap_or(source_key))
                .unwrap_or_else(|| display.to_string()))
        }
        "checkout" => {
            let root: Option<String> = conn
                .query_row(
                    "SELECT root FROM node_checkouts WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(root
                .map(|root| format!("checkout:{root}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "workspace" => {
            let root: Option<String> = conn
                .query_row(
                    "SELECT root FROM node_workspaces WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(root
                .map(|root| format!("workspace:{root}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "repo" => {
            let common_dir: Option<String> = conn
                .query_row(
                    "SELECT common_dir FROM node_repos WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(common_dir
                .map(|common_dir| format!("repo:{common_dir}"))
                .unwrap_or_else(|| display.to_string()))
        }
        "branch" => {
            let refname: Option<String> = conn
                .query_row(
                    "SELECT refname FROM node_branches WHERE node_id = ?1",
                    [display],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(refname
                .map(|refname| format!("branch:{refname}"))
                .unwrap_or_else(|| display.to_string()))
        }
        _ => Ok(display.to_string()),
    }
}

/// Returns `false` when the node doesn't exist in any `node_<kind>`
/// table; callers short-circuit with "not found" output.
fn write_node_summary_from_conn(
    out: &mut String,
    conn: &Connection,
    id: &NodeId,
    color: bool,
) -> rusqlite::Result<bool> {
    let id_display = id.to_string();
    let id_short = node_short_id_from_display(&id_display);
    write_section_header(out, &format!("node {id_short}"), color);
    let _ = writeln!(out, "  kind: {}", node_kind_label(id));
    let _ = writeln!(out, "  id:   {id_display}");
    let exists = match id {
        NodeId::Repo(_) => write_repo(out, conn, &id_display)?,
        NodeId::Checkout(_) => write_checkout(out, conn, &id_display)?,
        NodeId::Workspace(_) => write_workspace(out, conn, &id_display)?,
        NodeId::AgentSession(aid) => write_agent_session(out, conn, &id_display, aid)?,
        NodeId::MuxSession(_) => write_mux_session(out, conn, &id_display)?,
        NodeId::RuntimeProcess(_) => write_runtime_process(out, conn, &id_display)?,
        NodeId::Branch(bid) => write_branch(out, bid)?,
        NodeId::Fork(_) => write_fork(out, conn, &id_display)?,
        NodeId::ForgePr(_) => write_forge_pr(out, conn, &id_display)?,
    };
    Ok(exists)
}

fn write_repo(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT common_dir, source_paths FROM node_repos WHERE node_id = ?1",
            [node_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((common_dir, source_paths_json)) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  common_dir: {common_dir}");
    let source_paths: Vec<String> = serde_json::from_str(&source_paths_json).unwrap_or_default();
    if !source_paths.is_empty() {
        let _ = writeln!(out, "  source_paths:");
        for path in &source_paths {
            let _ = writeln!(out, "    - {path}");
        }
    }
    Ok(true)
}

fn write_checkout(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT root, git_dir FROM node_checkouts WHERE node_id = ?1",
            [node_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((root, git_dir)) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  root: {root}");
    if let Some(git_dir) = git_dir {
        let _ = writeln!(out, "  git_dir: {git_dir}");
    }
    Ok(true)
}

fn write_workspace(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    let row: Option<String> = conn
        .query_row(
            "SELECT root FROM node_workspaces WHERE node_id = ?1",
            [node_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(root) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  root: {root}");
    Ok(true)
}

fn write_agent_session(
    out: &mut String,
    conn: &Connection,
    node_id: &str,
    id: &crate::model::AgentSessionId,
) -> rusqlite::Result<bool> {
    let row: Option<(String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT harness_key, cwd, title FROM node_agent_sessions WHERE node_id = ?1",
            [node_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((harness, cwd, title)) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  harness:     {harness}");
    let _ = writeln!(out, "  state_scope: {}", id.state_scope);
    let _ = writeln!(out, "  session_key: {}", id.session_key);
    if let Some(cwd) = cwd {
        let _ = writeln!(out, "  cwd:         {cwd}");
    }
    // ADR 0029: alias hides title in default renders. Resolve via
    // the aliases table joined on the session's structural fields.
    let alias: Option<String> = conn
        .query_row(
            "SELECT display_name FROM aliases \
             WHERE node_kind = 'agent_session' \
               AND json_extract(node, '$.harness_key') = ?1 \
               AND json_extract(node, '$.state_scope') = ?2 \
               AND json_extract(node, '$.session_key') = ?3",
            [&harness, &id.state_scope, &id.session_key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(alias) = &alias {
        let _ = writeln!(out, "  alias:       {alias}");
    }
    if alias.is_none()
        && let Some(title) = title
    {
        let _ = writeln!(out, "  title:       {title}");
    }
    Ok(true)
}

fn write_mux_session(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT backend, native_id, cwd FROM node_mux_sessions WHERE node_id = ?1",
            [node_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((backend, native_id, cwd)) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  backend:   {backend}");
    let _ = writeln!(out, "  native_id: {native_id}");
    if let Some(cwd) = cwd {
        let _ = writeln!(out, "  cwd:       {cwd}");
    }
    Ok(true)
}

fn write_runtime_process(
    out: &mut String,
    conn: &Connection,
    node_id: &str,
) -> rusqlite::Result<bool> {
    struct ProcessRow {
        observation_key: String,
        pid: Option<i64>,
        parent_pid: Option<i64>,
        root_pane_pid: Option<i64>,
        command: Option<String>,
        cwd: Option<String>,
        harness_key: Option<String>,
        role: Option<String>,
        depth: Option<i64>,
        observed_epoch: Option<i64>,
    }
    let row: Option<ProcessRow> = conn
        .query_row(
            "SELECT observation_key, pid, parent_pid, root_pane_pid, command, cwd, \
                    harness_key, role, depth, observed_epoch \
             FROM node_runtime_processes WHERE node_id = ?1",
            [node_id],
            |r| {
                Ok(ProcessRow {
                    observation_key: r.get(0)?,
                    pid: r.get(1)?,
                    parent_pid: r.get(2)?,
                    root_pane_pid: r.get(3)?,
                    command: r.get(4)?,
                    cwd: r.get(5)?,
                    harness_key: r.get(6)?,
                    role: r.get(7)?,
                    depth: r.get(8)?,
                    observed_epoch: r.get(9)?,
                })
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  observation_key: {}", row.observation_key);
    if let Some(pid) = row.pid {
        let _ = writeln!(out, "  pid:             {pid}");
    }
    if let Some(parent_pid) = row.parent_pid {
        let _ = writeln!(out, "  parent_pid:      {parent_pid}");
    }
    if let Some(root_pane_pid) = row.root_pane_pid {
        let _ = writeln!(out, "  root_pane_pid:   {root_pane_pid}");
    }
    if let Some(command) = row.command {
        let _ = writeln!(out, "  command:         {command}");
    }
    if let Some(cwd) = row.cwd {
        let _ = writeln!(out, "  cwd:             {cwd}");
    }
    if let Some(harness_key) = row.harness_key {
        let _ = writeln!(out, "  harness:         {harness_key}");
    }
    if let Some(role) = row.role {
        let _ = writeln!(out, "  role:            {role}");
    }
    if let Some(depth) = row.depth {
        let _ = writeln!(out, "  depth:           {depth}");
    }
    if let Some(observed_epoch) = row.observed_epoch {
        let _ = writeln!(out, "  observed_epoch:  {observed_epoch}");
    }
    Ok(true)
}

fn write_branch(out: &mut String, id: &crate::model::BranchId) -> rusqlite::Result<bool> {
    // The summary lines for a branch come straight from its typed
    // BranchId — no extra columns on node_branches matter for the
    // section header. Reading the row still tells us whether the
    // node exists in the snapshot.
    let _ = writeln!(out, "  repo:    {}", id.repo);
    let _ = writeln!(out, "  refname: {}", id.refname);
    Ok(true)
}

fn write_fork(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    let row: Option<(String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT provider_name, provider_source_key, name, scope FROM node_forks \
             WHERE node_id = ?1",
            [node_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((provider, psk, name, scope)) = row else {
        return Ok(false);
    };
    let _ = writeln!(out, "  provider: {provider}");
    let _ = writeln!(out, "  provider_source_key: {psk}");
    if let Some(name) = name {
        let _ = writeln!(out, "  name:     {name}");
    }
    if let Some(scope) = scope {
        let _ = writeln!(out, "  scope:    {scope}");
    }
    Ok(true)
}

fn write_forge_pr(out: &mut String, conn: &Connection, node_id: &str) -> rusqlite::Result<bool> {
    struct PrRow {
        owner: String,
        repo: String,
        number: i64,
        state: Option<String>,
        url: Option<String>,
    }
    let row: Option<PrRow> = conn
        .query_row(
            "SELECT owner, repo, number, state, url FROM node_forge_prs WHERE node_id = ?1",
            [node_id],
            |r| {
                Ok(PrRow {
                    owner: r.get(0)?,
                    repo: r.get(1)?,
                    number: r.get(2)?,
                    state: r.get(3)?,
                    url: r.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(PrRow {
        owner,
        repo,
        number,
        state,
        url,
    }) = row
    else {
        return Ok(false);
    };
    let _ = writeln!(
        out,
        "  pr:    {owner}/{repo}#{number} ({})",
        state.as_deref().unwrap_or("?")
    );
    if let Some(url) = url {
        let _ = writeln!(out, "  url:   {url}");
    }
    Ok(true)
}

// -----------------------------------------------------------------------------
// Sections that walk candidate_links / resolved_relationships / diagnostics
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LinkRow {
    link_id: String,
    source_label: String,
    target_kind: String,
    target_node_label: Option<String>,
    target_node_type: Option<String>,
    target_harness_key: Option<String>,
    target_native_id: Option<String>,
    target_path: Option<String>,
    relation: String,
    provenance: String,
    confidence: String,
    state: String,
    source_adapter: String,
    source_evidence: Option<String>,
    source_fields_json: String,
}

fn fetch_link_rows(
    conn: &Connection,
    where_clause: &str,
    bind: &str,
) -> rusqlite::Result<Vec<LinkRow>> {
    let sql = format!(
        "SELECT link_id, source, target_kind, target_node, \
                target_node_type, target_harness_key, target_native_id, target_path, \
                relation, provenance, confidence, state, \
                source_adapter, source_evidence, source_fields \
         FROM candidate_links \
         WHERE {where_clause} \
         ORDER BY link_id",
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([bind], |row| {
        let source_json: String = row.get(1)?;
        let target_node_json: Option<String> = row.get(3)?;
        let source_id = crate::query::reader::parse_node_id_json(&source_json, 1)?;
        let source_display = source_id.to_string();
        let target_node_label = match target_node_json.as_deref() {
            Some(json) => {
                let target_id = crate::query::reader::parse_node_id_json(json, 3)?;
                Some(node_reference_label_from_display(
                    conn,
                    &target_id.to_string(),
                )?)
            }
            None => None,
        };
        Ok(LinkRow {
            link_id: row.get(0)?,
            source_label: node_reference_label_from_display(conn, &source_display)?,
            target_kind: row.get(2)?,
            target_node_label,
            target_node_type: row.get(4)?,
            target_harness_key: row.get(5)?,
            target_native_id: row.get(6)?,
            target_path: row.get(7)?,
            relation: row.get(8)?,
            provenance: row.get(9)?,
            confidence: row.get(10)?,
            state: row.get(11)?,
            source_adapter: row.get(12)?,
            source_evidence: row.get(13)?,
            source_fields_json: row.get(14)?,
        })
    })?;
    rows.collect()
}

fn write_candidate_links_from_conn(
    out: &mut String,
    conn: &Connection,
    id: &NodeId,
    color: bool,
) -> rusqlite::Result<()> {
    let id_json = serde_json::to_string(id).expect("NodeId serializes");
    let outgoing = fetch_link_rows(conn, "source = ?1", &id_json)?;
    let incoming = fetch_link_rows(conn, "target_kind = 'node' AND target_node = ?1", &id_json)?;

    out.push('\n');
    write_section_header(
        out,
        &format!("outgoing candidate links: {}", outgoing.len()),
        color,
    );
    for link in &outgoing {
        write_link(out, link, LinkDirection::Outgoing);
    }
    out.push('\n');
    write_section_header(
        out,
        &format!("incoming candidate links: {}", incoming.len()),
        color,
    );
    for link in &incoming {
        write_link(out, link, LinkDirection::Incoming);
    }
    Ok(())
}

enum LinkDirection {
    Outgoing,
    Incoming,
}

fn write_link(out: &mut String, link: &LinkRow, dir: LinkDirection) {
    let other = match dir {
        LinkDirection::Outgoing => match link.target_kind.as_str() {
            "node" => match &link.target_node_label {
                Some(display) => format!("→ {display}"),
                None => "→ ?".to_string(),
            },
            _ => {
                let mut parts: Vec<String> = vec![format!(
                    "type={}",
                    link.target_node_type.as_deref().unwrap_or("")
                )];
                if let Some(harness) = &link.target_harness_key {
                    parts.push(format!("harness={harness}"));
                }
                if let Some(native) = &link.target_native_id {
                    parts.push(format!("native_id={native}"));
                }
                if let Some(path) = &link.target_path {
                    parts.push(format!("path={path}"));
                }
                format!("→ unresolved({})", parts.join(", "))
            }
        },
        LinkDirection::Incoming => format!("← {}", link.source_label),
    };
    let _ = writeln!(
        out,
        "  - {relation:15} {other} [{ind}, {state}] (link={link_id})",
        relation = link.relation,
        ind = render::indicator_from_tags(&link.provenance, &link.confidence, false),
        state = link.state,
        link_id = link.link_id,
    );
    let _ = writeln!(out, "      adapter: {}", link.source_adapter);
    if let Some(evidence) = &link.source_evidence {
        let _ = writeln!(out, "      evidence: {evidence}");
    }
    let fields: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&link.source_fields_json).unwrap_or_default();
    if !fields.is_empty() {
        let _ = writeln!(out, "      fields:");
        for (key, value) in &fields {
            let _ = writeln!(out, "        {key}: {value}");
        }
    }
}

fn write_resolved_from_conn(
    out: &mut String,
    conn: &Connection,
    id: &NodeId,
    color: bool,
) -> rusqlite::Result<()> {
    let id_json = serde_json::to_string(id).expect("NodeId serializes");
    let mut stmt = conn.prepare(
        "SELECT source, target, relation, selected_link_id, competing_link_ids \
         FROM resolved_relationships \
         WHERE source = ?1 OR target = ?1 \
         ORDER BY relation, source, target",
    )?;
    #[derive(Clone)]
    struct Raw {
        source_label: String,
        target_label: String,
        relation: String,
        selected: String,
        competing: Vec<String>,
    }
    let rows = stmt.query_map([&id_json], |row| {
        let source_json: String = row.get(0)?;
        let target_json: String = row.get(1)?;
        let source_display = crate::query::reader::parse_node_id_json(&source_json, 0)?.to_string();
        let target_display = crate::query::reader::parse_node_id_json(&target_json, 1)?.to_string();
        Ok(Raw {
            source_label: node_reference_label_from_display(conn, &source_display)?,
            target_label: node_reference_label_from_display(conn, &target_display)?,
            relation: row.get(2)?,
            selected: row.get(3)?,
            competing: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        })
    })?;
    let resolved: Vec<Raw> = rows.collect::<rusqlite::Result<_>>()?;
    out.push('\n');
    write_section_header(
        out,
        &format!("resolved relationships: {}", resolved.len()),
        color,
    );
    for rel in &resolved {
        let _ = writeln!(
            out,
            "  - {relation:15} {source} → {target} (selected={selected})",
            relation = rel.relation,
            source = rel.source_label,
            target = rel.target_label,
            selected = rel.selected,
        );
        if !rel.competing.is_empty() {
            let _ = writeln!(out, "      competing: {}", rel.competing.join(", "));
        }
    }
    Ok(())
}

fn write_diagnostics_from_conn(
    out: &mut String,
    conn: &Connection,
    id: &NodeId,
    color: bool,
) -> rusqlite::Result<()> {
    let id_json = serde_json::to_string(id).expect("NodeId serializes");

    // UnresolvedEndpoint: diagnostic touches the node when the
    // referenced candidate link has source = this node. JOIN
    // diagnostics to candidate_links on link_id and filter.
    let mut unres_stmt = conn.prepare(
        "SELECT d.link_id, d.relation \
         FROM diagnostics d \
         JOIN candidate_links cl ON cl.link_id = d.link_id \
         WHERE d.kind = 'unresolved_endpoint' \
           AND cl.source = ?1 \
         ORDER BY d.link_id",
    )?;
    let unres_rows: Vec<(String, String)> = unres_stmt
        .query_map([&id_json], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Conflict: diagnostic touches the node when conflict_source =
    // this node.
    let mut conf_stmt = conn.prepare(
        "SELECT relation, conflict_selected_link_id, conflict_competing_link_ids \
         FROM diagnostics \
         WHERE kind = 'conflict' AND conflict_source = ?1 \
         ORDER BY conflict_selected_link_id",
    )?;
    let conf_rows: Vec<(String, String, Vec<String>)> = conf_stmt
        .query_map([&id_json], |row| {
            let competing_json: String = row.get(2)?;
            let competing: Vec<String> = serde_json::from_str(&competing_json).unwrap_or_default();
            Ok((row.get(0)?, row.get(1)?, competing))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let total = unres_rows.len() + conf_rows.len();
    out.push('\n');
    write_section_header(out, &format!("diagnostics: {total}"), color);
    for (link_id, relation) in &unres_rows {
        let _ = writeln!(out, "  - unresolved_endpoint {relation} (link={link_id})",);
    }
    for (relation, selected, competing) in &conf_rows {
        let _ = writeln!(
            out,
            "  - conflict {relation} source={id} selected={selected} competing={}",
            competing.join(", "),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode,
        LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RelationKind,
        SourceMetadata,
    };

    fn agent_node(harness: &str, scope: &str, key: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: Some("/work".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn mux_node(backend: &str, name: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: Some("/work".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux(id: &str, source: NodeId, target: NodeId) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn resolve_node_id_matches_short_hex_prefix() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let prefix = &node_short_id_from_display(&id.to_string())[..6];
        let resolved = resolve_node_id(prefix, &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_display_form() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id(&id.to_string(), &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_harness_label() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id("codex:alpha", &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_mux_label() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_node("tmux", "editor")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id("tmux:editor", &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_errors_on_unknown_input() {
        let snapshot = GraphSnapshot::empty();
        let err = resolve_node_id("does-not-exist", &snapshot).unwrap_err();
        assert!(matches!(err, NodeResolveError::NotFound { .. }));
    }

    #[test]
    fn resolve_node_id_errors_on_ambiguous_prefix() {
        // Build two agent sessions whose short_ids share at least one
        // hex character; that shared prefix is by construction
        // ambiguous. We can't predict the hash, so probe.
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_node("codex", "/state", "one"),
                agent_node("codex", "/state", "two"),
            ],
            ..GraphSnapshot::empty()
        };
        let first = node_short_id_from_display(&snapshot.nodes[0].id().to_string());
        let second = node_short_id_from_display(&snapshot.nodes[1].id().to_string());
        let shared_len = first
            .chars()
            .zip(second.chars())
            .take_while(|(a, b)| a == b)
            .count();
        if shared_len == 0 {
            return; // skip when the fixture happens not to collide
        }
        let prefix = &first[..shared_len];
        let err = resolve_node_id(prefix, &snapshot).unwrap_err();
        assert!(matches!(err, NodeResolveError::Ambiguous { .. }));
    }

    #[test]
    fn node_show_renders_alias_in_place_of_title() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("opencode", "/state", "alpha"),
                harness_key: "opencode".to_string(),
                cwd: Some("/work".to_string()),
                title: Some("harness title that should be hidden".to_string()),
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            })],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        snapshot
            .aliases
            .insert(id.clone(), "ingest-refactor".to_string());

        let output = render_node_show(&snapshot, &id, false);
        assert!(
            output.contains("alias:       ingest-refactor"),
            "alias row missing in:\n{output}",
        );
        assert!(
            !output.contains("title:"),
            "title row should be hidden when alias is set:\n{output}",
        );
    }

    #[test]
    fn node_show_falls_back_to_title_when_no_alias() {
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("opencode", "/state", "alpha"),
                harness_key: "opencode".to_string(),
                cwd: Some("/work".to_string()),
                title: Some("Phase 8 mockup".to_string()),
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            })],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();

        let output = render_node_show(&snapshot, &id, false);
        assert!(
            output.contains("title:       Phase 8 mockup"),
            "title row should appear when alias is unset:\n{output}",
        );
        assert!(!output.contains("alias:"));
    }

    #[test]
    fn render_node_show_includes_outgoing_link_and_resolved_relationship() {
        let agent = agent_node("codex", "/state", "alpha");
        let mux = mux_node("tmux", "editor");
        let agent_id = agent.id();
        let mux_id = mux.id();
        let snapshot = GraphSnapshot {
            nodes: vec![agent, mux],
            candidate_links: vec![linked_to_mux("link-1", agent_id.clone(), mux_id.clone())],
            resolved_relationships: vec![crate::model::ResolvedRelationship {
                source: agent_id.clone(),
                target: mux_id.clone(),
                relation: RelationKind::LinkedToMux,
                selected_link_id: "link-1".to_string(),
                competing_link_ids: vec![],
            }],
            ..GraphSnapshot::empty()
        };
        let rendered = render_node_show(&snapshot, &agent_id, false);
        assert!(rendered.contains("kind: agent_session"));
        assert!(rendered.contains("outgoing candidate links: 1"));
        assert!(rendered.contains("linked_to_mux"));
        assert!(
            rendered.contains("linked_to_mux   → tmux:editor"),
            "candidate link should use mux-native label:\n{rendered}",
        );
        assert!(rendered.contains("resolved relationships: 1"));
        assert!(
            rendered.contains("linked_to_mux   codex:alpha → tmux:editor"),
            "resolved relationship should use external labels:\n{rendered}",
        );
        assert!(rendered.contains("selected=link-1"));
    }
}
