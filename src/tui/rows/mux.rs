//! SQLite-backed mux-view row-tree builder.
//!
//! The mux view is mux-session oriented: one row per mux session, with
//! compact metrics about known attached agent sessions. The common
//! zero-or-one-agent case stays flat; muxes linked to multiple agents expose
//! those agents as child rows for drill-down.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{
    AgentSessionId, CheckoutId, GraphNode, GraphSnapshot, MuxSessionId, NodeId, RepoId,
    path_is_ancestor_of,
};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::MuxGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxIndicator, MuxSessionRow, Row, RowId, RowKind, RowTree,
    ViewLabel, format_recency, harness_label, shorten_home,
};

pub struct MuxBuildInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub home: Option<&'a Path>,
    pub filter: RowFilter,
    pub grouping: MuxGrouping,
}

pub struct MuxBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
    pub grouping: MuxGrouping,
}

#[derive(Clone, Debug)]
struct MuxSqlRow {
    node_id: String,
    id: MuxSessionId,
    backend: String,
    native_id: String,
    client_attached: Option<bool>,
    cwd: Option<String>,
    active_pane_current_path: Option<String>,
    activity_epoch: Option<i64>,
}

#[derive(Clone, Debug)]
struct AttachedAgent {
    node_id: String,
    id: AgentSessionId,
    harness_key: String,
    cwd: Option<String>,
    title: Option<String>,
    alias: Option<String>,
    preview: Option<String>,
    last_active_epoch: Option<i64>,
    candidate_count: usize,
}

pub fn build_mux_tree(inputs: MuxBuildInputs<'_>) -> RowTree {
    let conn = crate::query::materialize_snapshot(inputs.snapshot)
        .expect("materialize snapshot for mux TUI tree");
    build_mux_tree_from_conn(MuxBuildInputsFromConn {
        conn: &conn,
        home: inputs.home,
        now: None,
        filter: inputs.filter,
        grouping: inputs.grouping,
    })
    .expect("build mux TUI tree from materialized snapshot")
}

pub fn build_mux_tree_from_conn(inputs: MuxBuildInputsFromConn<'_>) -> rusqlite::Result<RowTree> {
    let muxes = fetch_muxes(inputs.conn)?;
    let attachments = fetch_attached_agents(inputs.conn)?;

    let mut node_ids: Vec<String> = muxes.iter().map(|mux| mux.node_id.clone()).collect();
    for attached in attachments.values() {
        node_ids.extend(attached.iter().map(|agent| agent.node_id.clone()));
    }
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

    let mut tree = RowTree {
        view: ViewLabel::Mux,
        ..RowTree::default()
    };

    // Build each visible mux as a (parent, children) group so the
    // grouping phase can keep parents and their child agent rows
    // adjacent and the optional float-top sort can move them as a
    // unit.
    let mut groups: Vec<MuxGroup> = Vec::new();
    for mux in &muxes {
        let attached = attachments.get(&mux.node_id).cloned().unwrap_or_default();
        let visible_attached: Vec<AttachedAgent> = attached
            .iter()
            .filter(|agent| agent_matches_filter(agent, inputs.now, &inputs.filter))
            .cloned()
            .collect();

        if !mux_matches_filter(&attached, &visible_attached, &inputs.filter) {
            continue;
        }

        let latest_agent_epoch = visible_attached
            .iter()
            .filter_map(|agent| agent.last_active_epoch)
            .max();
        let activity_epoch = latest_epoch(mux.activity_epoch, latest_agent_epoch);
        let ambiguous_count = visible_attached
            .iter()
            .filter(|agent| agent.candidate_count > 1)
            .count();
        let node_id = NodeId::MuxSession(mux.id.clone());
        let single_session_preview = if visible_attached.len() == 1 {
            visible_attached[0]
                .preview
                .clone()
                .filter(|preview| !preview.is_empty())
        } else {
            None
        };
        let attached_count = visible_attached.len();
        let cwd = mux.effective_cwd().map(|s| s.to_string());

        let parent_row = MuxSessionRow {
            mux: mux.id.clone(),
            backend: mux.backend.clone(),
            native_id: mux.native_id.clone(),
            client_attached: mux.client_attached,
            cwd_display: cwd.as_deref().map(|cwd| shorten_home(cwd, inputs.home)),
            attached_count,
            ambiguous_count,
            recency: format_recency(inputs.now, activity_epoch),
            activity_epoch,
            agent_labels: agent_labels(&visible_attached),
            single_session_preview,
            primary_node: node_id.clone(),
        };

        let children: Vec<AttachedAgent> = if attached_count > 1 {
            visible_attached.clone()
        } else {
            Vec::new()
        };

        groups.push(MuxGroup {
            parent_row,
            parent_node_id: node_id,
            children,
            attached_count,
            cwd,
        });
    }

    match inputs.grouping {
        MuxGrouping::Session | MuxGrouping::Workspace | MuxGrouping::Host => {
            emit_flat(&mut tree, groups, &inputs, &short_ids);
        }
        MuxGrouping::Repo => {
            let snapshot = crate::query::read_snapshot(inputs.conn)?;
            let path_index = PathIndex::from_snapshot(&snapshot);
            emit_repo_grouped(&mut tree, groups, &inputs, &short_ids, &path_index);
        }
    }

    Ok(tree)
}

/// Per-mux work product collected by the build loop. Holds enough
/// data for the emit phase to push the parent and its child agent
/// rows at any depth, in any order.
#[derive(Clone, Debug)]
struct MuxGroup {
    parent_row: MuxSessionRow,
    parent_node_id: NodeId,
    children: Vec<AttachedAgent>,
    attached_count: usize,
    /// Effective working directory used for repo grouping. `None`
    /// when neither the active-pane path nor the session cwd resolved.
    cwd: Option<String>,
}

impl MuxGroup {
    fn push_into(
        self,
        tree: &mut RowTree,
        depth: u8,
        short_ids: &HashMap<&str, String>,
        inputs: &MuxBuildInputsFromConn<'_>,
    ) {
        let MuxGroup {
            parent_row,
            parent_node_id,
            children,
            attached_count,
            cwd: _,
        } = self;
        tree.rows.push(Row {
            id: RowId::MuxSession(parent_node_id),
            depth,
            expandable: attached_count > 1,
            kind: RowKind::MuxSession(parent_row),
        });
        let child_depth = depth.saturating_add(1);
        for agent in children {
            tree.rows.push(agent_row(
                &agent,
                child_depth,
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

fn emit_flat(
    tree: &mut RowTree,
    mut groups: Vec<MuxGroup>,
    inputs: &MuxBuildInputsFromConn<'_>,
    short_ids: &HashMap<&str, String>,
) {
    if inputs.filter.float_attached_muxes_top {
        // Stable so the SQL `ORDER BY node_id` baseline is preserved
        // inside each of the two resulting halves.
        groups.sort_by_key(|group| usize::from(group.attached_count == 0));
    }
    for group in groups {
        group.push_into(tree, 0, short_ids, inputs);
    }
}

fn emit_repo_grouped(
    tree: &mut RowTree,
    groups: Vec<MuxGroup>,
    inputs: &MuxBuildInputsFromConn<'_>,
    short_ids: &HashMap<&str, String>,
    path_index: &PathIndex<'_>,
) {
    let mut buckets: BTreeMap<RepoBucketKey, Vec<MuxGroup>> = BTreeMap::new();
    let mut ungrouped: Vec<MuxGroup> = Vec::new();

    for group in groups {
        let resolved = group
            .cwd
            .as_deref()
            .and_then(|cwd| path_index.checkout_for_path(Path::new(cwd)));
        match resolved {
            Some(checkout_id) => {
                let key = RepoBucketKey {
                    repo_id: checkout_id.repo.clone(),
                    display_path: path_index.repo_display_path(&checkout_id.repo),
                };
                buckets.entry(key).or_default().push(group);
            }
            None => ungrouped.push(group),
        }
    }

    let float = inputs.filter.float_attached_muxes_top;

    for (key, mut groups) in buckets {
        let RepoBucketKey {
            repo_id,
            display_path,
        } = key;
        push_repo_header(tree, 0, &repo_id, &display_path, inputs.home);
        if float {
            groups.sort_by_key(|group| usize::from(group.attached_count == 0));
        }
        for group in groups {
            group.push_into(tree, 1, short_ids, inputs);
        }
    }

    if !ungrouped.is_empty() {
        tree.rows.push(Row {
            id: RowId::Synthetic("ungrouped"),
            depth: 0,
            expandable: true,
            kind: RowKind::Group(GroupRow {
                display_path: "Ungrouped".to_string(),
                primary_node: None,
                is_launch_context: false,
            }),
        });
        if float {
            ungrouped.sort_by_key(|group| usize::from(group.attached_count == 0));
        }
        for group in ungrouped {
            group.push_into(tree, 1, short_ids, inputs);
        }
    }
}

fn push_repo_header(
    tree: &mut RowTree,
    depth: u8,
    repo_id: &RepoId,
    display_path: &str,
    home: Option<&Path>,
) {
    let node_id = NodeId::Repo(repo_id.clone());
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(display_path, home),
            primary_node: Some(node_id),
            is_launch_context: false,
        }),
    });
}

/// Bucket key for repo grouping. Sorted by display path so the
/// rendered order is stable and human-meaningful.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoBucketKey {
    repo_id: RepoId,
    display_path: String,
}

impl Ord for RepoBucketKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.display_path
            .cmp(&other.display_path)
            .then_with(|| self.repo_id.common_dir.cmp(&other.repo_id.common_dir))
    }
}

impl PartialOrd for RepoBucketKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Path → (repo, checkout) lookup built once per repo-grouped build.
/// Uses the snapshot's checkout and repo nodes directly so the mux
/// builder can mirror sessions-view repo grouping without depending
/// on `SessionsData`.
struct PathIndex<'a> {
    snapshot: &'a GraphSnapshot,
    checkouts: Vec<(PathBuf, CheckoutId)>,
}

impl<'a> PathIndex<'a> {
    fn from_snapshot(snapshot: &'a GraphSnapshot) -> Self {
        let mut checkouts = Vec::new();
        for node in &snapshot.nodes {
            if let GraphNode::Checkout(c) = node {
                checkouts.push((PathBuf::from(&c.id.root), c.id.clone()));
            }
        }
        Self {
            snapshot,
            checkouts,
        }
    }

    fn checkout_for_path(&self, path: &Path) -> Option<&CheckoutId> {
        self.checkouts
            .iter()
            .filter(|(root, _)| path_is_ancestor_of(root, path))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, id)| id)
    }

    fn repo_display_path(&self, repo: &RepoId) -> String {
        let common_dir = repo
            .common_dir
            .strip_suffix("/.git")
            .unwrap_or(&repo.common_dir)
            .to_string();
        for node in &self.snapshot.nodes {
            if let GraphNode::Repo(r) = node
                && r.id == *repo
                && let Some(path) = r
                    .source_paths
                    .iter()
                    .find(|p| !p.contains("/.agent-deck/multi-repo-worktrees/"))
            {
                return path.clone();
            }
        }
        common_dir
    }
}

fn latest_epoch(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

impl MuxSqlRow {
    fn effective_cwd(&self) -> Option<&str> {
        self.active_pane_current_path
            .as_deref()
            .or(self.cwd.as_deref())
    }
}

fn agent_labels(agents: &[AttachedAgent]) -> Vec<String> {
    let mut labels: Vec<String> = agents
        .iter()
        .map(|agent| harness_label(&agent.harness_key))
        .collect();
    labels.sort();
    labels.dedup();
    labels
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

fn agent_matches_filter(agent: &AttachedAgent, now: Option<i64>, filter: &RowFilter) -> bool {
    if !filter.has_narrowing_predicates() {
        return true;
    }
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.harness_key,
        now_epoch: now,
        last_active_epoch: agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(agent.candidate_count),
    })
}

fn fetch_muxes(conn: &Connection) -> rusqlite::Result<Vec<MuxSqlRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, native_id, backend, client_attached, cwd, \
                active_pane_current_path, activity_epoch \
         FROM node_mux_sessions \
         ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let node_id: String = row.get(0)?;
        let id_native = node_id
            .strip_prefix("mux_session:")
            .unwrap_or(&node_id)
            .to_string();
        Ok(MuxSqlRow {
            node_id,
            id: MuxSessionId::new(id_native),
            native_id: row.get(1)?,
            backend: row.get(2)?,
            client_attached: row.get::<_, Option<i64>>(3)?.map(|value| value != 0),
            cwd: row.get(4)?,
            active_pane_current_path: row.get(5)?,
            activity_epoch: row.get(6)?,
        })
    })?;
    rows.collect()
}

fn fetch_attached_agents(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, Vec<AttachedAgent>>> {
    let candidate_counts = fetch_agent_mux_candidate_counts(conn)?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT \
                ('mux_session:' || json_extract(cl.target_node, '$.native_id')) AS mux_node_id, \
                a.node_id, a.harness_key, a.state_scope, a.session_key, a.cwd, a.title, \
                al.display_name, a.last_message_preview, a.last_active_epoch \
         FROM candidate_links cl \
         JOIN node_agent_sessions a \
           ON cl.source_kind = 'agent_session' \
          AND ('agent_session:' || json_extract(cl.source, '$.harness_key') || ':' || \
               json_extract(cl.source, '$.state_scope') || ':' || \
               json_extract(cl.source, '$.session_key')) = a.node_id \
         LEFT JOIN aliases al \
           ON al.node_kind = 'agent_session' \
          AND json_extract(al.node, '$.harness_key') = a.harness_key \
          AND json_extract(al.node, '$.state_scope') = a.state_scope \
          AND json_extract(al.node, '$.session_key') = a.session_key \
         WHERE cl.target_node_kind = 'mux_session' \
           AND cl.relation = 'linked_to_mux' \
           AND cl.state = 'active' \
         ORDER BY mux_node_id, a.node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let mux_node_id: String = row.get(0)?;
        let agent_node_id: String = row.get(1)?;
        let harness_key: String = row.get(2)?;
        let state_scope: String = row.get(3)?;
        let session_key: String = row.get(4)?;
        Ok((
            mux_node_id,
            AttachedAgent {
                node_id: agent_node_id.clone(),
                id: AgentSessionId::new(harness_key.clone(), state_scope, session_key),
                harness_key,
                cwd: row.get(5)?,
                title: row.get(6)?,
                alias: row.get(7)?,
                preview: row.get(8)?,
                last_active_epoch: row.get(9)?,
                candidate_count: candidate_counts.get(&agent_node_id).copied().unwrap_or(0),
            },
        ))
    })?;

    let mut out: HashMap<String, Vec<AttachedAgent>> = HashMap::new();
    for row in rows {
        let (mux_node_id, agent) = row?;
        out.entry(mux_node_id).or_default().push(agent);
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

fn agent_row(
    agent: &AttachedAgent,
    depth: u8,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.last_active_epoch),
            activity_epoch: agent.last_active_epoch,
            mux_state: mux_indicator(agent.candidate_count),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutNode, Confidence, Freshness, GraphLink,
        GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RelationKind, RepoNode, SourceMetadata,
    };
    use crate::resolve::resolve_snapshot;

    fn mux_node(native: &str) -> GraphNode {
        mux_node_with_paths(native, Some(format!("/p/{native}")), None)
    }

    fn mux_node_with_paths(
        native: &str,
        cwd: Option<String>,
        active_pane_current_path: Option<String>,
    ) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native}")),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: Some(1_700_000_050),
            created_epoch: None,
        })
    }

    fn session_node(key: &str, cwd: &str) -> GraphNode {
        session_node_with_preview(key, cwd, None)
    }

    fn session_node_with_preview(
        key: &str,
        cwd: &str,
        last_message_preview: Option<String>,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", key),
            harness_key: "codex".to_string(),
            cwd: Some(cwd.to_string()),
            title: None,
            last_message_preview,
            last_active_epoch: Some(1_700_000_100),
            session_kind: None,
        })
    }

    #[test]
    fn mux_view_emits_one_row_per_mux_with_agent_labels() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(session_node_with_preview(
            "abcdef123456",
            "/p/editor",
            Some("running cargo test".to_string()),
        ));
        let session = crate::model::NodeId::AgentSession(AgentSessionId::new(
            "codex",
            "/state",
            "abcdef123456",
        ));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: mux },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        assert_eq!(
            tree.rows.len(),
            1,
            "mux view should not emit agent children"
        );
        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert_eq!(row.attached_count, 1);
        assert_eq!(row.agent_labels, vec!["codex"]);
        assert_eq!(row.recency.as_deref(), Some("1m"));
        assert_eq!(
            row.single_session_preview.as_deref(),
            Some("running cargo test"),
            "single-session mux should carry the agent's last-message preview"
        );
    }

    #[test]
    fn mux_view_prefers_active_pane_cwd_for_display() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node_with_paths(
            "shell",
            Some("/started/here".to_string()),
            Some("/moved/there".to_string()),
        ));

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert_eq!(row.cwd_display.as_deref(), Some("/moved/there"));
    }

    #[test]
    fn mux_view_nests_session_rows_when_multiple_agents_link_to_one_mux() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot
            .nodes
            .push(session_node("abcdef123456", "/p/editor"));
        snapshot
            .nodes
            .push(session_node("123456abcdef", "/p/editor"));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        for key in ["abcdef123456", "123456abcdef"] {
            snapshot.candidate_links.push(GraphLink {
                id: format!("session-mux-{key}"),
                source: crate::model::NodeId::AgentSession(AgentSessionId::new(
                    "codex", "/state", key,
                )),
                target: LinkEndpoint::Node { id: mux.clone() },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        assert_eq!(tree.rows.len(), 3);
        assert!(tree.rows[0].expandable);
        let RowKind::MuxSession(mux_row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(
            mux_row.single_session_preview.is_none(),
            "multi-session mux should rely on child rows, not the inline preview"
        );
        assert_eq!(tree.rows[1].depth, 1);
        assert_eq!(tree.rows[2].depth, 1);
        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
        assert!(matches!(tree.rows[2].kind, RowKind::AgentSession(_)));
    }

    #[test]
    fn mux_view_omits_single_session_preview_when_unattached() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("solo"));

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(row.single_session_preview.is_none());
        assert_eq!(row.attached_count, 0);
    }

    #[test]
    fn mux_view_omits_preview_when_attached_agent_has_none() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot
            .nodes
            .push(session_node("abcdef123456", "/p/editor"));
        let session = crate::model::NodeId::AgentSession(AgentSessionId::new(
            "codex",
            "/state",
            "abcdef123456",
        ));
        let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: mux },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        let RowKind::MuxSession(row) = &tree.rows[0].kind else {
            panic!("expected mux row");
        };
        assert!(row.single_session_preview.is_none());
        assert_eq!(row.attached_count, 1);
    }

    #[test]
    fn float_attached_muxes_top_lifts_attached_above_unattached() {
        // Three muxes: alpha and gamma are unattached, beta has one
        // agent session linked to it. Default SQL order is by
        // node_id (alpha, beta, gamma); with the bool set we expect
        // beta first and the alpha/gamma order preserved after it.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("alpha"));
        snapshot.nodes.push(mux_node("beta"));
        snapshot.nodes.push(mux_node("gamma"));
        snapshot.nodes.push(session_node("agent-1", "/p/beta"));
        let agent =
            crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "agent-1"));
        let beta = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:beta"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-beta".to_string(),
            source: agent,
            target: LinkEndpoint::Node { id: beta },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");

        let native_ids = |tree: &RowTree| -> Vec<String> {
            tree.rows
                .iter()
                .filter_map(|row| match &row.kind {
                    RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
                    _ => None,
                })
                .collect()
        };

        let baseline = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("baseline mux tree");
        assert_eq!(
            native_ids(&baseline),
            vec!["alpha", "beta", "gamma"],
            "baseline order is alphabetical by node_id"
        );

        let floated = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter {
                float_attached_muxes_top: true,
                ..RowFilter::default()
            },
            grouping: MuxGrouping::Session,
        })
        .expect("floated mux tree");
        assert_eq!(
            native_ids(&floated),
            vec!["beta", "alpha", "gamma"],
            "beta rises and alpha/gamma keep their relative order"
        );
    }

    #[test]
    fn repo_grouping_buckets_muxes_under_repo_headers() {
        let mut snapshot = GraphSnapshot::empty();
        // Two repos, three muxes total: two under /p/foo, one under
        // /p/bar. A fourth mux has no resolvable cwd and lands in
        // the Ungrouped synthetic bucket.
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
                "/p/foo/.git",
            ))));
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
                "/p/bar/.git",
            ))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/foo/.git"),
                "/p/foo".to_string(),
            ),
            root: "/p/foo".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/bar/.git"),
                "/p/bar".to_string(),
            ),
            root: "/p/bar".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-a",
            Some("/p/foo".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-b",
            Some("/p/foo/sub".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "bar-a",
            Some("/p/bar".to_string()),
            None,
        ));
        snapshot.nodes.push(mux_node_with_paths(
            "orphan",
            Some("/elsewhere".to_string()),
            None,
        ));

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_160),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Repo,
        })
        .expect("repo-grouped mux tree");

        let row_summary: Vec<(u8, String)> = tree
            .rows
            .iter()
            .map(|row| {
                let label = match &row.kind {
                    RowKind::MuxSession(mux) => mux.native_id.clone(),
                    RowKind::Group(g) => g.display_path.clone(),
                    other => format!("{other:?}"),
                };
                (row.depth, label)
            })
            .collect();

        // Bucket order is repo display path asc, then Ungrouped last.
        // Expected shape:
        //   depth 0: /p/bar
        //     depth 1: bar-a
        //   depth 0: /p/foo
        //     depth 1: foo-a
        //     depth 1: foo-b
        //   depth 0: Ungrouped
        //     depth 1: orphan
        assert_eq!(row_summary[0], (0, "/p/bar".to_string()));
        assert_eq!(row_summary[1], (1, "bar-a".to_string()));
        assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
        assert_eq!(row_summary[3], (1, "foo-a".to_string()));
        assert_eq!(row_summary[4], (1, "foo-b".to_string()));
        assert_eq!(row_summary[5], (0, "Ungrouped".to_string()));
        assert_eq!(row_summary[6], (1, "orphan".to_string()));
        assert_eq!(row_summary.len(), 7, "no extra rows: {row_summary:?}");
    }
}
