//! SQLite-backed forks-view row-tree builder.
//!
//! Lists forks as parents and nests resolved child agent sessions when they
//! are present in the graph.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{AgentSessionId, ForkId, NodeId};
use crate::output::render::{node_short_id_from_display, unique_prefix_len};
use crate::tui::rows::{
    AgentSessionRow, ForkRow, MuxIndicator, Row, RowId, RowKind, RowTree, ViewLabel,
    format_recency, harness_label, shorten_home,
};

pub struct ForksBuildInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub home: Option<&'a Path>,
}

pub struct ForksBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
struct ForkSqlRow {
    node_id: String,
    id: ForkId,
    provider_source_key: String,
    provider: String,
    name: Option<String>,
    scope: Option<String>,
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

pub fn build_forks_tree(inputs: ForksBuildInputs<'_>) -> RowTree {
    let conn = crate::query::materialize_snapshot(inputs.snapshot)
        .expect("materialize snapshot for forks TUI tree");
    build_forks_tree_from_conn(ForksBuildInputsFromConn {
        conn: &conn,
        home: inputs.home,
        now: None,
        filter: RowFilter::default(),
    })
    .expect("build forks TUI tree from materialized snapshot")
}

pub fn build_forks_tree_from_conn(
    inputs: ForksBuildInputsFromConn<'_>,
) -> rusqlite::Result<RowTree> {
    let forks = fetch_forks(inputs.conn)?;
    let agents = fetch_agents(inputs.conn)?;
    let candidate_counts = fetch_agent_mux_candidate_counts(inputs.conn)?;
    let child_counts = fetch_child_counts(inputs.conn)?;
    let child_links = fetch_resolved_child_links(inputs.conn)?;
    let parent_labels = fetch_parent_labels(inputs.conn)?;

    let mut node_ids: Vec<String> = forks.iter().map(|fork| fork.node_id.clone()).collect();
    node_ids.extend(agents.iter().map(|agent| agent.node_id.clone()));
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

    let agents_by_node: HashMap<&str, &AgentSqlRow> = agents
        .iter()
        .map(|agent| (agent.node_id.as_str(), agent))
        .collect();

    let mut tree = RowTree {
        view: ViewLabel::Forks,
        ..RowTree::default()
    };

    for fork in &forks {
        let visible_children: Vec<&AgentSqlRow> = child_links
            .get(&fork.node_id)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|node_id| agents_by_node.get(node_id.as_str()).copied())
            .filter(|agent| {
                session_matches_filter(agent, &candidate_counts, inputs.now, &inputs.filter)
            })
            .collect();
        if inputs.filter.has_narrowing_predicates() && visible_children.is_empty() {
            continue;
        }

        let node_id = NodeId::Fork(fork.id.clone());
        tree.rows.push(Row {
            id: RowId::Fork(node_id.clone()),
            depth: 0,
            expandable: !visible_children.is_empty(),
            kind: RowKind::Fork(ForkRow {
                fork_label: fork
                    .name
                    .as_ref()
                    .map(|name| format!("{}:{name}", fork.provider))
                    .unwrap_or_else(|| fork.provider_source_key.clone()),
                provider: fork.provider.clone(),
                scope: fork.scope.clone(),
                parent_label: parent_labels.get(&fork.node_id).cloned(),
                child_count: child_counts.get(&fork.node_id).copied().unwrap_or(0),
                primary_node: node_id,
            }),
        });

        for agent in visible_children {
            tree.rows.push(agent_row(
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
            // P8-015 is sessions-view scoped — other views render
            // agent rows directly under their parent (PR, fork,
            // mux), where "same-harness collision in a project
            // group" doesn't apply. Leave the flag off and the
            // tree label stays alias-only.
            title_disambiguates: false,
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

fn fetch_forks(conn: &Connection) -> rusqlite::Result<Vec<ForkSqlRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, provider_source_key, provider_name, name, scope \
         FROM node_forks \
         ORDER BY provider_name, provider_source_key",
    )?;
    let rows = stmt.query_map([], |row| {
        let node_id: String = row.get(0)?;
        let provider_source_key: String = row.get(1)?;
        Ok(ForkSqlRow {
            node_id,
            id: ForkId::new(provider_source_key.clone()),
            provider_source_key,
            provider: row.get(2)?,
            name: row.get(3)?,
            scope: row.get(4)?,
        })
    })?;
    rows.collect()
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

fn fetch_child_counts(conn: &Connection) -> rusqlite::Result<HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(source, '$.provider_source_key')) AS fork_node_id, \
                COUNT(*) \
         FROM candidate_links \
         WHERE source_kind = 'fork' \
           AND relation = 'child_session' \
           AND state = 'active' \
           AND (target_node_kind = 'agent_session' OR target_kind = 'unresolved') \
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

fn fetch_resolved_child_links(
    conn: &Connection,
) -> rusqlite::Result<BTreeMap<String, Vec<String>>> {
    // H-UI-008: filter `child_session` candidates through
    // `resolved_relationships` so the fork tree only surfaces
    // resolver-blessed children. `ChildSession` is not a
    // `multi_target_relation`, but the only producer (atelier
    // adapter, `src/discovery/atelier.rs:450`) emits at most one
    // candidate per fork, so the slot key collapses to a single
    // winner per fork — exactly the cardinality the tree expects.
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(cl.source, '$.provider_source_key')) AS fork_node_id, \
                ('agent_session:' || json_extract(cl.target_node, '$.harness_key') || ':' || \
                 json_extract(cl.target_node, '$.state_scope') || ':' || \
                 json_extract(cl.target_node, '$.session_key')) AS agent_node_id \
         FROM candidate_links cl \
         JOIN resolved_relationships rr \
           ON rr.selected_link_id = cl.link_id \
          AND rr.relation = 'child_session' \
         WHERE cl.source_kind = 'fork' \
           AND cl.relation = 'child_session' \
           AND cl.state = 'active' \
           AND cl.target_node_kind = 'agent_session' \
         ORDER BY fork_node_id, agent_node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let (fork_node_id, agent_node_id) = row?;
        out.entry(fork_node_id).or_default().push(agent_node_id);
    }
    Ok(out)
}

fn fetch_parent_labels(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    // H-UI-008: filter `parent_session` candidates through the
    // resolver. The atelier adapter emits at most one
    // ParentSession candidate per fork, so the per-source slot
    // resolves to the same winner the previous "first-wins" pass
    // would have produced; the join keeps the row in sync with
    // the detail pane's validated zone.
    //
    // LEFT JOIN + `OR cl.target_kind = 'unresolved'` keeps the
    // unresolved-endpoint variant alive: the resolver never emits
    // a `ResolvedRelationship` for unresolved targets (it emits a
    // `Diagnostic::UnresolvedEndpoint` instead), but the operator
    // still needs to see "we observed this lineage reference but
    // the target session is missing" rendered as `?{native_id}`.
    // This is the explicit candidate-aware surface the H-UI-008
    // story carves out for resolver-can't-pick cases.
    let mut stmt = conn.prepare(
        "SELECT ('fork:' || json_extract(cl.source, '$.provider_source_key')) AS fork_node_id, \
                cl.target_kind, cl.target_node_kind, cl.target_node, cl.target_native_id \
         FROM candidate_links cl \
         LEFT JOIN resolved_relationships rr \
           ON rr.selected_link_id = cl.link_id \
          AND rr.relation = 'parent_session' \
         WHERE cl.source_kind = 'fork' \
           AND cl.relation = 'parent_session' \
           AND cl.state = 'active' \
           AND (cl.target_kind = 'unresolved' OR rr.selected_link_id IS NOT NULL) \
         ORDER BY fork_node_id, cl.link_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (fork_node_id, target_kind, target_node_kind, target_node, target_native_id) = row?;
        if out.contains_key(&fork_node_id) {
            continue;
        }
        let label = if target_kind == "node" && target_node_kind.as_deref() == Some("agent_session")
        {
            target_node
                .as_deref()
                .and_then(session_key_from_node_json)
                .map(|session_key| short_session_label(&session_key))
        } else if target_kind == "unresolved" {
            target_native_id
                .as_deref()
                .map(short_session_label)
                .map(|label| format!("?{label}"))
        } else {
            None
        };
        if let Some(label) = label {
            out.insert(fork_node_id, label);
        }
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

fn session_key_from_node_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value
        .get("session_key")
        .and_then(|session_key| session_key.as_str())
        .map(str::to_string)
}

fn short_session_label(value: &str) -> String {
    value.chars().take(8).collect()
}
