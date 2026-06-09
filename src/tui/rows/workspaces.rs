//! SQLite-backed workspaces-view row-tree builder (H-WS-002 MVP).
//!
//! Top-level rows are `Workspace` nodes; each one expands to up to
//! three labeled subgroups, in order:
//!
//! 1. **members** — the workspace's resolved `WorkspaceContainsRepo`
//!    member repos. Each row is a `RepoRow` styled to match the
//!    session / mux row rhythm (short id + `repo` chip + bold name +
//!    dim canonical path) and carrying the repo as its
//!    `primary_node` so the detail pane and left-tree navigation
//!    follow naturally.
//! 2. **in workspace** — (A)-class sessions whose `AssociatedWith`
//!    target is this workspace directly. The session was launched
//!    inside the workspace tree (composite directory or a member
//!    subdir). Rows are full `AgentSessionRow`s.
//! 3. **related** — (B)-class sessions: their cwd is inside a member
//!    repo's checkout, but they carry no direct `AssociatedWith
//!    Workspace` edge to *this* workspace. These are the
//!    cross-reference rows the (B) chip in the Sessions view points
//!    at. Rows are full `AgentSessionRow`s.
//!
//! Sessions that are (A)-class for this workspace are *excluded*
//! from the related subgroup so a single workspace+session pair
//! shows up exactly once.
//!
//! v1 ships only `WorkspacesGrouping::Flat`; Provider / Activity /
//! Repo groupings are deferred per
//! `docs/plans/workspace-view-redesign.md` §Axis 2.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{AgentSessionId, NodeId, RepoId, WorkspaceId};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, RepoRow, Row, RowId, RowKind, RowTree, ViewLabel,
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
    repo_node_id: String,
    repo_common_dir: String,
    /// Basename of the membership link's `logical_path` source field.
    /// Atelier emits `[[repos]].name`; agent-deck the symlink leaf;
    /// generic discovery the immediate child name. The label
    /// presented in the row.
    display_name: String,
    /// Operator-recognizable canonical path. First non-agent-deck
    /// `source_paths` entry on the `Repo` node, falling back to
    /// `common_dir` when no source path is recorded. `None` means
    /// the renderer should suppress the trailing path column rather
    /// than print the bare `common_dir`.
    canonical_path: Option<String>,
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
    let b_class_by_ws = fetch_b_class_sessions(inputs.conn)?;

    let mut node_ids: Vec<String> = agents.iter().map(|a| a.node_id.clone()).collect();
    node_ids.extend(workspaces.iter().map(|ws| ws.node_id.clone()));
    node_ids.extend(
        members_by_ws
            .values()
            .flat_map(|members| members.iter().map(|m| m.repo_node_id.clone())),
    );
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
        let a_class_set: HashSet<&str> = a_class_ids.iter().map(String::as_str).collect();
        let b_class_ids: Vec<&AgentSqlRow> = b_class_by_ws
            .get(&ws.node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter(|node_id| !a_class_set.contains(node_id.as_str()))
            .filter_map(|node_id| agents_by_node.get(node_id.as_str()).copied())
            .filter(|agent| {
                session_matches_filter(agent, &candidate_counts, inputs.now, &inputs.filter)
            })
            .collect();
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
        let provider_chip = ws
            .provider
            .as_deref()
            .map(|p| format!("  ({p})"))
            .unwrap_or_default();

        let has_any_children =
            !members.is_empty() || !a_class_rows.is_empty() || !b_class_ids.is_empty();
        tree.rows.push(Row {
            id: RowId::Group(workspace_node_id.clone()),
            depth: 0,
            expandable: has_any_children,
            kind: RowKind::Group(GroupRow {
                display_path: format!("{workspace_label}{provider_chip}"),
                primary_node: Some(workspace_node_id.clone()),
                is_launch_context: false,
            }),
        });

        if !members.is_empty() {
            tree.rows.push(Row {
                id: RowId::Subgroup {
                    parent: workspace_node_id.clone(),
                    label: "members",
                },
                depth: 1,
                expandable: true,
                kind: RowKind::Group(GroupRow {
                    display_path: format!("members ({})", members.len()),
                    primary_node: None,
                    is_launch_context: false,
                }),
            });
            for member in members {
                let repo_node_id = NodeId::Repo(RepoId::new(&member.repo_common_dir));
                let short_id = short_ids
                    .get(member.repo_node_id.as_str())
                    .cloned()
                    .unwrap_or_default();
                tree.rows.push(Row {
                    id: RowId::Repo {
                        workspace: Box::new(workspace_node_id.clone()),
                        repo: repo_node_id.clone(),
                    },
                    depth: 2,
                    expandable: false,
                    kind: RowKind::Repo(RepoRow {
                        short_id,
                        display_name: member.display_name.clone(),
                        canonical_path: member.canonical_path.clone(),
                        common_dir: member.repo_common_dir.clone(),
                        primary_node: repo_node_id,
                    }),
                });
            }
        }

        if !a_class_rows.is_empty() {
            tree.rows.push(Row {
                id: RowId::Subgroup {
                    parent: workspace_node_id.clone(),
                    label: "in workspace",
                },
                depth: 1,
                expandable: true,
                kind: RowKind::Group(GroupRow {
                    display_path: format!("in workspace ({})", a_class_rows.len()),
                    primary_node: None,
                    is_launch_context: false,
                }),
            });
            for agent in a_class_rows {
                tree.rows.push(agent_row(
                    agent,
                    2,
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

        if !b_class_ids.is_empty() {
            tree.rows.push(Row {
                id: RowId::Subgroup {
                    parent: workspace_node_id.clone(),
                    label: "related",
                },
                depth: 1,
                expandable: true,
                kind: RowKind::Group(GroupRow {
                    display_path: format!("related ({})", b_class_ids.len()),
                    primary_node: None,
                    is_launch_context: false,
                }),
            });
            for agent in b_class_ids {
                tree.rows.push(agent_row(
                    agent,
                    2,
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
    }

    Ok(tree)
}

fn agent_row(
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
        id: RowId::AgentSession(node_id.clone()),
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
            // H-WS-001: chip is Sessions-view-specific; surfacing it
            // again under the workspaces view would be redundant
            // (the workspace context is already the row's parent).
            workspace_chip: None,
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
    // selected candidate link's source_fields so we can render the
    // member's display name from the link's `logical_path` basename.
    // Left-joined to node_repos so we can surface the operator-visible
    // canonical path (first non-agent-deck `source_paths` entry) on
    // the row.
    let mut stmt = conn.prepare(
        "SELECT ('workspace:' || json_extract(r.source, '$.root')) AS workspace_node_id, \
                ('repo:' || json_extract(r.target, '$.common_dir')) AS repo_node_id, \
                json_extract(r.target, '$.common_dir') AS repo_common_dir, \
                cl.source_fields, \
                nr.source_paths AS repo_source_paths \
         FROM resolved_relationships r \
         JOIN candidate_links cl ON cl.link_id = r.selected_link_id \
         LEFT JOIN node_repos nr ON nr.node_id = \
              ('repo:' || json_extract(r.target, '$.common_dir')) \
         WHERE r.relation = 'workspace_contains_repo' \
           AND r.source_kind = 'workspace' \
         ORDER BY workspace_node_id, repo_node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let workspace_node_id: String = row.get(0)?;
        let repo_node_id: String = row.get(1)?;
        let repo_common_dir: String = row.get(2)?;
        let source_fields: String = row.get(3)?;
        let repo_source_paths: Option<String> = row.get(4)?;
        Ok((
            workspace_node_id,
            repo_node_id,
            repo_common_dir,
            source_fields,
            repo_source_paths,
        ))
    })?;

    let mut out: BTreeMap<String, Vec<MemberSqlRow>> = BTreeMap::new();
    for entry in rows {
        let (
            workspace_node_id,
            repo_node_id,
            repo_common_dir,
            source_fields_json,
            source_paths_json,
        ) = entry?;
        let display_name = display_name_from_fields(&source_fields_json)
            .unwrap_or_else(|| basename(&repo_common_dir).to_string());
        let canonical_path = canonical_path_from_source_paths(source_paths_json.as_deref());
        out.entry(workspace_node_id)
            .or_default()
            .push(MemberSqlRow {
                repo_node_id,
                repo_common_dir,
                display_name,
                canonical_path,
            });
    }
    Ok(out)
}

/// Pick the operator-recognizable canonical path from a repo's
/// `source_paths` JSON array. Prefers the first entry that does *not*
/// pass through `~/.agent-deck/multi-repo-worktrees/` because those
/// paths are symlink composites — the operator wants to see
/// `~/src/conspectus`, not `~/.agent-deck/.../conspectus`. Returns
/// `None` if the array is missing, empty, or all paths are
/// agent-deck composites.
fn canonical_path_from_source_paths(json: Option<&str>) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json?).ok()?;
    let array = value.as_array()?;
    let mut fallback: Option<&str> = None;
    for entry in array {
        let path = entry.as_str()?;
        if path.contains("/.agent-deck/multi-repo-worktrees/") {
            fallback.get_or_insert(path);
            continue;
        }
        return Some(path.to_string());
    }
    fallback.map(str::to_string)
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

fn fetch_b_class_sessions(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Vec<String>>> {
    // Walk agent → checkout → repo → workspace via resolved
    // relationships. Members live on the workspace side via
    // workspace_contains_repo; sessions live on the checkout side
    // via associated_with. A row in this query is one
    // (workspace, agent_session) pair where the session's checkout
    // belongs to a member repo of the workspace.
    //
    // (A)-class sessions for the same workspace are not filtered out
    // here — the caller subtracts them so this query stays a simple
    // multi-hop join.
    // Match the session's checkout target back to the workspace via
    // the checkout's `repo.common_dir` (embedded directly in the
    // Checkout NodeId JSON) compared against the workspace member's
    // target `common_dir`. No need to round-trip through
    // `node_checkouts` — the resolved row already carries the repo
    // path in the target JSON.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT \
                ('workspace:' || json_extract(wcr.source, '$.root')) AS workspace_node_id, \
                ('agent_session:' || json_extract(rel.source, '$.harness_key') || ':' || \
                 json_extract(rel.source, '$.state_scope') || ':' || \
                 json_extract(rel.source, '$.session_key')) AS agent_node_id \
         FROM resolved_relationships rel \
         JOIN resolved_relationships wcr \
              ON wcr.relation = 'workspace_contains_repo' \
              AND wcr.source_kind = 'workspace' \
              AND json_extract(wcr.target, '$.common_dir') = \
                  json_extract(rel.target, '$.repo.common_dir') \
         WHERE rel.source_kind = 'agent_session' \
           AND rel.target_kind = 'checkout' \
           AND rel.relation = 'associated_with' \
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
    fn workspace_with_members_renders_workspace_and_members_subgroup() {
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

        // workspace row + members subgroup row + 2 repo rows
        assert_eq!(tree.rows.len(), 4, "got:\n{:#?}", tree.rows);

        let ws_row = &tree.rows[0];
        assert_eq!(ws_row.depth, 0);
        match &ws_row.kind {
            RowKind::Group(g) => {
                assert!(g.display_path.contains("atelier-ws"));
                assert!(g.display_path.contains("(atelier)"));
                assert!(matches!(g.primary_node, Some(NodeId::Workspace(_))));
            }
            _ => panic!("expected Group row"),
        }

        let members_row = &tree.rows[1];
        assert_eq!(members_row.depth, 1);
        match &members_row.kind {
            RowKind::Group(g) => {
                assert_eq!(g.display_path, "members (2)");
                assert!(g.primary_node.is_none());
            }
            _ => panic!("expected Group row"),
        }
        assert!(matches!(
            members_row.id,
            RowId::Subgroup {
                label: "members",
                ..
            }
        ));

        // The two repo rows are styled like sessions/muxes via the
        // dedicated Repo row kind, carrying a Repo NodeId so the
        // detail pane and left-tree navigation follow naturally.
        for repo_row in &tree.rows[2..4] {
            assert_eq!(repo_row.depth, 2);
            match &repo_row.kind {
                RowKind::Repo(r) => {
                    assert!(matches!(r.primary_node, NodeId::Repo(_)));
                    assert!(!r.display_name.is_empty());
                    assert!(!r.common_dir.is_empty());
                }
                _ => panic!("expected Repo row, got {:?}", repo_row.kind),
            }
            assert!(matches!(repo_row.id, RowId::Repo { .. }));
        }
    }

    #[test]
    fn same_repo_in_two_workspaces_gets_distinct_row_ids() {
        // Regression: a single Repo NodeId can be a member of multiple
        // workspaces. The Row id keys on (workspace, repo) so the
        // selection state machine and renderer can tell duplicate
        // entries apart — keying on the repo alone collides and
        // every duplicate row highlights when one is selected.
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/ws-a", "ws-a", "agent-deck"),
                workspace_node("/home/op/ws-b", "ws-b", "agent-deck"),
                repo_node("/home/op/src/conspectus/.git"),
            ],
            candidate_links: vec![
                workspace_contains_repo(
                    "/home/op/ws-a",
                    "/home/op/src/conspectus/.git",
                    "/home/op/ws-a/conspectus",
                ),
                workspace_contains_repo(
                    "/home/op/ws-b",
                    "/home/op/src/conspectus/.git",
                    "/home/op/ws-b/conspectus",
                ),
            ],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);
        let repo_row_ids: Vec<&RowId> = tree
            .rows
            .iter()
            .filter(|r| matches!(&r.kind, RowKind::Repo(_)))
            .map(|r| &r.id)
            .collect();
        assert_eq!(repo_row_ids.len(), 2, "one row per (workspace, repo) pair");
        assert_ne!(
            repo_row_ids[0], repo_row_ids[1],
            "duplicate-repo rows must carry distinct RowIds so selection picks exactly one",
        );
    }

    fn repo_node_with_source(common_dir: &str, source_path: &str) -> GraphNode {
        let mut repo = RepoNode::new(RepoId::new(common_dir));
        repo.source_paths.push(source_path.to_string());
        GraphNode::Repo(repo)
    }

    #[test]
    fn repo_row_surfaces_canonical_source_path() {
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node(
                    "/home/op/.agent-deck/multi-repo-worktrees/abc",
                    "abc",
                    "agent-deck",
                ),
                repo_node_with_source("/home/op/src/conspectus/.git", "/home/op/src/conspectus"),
            ],
            candidate_links: vec![workspace_contains_repo(
                "/home/op/.agent-deck/multi-repo-worktrees/abc",
                "/home/op/src/conspectus/.git",
                "/home/op/.agent-deck/multi-repo-worktrees/abc/conspectus",
            )],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);
        let repo_row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Repo(repo) => Some(repo),
                _ => None,
            })
            .expect("repo row");
        assert_eq!(repo_row.display_name, "conspectus");
        assert_eq!(
            repo_row.canonical_path.as_deref(),
            Some("/home/op/src/conspectus"),
            "canonical_path should prefer the non-agent-deck source_paths entry",
        );
    }

    #[test]
    fn workspace_with_a_class_session_renders_in_workspace_subgroup() {
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
        let in_ws_row = tree
            .rows
            .iter()
            .find(|r| {
                matches!(
                    &r.id,
                    RowId::Subgroup {
                        label: "in workspace",
                        ..
                    }
                )
            })
            .expect("in workspace subgroup row");
        match &in_ws_row.kind {
            RowKind::Group(g) => assert_eq!(g.display_path, "in workspace (1)"),
            _ => panic!("expected Group row"),
        }
        // session row should follow the subgroup at depth 2
        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::AgentSession(_)))
            .expect("agent session row");
        assert_eq!(session_row.depth, 2);
    }

    #[test]
    fn workspace_with_b_class_session_renders_related_subgroup() {
        // Session lives at the canonical repo path (outside the workspace
        // tree) — exactly the (B) shape the chip surfaces in Sessions/Graph.
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

        let related_row = tree
            .rows
            .iter()
            .find(|r| {
                matches!(
                    &r.id,
                    RowId::Subgroup {
                        label: "related",
                        ..
                    }
                )
            })
            .expect("related subgroup row");
        match &related_row.kind {
            RowKind::Group(g) => assert_eq!(g.display_path, "related (1)"),
            _ => panic!("expected Group row"),
        }
        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::AgentSession(_)))
            .expect("agent session row");
        assert_eq!(session_row.depth, 2);
    }

    #[test]
    fn a_class_session_excluded_from_related_subgroup() {
        // Session is BOTH (A)-class (AssociatedWith workspace) AND
        // happens to have an AssociatedWith checkout in a workspace
        // member repo. It must show up in "in workspace" only —
        // appearing twice would be confusing.
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "active"));
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node("/home/op/atelier", "atelier-ws", "atelier"),
                repo_node("/home/op/atelier/conspectus/.git"),
                checkout_node(
                    "/home/op/atelier/conspectus/.git",
                    "/home/op/atelier/conspectus",
                ),
                agent_session(
                    "codex",
                    "/state",
                    "active",
                    Some("/home/op/atelier/conspectus"),
                ),
            ],
            candidate_links: vec![
                workspace_contains_repo(
                    "/home/op/atelier",
                    "/home/op/atelier/conspectus/.git",
                    "/home/op/atelier/conspectus",
                ),
                associated_with_workspace(session_id.clone(), "/home/op/atelier"),
                associated_with_checkout(
                    session_id,
                    "/home/op/atelier/conspectus/.git",
                    "/home/op/atelier/conspectus",
                ),
            ],
            ..GraphSnapshot::empty()
        });

        let tree = build(&snapshot);

        assert!(
            tree.rows.iter().any(|r| matches!(
                &r.id,
                RowId::Subgroup {
                    label: "in workspace",
                    ..
                }
            )),
            "in workspace subgroup expected:\n{:#?}",
            tree.rows
        );
        assert!(
            !tree.rows.iter().any(|r| matches!(
                &r.id,
                RowId::Subgroup {
                    label: "related",
                    ..
                }
            )),
            "related subgroup should be suppressed when the (A)-class session also touches the member's checkout:\n{:#?}",
            tree.rows
        );
        let session_rows = tree
            .rows
            .iter()
            .filter(|r| matches!(&r.kind, RowKind::AgentSession(_)))
            .count();
        assert_eq!(
            session_rows, 1,
            "the same session must not appear twice in one workspace block",
        );
    }
}
