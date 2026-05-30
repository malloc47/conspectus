//! SQLite-backed union-view row-tree builder.
//!
//! Interleaves agent sessions and mux sessions in a single flat list while
//! keeping agent rows visually identical to the sessions view.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{AgentSessionId, MuxSessionId, NodeId};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, MuxIndicator, MuxSessionRow, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
};

pub struct UnionBuildInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub home: Option<&'a Path>,
    pub filter: RowFilter,
}

pub struct UnionBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
enum UnionSqlRow {
    Agent(AgentSqlRow),
    Mux(MuxSqlRow),
}

impl UnionSqlRow {
    fn node_id_text(&self) -> &str {
        match self {
            Self::Agent(row) => &row.node_id,
            Self::Mux(row) => &row.node_id,
        }
    }
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

#[derive(Clone, Debug)]
struct MuxSqlRow {
    node_id: String,
    id: MuxSessionId,
    backend: String,
    native_id: String,
    client_attached: Option<bool>,
    cwd: Option<String>,
    active_pane_current_path: Option<String>,
    attached_count: usize,
    activity_epoch: Option<i64>,
}

pub fn build_union_tree(inputs: UnionBuildInputs<'_>) -> RowTree {
    let conn = crate::query::materialize_snapshot(inputs.snapshot)
        .expect("materialize snapshot for union TUI tree");
    build_union_tree_from_conn(UnionBuildInputsFromConn {
        conn: &conn,
        home: inputs.home,
        now: None,
        filter: inputs.filter,
    })
    .expect("build union TUI tree from materialized snapshot")
}

pub fn build_union_tree_from_conn(
    inputs: UnionBuildInputsFromConn<'_>,
) -> rusqlite::Result<RowTree> {
    let rows = fetch_union_rows(inputs.conn)?;
    let candidate_counts = fetch_agent_mux_candidate_counts(inputs.conn)?;

    let full_ids: Vec<String> = rows
        .iter()
        .map(|row| node_short_id_from_display(row.node_id_text()))
        .collect();
    let id_len = unique_prefix_len(&full_ids);
    let short_ids: HashMap<&str, String> = rows
        .iter()
        .zip(full_ids.iter())
        .map(|(row, full)| (row.node_id_text(), full[..id_len].to_string()))
        .collect();

    let mut tree = RowTree {
        view: ViewLabel::Union,
        ..RowTree::default()
    };

    for row in &rows {
        match row {
            UnionSqlRow::Agent(agent) => {
                if !session_matches_filter(agent, &candidate_counts, inputs.now, &inputs.filter) {
                    continue;
                }
                tree.rows.push(agent_row(
                    agent,
                    &candidate_counts,
                    short_ids
                        .get(agent.node_id.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    inputs.home,
                    inputs.now,
                ));
            }
            UnionSqlRow::Mux(mux) => {
                if inputs.filter.has_narrowing_predicates() {
                    continue;
                }
                let node_id = NodeId::MuxSession(mux.id.clone());
                tree.rows.push(Row {
                    id: RowId::MuxSession(node_id.clone()),
                    depth: 0,
                    expandable: false,
                    kind: RowKind::MuxSession(MuxSessionRow {
                        mux: mux.id.clone(),
                        backend: mux.backend.clone(),
                        native_id: mux.native_id.clone(),
                        client_attached: mux.client_attached,
                        cwd_display: mux
                            .effective_cwd()
                            .map(|cwd| shorten_home(cwd, inputs.home)),
                        attached_count: mux.attached_count,
                        ambiguous_count: 0,
                        recency: format_recency(inputs.now, mux.activity_epoch),
                        activity_epoch: mux.activity_epoch,
                        agent_labels: Vec::new(),
                        single_session_preview: None,
                        primary_node: node_id,
                    }),
                });
            }
        }
    }

    Ok(tree)
}

fn agent_row(
    agent: &AgentSqlRow,
    candidate_counts: &HashMap<String, usize>,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth: 0,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
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

impl MuxSqlRow {
    fn effective_cwd(&self) -> Option<&str> {
        self.active_pane_current_path
            .as_deref()
            .or(self.cwd.as_deref())
    }
}

fn fetch_union_rows(conn: &Connection) -> rusqlite::Result<Vec<UnionSqlRow>> {
    let mut stmt = conn.prepare(
        "WITH mux_counts AS ( \
             SELECT ('mux_session:' || json_extract(target_node, '$.native_id')) AS mux_node_id, \
                    COUNT(DISTINCT source) AS attached_count \
             FROM candidate_links \
             WHERE source_kind = 'agent_session' \
               AND target_node_kind = 'mux_session' \
               AND relation = 'linked_to_mux' \
               AND state = 'active' \
             GROUP BY target_node \
         ) \
         SELECT v.node_kind, v.node_id, \
                a.harness_key, a.state_scope, a.session_key, a.cwd AS agent_cwd, \
                a.title, al.display_name, a.last_message_preview, a.last_active_epoch, \
                m.backend, m.native_id, m.client_attached, m.cwd AS mux_cwd, \
                m.active_pane_current_path, COALESCE(mc.attached_count, 0), \
                m.activity_epoch \
         FROM v_nodes v \
         LEFT JOIN node_agent_sessions a \
           ON v.node_kind = 'agent_session' AND a.node_id = v.node_id \
         LEFT JOIN aliases al \
           ON v.node_kind = 'agent_session' \
          AND al.node_kind = 'agent_session' \
          AND json_extract(al.node, '$.harness_key') = a.harness_key \
          AND json_extract(al.node, '$.state_scope') = a.state_scope \
          AND json_extract(al.node, '$.session_key') = a.session_key \
         LEFT JOIN node_mux_sessions m \
           ON v.node_kind = 'mux_session' AND m.node_id = v.node_id \
         LEFT JOIN mux_counts mc ON mc.mux_node_id = m.node_id \
         WHERE v.node_kind IN ('agent_session', 'mux_session') \
         ORDER BY COALESCE(a.last_active_epoch, 0) DESC, \
                  CASE v.node_kind WHEN 'agent_session' THEN 0 ELSE 1 END, \
                  v.node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind: String = row.get(0)?;
        let node_id: String = row.get(1)?;
        if kind == "agent_session" {
            let harness_key: String = row.get(2)?;
            let state_scope: String = row.get(3)?;
            let session_key: String = row.get(4)?;
            Ok(UnionSqlRow::Agent(AgentSqlRow {
                node_id,
                id: AgentSessionId::new(harness_key, state_scope, session_key),
                cwd: row.get(5)?,
                title: row.get(6)?,
                alias: row.get(7)?,
                preview: row.get(8)?,
                last_active_epoch: row.get(9)?,
            }))
        } else {
            let id_native = node_id
                .strip_prefix("mux_session:")
                .unwrap_or(&node_id)
                .to_string();
            let attached_count: i64 = row.get(15)?;
            Ok(UnionSqlRow::Mux(MuxSqlRow {
                node_id,
                id: MuxSessionId::new(id_native),
                backend: row.get(10)?,
                native_id: row.get(11)?,
                client_attached: row.get::<_, Option<i64>>(12)?.map(|value| value != 0),
                cwd: row.get(13)?,
                active_pane_current_path: row.get(14)?,
                attached_count: attached_count as usize,
                activity_epoch: row.get(16)?,
            }))
        }
    })?;
    rows.collect()
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
