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
    AgentSessionRow, GroupRow, MuxIndicator, MuxSessionRow, PinRow, Row, RowId, RowKind, RowTree,
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
    let pins = fetch_bound_pins(inputs.conn)?;

    // `mux_native_id -> pin_id` for the bound-pin glyph. The mux
    // view's native_id column carries the bare tmux session name
    // (e.g. `editor`), while the pins table stores
    // `pin.mux.native_id()` (e.g. `tmux:editor` or
    // `tmux:scratch:editor` for non-default sockets). We match the
    // bound case by stripping the `tmux:` / `tmux:<socket>:` prefix
    // on the pin side so they line up with the mux row's
    // `native_id`.
    let mut pin_id_by_bound_mux: HashMap<String, String> = HashMap::new();
    for pin in &pins {
        if pin.binding_kind.as_deref() != Some("bound") {
            continue;
        }
        if let Some(bare) = bare_tmux_name(&pin.mux_native_id) {
            pin_id_by_bound_mux.insert(bare.to_string(), pin.pin_id.clone());
        }
    }

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
            pin_id: pin_id_by_bound_mux.get(&mux.native_id).cloned(),
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
        MuxGrouping::Session | MuxGrouping::Host => {
            emit_flat(&mut tree, groups, &inputs, &short_ids);
        }
        MuxGrouping::Repo => {
            // Pins group sits above the repo-grouped muxes so the
            // operator sees the deck's pinned work first. The
            // emission is gated on the Repo grouping because that's
            // the only mux grouping that introduces header rows;
            // the flat groupings keep their flat appearance.
            emit_pins_group_for_mux(&mut tree, &pins, inputs.home);

            let snapshot = crate::query::read_snapshot(inputs.conn)?;
            let path_index = PathIndex::from_snapshot(&snapshot);
            emit_repo_grouped(&mut tree, groups, &inputs, &short_ids, &path_index);
        }
    }

    Ok(tree)
}

/// Strip the `tmux:` (default socket) or `tmux:<socket>:` (non-
/// default socket) prefix from a pin's `mux.native_id()` so it
/// matches the bare tmux session name carried on
/// `MuxSqlRow.native_id`. Returns `None` when the prefix is
/// absent — anything from a non-tmux backend won't match a tmux
/// mux row anyway.
fn bare_tmux_name(pin_mux_native_id: &str) -> Option<&str> {
    let rest = pin_mux_native_id.strip_prefix("tmux:")?;
    match rest.find(':') {
        // tmux:<socket>:<name> — the second segment is the session name.
        Some(socket_end) => Some(&rest[socket_end + 1..]),
        // tmux:<name> — default socket.
        None => Some(rest),
    }
}

fn emit_pins_group_for_mux(tree: &mut RowTree, pins: &[BoundPinRow], home: Option<&Path>) {
    use crate::model::PinCandidate;

    if pins.is_empty() {
        return;
    }

    tree.rows.push(Row {
        id: RowId::Synthetic("pins"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "Pins".to_string(),
            primary_node: None,
            is_launch_context: false,
        }),
    });

    for pin in pins {
        // Pull the full PinCandidate out of the `details` JSON so
        // the row carries the same fields the sessions Pins group
        // does (launch_argv, mux_socket, etc.). If the JSON is
        // malformed we surface a minimal row from the columnar
        // fields rather than dropping the pin silently.
        let parsed: Option<PinCandidate> = serde_json::from_str(&pin.details).ok();
        let state_label = match pin.binding_kind.as_deref() {
            Some("bound") => "bound",
            Some("stale_mux") => "stale-mux",
            Some("unbound") => "unbound",
            _ => "unresolved",
        };
        let mux_socket = parsed.as_ref().and_then(|p| p.mux.socket_name.clone());
        let launch_argv = parsed
            .as_ref()
            .and_then(|p| p.launch_argv.clone())
            .unwrap_or_default();
        let mux_label = match mux_socket.as_deref() {
            Some(socket) => format!(
                "tmux:{socket}:{}",
                bare_tmux_name(&pin.mux_native_id).unwrap_or(&pin.mux_native_id)
            ),
            None => format!(
                "tmux:{}",
                bare_tmux_name(&pin.mux_native_id).unwrap_or(&pin.mux_native_id)
            ),
        };
        let mux_name = bare_tmux_name(&pin.mux_native_id)
            .unwrap_or(&pin.mux_native_id)
            .to_string();
        tree.rows.push(Row {
            id: RowId::Pin {
                pin_id: pin.pin_id.clone(),
            },
            depth: 1,
            expandable: false,
            kind: RowKind::Pin(PinRow {
                pin_id: pin.pin_id.clone(),
                display_name: pin.display_name.clone(),
                harness: pin.harness.clone(),
                cwd: pin.cwd.clone(),
                mux_name,
                mux_socket,
                launch_argv,
                store_path: pin.store_path.clone(),
                harness_label: harness_label(&pin.harness),
                cwd_display: shorten_home(&pin.cwd, home),
                mux_label,
                state_label,
            }),
        });
    }
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

/// Read every pin from the `pins` table along with its mux
/// `native_id` and binding state. Used to build a
/// `mux_native_id -> pin_id` map so the mux view can paint a pin
/// glyph next to bound muxes and emit a Pins group when grouping
/// is in effect.
fn fetch_bound_pins(conn: &Connection) -> rusqlite::Result<Vec<BoundPinRow>> {
    let mut stmt = conn.prepare(
        "SELECT pin_id, display_name, harness, cwd, mux_native_id, \
                store_path, binding_kind, details \
         FROM pins ORDER BY pin_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(BoundPinRow {
            pin_id: row.get(0)?,
            display_name: row.get(1)?,
            harness: row.get(2)?,
            cwd: row.get(3)?,
            mux_native_id: row.get(4)?,
            store_path: row.get(5)?,
            binding_kind: row.get(6)?,
            details: row.get(7)?,
        })
    })?;
    rows.collect()
}

#[derive(Clone, Debug)]
struct BoundPinRow {
    pin_id: String,
    display_name: String,
    harness: String,
    cwd: String,
    mux_native_id: String,
    store_path: String,
    binding_kind: Option<String>,
    details: String,
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
    // H-UI-008: the mux view's "attached agents" list filters to
    // resolver-blessed `LinkedToMux` links so what shows up in the
    // tree matches the detail pane's validated zone. Joining
    // `resolved_relationships` on `selected_link_id` drops the
    // candidates the resolver did not pick (false-positive
    // attachments under non-winning cwd evidence).
    let mut stmt = conn.prepare(
        "SELECT DISTINCT \
                ('mux_session:' || json_extract(cl.target_node, '$.native_id')) AS mux_node_id, \
                a.node_id, a.harness_key, a.state_scope, a.session_key, a.cwd, a.title, \
                al.display_name, a.last_message_preview, a.last_active_epoch \
         FROM candidate_links cl \
         JOIN resolved_relationships rr \
           ON rr.selected_link_id = cl.link_id \
          AND rr.relation = 'linked_to_mux' \
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
            pin_id: None,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.last_active_epoch),
            activity_epoch: agent.last_active_epoch,
            mux_state: mux_indicator(agent.candidate_count),
            preview: agent.preview.clone(),
            title: agent.title.clone(),
            alias: agent.alias.clone(),
            // P8-015 is sessions-view scoped; agent rows nested
            // under a mux row never gain the title-as-disambiguator
            // treatment here.
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
    fn mux_view_drops_non_winner_linked_to_mux_candidate() {
        // H-UI-008: the mux view's attached-agents list filters
        // through `resolved_relationships`. A `LinkedToMux`
        // candidate that the resolver did not pick (e.g. a weaker
        // cwd evidence pointing at a different mux) must not
        // surface as an attached agent under either mux.
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(mux_node("scratch"));
        snapshot.nodes.push(session_node("session-x", "/p/editor"));
        let session =
            crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "session-x"));
        let editor = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
        let scratch = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
        // Editor link is StrongDiscovered (winner); scratch link
        // is Discovered (loses the LinkedToMux slot).
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux-editor".to_string(),
            source: session.clone(),
            target: LinkEndpoint::Node { id: editor },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux-scratch".to_string(),
            source: session,
            target: LinkEndpoint::Node { id: scratch },
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

        let mux_rows: Vec<_> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux),
                _ => None,
            })
            .collect();
        let editor_row = mux_rows
            .iter()
            .find(|mux| mux.native_id == "editor")
            .expect("editor row");
        let scratch_row = mux_rows
            .iter()
            .find(|mux| mux.native_id == "scratch")
            .expect("scratch row");
        assert_eq!(
            editor_row.attached_count, 1,
            "editor (resolver winner) keeps the attachment",
        );
        assert_eq!(
            scratch_row.attached_count, 0,
            "scratch (non-winner) must not surface a false-positive attachment",
        );
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

    #[test]
    fn mux_view_paints_pin_id_on_bound_mux_rows() {
        // A pin bound to `tmux:editor` should leave its `pin_id` on
        // the mux row so the renderer can paint the bound-pin glyph
        // (regardless of grouping). Other muxes stay None. We set
        // `binding` directly rather than running the resolver
        // because the resolver test fixtures and production
        // discovery encode `MuxSessionNode.native_id` differently
        // (resolver-test fixtures prefix `tmux:`, production
        // discovery emits the bare name); the mux-view code path
        // we're testing only cares about the value already stored
        // on the pin.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.nodes.push(mux_node("scratch"));
        snapshot.pins.push(PinCandidate {
            id: "code".to_string(),
            display_name: "Code Review".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p/work".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "editor".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/work/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Bound {
                mux: MuxSessionId::new("tmux:editor"),
                session: AgentSessionId::new("claude-code", "/state", "session"),
            }),
        });

        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("mux tree");

        let mut pin_ids: Vec<(String, Option<String>)> = tree
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some((mux.native_id.clone(), mux.pin_id.clone())),
                _ => None,
            })
            .collect();
        pin_ids.sort();
        assert_eq!(
            pin_ids,
            vec![
                ("editor".to_string(), Some("code".to_string())),
                ("scratch".to_string(), None),
            ],
        );
    }

    #[test]
    fn mux_view_emits_pins_group_at_top_under_repo_grouping() {
        // Repo grouping introduces header rows. The Pins group
        // should sit above every repo bucket so the operator sees
        // pinned work first.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: crate::model::RepoId::new("/p/foo/.git"),
            common_dir: "/p/foo/.git".to_string(),
            source_paths: vec!["/p/foo".to_string()],
            remotes: vec![],
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(
                crate::model::RepoId::new("/p/foo/.git"),
                "/p/foo".to_string(),
            ),
            root: "/p/foo".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(mux_node_with_paths(
            "foo-a",
            Some("/p/foo".to_string()),
            None,
        ));
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest Pin".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/foo".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/foo/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Unbound),
        });

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Repo,
        })
        .expect("repo-grouped mux tree");

        let row_summary: Vec<(u8, String)> = tree
            .rows
            .iter()
            .map(|row| {
                let label = match &row.kind {
                    RowKind::MuxSession(mux) => format!("mux:{}", mux.native_id),
                    RowKind::Group(g) => g.display_path.clone(),
                    RowKind::Pin(p) => format!("pin:{}", p.pin_id),
                    other => format!("{other:?}"),
                };
                (row.depth, label)
            })
            .collect();
        // Pins group first, then the repo bucket.
        assert_eq!(row_summary[0], (0, "Pins".to_string()));
        assert_eq!(row_summary[1], (1, "pin:ingest".to_string()));
        assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
        assert_eq!(row_summary[3], (1, "mux:foo-a".to_string()));
    }

    #[test]
    fn mux_view_skips_pins_group_under_flat_groupings() {
        // The non-Repo groupings (Session / Host) flow
        // through `emit_flat` without header rows; adding a Pins
        // group on its own would feel like an unmotivated heading.
        // Pin glyphs still appear on bound mux rows, but the
        // synthetic group is gated on Repo grouping.
        use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(mux_node("editor"));
        snapshot.pins.push(PinCandidate {
            id: "code".to_string(),
            display_name: "Pin".to_string(),
            harness: "claude-code".to_string(),
            cwd: "/p".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "editor".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Unbound),
        });

        let snapshot = resolve_snapshot(snapshot);
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let tree = build_mux_tree_from_conn(MuxBuildInputsFromConn {
            conn: &conn,
            home: None,
            now: Some(1_700_000_000),
            filter: RowFilter::default(),
            grouping: MuxGrouping::Session,
        })
        .expect("session-grouped mux tree");

        let has_pins_group = tree
            .rows
            .iter()
            .any(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"));
        assert!(
            !has_pins_group,
            "session/host groupings should not emit a Pins group: {:#?}",
            tree.rows,
        );
    }

    #[test]
    fn bare_tmux_name_strips_default_and_socket_prefixes() {
        assert_eq!(super::bare_tmux_name("tmux:editor"), Some("editor"));
        assert_eq!(super::bare_tmux_name("tmux:scratch:editor"), Some("editor"),);
        assert_eq!(super::bare_tmux_name("editor"), None);
        assert_eq!(super::bare_tmux_name("zellij:foo"), None);
    }
}
