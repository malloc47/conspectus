//! SQLite-backed PRs-view row-tree builder.
//!
//! Lists forge PRs as parents and nests agent sessions whose cwd is under a
//! checkout for the PR's linked branch.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::filter::{MuxStateKey, RowFilter, SessionMatchInputs};
use crate::model::{AgentSessionId, ForgePrId, NodeId, path_is_ancestor_of};
use crate::output::render::{
    node_short_id_from_display, pick_strongest, strip_branch_prefix, unique_prefix_len,
};
use crate::tui::rows::{
    AgentSessionRow, MuxIndicator, PrRow, Row, RowId, RowKind, RowTree, ViewLabel, format_recency,
    harness_label, shorten_home,
};

pub struct PrsBuildInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub home: Option<&'a Path>,
}

pub struct PrsBuildInputsFromConn<'a> {
    pub conn: &'a Connection,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub filter: RowFilter,
}

#[derive(Clone, Debug)]
struct PrSqlRow {
    node_id: String,
    id: ForgePrId,
    owner: String,
    repo: String,
    number: u64,
    state: Option<String>,
    is_draft: bool,
    url: Option<String>,
    updated_epoch: Option<i64>,
}

#[derive(Clone, Debug)]
struct BranchLink {
    branch_node_id: String,
    refname: String,
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

pub fn build_prs_tree(inputs: PrsBuildInputs<'_>) -> RowTree {
    let conn = crate::query::materialize_snapshot(inputs.snapshot)
        .expect("materialize snapshot for prs TUI tree");
    build_prs_tree_from_conn(PrsBuildInputsFromConn {
        conn: &conn,
        home: inputs.home,
        now: None,
        filter: RowFilter::default(),
    })
    .expect("build prs TUI tree from materialized snapshot")
}

pub fn build_prs_tree_from_conn(inputs: PrsBuildInputsFromConn<'_>) -> rusqlite::Result<RowTree> {
    let prs = fetch_prs(inputs.conn)?;
    let agents = fetch_agents(inputs.conn)?;
    let candidate_counts = fetch_agent_mux_candidate_counts(inputs.conn)?;
    let branches = fetch_preferred_branch_per_pr(inputs.conn)?;
    let checkout_roots = fetch_checkout_roots_per_branch(inputs.conn)?;

    let mut node_ids: Vec<String> = prs.iter().map(|pr| pr.node_id.clone()).collect();
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

    let mut tree = RowTree {
        view: ViewLabel::Prs,
        ..RowTree::default()
    };

    for pr in &prs {
        let branch = branches.get(&pr.node_id);
        let roots = branch
            .and_then(|branch| checkout_roots.get(&branch.branch_node_id))
            .cloned()
            .unwrap_or_default();
        let visible_agents: Vec<&AgentSqlRow> = agents
            .iter()
            .filter(|agent| agent_attached_to_roots(agent, &roots))
            .filter(|agent| {
                session_matches_filter(agent, &candidate_counts, inputs.now, &inputs.filter)
            })
            .collect();
        if inputs.filter.has_narrowing_predicates() && visible_agents.is_empty() {
            continue;
        }

        let node_id = NodeId::ForgePr(pr.id.clone());
        tree.rows.push(Row {
            id: RowId::Pr(node_id.clone()),
            depth: 0,
            expandable: !visible_agents.is_empty(),
            kind: RowKind::Pr(PrRow {
                pr_number: pr.number,
                repo_display: format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
                state: pr.state.clone(),
                is_draft: pr.is_draft,
                branch_name: branch
                    .map(|branch| branch.refname.as_str())
                    .map(strip_branch_prefix)
                    .map(str::to_string),
                updated_recency: format_recency(inputs.now, pr.updated_epoch),
                attached_count: visible_agents.len(),
                url: pr.url.clone(),
                primary_node: node_id,
            }),
        });

        for agent in visible_agents {
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

fn agent_attached_to_roots(agent: &AgentSqlRow, roots: &[String]) -> bool {
    let Some(cwd) = agent.cwd.as_deref() else {
        return false;
    };
    let cwd = Path::new(cwd);
    roots
        .iter()
        .any(|root| path_is_ancestor_of(Path::new(root), cwd))
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
            // H-WS-001: chip is Sessions-view-specific; H-WS-003
            // will audit whether the Prs view needs an equivalent.
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

fn fetch_prs(conn: &Connection) -> rusqlite::Result<Vec<PrSqlRow>> {
    let mut stmt = conn.prepare(
        "SELECT pr.node_id, pr.provider_name, pr.host, pr.owner, pr.repo, pr.number, \
                pr.state, pr.is_draft, pr.url, pr.updated_epoch \
         FROM node_forge_prs pr \
         ORDER BY pr.owner, pr.repo, pr.number DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let provider: String = row.get(1)?;
        let host: String = row.get(2)?;
        let owner: String = row.get(3)?;
        let repo: String = row.get(4)?;
        let number_i64: i64 = row.get(5)?;
        let is_draft: i64 = row.get(7)?;
        Ok(PrSqlRow {
            node_id: row.get(0)?,
            id: ForgePrId::new(
                provider,
                host,
                owner.clone(),
                repo.clone(),
                number_i64 as u64,
            ),
            owner,
            repo,
            number: number_i64 as u64,
            state: row.get(6)?,
            is_draft: is_draft != 0,
            url: row.get(8)?,
            updated_epoch: row.get(9)?,
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
         WHERE a.cwd IS NOT NULL \
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

fn fetch_preferred_branch_per_pr(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, BranchLink>> {
    let mut stmt = conn.prepare(
        "SELECT ('forge_pr:' || \
                 json_extract(cl.source, '$.provider') || ':' || \
                 json_extract(cl.source, '$.host') || '/' || \
                 json_extract(cl.source, '$.owner') || '/' || \
                 json_extract(cl.source, '$.repo') || '#' || \
                 json_extract(cl.source, '$.number')) AS pr_node_id, \
                ('branch:repo:' || json_extract(cl.target_node, '$.repo.common_dir') || '@' || \
                 json_extract(cl.target_node, '$.refname')) AS branch_node_id, \
                json_extract(cl.target_node, '$.refname') AS refname, \
                cl.link_id, cl.provenance, cl.confidence \
         FROM candidate_links cl \
         WHERE cl.source_kind = 'forge_pr' \
           AND cl.target_node_kind = 'branch' \
           AND cl.relation = 'branch_has_forge_pr' \
           AND cl.state = 'active'",
    )?;
    #[derive(Clone)]
    #[allow(dead_code)]
    struct Raw {
        pr_node_id: String,
        branch_node_id: String,
        refname: String,
        link_id: String,
        provenance: String,
        confidence: String,
    }
    let rows = stmt.query_map([], |row| {
        Ok(Raw {
            pr_node_id: row.get(0)?,
            branch_node_id: row.get(1)?,
            refname: row.get(2)?,
            link_id: row.get(3)?,
            provenance: row.get(4)?,
            confidence: row.get(5)?,
        })
    })?;
    let mut per_pr: HashMap<String, Vec<Raw>> = HashMap::new();
    for row in rows {
        let raw = row?;
        per_pr.entry(raw.pr_node_id.clone()).or_default().push(raw);
    }

    let mut out = HashMap::new();
    for (pr_node_id, candidates) in per_pr {
        let Some(best) = pick_strongest(candidates, |raw: &Raw| {
            (&raw.provenance, &raw.confidence, &raw.link_id)
        }) else {
            continue;
        };
        out.insert(
            pr_node_id,
            BranchLink {
                branch_node_id: best.branch_node_id,
                refname: best.refname,
            },
        );
    }
    Ok(out)
}

fn fetch_checkout_roots_per_branch(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT ('branch:repo:' || json_extract(cl.target_node, '$.repo.common_dir') || '@' || \
                json_extract(cl.target_node, '$.refname')) AS branch_node_id, \
                c.root \
         FROM candidate_links cl \
         JOIN node_checkouts c \
           ON cl.source_kind = 'checkout' \
          AND ('checkout:repo:' || json_extract(cl.source, '$.repo.common_dir') || '@' || \
               json_extract(cl.source, '$.root')) = c.node_id \
         WHERE cl.source_kind = 'checkout' \
          AND cl.target_node_kind = 'branch' \
          AND cl.relation = 'checked_out_branch' \
          AND cl.state = 'active' \
         ORDER BY branch_node_id, c.root",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (branch_node_id, root) = row?;
        out.entry(branch_node_id).or_default().push(root);
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
