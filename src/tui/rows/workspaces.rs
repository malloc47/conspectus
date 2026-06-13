//! SQLite-backed workspaces-view row-tree builder (H-WS-002 + polish).
//!
//! Top-level rows are `Workspace` nodes. The display string carries
//! the workspace name, an inline `+`-joined member-repo list
//! (mirroring the agent-table workspace column convention from
//! ADR 0060), and a parenthesized provider chip:
//!
//! ```text
//! atelier-ws  conspectus+config+atelier  (atelier)
//! ```
//!
//! Below each workspace the tree lists its (A)-class agent sessions
//! directly — sessions whose `AssociatedWith` target is this
//! workspace, i.e. sessions launched inside the workspace tree
//! (composite directory or a member subdir). There is no labeled
//! `sessions` subgroup wrapper; the workspace's expanded children
//! are the sessions themselves.
//!
//! (B)-class cross-references (sessions in a workspace member's
//! checkout but not workspace-rooted) are intentionally *not*
//! surfaced here — that signal lives only as the `[ws-name]` chip
//! in the Sessions / Graph view (H-WS-001). Surfacing it twice
//! risked the same (A)/(B) conflation H-WS-001 removed.
//!
//! The members subgroup that previously listed every member repo
//! as a row in the left tree is gone — the detail pane's
//! `workspace_member_fields` already emits one `member` field per
//! resolved `WorkspaceContainsRepo`, so dropping it loses no
//! information.
//!
//! v1 ships only `WorkspacesGrouping::Flat`; Provider / Activity /
//! Repo groupings are deferred per
//! `docs/plans/workspace-view-redesign.md` §Axis 2.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{AgentSessionId, NodeId, WorkspaceId};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
};

pub struct WorkspacesBuildInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub home: Option<&'a Path>,
}

pub struct WorkspacesBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
struct WorkspaceSqlRow {
    node_id: String,
    root: String,
    name: Option<String>,
    provider: Option<String>,
}

#[derive(Clone, Debug)]
struct MemberSqlRow {
    /// Basename of the membership link's `logical_path` source field.
    /// Atelier emits `[[repos]].name`; agent-deck the symlink leaf;
    /// generic discovery the immediate child name. Joined into the
    /// inline member list on the workspace row.
    display_name: String,
}

#[derive(Clone, Debug)]
struct AgentSqlRow {
    node_id: String,
    id: AgentSessionId,
    cwd: Option<String>,
    title: Option<String>,
    alias: Option<String>,
    preview: Option<String>,
    last_active_epoch: Option<i64>,
}

pub fn build_workspaces_tree(inputs: WorkspacesBuildInputs<'_>) -> RowTree {
    let conn = crate::query::materialize_snapshot(inputs.snapshot)
        .expect("materialize snapshot for workspaces TUI tree");
    build_workspaces_tree_from_conn(WorkspacesBuildInputsFromConn {
        conn: &conn,
        home: inputs.home,
        now: None,
        filter: RowFilter::default(),
    })
    .expect("build workspaces TUI tree from materialized snapshot")
}

pub fn build_workspaces_tree_from_conn(
    inputs: WorkspacesBuildInputsFromConn<'_>,
) -> rusqlite::Result<RowTree> {
    let workspaces = fetch_workspaces(inputs.conn)?;
    let members_by_ws = fetch_members(inputs.conn)?;
    let agents = fetch_agents(inputs.conn)?;
    let candidate_counts = fetch_agent_mux_candidate_counts(inputs.conn)?;
    let a_class_by_ws = fetch_a_class_sessions(inputs.conn)?;

    let mut node_ids: Vec<String> = agents.iter().map(|a| a.node_id.clone()).collect();
    node_ids.extend(workspaces.iter().map(|ws| ws.node_id.clone()));
    let full_ids: Vec<String> = node_ids
        .iter()
        .map(|id| node_short_id_from_display(id))
        .collect();
    let id_len = unique_prefix_len(&full_ids);
    let short_ids: HashMap<&str, String> = node_ids
        .iter()
        .zip(full_ids.iter())
        .map(|(node_id, full)| (node_id.as_str(), full[..id_len].to_string()))
        .collect();

    let agents_by_node: HashMap<&str, &AgentSqlRow> =
        agents.iter().map(|a| (a.node_id.as_str(), a)).collect();

    let mut tree = RowTree {
        view: ViewLabel::Workspaces,
        ..RowTree::default()
    };

    for ws in &workspaces {
        let members = members_by_ws
            .get(&ws.node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let a_class_ids: &[String] = a_class_by_ws
            .get(&ws.node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let a_class_rows: Vec<&AgentSqlRow> = a_class_ids
            .iter()
            .filter_map(|node_id| agents_by_node.get(node_id.as_str()).copied())
            .filter(|agent| {
                session_matches_filter(agent, &candidate_counts, inputs.now, &inputs.filter)
            })
            .collect();

        let workspace_node_id = NodeId::Workspace(WorkspaceId::new(&ws.root));
        let workspace_label = ws
            .name
            .clone()
            .unwrap_or_else(|| basename(&ws.root).to_string());
        let display_path =
            format_workspace_display(&workspace_label, members, ws.provider.as_deref());

        tree.rows.push(Row {
            id: RowId::Group(workspace_node_id.clone()),
            depth: 0,
            expandable: !a_class_rows.is_empty(),
            kind: RowKind::Group(GroupRow {
                display_path,
                primary_node: Some(workspace_node_id.clone()),
                is_launch_context: false,
            }),
        });

        for agent in a_class_rows {
            tree.rows.push(agent_row(
                &workspace_node_id,
                agent,
                1,
                &candidate_counts,
                short_ids
                    .get(agent.node_id.as_str())
                    .cloned()
                    .unwrap_or_default(),
                inputs.home,
                inputs.now,
            ));
        }
    }

    Ok(tree)
}

/// Assemble the workspace top-row display string:
/// `<name>  <repo-a+repo-b+...>  (<provider>)`. The member list and
/// provider chip are each prefixed with two spaces so the eye can
/// pick out the three slots without a glyph budget. Sections are
/// omitted when their data is missing — a workspace with no members
/// or no provider drops the corresponding segment cleanly.
fn format_workspace_display(
    workspace_label: &str,
    members: &[MemberSqlRow],
    provider: Option<&str>,
) -> String {
    let mut out = workspace_label.to_string();
    if !members.is_empty() {
        let joined: Vec<&str> = members.iter().map(|m| m.display_name.as_str()).collect();
        out.push_str("  ");
        out.push_str(&joined.join("+"));
    }
    if let Some(provider) = provider {
        out.push_str("  (");
        out.push_str(provider);
        out.push(')');
    }
    out
}

fn agent_row(
    workspace: &NodeId,
    agent: &AgentSqlRow,
    depth: u8,
    candidate_counts: &HashMap<String, usize>,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    Row {
        id: RowId::WorkspaceAgentSession {
            workspace: Box::new(workspace.clone()),
            session: node_id.clone(),
        },
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
            pin_id: None,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.last_active_epoch),
            activity_epoch: agent.last_active_epoch,
            mux_state: mux_indicator(candidate_count),
            preview: agent.preview.clone(),
            title: agent.title.clone(),
            alias: agent.alias.clone(),
            primary_node: node_id,
        }),
    }
}

fn mux_indicator(candidate_count: usize) -> MuxIndicator {
    match candidate_count {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    }
}

fn session_matches_filter(
    agent: &AgentSqlRow,
    candidate_counts: &HashMap<String, usize>,
    now: Option<i64>,
    filter: &RowFilter,
) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.id.harness_key,
        now_epoch: now,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

fn fetch_workspaces(conn: &Connection) -> rusqlite::Result<Vec<WorkspaceSqlRow>> {
    let mut stmt = conn
        .prepare("SELECT node_id, root, name, provider_name FROM node_workspaces ORDER BY root")?;
    let rows = stmt.query_map([], |row| {
        Ok(WorkspaceSqlRow {
            node_id: row.get(0)?,
            root: row.get(1)?,
            name: row.get(2)?,
            provider: row.get(3)?,
        })
    })?;
    rows.collect()
}

fn fetch_members(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Vec<MemberSqlRow>>> {
    // Pull resolved workspace_contains_repo selections joined to the
    // selected candidate link's source_fields so the member's display
    // name comes from the link's `logical_path` basename. The polish
    // pass folded the per-member subgroup into an inline `+`-joined
    // list on the workspace row, so the renderer only needs the
    // display name; the `repo_common_dir` and `source_paths`
    // columns the previous version threaded through this query are
    // no longer needed.
    let mut stmt = conn.prepare(
        "SELECT ('workspace:' || json_extract(r.source, '$.root')) AS workspace_node_id, \
                json_extract(r.target, '$.common_dir') AS repo_common_dir, \
                cl.source_fields \
         FROM resolved_relationships r \
         JOIN candidate_links cl ON cl.link_id = r.selected_link_id \
         WHERE r.relation = 'workspace_contains_repo' \
           AND r.source_kind = 'workspace' \
         ORDER BY workspace_node_id, repo_common_dir",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut out: BTreeMap<String, Vec<MemberSqlRow>> = BTreeMap::new();
    for entry in rows {
        let (workspace_node_id, repo_common_dir, source_fields_json) = entry?;
        let display_name = display_name_from_fields(&source_fields_json)
            .unwrap_or_else(|| basename(&repo_common_dir).to_string());
        out.entry(workspace_node_id)
            .or_default()
            .push(MemberSqlRow { display_name });
    }
    Ok(out)
}

fn display_name_from_fields(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let logical_path = value.get("logical_path")?.as_str()?;
    Path::new(logical_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
}

fn fetch_agents(conn: &Connection) -> rusqlite::Result<Vec<AgentSqlRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.node_id, a.harness_key, a.state_scope, a.session_key, \
                a.cwd, a.title, al.display_name, a.last_message_preview, a.last_active_epoch \
         FROM node_agent_sessions a \
         LEFT JOIN aliases al \
           ON al.node_kind = 'agent_session' \
          AND json_extract(al.node, '$.harness_key') = a.harness_key \
          AND json_extract(al.node, '$.state_scope') = a.state_scope \
          AND json_extract(al.node, '$.session_key') = a.session_key \
         ORDER BY a.node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let harness_key: String = row.get(1)?;
        let state_scope: String = row.get(2)?;
        let session_key: String = row.get(3)?;
        Ok(AgentSqlRow {
            node_id: row.get(0)?,
            id: AgentSessionId::new(harness_key, state_scope, session_key),
            cwd: row.get(4)?,
            title: row.get(5)?,
            alias: row.get(6)?,
            preview: row.get(7)?,
            last_active_epoch: row.get(8)?,
        })
    })?;
    rows.collect()
}

fn fetch_a_class_sessions(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT ('workspace:' || json_extract(r.target, '$.root')) AS workspace_node_id, \
                ('agent_session:' || json_extract(r.source, '$.harness_key') || ':' || \
                 json_extract(r.source, '$.state_scope') || ':' || \
                 json_extract(r.source, '$.session_key')) AS agent_node_id \
         FROM resolved_relationships r \
         WHERE r.source_kind = 'agent_session' \
           AND r.target_kind = 'workspace' \
           AND r.relation = 'associated_with' \
         ORDER BY workspace_node_id, agent_node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let (ws, agent) = row?;
        out.entry(ws).or_default().push(agent);
    }
    Ok(out)
}

fn fetch_agent_mux_candidate_counts(conn: &Connection) -> rusqlite::Result<HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT ('agent_session:' || json_extract(source, '$.harness_key') || ':' || \
                 json_extract(source, '$.state_scope') || ':' || \
                 json_extract(source, '$.session_key')) AS agent_node_id, \
                COUNT(*) \
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
    for row in rows {
        let (node_id, count) = row?;
        out.insert(node_id, count);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, Freshness,
        GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, Metadata, Provenance,
        RelationKind, RepoId, RepoNode, SourceMetadata, WorkspaceId, WorkspaceNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    fn workspace_node(root: &str, name: &str, provider: &str) -> GraphNode {
        GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: Some(provider.to_string()),
            name: Some(name.to_string()),
        })
    }

    fn repo_node(common_dir: &str) -> GraphNode {
        GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
    }

    fn checkout_node(common_dir: &str, root: &str) -> GraphNode {
        GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(common_dir), root),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
        })
    }

    fn agent_session(harness: &str, scope: &str, key: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn workspace_contains_repo(
        workspace_root: &str,
        repo_common_dir: &str,
        logical_path: &str,
    ) -> GraphLink {
        let mut fields = Metadata::new();
        fields.insert(
            "logical_path".to_string(),
            serde_json::Value::String(logical_path.to_string()),
        );
        GraphLink {
            id: format!("wcr:{workspace_root}:{repo_common_dir}"),
            source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(RepoId::new(repo_common_dir)),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "test".to_string(),
                evidence: None,
                fields,
            },
            state: LinkState::Active,
        }
    }

    fn associated_with_workspace(session: NodeId, workspace_root: &str) -> GraphLink {
        GraphLink {
            id: format!("assoc-ws:{session}:{workspace_root}"),
            source: session,
            target: LinkEndpoint::Node {
                id: NodeId::Workspace(WorkspaceId::new(workspace_root)),
            },
            relation: RelationKind::AssociatedWith,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn associated_with_checkout(session: NodeId, common_dir: &str, root: &str) -> GraphLink {
        GraphLink {
            id: format!("assoc-co:{session}:{root}"),
            source: session,
            target: LinkEndpoint::Node {
                id: NodeId::Checkout(CheckoutId::new(RepoId::new(common_dir), root)),
            },
            relation: RelationKind::AssociatedWith,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn build(snapshot: &GraphSnapshot) -> RowTree {
        build_workspaces_tree(WorkspacesBuildInputs {
            snapshot,
            home: Some(home().as_path()),
        })
    }

    #[test]
    fn empty_snapshot_produces_empty_tree() {
        let snapshot = resolve_snapshot(GraphSnapshot::empty());
        let tree = build(&snapshot);
        assert_eq!(tree.view, ViewLabel::Workspaces);
        assert!(tree.rows.is_empty());
    }

    #[test]
    fn workspace_with_members_renders_inline_member_list() {
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/atelier", "atelier-ws", "atelier"),
                repo_node("/home/op/atelier/conspectus/.git"),
                repo_node("/home/op/atelier/config/.git"),
            ],
            candidate_links: vec![
                workspace_contains_repo(
                    "/home/op/atelier",
                    "/home/op/atelier/conspectus/.git",
                    "/home/op/atelier/conspectus",
                ),
                workspace_contains_repo(
                    "/home/op/atelier",
                    "/home/op/atelier/config/.git",
                    "/home/op/atelier/config",
                ),
            ],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);

        // Just the workspace row — no members subgroup, no repo rows.
        assert_eq!(tree.rows.len(), 1, "got:\n{:#?}", tree.rows);

        let ws_row = &tree.rows[0];
        assert_eq!(ws_row.depth, 0);
        assert!(
            !ws_row.expandable,
            "workspace with members but no sessions has nothing to expand",
        );
        match &ws_row.kind {
            RowKind::Group(g) => {
                // Member list is alphabetical by `repo_common_dir`
                // (the SQL `ORDER BY`); `config` < `conspectus`.
                assert_eq!(g.display_path, "atelier-ws  config+conspectus  (atelier)");
                assert!(matches!(g.primary_node, Some(NodeId::Workspace(_))));
            }
            _ => panic!("expected Group row"),
        }
    }

    #[test]
    fn workspace_without_members_omits_member_segment() {
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![workspace_node("/home/op/atelier", "atelier-ws", "atelier")],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);
        assert_eq!(tree.rows.len(), 1);
        match &tree.rows[0].kind {
            RowKind::Group(g) => assert_eq!(g.display_path, "atelier-ws  (atelier)"),
            _ => panic!("expected Group row"),
        }
    }

    #[test]
    fn a_class_session_appears_directly_under_workspace() {
        // No labeled subgroup wrapper: a workspace's A-class sessions
        // sit at depth 1 immediately beneath the workspace row.
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "active"));
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/atelier", "atelier-ws", "atelier"),
                agent_session("codex", "/state", "active", Some("/home/op/atelier")),
            ],
            candidate_links: vec![associated_with_workspace(session_id, "/home/op/atelier")],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);

        assert!(
            !tree
                .rows
                .iter()
                .any(|r| matches!(&r.id, RowId::Subgroup { .. })),
            "polish dropped all subgroup wrappers from the workspaces view:\n{:#?}",
            tree.rows,
        );

        let ws_row = &tree.rows[0];
        assert_eq!(ws_row.depth, 0);
        assert!(
            ws_row.expandable,
            "workspace with sessions must be expandable"
        );

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::AgentSession(_)))
            .expect("agent session row");
        assert_eq!(session_row.depth, 1);
        assert!(
            matches!(&session_row.id, RowId::WorkspaceAgentSession { .. }),
            "workspaces-view session rows must use the scoped RowId variant",
        );
    }

    #[test]
    fn b_class_session_is_not_surfaced_in_workspaces_view() {
        // Session lives at the canonical repo path (outside the
        // workspace tree) — the (B) shape the chip surfaces in
        // Sessions/Graph. The Workspaces view intentionally does not
        // mirror that cross-reference: only (A)-class sessions appear
        // here. See ADR 0062 / the H-WS-002 polish notes for the
        // narrowing rationale.
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "shared"));
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/atelier", "atelier-ws", "atelier"),
                repo_node("/home/op/src/conspectus/.git"),
                checkout_node("/home/op/src/conspectus/.git", "/home/op/src/conspectus"),
                agent_session(
                    "codex",
                    "/state",
                    "shared",
                    Some("/home/op/src/conspectus/foo"),
                ),
            ],
            candidate_links: vec![
                workspace_contains_repo(
                    "/home/op/atelier",
                    "/home/op/src/conspectus/.git",
                    "/home/op/atelier/conspectus",
                ),
                associated_with_checkout(
                    session_id,
                    "/home/op/src/conspectus/.git",
                    "/home/op/src/conspectus",
                ),
            ],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);

        assert!(
            !tree
                .rows
                .iter()
                .any(|r| matches!(&r.kind, RowKind::AgentSession(_))),
            "(B)-class session must not appear in the Workspaces view:\n{:#?}",
            tree.rows,
        );
        let ws_row = tree.rows.first().expect("workspace row");
        assert!(
            !ws_row.expandable,
            "workspace with only a (B)-class session has nothing to expand",
        );
    }

    #[test]
    fn member_display_name_uses_logical_path_basename() {
        // The inline member list reads names from the
        // WorkspaceContainsRepo link's `logical_path` source field
        // (basename) — that's the workspace-visible directory name
        // (atelier `[[repos]].name`, agent-deck symlink leaf), not
        // the repo's common_dir basename. Verifies the polish's
        // `display_name_from_fields` path still feeds the row.
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/atelier", "atelier-ws", "atelier"),
                repo_node("/home/op/repo/conspectus.git"),
            ],
            candidate_links: vec![workspace_contains_repo(
                "/home/op/atelier",
                "/home/op/repo/conspectus.git",
                "/home/op/atelier/conspectus-pinned",
            )],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);
        match &tree.rows[0].kind {
            RowKind::Group(g) => {
                assert!(
                    g.display_path.contains("conspectus-pinned"),
                    "member display should use logical_path basename, got `{}`",
                    g.display_path,
                );
            }
            _ => panic!("expected Group row"),
        }
    }
}
