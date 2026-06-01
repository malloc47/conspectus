//! Detail-pane graph explorer view model (T8-027).
//!
//! Replaces the recursive `HeaderField.expanded_fields` model that
//! `detail.rs` exposes. The renderer wants three distinct surfaces
//! per focused node:
//!
//! - A focused **Node** zone with the node's own core summary
//!   fields. The per-kind picks come from the Core column of the
//!   [Node Core-Summary Fields Reference] table in
//!   `docs/tui-detail-mockup.md`.
//! - **Upstream** and **Downstream** relationship explorers. Each
//!   one groups the active candidate links by `(relation,
//!   neighbor_kind)` triple. Multi-link groups carry a count and a
//!   collapsed glyph; single-link groups render as a composite row
//!   inline (locked decision 5).
//! - A **Preview** zone targeted at the currently highlighted
//!   relationship row. Carries the neighbor's core summary plus an
//!   `edge` row that summarizes provenance, confidence, state, and
//!   the resolver verdict.
//!
//! Drilldown and breadcrumbs are tracked by the reducer (T8-028); the
//! view model exposes the data each frame needs, not the navigation
//! state itself.
//!
//! [Node Core-Summary Fields Reference]: ../docs/tui-detail-mockup.md

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{
    AgentSessionNode, BranchNode, CheckoutNode, Confidence, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance,
    RelationKind, RepoNode, ResolvedRelationship, RuntimeProcessNode, RuntimeProcessRole,
    UnresolvedEndpoint, WorkspaceNode,
};
use crate::output::table::node_short_id;
use crate::tui::rows::shorten_home;

// -----------------------------------------------------------------------------
// Builder entry points
// -----------------------------------------------------------------------------

/// Inputs to the explorer view builder.
#[derive(Debug, Clone)]
pub struct ExplorerInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub target: &'a NodeId,
    /// Home directory for `~`-shortening. `None` leaves paths in
    /// their full form.
    pub home: Option<&'a Path>,
}

/// SQLite-backed entry point. Materializes a typed snapshot from
/// `conn` and runs the typed-Rust assembly below. Mirrors
/// [`crate::tui::detail::build_node_detail_from_conn`].
pub fn build_node_view_from_conn(
    conn: &rusqlite::Connection,
    target: &NodeId,
    home: Option<&Path>,
) -> rusqlite::Result<Option<NodeView>> {
    let snapshot = crate::query::read_snapshot(conn)?;
    Ok(build_node_view(ExplorerInputs {
        snapshot: &snapshot,
        target,
        home,
    }))
}

/// Build the explorer view model for the given node id. Returns
/// `None` when the node isn't in the snapshot.
pub fn build_node_view(inputs: ExplorerInputs<'_>) -> Option<NodeView> {
    let node = inputs
        .snapshot
        .nodes
        .iter()
        .find(|n| n.id() == *inputs.target)?;
    let id = node.id();
    let kind_label = kind_label(node);
    let title_line = title_line(inputs.snapshot, node);
    let short_id = node_short_id(&id);
    let core_fields = core_fields(inputs.snapshot, node, inputs.home);
    let all_fields = all_fields(inputs.snapshot, node, inputs.home);
    let upstream = build_explorer(inputs.snapshot, &id, Direction::Upstream, inputs.home);
    let downstream = build_explorer(inputs.snapshot, &id, Direction::Downstream, inputs.home);

    Some(NodeView {
        focused: id.clone(),
        kind_label,
        title_line,
        short_id,
        full_id: id,
        core_fields,
        all_fields,
        upstream,
        downstream,
    })
}

// -----------------------------------------------------------------------------
// View model types
// -----------------------------------------------------------------------------

/// Top-level view model for a focused node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeView {
    pub focused: NodeId,
    /// `agent_session`, `mux_session`, etc.
    pub kind_label: &'static str,
    /// Compact identity line shown in the title.
    pub title_line: String,
    /// FNV-1a 64-bit short id (H-TBL-002 length).
    pub short_id: String,
    pub full_id: NodeId,
    /// Top-5 Core summary fields per the mockup's reference table.
    pub core_fields: Vec<CoreField>,
    /// Every available field, in Core-then-extra order. The "full
    /// node" toggle (T8-034) renders this list in place of
    /// [`Self::core_fields`].
    pub all_fields: Vec<CoreField>,
    pub upstream: RelationshipExplorer,
    pub downstream: RelationshipExplorer,
}

/// One field row in the Node or Preview zone.
#[derive(Clone, Debug, PartialEq)]
pub struct CoreField {
    pub label: &'static str,
    /// Display value, with `~`-shortening already applied.
    pub value: String,
    /// `true` when [`Self::value`] is a placeholder (`— (unknown)`,
    /// etc.). The renderer dims placeholder rows.
    pub placeholder: bool,
    /// Optional trailing annotation glyph (`⚠`, `⟳`, …).
    pub annotation: Option<&'static str>,
    /// Full untruncated value when [`Self::value`] is a truncated
    /// preview. `o` opens it in the full-value modal (T8-030).
    pub long_value: Option<String>,
}

impl CoreField {
    fn plain(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            placeholder: false,
            annotation: None,
            long_value: None,
        }
    }

    fn placeholder(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            placeholder: true,
            annotation: None,
            long_value: None,
        }
    }

    fn with_long(mut self, full: String) -> Self {
        self.long_value = Some(full);
        self
    }
}

/// Upstream / Downstream direction encoded by section, per locked
/// decision 1 in the mockup.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Direction {
    /// Incoming edges — the focused node is the link's *target*.
    Upstream,
    /// Outgoing edges — the focused node is the link's *source*.
    Downstream,
}

impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Upstream => "Upstream",
            Self::Downstream => "Downstream",
        }
    }
}

/// Per-direction relationship explorer. Groups carry the `(relation,
/// neighbor_kind)` triple key with the direction fixed by the parent.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipExplorer {
    pub direction: Direction,
    pub groups: Vec<RelationshipGroup>,
}

impl RelationshipExplorer {
    /// Total active candidate links across all groups, including
    /// unresolved-evidence stubs.
    pub fn link_count(&self) -> usize {
        self.groups.iter().map(RelationshipGroup::link_count).sum()
    }

    /// Number of groups carrying at least one resolver-flagged
    /// ambiguity.
    pub fn ambiguous_groups(&self) -> usize {
        self.groups.iter().filter(|g| g.ambiguous).count()
    }

    /// Number of groups carrying unresolved-evidence stubs.
    pub fn unresolved_groups(&self) -> usize {
        self.groups.iter().filter(|g| g.unresolved_count > 0).count()
    }
}

/// One row group in a [`RelationshipExplorer`]. The triple
/// `(direction, relation, neighbor_kind)` uniquely keys each group.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipGroup {
    pub relation: RelationKind,
    /// Kind label of the neighbor node (`runtime_process`,
    /// `agent_session`, …). For unresolved-only groups this is the
    /// declared `node_type` carried in the evidence.
    pub neighbor_kind: String,
    /// Resolver-preferred candidates sort first; the rest follow in
    /// `(provenance, confidence, link_id)` order matching
    /// [`crate::tui::detail::preferred_link`].
    pub links: Vec<RelationshipLink>,
    /// Unresolved-evidence stubs not represented by a concrete
    /// neighbor.
    pub unresolved: Vec<UnresolvedRow>,
    /// `true` when at least one link in this group is flagged
    /// `conflict` (see [`EdgeStateLabel::Conflict`]).
    pub ambiguous: bool,
    /// Convenience cache: `unresolved.len()`. Kept on the struct so
    /// renderers can branch without dipping into the vec.
    pub unresolved_count: usize,
}

impl RelationshipGroup {
    /// Number of selectable rows in this group, including unresolved
    /// stubs.
    pub fn link_count(&self) -> usize {
        self.links.len() + self.unresolved.len()
    }

    /// `true` when the group has exactly one selectable row. The
    /// renderer collapses these to a two-line composite row per
    /// locked decision 5.
    pub fn is_single(&self) -> bool {
        self.link_count() == 1
    }
}

/// One concrete (non-unresolved) candidate link.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipLink {
    pub link_id: String,
    pub neighbor_id: NodeId,
    pub neighbor_kind: &'static str,
    /// Short label rendered in the row (`proc:claude · pid 82310`).
    pub neighbor_label: String,
    /// FNV-1a short id of the neighbor, exposed for the breadcrumb
    /// renderer.
    pub neighbor_short_id: String,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub state: LinkStateLabel,
    /// `true` when this link is the resolver's chosen winner for the
    /// corresponding `ResolvedRelationship`.
    pub resolved_winner: bool,
    pub edge_state: EdgeStateLabel,
    /// Top-5 Core fields of the neighbor, used by the Preview zone.
    pub preview: Vec<CoreField>,
}

/// A piece of unresolved-endpoint evidence rendered as a placeholder
/// row in its group. `Enter` is inert in v1; see T8-032.
#[derive(Clone, Debug, PartialEq)]
pub struct UnresolvedRow {
    pub link_id: String,
    /// Declared `node_type` of the missing neighbor (`agent_session`,
    /// etc.).
    pub node_type: String,
    /// Evidence preview values used in the placeholder render.
    pub evidence: UnresolvedEvidence,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub state: LinkStateLabel,
}

/// Subset of [`UnresolvedEndpoint`] surfaced in the explorer rows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UnresolvedEvidence {
    pub harness_key: Option<String>,
    pub native_id: Option<String>,
    pub state_scope: Option<String>,
    pub path: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkStateLabel {
    Active,
    Ignored,
    Overridden,
}

impl LinkStateLabel {
    pub fn snake_case(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Ignored => "ignored",
            Self::Overridden => "overridden",
        }
    }
}

// -----------------------------------------------------------------------------
// Navigation cursor + breadcrumb
// -----------------------------------------------------------------------------

/// Stable identity for a flat selectable row in the explorer. Used by
/// the reducer to preserve cursor position across rebuilds (snapshot
/// refresh, group toggle, drilldown).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExplorerRowKey {
    /// Title bar acts as the "go back to the inherent identity"
    /// landing target; not currently selectable but reserved.
    Title,
    /// One of the [`NodeView::core_fields`] rows (by label, so the
    /// key stays stable across renames).
    NodeField {
        label: String,
    },
    /// A multi-link group's header row.
    GroupHeader {
        direction: Direction,
        relation: RelationKind,
        neighbor_kind: String,
    },
    /// A concrete link child row inside an expanded multi-link
    /// group, or the composite row for a single-link group.
    Link {
        direction: Direction,
        link_id: String,
    },
    /// An unresolved-evidence placeholder row inside a group.
    Unresolved {
        direction: Direction,
        link_id: String,
    },
}

/// Flat selectable row in the explorer. The reducer builds this
/// list from the [`NodeView`] plus the per-frame
/// `expanded_groups` set. The order matches what the renderer
/// draws top-to-bottom so cursor index and rendered row index
/// agree.
#[derive(Clone, Debug, PartialEq)]
pub enum ExplorerRow {
    /// One of the focused node's core summary rows. Carries the
    /// underlying [`CoreField`] index so `o` can find the long
    /// value when the row was truncated.
    NodeField {
        index: usize,
        label: &'static str,
        long_value: Option<String>,
    },
    /// Multi-link group header (≥ 2 selectable rows). Selecting it
    /// and hitting `Enter` or `e` toggles the group's expansion.
    GroupHeader {
        direction: Direction,
        group_index: usize,
        expanded: bool,
    },
    /// One link inside an expanded multi-link group, or the
    /// composite row for a single-link group. `Enter` drills.
    Link {
        direction: Direction,
        group_index: usize,
        link_index: usize,
    },
    /// Unresolved-evidence placeholder. `Enter` is inert in v1; `o`
    /// opens the evidence (see T8-032).
    Unresolved {
        direction: Direction,
        group_index: usize,
        unresolved_index: usize,
    },
}

impl ExplorerRow {
    /// Stable key for cursor persistence across rebuilds.
    pub fn key(&self, view: &NodeView) -> ExplorerRowKey {
        match self {
            Self::NodeField { label, .. } => ExplorerRowKey::NodeField {
                label: (*label).to_string(),
            },
            Self::GroupHeader {
                direction,
                group_index,
                ..
            } => {
                let group = explorer_for(view, *direction).groups.get(*group_index);
                ExplorerRowKey::GroupHeader {
                    direction: *direction,
                    relation: group
                        .map(|g| g.relation.clone())
                        .unwrap_or(RelationKind::AssociatedWith),
                    neighbor_kind: group
                        .map(|g| g.neighbor_kind.clone())
                        .unwrap_or_default(),
                }
            }
            Self::Link {
                direction,
                group_index,
                link_index,
            } => {
                let link_id = explorer_for(view, *direction)
                    .groups
                    .get(*group_index)
                    .and_then(|g| g.links.get(*link_index))
                    .map(|l| l.link_id.clone())
                    .unwrap_or_default();
                ExplorerRowKey::Link {
                    direction: *direction,
                    link_id,
                }
            }
            Self::Unresolved {
                direction,
                group_index,
                unresolved_index,
            } => {
                let link_id = explorer_for(view, *direction)
                    .groups
                    .get(*group_index)
                    .and_then(|g| g.unresolved.get(*unresolved_index))
                    .map(|u| u.link_id.clone())
                    .unwrap_or_default();
                ExplorerRowKey::Unresolved {
                    direction: *direction,
                    link_id,
                }
            }
        }
    }

    /// Long value attached to this row, when present. Drives the `o`
    /// open-value modal.
    pub fn long_value(&self) -> Option<&str> {
        match self {
            Self::NodeField { long_value, .. } => long_value.as_deref(),
            _ => None,
        }
    }
}

fn explorer_for(view: &NodeView, direction: Direction) -> &RelationshipExplorer {
    match direction {
        Direction::Upstream => &view.upstream,
        Direction::Downstream => &view.downstream,
    }
}

/// Per-group expansion identity. Multi-link groups stay collapsed by
/// default; `e` or `Enter` on the header inserts the key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupKey {
    pub direction: Direction,
    pub relation: RelationKind,
    pub neighbor_kind: String,
}

impl GroupKey {
    pub fn for_group(direction: Direction, group: &RelationshipGroup) -> Self {
        Self {
            direction,
            relation: group.relation.clone(),
            neighbor_kind: group.neighbor_kind.clone(),
        }
    }
}

impl NodeView {
    /// Build the ordered list of selectable rows the cursor steps
    /// through. `expanded` is the set of multi-link group headers
    /// whose children are visible.
    ///
    /// Single-link groups always render as one composite row regardless
    /// of `expanded`. Per locked decision 5 there is no header form for
    /// a count-of-one group.
    pub fn flat_rows(&self, expanded: &std::collections::BTreeSet<GroupKey>) -> Vec<ExplorerRow> {
        let mut rows = Vec::new();
        for (index, field) in self.core_fields.iter().enumerate() {
            rows.push(ExplorerRow::NodeField {
                index,
                label: field.label,
                long_value: field.long_value.clone(),
            });
        }
        for explorer in [&self.upstream, &self.downstream] {
            push_explorer_rows(&mut rows, explorer, expanded);
        }
        rows
    }
}

fn push_explorer_rows(
    rows: &mut Vec<ExplorerRow>,
    explorer: &RelationshipExplorer,
    expanded: &std::collections::BTreeSet<GroupKey>,
) {
    for (group_index, group) in explorer.groups.iter().enumerate() {
        let key = GroupKey::for_group(explorer.direction, group);
        let is_single = group.is_single();
        if is_single {
            if let Some(_) = group.links.first() {
                rows.push(ExplorerRow::Link {
                    direction: explorer.direction,
                    group_index,
                    link_index: 0,
                });
            } else if !group.unresolved.is_empty() {
                rows.push(ExplorerRow::Unresolved {
                    direction: explorer.direction,
                    group_index,
                    unresolved_index: 0,
                });
            }
        } else {
            let expanded = expanded.contains(&key);
            rows.push(ExplorerRow::GroupHeader {
                direction: explorer.direction,
                group_index,
                expanded,
            });
            if expanded {
                for link_index in 0..group.links.len() {
                    rows.push(ExplorerRow::Link {
                        direction: explorer.direction,
                        group_index,
                        link_index,
                    });
                }
                for unresolved_index in 0..group.unresolved.len() {
                    rows.push(ExplorerRow::Unresolved {
                        direction: explorer.direction,
                        group_index,
                        unresolved_index,
                    });
                }
            }
        }
    }
}

/// One frame of drill history. The reducer pushes a hop when the
/// operator presses `Enter` on a link row; Backspace pops the
/// most-recent hop and restores the saved cursor + expansion state.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbHop {
    /// The node the cursor was focused on *before* the drill that
    /// produced this hop.
    pub focused: NodeId,
    /// Short identity label for the hop, used in the breadcrumb
    /// header rendered above the Node zone.
    pub display: String,
    /// Cursor row identity at the time of the drill, so Backspace
    /// can re-find it.
    pub cursor_key: Option<ExplorerRowKey>,
    /// Expanded-group set at the time of the drill.
    pub expanded_groups: std::collections::BTreeSet<GroupKey>,
}

/// Identity for one slot in the [`NodeView`]'s neighbor list — used
/// by the reducer to pick which link to drill into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRef {
    pub direction: Direction,
    pub group_index: usize,
    pub link_index: usize,
}

impl NodeView {
    /// Lookup helper for the reducer: resolve the currently-selected
    /// flat row into the neighbor `NodeId` to drill into. Returns
    /// `None` for non-drillable rows (node fields, group headers,
    /// unresolved evidence).
    pub fn drill_target(&self, row: &ExplorerRow) -> Option<NodeId> {
        if let ExplorerRow::Link {
            direction,
            group_index,
            link_index,
        } = row
        {
            let explorer = explorer_for(self, *direction);
            let group = explorer.groups.get(*group_index)?;
            let link = group.links.get(*link_index)?;
            return Some(link.neighbor_id.clone());
        }
        None
    }

    /// The preview content for a given cursor row — the neighbor's
    /// core fields plus an edge summary. Returns `None` for rows
    /// that don't carry a neighbor (node fields).
    pub fn row_preview(&self, row: &ExplorerRow) -> Option<RowPreview<'_>> {
        match row {
            ExplorerRow::NodeField { .. } => None,
            ExplorerRow::GroupHeader {
                direction,
                group_index,
                ..
            } => {
                let explorer = explorer_for(self, *direction);
                let group = explorer.groups.get(*group_index)?;
                // Header preview targets the resolver winner if any,
                // else the first link.
                let link = group
                    .links
                    .iter()
                    .find(|l| l.resolved_winner)
                    .or_else(|| group.links.first())?;
                Some(RowPreview::Link {
                    neighbor_label: &link.neighbor_label,
                    fields: &link.preview,
                    provenance: link.provenance,
                    confidence: link.confidence,
                    state: link.state,
                    edge_state: &link.edge_state,
                })
            }
            ExplorerRow::Link {
                direction,
                group_index,
                link_index,
            } => {
                let explorer = explorer_for(self, *direction);
                let group = explorer.groups.get(*group_index)?;
                let link = group.links.get(*link_index)?;
                Some(RowPreview::Link {
                    neighbor_label: &link.neighbor_label,
                    fields: &link.preview,
                    provenance: link.provenance,
                    confidence: link.confidence,
                    state: link.state,
                    edge_state: &link.edge_state,
                })
            }
            ExplorerRow::Unresolved {
                direction,
                group_index,
                unresolved_index,
            } => {
                let explorer = explorer_for(self, *direction);
                let group = explorer.groups.get(*group_index)?;
                let row = group.unresolved.get(*unresolved_index)?;
                Some(RowPreview::Unresolved {
                    node_type: &row.node_type,
                    evidence: &row.evidence,
                    provenance: row.provenance,
                    confidence: row.confidence,
                    state: row.state,
                })
            }
        }
    }
}

/// Borrowed view of the Preview-zone content for a single cursor row.
#[derive(Clone, Debug, PartialEq)]
pub enum RowPreview<'a> {
    Link {
        neighbor_label: &'a str,
        fields: &'a [CoreField],
        provenance: Provenance,
        confidence: Confidence,
        state: LinkStateLabel,
        edge_state: &'a EdgeStateLabel,
    },
    Unresolved {
        node_type: &'a str,
        evidence: &'a UnresolvedEvidence,
        provenance: Provenance,
        confidence: Confidence,
        state: LinkStateLabel,
    },
}

/// Resolver-outcome axis. Distinct from `LinkStateLabel` (lifecycle
/// axis) per the mockup's Glossary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EdgeStateLabel {
    /// Resolver's chosen winner for the corresponding
    /// `ResolvedRelationship`.
    Resolves,
    /// Non-winning candidate competing for the same `(source,
    /// relation)` slot as a resolved winner.
    AltOf(RelationKind),
    /// Multiple candidates with non-trivial provenance/confidence are
    /// competing for the same `(source, relation)` slot and the
    /// resolver has flagged the result as ambiguous.
    Conflict,
}

impl EdgeStateLabel {
    pub fn snake_case(&self) -> String {
        match self {
            Self::Resolves => "resolves".to_string(),
            Self::AltOf(rel) => format!("alt of {}", rel.snake_case()),
            Self::Conflict => "conflict".to_string(),
        }
    }
}

// -----------------------------------------------------------------------------
// Builder internals
// -----------------------------------------------------------------------------

fn kind_label(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Repo(_) => "repo",
        GraphNode::Checkout(_) => "checkout",
        GraphNode::Workspace(_) => "workspace",
        GraphNode::AgentSession(_) => "agent_session",
        GraphNode::MuxSession(_) => "mux_session",
        GraphNode::RuntimeProcess(_) => "runtime_process",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

fn title_line(snapshot: &GraphSnapshot, node: &GraphNode) -> String {
    match node {
        GraphNode::AgentSession(s) => {
            let alias = snapshot
                .aliases
                .get(&NodeId::AgentSession(s.id.clone()))
                .map(str::trim)
                .filter(|s| !s.is_empty());
            match alias {
                Some(alias) => format!("{}:{} · {alias}", s.harness_key, s.id.session_key),
                None => format!("{}:{}", s.harness_key, s.id.session_key),
            }
        }
        GraphNode::MuxSession(m) => format!("{}:{}", m.backend, m.native_id),
        GraphNode::RuntimeProcess(p) => match (p.pid, p.command.as_deref()) {
            (Some(pid), Some(cmd)) => format!("pid {pid}: {}", truncate(cmd, 48)),
            (Some(pid), None) => format!("pid {pid}"),
            (None, Some(cmd)) => truncate(cmd, 48),
            (None, None) => p.observation_key.clone(),
        },
        GraphNode::ForgePr(pr) => format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(f) => f
            .name
            .clone()
            .unwrap_or_else(|| f.provider_source_key.clone()),
        GraphNode::Repo(r) => r.common_dir.clone(),
        GraphNode::Checkout(c) => c.root.clone(),
        GraphNode::Workspace(w) => w
            .name
            .clone()
            .unwrap_or_else(|| w.root.clone()),
        GraphNode::Branch(b) => b.refname.clone(),
    }
}

fn core_fields(snapshot: &GraphSnapshot, node: &GraphNode, home: Option<&Path>) -> Vec<CoreField> {
    match node {
        GraphNode::Repo(r) => repo_core(r, home),
        GraphNode::Checkout(c) => checkout_core(c, home),
        GraphNode::Workspace(w) => workspace_core(w, home),
        GraphNode::AgentSession(s) => agent_session_core(snapshot, s, home),
        GraphNode::MuxSession(m) => mux_session_core(m, home),
        GraphNode::RuntimeProcess(p) => runtime_process_core(p, home),
        GraphNode::Branch(b) => branch_core(b),
        GraphNode::Fork(f) => fork_core(f),
        GraphNode::ForgePr(pr) => forge_pr_core(pr),
    }
}

fn all_fields(snapshot: &GraphSnapshot, node: &GraphNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = core_fields(snapshot, node, home);
    let extras = extra_fields(snapshot, node, home);
    for extra in extras {
        if fields.iter().any(|f| f.label == extra.label) {
            continue;
        }
        fields.push(extra);
    }
    fields
}

fn extra_fields(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
) -> Vec<CoreField> {
    match node {
        GraphNode::AgentSession(s) => agent_session_extras(snapshot, s, home),
        GraphNode::MuxSession(m) => mux_session_extras(m),
        GraphNode::RuntimeProcess(p) => runtime_process_extras(p, home),
        GraphNode::Fork(f) => fork_extras(f),
        GraphNode::ForgePr(pr) => forge_pr_extras(pr),
        _ => Vec::new(),
    }
}

// ----- Per-kind Core builders ------------------------------------------------

fn repo_core(r: &RepoNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&NodeId::Repo(r.id.clone()))),
        CoreField::plain("common_dir", shorten_home(&r.common_dir, home)),
    ];
    if let Some(remote) = r.remotes.first() {
        let value = if r.remotes.len() > 1 {
            format!("{remote}  (+{})", r.remotes.len() - 1)
        } else {
            remote.clone()
        };
        fields.push(CoreField::plain("remotes", value));
    } else {
        fields.push(CoreField::placeholder("remotes", "—"));
    }
    if let Some(path) = r.source_paths.first() {
        let value = if r.source_paths.len() > 1 {
            format!(
                "{}  (+{})",
                shorten_home(path, home),
                r.source_paths.len() - 1
            )
        } else {
            shorten_home(path, home)
        };
        fields.push(CoreField::plain("source_paths", value));
    } else {
        fields.push(CoreField::placeholder("source_paths", "—"));
    }
    fields.push(CoreField::plain(
        "full_id",
        format!("{}", NodeId::Repo(r.id.clone())),
    ));
    fields
}

fn checkout_core(c: &CheckoutNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&NodeId::Checkout(c.id.clone()))),
        CoreField::plain("root", shorten_home(&c.root, home)),
    ];
    if let Some(branch) = &c.current_branch {
        fields.push(CoreField::plain("current_branch", branch.refname.clone()));
    } else {
        fields.push(CoreField::placeholder("current_branch", "— (detached)"));
    }
    fields.push(CoreField::plain("repo", format!("{}", c.id.repo)));
    if let Some(git_dir) = &c.git_dir {
        fields.push(CoreField::plain("git_dir", shorten_home(git_dir, home)));
    } else {
        fields.push(CoreField::placeholder("git_dir", "—"));
    }
    fields
}

fn workspace_core(w: &WorkspaceNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![CoreField::plain(
        "id",
        node_short_id(&NodeId::Workspace(w.id.clone())),
    )];
    if let Some(name) = &w.name {
        fields.push(CoreField::plain("name", name.clone()));
    } else {
        fields.push(CoreField::placeholder("name", "—"));
    }
    fields.push(CoreField::plain("root", shorten_home(&w.root, home)));
    if let Some(provider) = &w.provider {
        fields.push(CoreField::plain("provider", provider.clone()));
    } else {
        fields.push(CoreField::placeholder("provider", "—"));
    }
    fields.push(CoreField::plain(
        "full_id",
        format!("{}", NodeId::Workspace(w.id.clone())),
    ));
    fields
}

fn agent_session_core(
    snapshot: &GraphSnapshot,
    s: &AgentSessionNode,
    home: Option<&Path>,
) -> Vec<CoreField> {
    let id_node = NodeId::AgentSession(s.id.clone());
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&id_node)),
        CoreField::plain("harness", s.harness_key.clone()),
    ];
    let alias = snapshot
        .aliases
        .get(&id_node)
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_string);
    if let Some(alias) = &alias {
        fields.push(CoreField::plain("alias", alias.clone()));
    } else if let Some(title) = s
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        fields.push(CoreField::plain("alias", title.to_string()));
    } else {
        fields.push(CoreField::placeholder("alias", "—"));
    }
    fields.push(match &s.cwd {
        Some(cwd) => CoreField::plain("cwd", shorten_home(cwd, home)),
        None => CoreField::placeholder("cwd", "— (unknown)"),
    });
    fields.push(CoreField::plain("status", session_status(s)));
    fields
}

fn agent_session_extras(
    _snapshot: &GraphSnapshot,
    s: &AgentSessionNode,
    _home: Option<&Path>,
) -> Vec<CoreField> {
    let mut fields = Vec::new();
    fields.push(CoreField::plain(
        "full_id",
        format!("{}", NodeId::AgentSession(s.id.clone())),
    ));
    fields.push(CoreField::plain("state_scope", s.id.state_scope.clone()));
    fields.push(CoreField::plain("session_key", s.id.session_key.clone()));
    if let Some(title) = s
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        fields.push(CoreField::plain("title", title.to_string()));
    }
    if let Some(epoch) = s.last_active_epoch {
        fields.push(CoreField::plain("last_active_epoch", epoch.to_string()));
    }
    if let Some(preview) = &s.last_message_preview {
        let truncated = truncate(preview, 48);
        let mut field = CoreField::plain("last_message_preview", truncated.clone());
        if truncated.len() < preview.len() {
            field = field.with_long(preview.clone());
        }
        fields.push(field);
    }
    if let Some(kind) = s.session_kind {
        fields.push(CoreField::plain(
            "session_kind",
            match kind {
                crate::model::SessionKind::Human => "human",
                crate::model::SessionKind::Subagent => "subagent",
            }
            .to_string(),
        ));
    }
    fields
}

fn mux_session_core(m: &MuxSessionNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&NodeId::MuxSession(m.id.clone()))),
        CoreField::plain(
            "backend · native_id",
            format!("{} · {}", m.backend, m.native_id),
        ),
    ];
    fields.push(match &m.cwd {
        Some(cwd) => CoreField::plain("cwd", shorten_home(cwd, home)),
        None => CoreField::placeholder("cwd", "— (unknown)"),
    });
    let attached = match m.client_attached {
        Some(true) => "yes".to_string(),
        Some(false) => "no".to_string(),
        None => "—".to_string(),
    };
    let mut attached_field = CoreField::plain("attached", attached);
    if m.client_attached.is_none() {
        attached_field.placeholder = true;
    }
    fields.push(attached_field);
    fields.push(match m.activity_epoch {
        Some(epoch) => CoreField::plain("last_active", relative_epoch(epoch)),
        None => CoreField::placeholder("last_active", "—"),
    });
    fields
}

fn mux_session_extras(m: &MuxSessionNode) -> Vec<CoreField> {
    let mut fields = Vec::new();
    if let Some(epoch) = m.created_epoch {
        fields.push(CoreField::plain("created", relative_epoch(epoch)));
    }
    if let Some(cmd) = &m.active_pane_command {
        let truncated = truncate(cmd, 48);
        let mut field = CoreField::plain("active_pane_command", truncated.clone());
        if truncated.len() < cmd.len() {
            field = field.with_long(cmd.clone());
        }
        fields.push(field);
    }
    if let Some(pid) = m.active_pane_pid {
        fields.push(CoreField::plain("active_pane_pid", pid.to_string()));
    }
    if let Some(path) = &m.active_pane_current_path {
        fields.push(CoreField::plain("active_pane_current_path", path.clone()));
    }
    if let Some(cmd) = &m.active_pane_start_command {
        let truncated = truncate(cmd, 48);
        let mut field = CoreField::plain("active_pane_start_command", truncated.clone());
        if truncated.len() < cmd.len() {
            field = field.with_long(cmd.clone());
        }
        fields.push(field);
    }
    fields
}

fn runtime_process_core(p: &RuntimeProcessNode, _home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![CoreField::plain(
        "id",
        node_short_id(&NodeId::RuntimeProcess(p.id.clone())),
    )];
    let pid_value = match p.pid {
        Some(pid) => {
            let mut annotation = String::new();
            if let Some(parent) = p.parent_pid {
                annotation.push_str(&format!("  parent {parent}"));
            }
            if let Some(pane) = p.root_pane_pid
                && Some(pane) != p.pid
            {
                annotation.push_str(&format!("  pane {pane}"));
            }
            format!("{pid}{annotation}")
        }
        None => "—".to_string(),
    };
    fields.push(CoreField {
        label: "pid",
        value: pid_value,
        placeholder: p.pid.is_none(),
        annotation: None,
        long_value: None,
    });
    let command_field = match &p.command {
        Some(cmd) => {
            let truncated = truncate(cmd, 48);
            let mut field = CoreField::plain("command", truncated.clone());
            if truncated.len() < cmd.len() {
                field = field.with_long(cmd.clone());
            }
            field
        }
        None => CoreField::placeholder("command", "—"),
    };
    fields.push(command_field);
    fields.push(CoreField::plain(
        "role",
        match p.role {
            Some(RuntimeProcessRole::HumanAgent) => "human_agent",
            Some(RuntimeProcessRole::Subagent) => "subagent",
            Some(RuntimeProcessRole::Shell) => "shell",
            Some(RuntimeProcessRole::Unknown) => "unknown",
            None => "—",
        }
        .to_string(),
    ));
    fields.push(match p.observed_epoch {
        Some(epoch) => CoreField::plain("observed", relative_epoch(epoch)),
        None => CoreField::placeholder("observed", "—"),
    });
    fields
}

fn runtime_process_extras(p: &RuntimeProcessNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = Vec::new();
    if let Some(cwd) = &p.cwd {
        fields.push(CoreField::plain("cwd", shorten_home(cwd, home)));
    }
    if let Some(harness) = &p.harness_key {
        fields.push(CoreField::plain("harness_key", harness.clone()));
    }
    if let Some(depth) = p.depth {
        fields.push(CoreField::plain("depth", depth.to_string()));
    }
    if let Some(parent) = p.parent_pid {
        fields.push(CoreField::plain("parent_pid", parent.to_string()));
    }
    if let Some(pane) = p.root_pane_pid {
        fields.push(CoreField::plain("root_pane_pid", pane.to_string()));
    }
    fields.push(CoreField::plain(
        "observation_key",
        p.observation_key.clone(),
    ));
    fields
}

fn branch_core(b: &BranchNode) -> Vec<CoreField> {
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&NodeId::Branch(b.id.clone()))),
        CoreField::plain("refname", b.refname.clone()),
    ];
    fields.push(match &b.current_commit {
        Some(commit) => {
            let short = commit.chars().take(12).collect::<String>();
            CoreField::plain("current_commit", short)
        }
        None => CoreField::placeholder("current_commit", "—"),
    });
    fields.push(match &b.upstream {
        Some(upstream) => CoreField::plain("upstream", upstream.clone()),
        None => CoreField::placeholder("upstream", "—"),
    });
    fields.push(CoreField::plain("repo", format!("{}", b.id.repo)));
    fields
}

fn fork_core(f: &ForkNode) -> Vec<CoreField> {
    let mut fields = vec![CoreField::plain(
        "id",
        node_short_id(&NodeId::Fork(f.id.clone())),
    )];
    fields.push(match &f.name {
        Some(name) => CoreField::plain("name", name.clone()),
        None => CoreField::placeholder("name", "—"),
    });
    fields.push(CoreField::plain("provider", f.provider.clone()));
    fields.push(match &f.scope {
        Some(scope) => CoreField::plain("scope", scope.clone()),
        None => CoreField::placeholder("scope", "—"),
    });
    fields.push(match f.capabilities.first() {
        Some(first) => {
            let value = if f.capabilities.len() > 1 {
                format!("{first}  (+{})", f.capabilities.len() - 1)
            } else {
                first.clone()
            };
            CoreField::plain("capabilities", value)
        }
        None => CoreField::placeholder("capabilities", "—"),
    });
    fields
}

fn fork_extras(f: &ForkNode) -> Vec<CoreField> {
    vec![CoreField::plain(
        "provider_source_key",
        f.provider_source_key.clone(),
    )]
}

fn forge_pr_core(pr: &ForgePrNode) -> Vec<CoreField> {
    let id_node = NodeId::ForgePr(pr.id.clone());
    let state = pr.state.as_deref().unwrap_or("?");
    let composite = format!(
        "{}/{}#{} ({state})",
        pr.owner, pr.repo, pr.number
    );
    let mut fields = vec![
        CoreField::plain("id", node_short_id(&id_node)),
        CoreField::plain("pr", composite),
    ];
    if pr.is_draft {
        fields.push(CoreField::plain("draft", "true".to_string()));
    } else {
        fields.push(CoreField::placeholder("draft", "false"));
    }
    fields.push(match pr.updated_epoch {
        Some(epoch) => CoreField::plain("updated", relative_epoch(epoch)),
        None => CoreField::placeholder("updated", "—"),
    });
    fields.push(match &pr.url {
        Some(url) => {
            let truncated = truncate(url, 48);
            let mut field = CoreField::plain("url", truncated.clone());
            if truncated.len() < url.len() {
                field = field.with_long(url.clone());
            }
            field
        }
        None => CoreField::placeholder("url", "—"),
    });
    fields
}

fn forge_pr_extras(pr: &ForgePrNode) -> Vec<CoreField> {
    vec![
        CoreField::plain("provider", pr.provider.clone()),
        CoreField::plain("host", pr.host.clone()),
        CoreField::plain("state", pr.state.clone().unwrap_or_else(|| "?".to_string())),
        CoreField::plain("owner", pr.owner.clone()),
        CoreField::plain("repo", pr.repo.clone()),
        CoreField::plain("number", pr.number.to_string()),
    ]
}

// ----- Helpers ---------------------------------------------------------------

fn session_status(s: &AgentSessionNode) -> String {
    match s.last_active_epoch {
        Some(epoch) => format!("active · last {}", relative_epoch(epoch)),
        None => "—".to_string(),
    }
}

fn relative_epoch(_epoch: i64) -> String {
    // v1 keeps the relative-recency formatting simple — the existing
    // sessions renderer already maps epochs to "Xs / Xm / Xh / Xd"
    // strings. Until the explorer wiring story routes that helper in,
    // we surface the raw epoch so tests are deterministic and the
    // renderer can substitute the proper relative string later.
    format!("{}s", _epoch)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// ----- Relationship explorer assembly ---------------------------------------

fn build_explorer(
    snapshot: &GraphSnapshot,
    focused: &NodeId,
    direction: Direction,
    home: Option<&Path>,
) -> RelationshipExplorer {
    let mut groups_by_key: BTreeMap<(RelationKind, String), GroupBuilder> = BTreeMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        let matches_direction = match direction {
            Direction::Upstream => link.target_node_id() == Some(focused),
            Direction::Downstream => &link.source == focused,
        };
        if !matches_direction {
            continue;
        }
        let neighbor_endpoint = match direction {
            Direction::Upstream => Some(NeighborEndpoint::Node(link.source.clone())),
            Direction::Downstream => match &link.target {
                LinkEndpoint::Node { id } => Some(NeighborEndpoint::Node(id.clone())),
                LinkEndpoint::Unresolved { evidence } => {
                    Some(NeighborEndpoint::Unresolved(evidence.clone()))
                }
            },
        };
        let Some(endpoint) = neighbor_endpoint else {
            continue;
        };
        let neighbor_kind = match &endpoint {
            NeighborEndpoint::Node(id) => snapshot
                .nodes
                .iter()
                .find(|n| n.id() == *id)
                .map(kind_label)
                .unwrap_or("unknown")
                .to_string(),
            NeighborEndpoint::Unresolved(e) => e.node_type.clone(),
        };
        let key = (link.relation.clone(), neighbor_kind.clone());
        let builder = groups_by_key.entry(key).or_insert_with(|| GroupBuilder {
            relation: link.relation.clone(),
            neighbor_kind: neighbor_kind.clone(),
            links: Vec::new(),
            unresolved: Vec::new(),
        });
        match endpoint {
            NeighborEndpoint::Node(neighbor_id) => {
                builder.links.push((link.clone(), neighbor_id));
            }
            NeighborEndpoint::Unresolved(evidence) => {
                builder.unresolved.push((link.clone(), evidence));
            }
        }
    }

    let groups = groups_by_key
        .into_values()
        .map(|builder| finalize_group(snapshot, focused, direction, builder, home))
        .collect();
    RelationshipExplorer { direction, groups }
}

struct GroupBuilder {
    relation: RelationKind,
    neighbor_kind: String,
    links: Vec<(GraphLink, NodeId)>,
    unresolved: Vec<(GraphLink, UnresolvedEndpoint)>,
}

enum NeighborEndpoint {
    Node(NodeId),
    Unresolved(UnresolvedEndpoint),
}

fn finalize_group(
    snapshot: &GraphSnapshot,
    focused: &NodeId,
    direction: Direction,
    builder: GroupBuilder,
    home: Option<&Path>,
) -> RelationshipGroup {
    let GroupBuilder {
        relation,
        neighbor_kind,
        mut links,
        mut unresolved,
    } = builder;

    let resolved = resolved_for(snapshot, focused, direction, &relation);
    let winner_link_id = resolved.as_ref().map(|r| r.selected_link_id.clone());
    let competing: Vec<String> = resolved
        .as_ref()
        .map(|r| r.competing_link_ids.clone())
        .unwrap_or_default();

    let ambiguous = !competing.is_empty();

    links.sort_by(|(left, _), (right, _)| {
        let left_winner = winner_link_id.as_deref() == Some(&left.id);
        let right_winner = winner_link_id.as_deref() == Some(&right.id);
        right_winner
            .cmp(&left_winner)
            .then_with(|| {
                right
                    .provenance
                    .precedence()
                    .cmp(&left.provenance.precedence())
            })
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    unresolved.sort_by(|(left, _), (right, _)| left.id.cmp(&right.id));

    let link_rows = links
        .into_iter()
        .map(|(link, neighbor_id)| {
            let resolved_winner = winner_link_id.as_deref() == Some(&link.id);
            let edge_state = if resolved_winner {
                EdgeStateLabel::Resolves
            } else if ambiguous && competing.contains(&link.id) {
                EdgeStateLabel::Conflict
            } else if winner_link_id.is_some() {
                EdgeStateLabel::AltOf(relation.clone())
            } else {
                EdgeStateLabel::AltOf(relation.clone())
            };
            let neighbor_node = snapshot.nodes.iter().find(|n| n.id() == neighbor_id);
            let neighbor_kind_label = neighbor_node.map(kind_label).unwrap_or("unknown");
            let neighbor_label = neighbor_node
                .map(|n| neighbor_display_label(n, home))
                .unwrap_or_else(|| format!("{}", neighbor_id));
            let preview = neighbor_node
                .map(|n| core_fields(snapshot, n, home))
                .unwrap_or_default();
            RelationshipLink {
                link_id: link.id.clone(),
                neighbor_short_id: node_short_id(&neighbor_id),
                neighbor_id,
                neighbor_kind: neighbor_kind_label,
                neighbor_label,
                provenance: link.provenance,
                confidence: link.confidence,
                state: match link.state {
                    LinkState::Active => LinkStateLabel::Active,
                    LinkState::Ignored { .. } => LinkStateLabel::Ignored,
                    LinkState::Overridden { .. } => LinkStateLabel::Overridden,
                },
                resolved_winner,
                edge_state,
                preview,
            }
        })
        .collect::<Vec<_>>();

    let unresolved_rows: Vec<UnresolvedRow> = unresolved
        .into_iter()
        .map(|(link, evidence)| UnresolvedRow {
            link_id: link.id.clone(),
            node_type: evidence.node_type.clone(),
            evidence: UnresolvedEvidence {
                harness_key: evidence.harness_key,
                native_id: evidence.native_id,
                state_scope: evidence.state_scope,
                path: evidence.path,
            },
            provenance: link.provenance,
            confidence: link.confidence,
            state: match link.state {
                LinkState::Active => LinkStateLabel::Active,
                LinkState::Ignored { .. } => LinkStateLabel::Ignored,
                LinkState::Overridden { .. } => LinkStateLabel::Overridden,
            },
        })
        .collect();

    let unresolved_count = unresolved_rows.len();
    RelationshipGroup {
        relation,
        neighbor_kind,
        links: link_rows,
        unresolved: unresolved_rows,
        ambiguous,
        unresolved_count,
    }
}

fn resolved_for<'a>(
    snapshot: &'a GraphSnapshot,
    focused: &NodeId,
    direction: Direction,
    relation: &RelationKind,
) -> Option<&'a ResolvedRelationship> {
    snapshot
        .resolved_relationships
        .iter()
        .find(|rel| match direction {
            Direction::Upstream => &rel.target == focused && &rel.relation == relation,
            Direction::Downstream => &rel.source == focused && &rel.relation == relation,
        })
}

fn neighbor_display_label(node: &GraphNode, home: Option<&Path>) -> String {
    match node {
        GraphNode::AgentSession(s) => format!("{}:{}", s.harness_key, s.id.session_key),
        GraphNode::MuxSession(m) => format!("{}:{}", m.backend, m.native_id),
        GraphNode::RuntimeProcess(p) => match (p.pid, p.command.as_deref()) {
            (Some(pid), Some(cmd)) => format!("{} · pid {pid}", truncate(cmd, 24)),
            (Some(pid), None) => format!("pid {pid}"),
            (None, Some(cmd)) => truncate(cmd, 32),
            (None, None) => p.observation_key.clone(),
        },
        GraphNode::ForgePr(pr) => format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(f) => f
            .name
            .clone()
            .unwrap_or_else(|| f.provider_source_key.clone()),
        GraphNode::Repo(r) => shorten_home(&r.common_dir, home),
        GraphNode::Checkout(c) => shorten_home(&c.root, home),
        GraphNode::Workspace(w) => {
            w.name.clone().unwrap_or_else(|| shorten_home(&w.root, home))
        }
        GraphNode::Branch(b) => b.refname.clone(),
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, ForgePrId,
        ForgePrNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RepoId, RepoNode, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole,
        SourceMetadata, UnresolvedEndpoint,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    fn agent(harness: &str, key: &str, cwd: Option<&str>, title: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, "/state", key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: title.map(str::to_string),
            last_message_preview: None,
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        })
    }

    fn mux(backend: &str, native: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native),
            backend: backend.to_string(),
            native_id: native.to_string(),
            cwd: cwd.map(str::to_string),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: Some(true),
            activity_epoch: Some(1_700_000_005),
            created_epoch: Some(1_699_000_000),
        })
    }

    fn process(observation_key: &str, pid: i64, command: &str) -> GraphNode {
        GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: RuntimeProcessId::new(observation_key),
            observation_key: observation_key.to_string(),
            pid: Some(pid),
            parent_pid: Some(1),
            root_pane_pid: Some(pid),
            command: Some(command.to_string()),
            cwd: Some("/home/op/src/x".to_string()),
            harness_key: Some("claude-code".to_string()),
            role: Some(RuntimeProcessRole::HumanAgent),
            depth: Some(0),
            observed_epoch: Some(1_700_000_002),
        })
    }

    fn link(id: &str, source: NodeId, target: NodeId, relation: RelationKind) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source,
            target: LinkEndpoint::Node { id: target },
            relation,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn unresolved_link(
        id: &str,
        source: NodeId,
        evidence: UnresolvedEndpoint,
        relation: RelationKind,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source,
            target: LinkEndpoint::Unresolved { evidence },
            relation,
            provenance: Provenance::Discovered,
            confidence: Confidence::Low,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn build(snapshot: &GraphSnapshot, target: &NodeId, home: Option<&Path>) -> NodeView {
        build_node_view(ExplorerInputs {
            snapshot,
            target,
            home,
        })
        .expect("view exists")
    }

    #[test]
    fn unknown_node_returns_none() {
        let snapshot = GraphSnapshot::empty();
        let phantom = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "nope"));
        assert!(
            build_node_view(ExplorerInputs {
                snapshot: &snapshot,
                target: &phantom,
                home: None,
            })
            .is_none()
        );
    }

    #[test]
    fn sparse_agent_session_renders_core_and_empty_explorers() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));

        let view = build(&snapshot, &target, Some(home().as_path()));
        assert_eq!(view.kind_label, "agent_session");
        assert_eq!(view.title_line, "claude-code:abc");
        let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
        assert_eq!(labels, vec!["id", "harness", "alias", "cwd", "status"]);
        assert_eq!(view.upstream.groups.len(), 0);
        assert_eq!(view.downstream.groups.len(), 0);
        assert_eq!(view.upstream.link_count(), 0);
    }

    #[test]
    fn agent_session_groups_link_to_mux_downstream() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
        snapshot.nodes.push(mux("tmux", "work-claude", None));
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("work-claude"));
        snapshot.candidate_links.push(link(
            "l1",
            session_id.clone(),
            mux_id,
            RelationKind::LinkedToMux,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let view = build(&snapshot, &session_id, Some(home().as_path()));
        assert_eq!(view.downstream.groups.len(), 1);
        let group = &view.downstream.groups[0];
        assert_eq!(group.relation, RelationKind::LinkedToMux);
        assert_eq!(group.neighbor_kind, "mux_session");
        assert!(group.is_single());
        assert_eq!(group.links.len(), 1);
        assert!(group.links[0].resolved_winner);
        assert_eq!(group.links[0].edge_state, EdgeStateLabel::Resolves);
        // Preview carries mux core fields.
        let labels: Vec<&str> = group.links[0].preview.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec!["id", "backend · native_id", "cwd", "attached", "last_active"]
        );
    }

    #[test]
    fn multi_process_candidates_flag_ambiguity() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
        snapshot.nodes.push(process("obs:1", 100, "/usr/bin/claude"));
        snapshot
            .nodes
            .push(process("obs:2", 200, "/usr/bin/claude-sub"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let proc1 = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:1"));
        let proc2 = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:2"));
        let mut high = link(
            "p1",
            proc1.clone(),
            session_id.clone(),
            RelationKind::ProcessIdentifiesSession,
        );
        high.provenance = Provenance::StrongDiscovered;
        let mut low = link(
            "p2",
            proc2.clone(),
            session_id.clone(),
            RelationKind::ProcessCandidatesSession,
        );
        low.provenance = Provenance::Discovered;
        low.confidence = Confidence::Medium;
        snapshot.candidate_links.push(high);
        snapshot.candidate_links.push(low);
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &session_id, Some(home().as_path()));
        // Both groups are upstream because processes point at sessions.
        assert_eq!(view.upstream.groups.len(), 2);
        let identifies = view
            .upstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::ProcessIdentifiesSession)
            .expect("identifies group");
        assert!(identifies.is_single());
        assert!(identifies.links[0].resolved_winner);
        assert_eq!(identifies.links[0].edge_state, EdgeStateLabel::Resolves);

        let candidates = view
            .upstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::ProcessCandidatesSession)
            .expect("candidates group");
        assert!(candidates.is_single());
        // `process_candidates` shows a runner-up — its slot is its own
        // resolved relationship since nothing else competes for it.
        assert_eq!(candidates.links[0].edge_state, EdgeStateLabel::Resolves);
    }

    #[test]
    fn child_session_group_multi_link_keeps_winner_first() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "claude-code",
            "parent",
            Some("/home/op/src/x"),
            None,
        ));
        snapshot
            .nodes
            .push(agent("claude-code", "child-a", Some("/home/op/src/y"), None));
        snapshot
            .nodes
            .push(agent("claude-code", "child-b", Some("/home/op/src/z"), None));
        let parent = NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "/state",
            "parent",
        ));
        let child_a = NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "/state",
            "child-a",
        ));
        let child_b = NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "/state",
            "child-b",
        ));
        snapshot.candidate_links.push(link(
            "ca",
            child_a,
            parent.clone(),
            RelationKind::ParentSession,
        ));
        snapshot.candidate_links.push(link(
            "cb",
            child_b,
            parent.clone(),
            RelationKind::ParentSession,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &parent, Some(home().as_path()));
        let group = view
            .upstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::ParentSession)
            .expect("parent_session group on parent side");
        assert!(!group.is_single());
        assert_eq!(group.links.len(), 2);
    }

    #[test]
    fn unresolved_endpoint_renders_as_unresolved_row() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "claude-code",
            "child",
            Some("/home/op/src/x"),
            None,
        ));
        let child = NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "/state",
            "child",
        ));
        let evidence = UnresolvedEndpoint {
            node_type: "agent_session".to_string(),
            harness_key: Some("claude-code".to_string()),
            native_id: Some("9a1f".to_string()),
            state_scope: None,
            path: None,
            metadata: crate::model::Metadata::default(),
        };
        snapshot.candidate_links.push(unresolved_link(
            "u1",
            child.clone(),
            evidence,
            RelationKind::ParentSession,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &child, Some(home().as_path()));
        let group = view
            .downstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::ParentSession)
            .expect("parent_session downstream");
        assert_eq!(group.links.len(), 0);
        assert_eq!(group.unresolved_count, 1);
        assert_eq!(group.unresolved[0].node_type, "agent_session");
        assert_eq!(
            group.unresolved[0].evidence.native_id.as_deref(),
            Some("9a1f")
        );
        assert!(group.is_single());
    }

    #[test]
    fn long_command_truncates_with_full_value_available() {
        let mut snapshot = GraphSnapshot::empty();
        let long_command = "/usr/bin/claude --resume 7f3c2a917b8c4d556e6f7a8b9c0d1e2f3a4b5c6d --extra";
        snapshot
            .nodes
            .push(process("obs:1", 82310, long_command));
        let proc_id = NodeId::RuntimeProcess(RuntimeProcessId::new("obs:1"));
        let snapshot = resolve_snapshot(snapshot);
        let view = build(&snapshot, &proc_id, Some(home().as_path()));
        let command_field = view
            .core_fields
            .iter()
            .find(|f| f.label == "command")
            .expect("command field");
        assert!(command_field.value.contains('…'));
        assert_eq!(command_field.long_value.as_deref(), Some(long_command));
    }

    #[test]
    fn full_id_visible_in_all_fields_but_not_core() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "claude-code",
            "abc",
            Some("/home/op/src/x"),
            Some("title"),
        ));
        let target =
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let snapshot = resolve_snapshot(snapshot);
        let view = build(&snapshot, &target, Some(home().as_path()));
        let core_labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
        assert!(!core_labels.contains(&"full_id"));
        let all_labels: Vec<&str> = view.all_fields.iter().map(|f| f.label).collect();
        assert!(all_labels.contains(&"full_id"));
        assert!(all_labels.contains(&"state_scope"));
        assert!(all_labels.contains(&"session_key"));
        // No duplication of Core entries.
        for label in &core_labels {
            let count = all_labels.iter().filter(|l| *l == label).count();
            assert_eq!(count, 1, "duplicate label {label} in all_fields");
        }
    }

    #[test]
    fn checkout_repo_branch_fork_have_top5_only() {
        let repo = RepoNode {
            id: RepoId::new("/srv/git/conspectus.git"),
            common_dir: "/srv/git/conspectus.git".to_string(),
            source_paths: vec!["/home/op/src/conspectus".to_string()],
            remotes: vec!["git@github.com:malloc47/conspectus.git".to_string()],
        };
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::Repo(repo));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::Repo(RepoId::new("/srv/git/conspectus.git"));
        let view = build(&snapshot, &target, Some(home().as_path()));
        assert_eq!(view.kind_label, "repo");
        let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec!["id", "common_dir", "remotes", "source_paths", "full_id"]
        );
    }

    #[test]
    fn forge_pr_core_renders_composite_label() {
        let pr = ForgePrNode {
            id: ForgePrId::new("github", "github.com", "malloc47", "conspectus", 42),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "malloc47".to_string(),
            repo: "conspectus".to_string(),
            number: 42,
            state: Some("open".to_string()),
            url: Some("https://github.com/malloc47/conspectus/pull/42".to_string()),
            updated_epoch: Some(1_700_000_100),
            is_draft: false,
        };
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::ForgePr(pr.clone()));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::ForgePr(pr.id.clone());
        let view = build(&snapshot, &target, Some(home().as_path()));
        let pr_field = view
            .core_fields
            .iter()
            .find(|f| f.label == "pr")
            .expect("pr field");
        assert_eq!(pr_field.value, "malloc47/conspectus#42 (open)");
        // Extras include state and provider.
        let extra_labels: Vec<&str> = view.all_fields.iter().map(|f| f.label).collect();
        assert!(extra_labels.contains(&"provider"));
        assert!(extra_labels.contains(&"state"));
    }

    #[test]
    fn from_conn_matches_snapshot_path() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
        snapshot.nodes.push(mux("tmux", "work-claude", None));
        let session_id = NodeId::AgentSession(AgentSessionId::new(
            "claude-code",
            "/state",
            "abc",
        ));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("work-claude"));
        snapshot.candidate_links.push(link(
            "l1",
            session_id.clone(),
            mux_id,
            RelationKind::LinkedToMux,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let direct = build_node_view(ExplorerInputs {
            snapshot: &snapshot,
            target: &session_id,
            home: Some(home().as_path()),
        });
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let via_conn =
            build_node_view_from_conn(&conn, &session_id, Some(home().as_path())).expect("ok");
        assert_eq!(direct, via_conn);
    }
}
