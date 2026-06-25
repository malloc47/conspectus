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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::model::{
    AgentSessionNode, BranchNode, CheckoutNode, Confidence, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId, PinNode, Provenance,
    RelationKind, RepoNode, ResolvedRelationship, RuntimeProcessNode, RuntimeProcessRole,
    UnresolvedEndpoint, WorkspaceNode,
};
use crate::output::table::node_short_id;
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::rows::shorten_home;
use crate::tui::theme::Theme;

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
    let mut groups = upstream.groups;
    groups.extend(downstream.groups);
    sort_relationship_groups(&mut groups);
    let relationships = RelationshipExplorer { groups };

    let short_label = short_node_label(node);
    Some(NodeView {
        focused: id.clone(),
        kind_label,
        title_line,
        short_label,
        short_id,
        full_id: id,
        core_fields,
        all_fields,
        relationships,
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
    /// Compact `kind:short_tag` label for this node (T8-038). Used
    /// by [`BreadcrumbHop::short_label`] when this view is the
    /// before-drill focused node, and by the right-pane title to
    /// render the full drilldown chain.
    pub short_label: String,
    /// FNV-1a 64-bit short id (H-TBL-002 length).
    pub short_id: String,
    pub full_id: NodeId,
    /// Top-5 Core summary fields per the mockup's reference table.
    pub core_fields: Vec<CoreField>,
    /// Every available field, in Core-then-extra order. The "full
    /// node" toggle (T8-034) renders this list in place of
    /// [`Self::core_fields`].
    pub all_fields: Vec<CoreField>,
    /// Combined relationship list (ADR 0074). Each group carries its
    /// own direction; the legacy
    /// [`Self::upstream_groups`] / [`Self::downstream_groups`]
    /// helpers filter on it during the H-UI-003 pass 1 transition so
    /// the reducer and renderer can compile against the new shape
    /// before passes 2 and 3 land the flat-list cursor + render
    /// rewrites.
    pub relationships: RelationshipExplorer,
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
    /// Optional graph-kind chip rendered next to the value (T8-039).
    /// Used by the renderer to surface that e.g. a `cwd` path
    /// resolves to a `repo` / `workspace` / `checkout` node, without
    /// stuffing that metadata into the value string.
    pub kind_chip: Option<&'static str>,
}

impl CoreField {
    fn plain(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            placeholder: false,
            annotation: None,
            long_value: None,
            kind_chip: None,
        }
    }

    fn placeholder(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            placeholder: true,
            annotation: None,
            long_value: None,
            kind_chip: None,
        }
    }

    fn with_long(mut self, full: String) -> Self {
        self.long_value = Some(full);
        self
    }

    fn with_kind_chip(mut self, kind: &'static str) -> Self {
        self.kind_chip = Some(kind);
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

/// Sort the merged relationship-group list by ADR 0074 §4 order:
/// `(NodeKind ordinal, verb, neighbor_label)`. The neighbor label
/// comes from the first link (resolver winner sorts first within
/// the group by `finalize_group`'s preferred-link ordering, so this
/// is stable). Groups with no concrete links fall back to the empty
/// string; with no neighbor label they sort under the kind's other
/// groups but after labelled ones.
fn sort_relationship_groups(groups: &mut [RelationshipGroup]) {
    groups.sort_by(|a, b| {
        let ord_a = crate::tui::icons::NodeKind::from_snake_case(&a.neighbor_kind)
            .map(|k| k.ordinal())
            .unwrap_or(usize::MAX);
        let ord_b = crate::tui::icons::NodeKind::from_snake_case(&b.neighbor_kind)
            .map(|k| k.ordinal())
            .unwrap_or(usize::MAX);
        ord_a
            .cmp(&ord_b)
            .then_with(|| {
                directional_verb(&a.relation, a.direction)
                    .cmp(directional_verb(&b.relation, b.direction))
            })
            .then_with(|| group_neighbor_label(a).cmp(group_neighbor_label(b)))
    });
}

fn group_neighbor_label(group: &RelationshipGroup) -> &str {
    group
        .links
        .first()
        .map(|l| l.neighbor_label.as_str())
        .unwrap_or("")
}

/// Surface-language verb for a `(RelationKind, Direction)` pair
/// (ADR 0074 §2). Replaces the prior `relation.snake_case()` text
/// the renderer used to label group headers and single-link
/// composites. Read as `<focused> <verb> <neighbor>`, e.g. a
/// `WorkspaceContainsRepo` link with the workspace focused reads as
/// `<workspace> contains <repo>`; with the repo focused it reads as
/// `<repo> member of <workspace>`.
///
/// The catalog is exhaustive over `RelationKind` so adding a new
/// variant requires picking both direction's verbs at compile time.
pub fn directional_verb(relation: &RelationKind, direction: Direction) -> &'static str {
    use Direction::*;
    use RelationKind::*;
    match (relation, direction) {
        (AssociatedWith, _) => "associated with",
        (BelongsToRepo, Downstream) => "belongs to",
        (BelongsToRepo, Upstream) => "checked out at",
        (CheckedOutBranch, Downstream) => "on branch",
        (CheckedOutBranch, Upstream) => "checked out by",
        (WorkspaceContainsRepo, Downstream) => "contains",
        (WorkspaceContainsRepo, Upstream) => "member of",
        (BranchHasForgePr, Downstream) => "has PR",
        (BranchHasForgePr, Upstream) => "for branch",
        (LinkedToMux, Downstream) => "attached to",
        (LinkedToMux, Upstream) => "attached session",
        (RootedIn, Downstream) => "rooted in",
        (RootedIn, Upstream) => "hosts",
        (ForksWorkspace, Downstream) => "forks workspace",
        (ForksWorkspace, Upstream) => "forked by",
        (ForksRepo, Downstream) => "forks repo",
        (ForksRepo, Upstream) => "forked by",
        (CreatedCheckout, Downstream) => "created",
        (CreatedCheckout, Upstream) => "created by",
        (ReferencedCheckout, Downstream) => "references",
        (ReferencedCheckout, Upstream) => "referenced by",
        (ParentSession, Downstream) => "parent of",
        (ParentSession, Upstream) => "child of",
        (ChildSession, Downstream) => "child of",
        (ChildSession, Upstream) => "parent of",
        (CreatedBranch, Downstream) => "created branch",
        (CreatedBranch, Upstream) => "created by",
        (AssociatedBranch, Downstream) => "associated branch",
        (AssociatedBranch, Upstream) => "associated by",
        (ParentFork, Downstream) => "forked from",
        (ParentFork, Upstream) => "forked by",
        (RootedAtPath, Downstream) => "rooted at",
        (RootedAtPath, Upstream) => "hosts",
        (MuxContainsProcess, Downstream) => "contains process",
        (MuxContainsProcess, Upstream) => "in mux",
        (ProcessIdentifiesSession, Downstream) => "identifies",
        (ProcessIdentifiesSession, Upstream) => "identified by",
        (ProcessCandidatesSession, Downstream) => "candidate for",
        (ProcessCandidatesSession, Upstream) => "candidate process",
        (PinTargetsMux, Downstream) => "targets mux",
        (PinTargetsMux, Upstream) => "targeted by pin",
        (PinRealizedBySession, Downstream) => "realized by",
        (PinRealizedBySession, Upstream) => "realizes pin",
    }
}

/// Combined relationship explorer (ADR 0074). Groups carry direction
/// internally so a single explorer surface can hold both inbound and
/// outbound neighbors. The prior per-direction split lives on as a
/// transitional convenience via [`NodeView::upstream_groups`] /
/// [`NodeView::downstream_groups`] until passes 2 and 3 refactor the
/// reducer + renderer onto the flat list.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipExplorer {
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
        self.groups
            .iter()
            .filter(|g| g.unresolved_count > 0)
            .count()
    }
}

/// One row group in a [`RelationshipExplorer`]. The triple
/// `(direction, relation, neighbor_kind)` uniquely keys each group.
/// ADR 0074: `direction` lives on the group itself rather than being
/// implied by the parent explorer, so the merged explorer can mix
/// inbound and outbound groups in one ordered list.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationshipGroup {
    /// Whether the focused node sits at the link's source
    /// (`Direction::Downstream` — outbound) or target
    /// (`Direction::Upstream` — inbound) end. Drives the verb
    /// catalog ([`directional_verb`]) at render time.
    pub direction: Direction,
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

/// Stable identity for a flat selectable row in the explorer (ADR
/// 0074 §6). Used by the reducer to preserve cursor position across
/// rebuilds (snapshot refresh, drilldown). The two-zone layout
/// (validated / Other) collapses direction out of the key — the
/// row's verb carries it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExplorerRowKey {
    /// Title bar acts as the "go back to the inherent identity"
    /// landing target; not currently selectable but reserved.
    Title,
    /// One of the [`NodeView::core_fields`] rows (by label, so the
    /// key stays stable across renames).
    NodeField { label: String },
    /// A resolver-winner row in the validated zone.
    ValidatedLink { link_id: String },
    /// The collapsible `Other` header row.
    OtherHeader,
    /// A non-winner link row inside the `Other` zone
    /// (`EdgeStateLabel::AltOf` or `Conflict`).
    OtherLink { link_id: String },
    /// An unresolved-evidence placeholder inside the `Other` zone.
    OtherUnresolved { link_id: String },
}

/// Flat selectable row in the explorer. The reducer builds this
/// list from the [`NodeView`] plus the per-frame
/// `other_expanded` flag. The order matches what the renderer
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
    /// A resolver-winner row in the validated zone. `Enter` drills.
    ValidatedLink {
        group_index: usize,
        link_index: usize,
    },
    /// The collapsible `Other` header row. `Enter`/`e` toggles
    /// expansion.
    OtherHeader { expanded: bool },
    /// A non-winner link row inside the expanded `Other` zone.
    OtherLink {
        group_index: usize,
        link_index: usize,
    },
    /// An unresolved-evidence placeholder inside the expanded
    /// `Other` zone. `Enter` is inert in v1; `o` opens the
    /// evidence (see T8-032).
    OtherUnresolved {
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
            Self::ValidatedLink {
                group_index,
                link_index,
            } => {
                let link_id = view
                    .relationships
                    .groups
                    .get(*group_index)
                    .and_then(|g| g.links.get(*link_index))
                    .map(|l| l.link_id.clone())
                    .unwrap_or_default();
                ExplorerRowKey::ValidatedLink { link_id }
            }
            Self::OtherHeader { .. } => ExplorerRowKey::OtherHeader,
            Self::OtherLink {
                group_index,
                link_index,
            } => {
                let link_id = view
                    .relationships
                    .groups
                    .get(*group_index)
                    .and_then(|g| g.links.get(*link_index))
                    .map(|l| l.link_id.clone())
                    .unwrap_or_default();
                ExplorerRowKey::OtherLink { link_id }
            }
            Self::OtherUnresolved {
                group_index,
                unresolved_index,
            } => {
                let link_id = view
                    .relationships
                    .groups
                    .get(*group_index)
                    .and_then(|g| g.unresolved.get(*unresolved_index))
                    .map(|u| u.link_id.clone())
                    .unwrap_or_default();
                ExplorerRowKey::OtherUnresolved { link_id }
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

    /// True for rows that participate in the Other-zone toggle
    /// (header + its children). Used by the reducer to decide which
    /// row identifies the toggle target.
    pub fn is_other_header(&self) -> bool {
        matches!(self, Self::OtherHeader { .. })
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
    ///
    /// `expanded_detail` (T8-034) swaps the Node zone's top-5 render
    /// for the full per-kind field set when the operator has toggled
    /// the Expanded Node Detail view on. The relationship rows are
    /// unaffected.
    pub fn flat_rows(&self, other_expanded: bool, expanded_detail: bool) -> Vec<ExplorerRow> {
        let mut rows = Vec::new();
        for (index, field) in self.fields(expanded_detail).iter().enumerate() {
            rows.push(ExplorerRow::NodeField {
                index,
                label: field.label,
                long_value: field.long_value.clone(),
            });
        }
        // Validated zone: every link whose resolver state is
        // `Resolves` becomes a flat selectable row. The groups are
        // pre-sorted (ADR 0074 §4) so the rows inherit a stable
        // kind → verb → label scan order.
        for (group_index, group) in self.relationships.groups.iter().enumerate() {
            for (link_index, link) in group.links.iter().enumerate() {
                if matches!(link.edge_state, EdgeStateLabel::Resolves) {
                    rows.push(ExplorerRow::ValidatedLink {
                        group_index,
                        link_index,
                    });
                }
            }
        }
        // Other zone: alts, conflicts, and unresolved stubs under a
        // single collapsible header. Skip the header entirely when
        // the zone is empty so a fully-validated node doesn't carry
        // a dangling control row.
        let has_other = self.has_other_rows();
        if has_other {
            rows.push(ExplorerRow::OtherHeader {
                expanded: other_expanded,
            });
            if other_expanded {
                for (group_index, group) in self.relationships.groups.iter().enumerate() {
                    for (link_index, link) in group.links.iter().enumerate() {
                        if !matches!(link.edge_state, EdgeStateLabel::Resolves) {
                            rows.push(ExplorerRow::OtherLink {
                                group_index,
                                link_index,
                            });
                        }
                    }
                    for (unresolved_index, _) in group.unresolved.iter().enumerate() {
                        rows.push(ExplorerRow::OtherUnresolved {
                            group_index,
                            unresolved_index,
                        });
                    }
                }
            }
        }
        rows
    }

    /// Whether the focused node has any rows that belong in the
    /// `Other` zone — non-`Resolves` links or unresolved stubs.
    pub fn has_other_rows(&self) -> bool {
        self.relationships.groups.iter().any(|g| {
            g.unresolved_count > 0
                || g.links
                    .iter()
                    .any(|l| !matches!(l.edge_state, EdgeStateLabel::Resolves))
        })
    }

    /// Counts for the `Related` divider summary (ADR 0074 §5).
    pub fn relationship_counts(&self) -> RelationshipCounts {
        let mut counts = RelationshipCounts::default();
        for group in &self.relationships.groups {
            for link in &group.links {
                if matches!(link.edge_state, EdgeStateLabel::Resolves) {
                    counts.validated += 1;
                } else {
                    counts.other += 1;
                    if matches!(link.edge_state, EdgeStateLabel::Conflict) {
                        counts.ambiguous += 1;
                    }
                }
            }
            counts.other += group.unresolved.len();
            counts.unresolved += group.unresolved.len();
        }
        counts
    }

    /// Node-zone fields to render given the Expanded Node Detail
    /// toggle (T8-034). For node kinds whose `all_fields` equals
    /// `core_fields` the two returns are identical, so the toggle is
    /// a visual no-op on those kinds.
    pub fn fields(&self, expanded_detail: bool) -> &[CoreField] {
        if expanded_detail {
            &self.all_fields
        } else {
            &self.core_fields
        }
    }
}

/// Counts for the `Related` divider summary (ADR 0074 §5). Carried
/// by [`NodeView::relationship_counts`] so the renderer reads one
/// value rather than recomputing the loops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationshipCounts {
    pub validated: usize,
    pub other: usize,
    pub ambiguous: usize,
    pub unresolved: usize,
}

/// One frame of drill history. The reducer pushes a hop when the
/// operator presses `Enter` on a link row; Backspace pops the
/// most-recent hop and restores the saved cursor + expansion state.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbHop {
    /// The node the cursor was focused on *before* the drill that
    /// produced this hop.
    pub focused: NodeId,
    /// Compact `kind:short_tag` identity label for the hop (T8-038),
    /// rendered as one segment of the breadcrumb chain. Callers can
    /// disambiguate same-short-label collisions across the chain
    /// using [`render_breadcrumb_chain`], which suffixes the last-4
    /// of the focused node's display when two hops collide.
    pub short_label: String,
    /// Cursor row identity at the time of the drill, so Backspace
    /// can re-find it.
    pub cursor_key: Option<ExplorerRowKey>,
    /// Whether the `Other` zone was expanded at the time of the
    /// drill, so Backspace can restore it (ADR 0074 §6 — replaces
    /// the prior per-group expansion set).
    pub other_expanded: bool,
    /// Whether the Node zone had its Expanded Detail toggle on at
    /// the time of the drill (T8-034), so Backspace can restore it.
    pub full_detail_expanded: bool,
    /// Left-pane row selection at the time of the drill (T8-035 —
    /// `[tui.detail].left_pane_sync = "mirror"`), so Backspace can
    /// restore both panes together. `None` when no row was selected
    /// (the empty-tree case at boot).
    pub left_pane_selection: Option<crate::tui::rows::RowId>,
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
        let (group_index, link_index) = match row {
            ExplorerRow::ValidatedLink {
                group_index,
                link_index,
            }
            | ExplorerRow::OtherLink {
                group_index,
                link_index,
            } => (*group_index, *link_index),
            _ => return None,
        };
        let group = self.relationships.groups.get(group_index)?;
        let link = group.links.get(link_index)?;
        Some(link.neighbor_id.clone())
    }

    /// The preview content for a given cursor row — the neighbor's
    /// core fields plus an edge summary. Returns `None` for rows
    /// that don't carry a neighbor (node fields).
    pub fn row_preview(&self, row: &ExplorerRow) -> Option<RowPreview<'_>> {
        match row {
            ExplorerRow::NodeField { .. } | ExplorerRow::OtherHeader { .. } => None,
            ExplorerRow::ValidatedLink {
                group_index,
                link_index,
            }
            | ExplorerRow::OtherLink {
                group_index,
                link_index,
            } => {
                let group = self.relationships.groups.get(*group_index)?;
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
            ExplorerRow::OtherUnresolved {
                group_index,
                unresolved_index,
            } => {
                let group = self.relationships.groups.get(*group_index)?;
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

/// Format a breadcrumb chain for the right-pane title (T8-038).
/// Each hop renders as `<kind-glyph> <tag>` so the operator can
/// scan the drill chain by symbol rather than reading the verbose
/// `kind:tag` text form. The glyph is drawn in the kind color
/// (ADR 0073); the tag and separators use the theme's secondary
/// text color so the row reads as one quiet line with bursts of
/// kind identity. Hops that share a `short_label` get a `·xxxx`
/// suffix on the tag (last-4 of the focused node's display) so
/// two same-named hops disambiguate. Elision keeps the chain
/// inside `available` cells: full chain → `first … last` →
/// `… last` → just `last` as the chain narrows. Returns `None`
/// when `hops` is empty so callers can skip the breadcrumb glyph
/// entirely.
pub fn render_breadcrumb_chain(
    hops: &[BreadcrumbHop],
    theme: &Theme,
    available: usize,
) -> Option<Line<'static>> {
    if hops.is_empty() {
        return None;
    }
    let rendered = build_breadcrumb_hops(hops, theme);
    Some(build_breadcrumb_line(&rendered, theme, available))
}

/// Per-hop render data. Owns the glyph + tag strings plus the
/// effective color so the rendering pass can splice spans without
/// re-resolving the theme. `width` is the display-cell count for
/// `<glyph> <tag>` so elision can sum cells across hops.
#[derive(Clone, Debug, PartialEq)]
struct RenderedBreadcrumbHop {
    glyph: String,
    glyph_color: Color,
    tag: String,
    width: usize,
}

fn build_breadcrumb_hops(hops: &[BreadcrumbHop], theme: &Theme) -> Vec<RenderedBreadcrumbHop> {
    let mut counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::with_capacity(hops.len());
    for hop in hops {
        *counts.entry(hop.short_label.as_str()).or_insert(0) += 1;
    }
    hops.iter()
        .map(|hop| {
            let kind = NodeKind::from(&hop.focused);
            let style = node_kind_style(kind, theme);
            // ForgePr's slate color is `Color::Reset` (the caller
            // is supposed to pick `theme.pr_open` / `pr_closed` /
            // `pr_merged` / `pr_draft` from PR state). The
            // breadcrumb does not have PR state on hand, so fall
            // back to `pr_open` — the same dodge `kind_chip_span`
            // uses for the same reason.
            let color = if matches!(kind, NodeKind::ForgePr) {
                theme.pr_open
            } else {
                style.color
            };
            let raw_tag = hop
                .short_label
                .split_once(':')
                .map(|(_, tag)| tag)
                .unwrap_or(hop.short_label.as_str());
            let tag = if counts.get(hop.short_label.as_str()).copied().unwrap_or(0) > 1 {
                let id_text = hop.focused.to_string();
                let tail: String = id_text.chars().rev().take(4).collect();
                let tail: String = tail.chars().rev().collect();
                format!("{raw_tag}·{tail}")
            } else {
                raw_tag.to_string()
            };
            let width = style.width + 1 + UnicodeWidthStr::width(tag.as_str());
            RenderedBreadcrumbHop {
                glyph: style.glyph,
                glyph_color: color,
                tag,
                width,
            }
        })
        .collect()
}

const BREADCRUMB_SEPARATOR: &str = " › ";
const BREADCRUMB_SEPARATOR_WIDTH: usize = 3;
const BREADCRUMB_ELLIPSIS: &str = "…";
const BREADCRUMB_ELLIPSIS_WIDTH: usize = 1;

fn build_breadcrumb_line(
    rendered: &[RenderedBreadcrumbHop],
    theme: &Theme,
    available: usize,
) -> Line<'static> {
    let secondary = Style::default().fg(theme.secondary_text);
    let full_width: usize = rendered.iter().map(|h| h.width).sum::<usize>()
        + BREADCRUMB_SEPARATOR_WIDTH * rendered.len().saturating_sub(1);
    if full_width <= available || rendered.len() <= 1 {
        return spans_for_hops(rendered, secondary);
    }
    // Try `first <SEP> … <SEP> last`.
    let first = rendered.first().expect("rendered non-empty");
    let last = rendered.last().expect("rendered non-empty");
    let with_first =
        first.width + last.width + BREADCRUMB_ELLIPSIS_WIDTH + BREADCRUMB_SEPARATOR_WIDTH * 2;
    if with_first <= available {
        return Line::from(vec![
            styled_glyph(first, secondary),
            Span::raw(" "),
            Span::styled(first.tag.clone(), secondary),
            Span::styled(BREADCRUMB_SEPARATOR, secondary),
            Span::styled(BREADCRUMB_ELLIPSIS, secondary),
            Span::styled(BREADCRUMB_SEPARATOR, secondary),
            styled_glyph(last, secondary),
            Span::raw(" "),
            Span::styled(last.tag.clone(), secondary),
        ]);
    }
    // Try `… <SEP> last`.
    let only_last = last.width + BREADCRUMB_ELLIPSIS_WIDTH + BREADCRUMB_SEPARATOR_WIDTH;
    if only_last <= available {
        return Line::from(vec![
            Span::styled(BREADCRUMB_ELLIPSIS, secondary),
            Span::styled(BREADCRUMB_SEPARATOR, secondary),
            styled_glyph(last, secondary),
            Span::raw(" "),
            Span::styled(last.tag.clone(), secondary),
        ]);
    }
    // Last-resort: just the last hop, no elision marker.
    Line::from(vec![
        styled_glyph(last, secondary),
        Span::raw(" "),
        Span::styled(last.tag.clone(), secondary),
    ])
}

fn spans_for_hops(rendered: &[RenderedBreadcrumbHop], secondary: Style) -> Line<'static> {
    let mut spans = Vec::with_capacity(rendered.len() * 4);
    for (idx, hop) in rendered.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(BREADCRUMB_SEPARATOR, secondary));
        }
        spans.push(styled_glyph(hop, secondary));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(hop.tag.clone(), secondary));
    }
    Line::from(spans)
}

fn styled_glyph(hop: &RenderedBreadcrumbHop, _secondary: Style) -> Span<'static> {
    Span::styled(hop.glyph.clone(), Style::default().fg(hop.glyph_color))
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
        GraphNode::Pin(_) => "pin",
        GraphNode::RuntimeProcess(_) => "runtime_process",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

/// Reverse-lookup helper for the cwd field (T8-039): returns the
/// owning node's kind label when `cwd` matches a Repo, Workspace,
/// or Checkout in the snapshot. Match precedence is Checkout (most
/// specific) → Workspace → Repo (matches by `common_dir` or any
/// `source_paths` entry). Returns `None` when nothing in the
/// snapshot claims the path, in which case the renderer leaves the
/// cwd value bare rather than guessing.
fn cwd_owner_kind(snapshot: &GraphSnapshot, cwd: &str) -> Option<&'static str> {
    let cwd = cwd.trim_end_matches('/');
    if cwd.is_empty() {
        return None;
    }
    let matches = |candidate: &str| -> bool { candidate.trim_end_matches('/') == cwd };
    let mut found_checkout = false;
    let mut found_workspace = false;
    let mut found_repo = false;
    for node in &snapshot.nodes {
        match node {
            GraphNode::Checkout(c) if matches(&c.root) => {
                found_checkout = true;
            }
            GraphNode::Workspace(w) if matches(&w.root) => {
                found_workspace = true;
            }
            GraphNode::Repo(r) => {
                if matches(&r.common_dir) || r.source_paths.iter().any(|p| matches(p)) {
                    found_repo = true;
                }
            }
            _ => {}
        }
    }
    if found_checkout {
        Some("checkout")
    } else if found_workspace {
        Some("workspace")
    } else if found_repo {
        Some("repo")
    } else {
        None
    }
}

/// Compact `kind:short_tag` label for a node, used in breadcrumb
/// hops (T8-038) so a deep drill chain stays visible at a glance.
/// Pure: no snapshot / alias context, no truncation against terminal
/// width — callers handle elision over the rendered chain.
pub fn short_node_label(node: &GraphNode) -> String {
    fn basename(path: &str) -> &str {
        path.rsplit(['/', '\\'])
            .find(|seg| !seg.is_empty())
            .unwrap_or(path)
    }
    match node {
        GraphNode::AgentSession(s) => {
            let tag =
                s.id.session_key
                    .rsplit(['-', '/'])
                    .find(|seg| !seg.is_empty())
                    .unwrap_or(s.id.session_key.as_str());
            let tag = if tag.chars().count() > 8 {
                &tag[tag.len().saturating_sub(8)..]
            } else {
                tag
            };
            format!("session:{tag}")
        }
        GraphNode::MuxSession(m) => format!("mux:{}", m.native_id),
        GraphNode::Pin(p) => format!("pin:{}", truncate(&p.display_name, 16)),
        GraphNode::RuntimeProcess(p) => {
            let head = p
                .command
                .as_deref()
                .map(|cmd| basename(cmd.split_whitespace().next().unwrap_or(cmd)).to_string())
                .or_else(|| p.pid.map(|pid| format!("pid {pid}")))
                .unwrap_or_else(|| basename(&p.observation_key).to_string());
            let head = truncate(&head, 14);
            format!("proc:{head}")
        }
        GraphNode::ForgePr(pr) => format!("pr:{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(f) => {
            let tag = f.name.as_deref().unwrap_or(f.provider_source_key.as_str());
            format!("fork:{}", truncate(tag, 16))
        }
        GraphNode::Repo(r) => format!("repo:{}", basename(&r.common_dir)),
        GraphNode::Checkout(c) => format!("co:{}", basename(&c.root)),
        GraphNode::Workspace(w) => {
            let tag = w.name.as_deref().unwrap_or_else(|| basename(&w.root));
            format!("ws:{tag}")
        }
        GraphNode::Branch(b) => format!("branch:{}", b.refname),
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
        GraphNode::Pin(p) => format!("pin:{}", p.display_name),
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
        GraphNode::Workspace(w) => w.name.clone().unwrap_or_else(|| w.root.clone()),
        GraphNode::Branch(b) => b.refname.clone(),
    }
}

fn core_fields(snapshot: &GraphSnapshot, node: &GraphNode, home: Option<&Path>) -> Vec<CoreField> {
    match node {
        GraphNode::Repo(r) => repo_core(r, home),
        GraphNode::Checkout(c) => checkout_core(c, home),
        GraphNode::Workspace(w) => workspace_core(w, home),
        GraphNode::AgentSession(s) => agent_session_core(snapshot, s, home),
        GraphNode::MuxSession(m) => mux_session_core(snapshot, m, home),
        GraphNode::Pin(p) => pin_core(p, home),
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

fn extra_fields(snapshot: &GraphSnapshot, node: &GraphNode, home: Option<&Path>) -> Vec<CoreField> {
    match node {
        GraphNode::AgentSession(s) => agent_session_extras(snapshot, s, home),
        GraphNode::MuxSession(m) => mux_session_extras(m),
        GraphNode::Pin(p) => pin_extras(p, home),
        GraphNode::RuntimeProcess(p) => runtime_process_extras(snapshot, p, home),
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
        CoreField::plain("id", s.id.session_key.clone()),
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
    } else if let Some(title) = s.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let truncated = truncate(title, 48);
        let mut field = CoreField::plain("title", truncated.clone());
        if truncated.len() < title.len() {
            field = field.with_long(title.to_string());
        }
        fields.push(field);
    } else {
        fields.push(CoreField::placeholder("alias", "—"));
    }
    fields.push(match &s.cwd {
        Some(cwd) => {
            let field = CoreField::plain("cwd", shorten_home(cwd, home));
            match cwd_owner_kind(snapshot, cwd) {
                Some(kind) => field.with_kind_chip(kind),
                None => field,
            }
        }
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
    if let Some(title) = s.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
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

fn mux_session_core(
    snapshot: &GraphSnapshot,
    m: &MuxSessionNode,
    home: Option<&Path>,
) -> Vec<CoreField> {
    // `id` carries the external mux session name (e.g. the raw tmux
    // session id), mirroring the agent-session detail's external
    // `id` field. The internal backend-prefixed graph id stays
    // available through `NodeId::MuxSession`; surfacing it here
    // would duplicate `backend` + `id`.
    let mut fields = vec![
        CoreField::plain("id", m.native_id.clone()),
        CoreField::plain("backend", m.backend.clone()),
    ];
    fields.push(match &m.cwd {
        Some(cwd) => {
            let field = CoreField::plain("cwd", shorten_home(cwd, home));
            match cwd_owner_kind(snapshot, cwd) {
                Some(kind) => field.with_kind_chip(kind),
                None => field,
            }
        }
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

fn pin_core(p: &PinNode, home: Option<&Path>) -> Vec<CoreField> {
    vec![
        CoreField::plain("id", p.id.id.clone()),
        CoreField::plain("name", p.display_name.clone()),
        CoreField::plain("harness", p.harness.clone()),
        CoreField::plain("cwd", shorten_home(&p.cwd, home)),
        CoreField::plain("mux", p.mux.native_id()),
    ]
}

fn pin_extras(p: &PinNode, home: Option<&Path>) -> Vec<CoreField> {
    let mut fields = vec![
        CoreField::plain("store", shorten_home(&p.store_path, home)),
        CoreField::plain("source", p.provenance.snake_case().to_string()),
    ];
    if let Some(argv) = &p.launch_argv
        && !argv.is_empty()
    {
        fields.push(CoreField::plain("launch", argv.join(" ")));
    }
    if let Some(reason) = &p.reason {
        fields.push(CoreField::plain("reason", reason.clone()));
    }
    fields.push(CoreField::plain(
        "full_id",
        format!("{}", NodeId::Pin(p.id.clone())),
    ));
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
        kind_chip: None,
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
            Some(RuntimeProcessRole::Background) => "background",
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

fn runtime_process_extras(
    snapshot: &GraphSnapshot,
    p: &RuntimeProcessNode,
    home: Option<&Path>,
) -> Vec<CoreField> {
    let mut fields = Vec::new();
    if let Some(cwd) = &p.cwd {
        let field = CoreField::plain("cwd", shorten_home(cwd, home));
        let field = match cwd_owner_kind(snapshot, cwd) {
            Some(kind) => field.with_kind_chip(kind),
            None => field,
        };
        fields.push(field);
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
    let composite = format!("{}/{}#{} ({state})", pr.owner, pr.repo, pr.number);
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
    RelationshipExplorer { groups }
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

    // The resolver keys multi-target relations (`AssociatedWith`,
    // `WorkspaceContainsRepo`, `MuxContainsProcess`, …) by
    // `(source, relation, target)` so each neighbor target gets its
    // own `ResolvedRelationship` with its own winner. Collect every
    // slot for `(focused, relation)` matching this direction so all
    // per-target winners land in the validated zone, not just the
    // first one.
    let resolved_slots = resolved_slots_for(snapshot, focused, direction, &relation);

    // ADR 0077: `selected_link_id` is now `Option<String>`.
    // `filter_map` skips no-winner slots when collecting winners
    // so the validated-zone set stays empty for ambiguous slots —
    // every candidate in such a slot drops into the Other zone
    // below.
    let winner_ids: BTreeSet<String> = resolved_slots
        .iter()
        .filter_map(|r| r.selected_link_id.clone())
        .collect();
    let competing_ids: BTreeSet<String> = resolved_slots
        .iter()
        .flat_map(|r| r.competing_link_ids.iter().cloned())
        .collect();
    let any_slot_has_conflict = resolved_slots
        .iter()
        .any(|r| !r.competing_link_ids.is_empty());
    // H-UI-006 retired the H-UI-007 candidate-fan-out fallback:
    // the resolver now keeps the slot alive with
    // `selected_link_id = None` for `suppress_ambiguous_cwd_mux_links`,
    // so "has any slot in this group been ambiguously resolved"
    // is the direct read. A no-winner slot signals ambiguity even
    // if its `competing_link_ids` happens to be empty (the slot
    // itself is the signal).
    let any_slot_unresolved = resolved_slots.iter().any(|r| r.selected_link_id.is_none());
    let ambiguous = any_slot_has_conflict || any_slot_unresolved;

    links.sort_by(|(left, _), (right, _)| {
        let left_winner = winner_ids.contains(&left.id);
        let right_winner = winner_ids.contains(&right.id);
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
            let resolved_winner = winner_ids.contains(&link.id);
            let edge_state = if resolved_winner {
                EdgeStateLabel::Resolves
            } else if competing_ids.contains(&link.id) {
                EdgeStateLabel::Conflict
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
        direction,
        relation,
        neighbor_kind,
        links: link_rows,
        unresolved: unresolved_rows,
        ambiguous,
        unresolved_count,
    }
}

/// Every `ResolvedRelationship` slot the focused node owns for this
/// `(relation, direction)` pair. The resolver keys multi-target
/// relations (`AssociatedWith`, `WorkspaceContainsRepo`,
/// `MuxContainsProcess`, `ProcessIdentifiesSession`,
/// `ProcessCandidatesSession`) by `(source, relation, target)`, so a
/// workspace with three `WorkspaceContainsRepo` repos yields three
/// slots — each its own winner. Returning the full set lets the
/// detail-pane mark every per-target winner as `Resolves` instead of
/// privileging whichever slot happens to come first in iteration order.
fn resolved_slots_for<'a>(
    snapshot: &'a GraphSnapshot,
    focused: &NodeId,
    direction: Direction,
    relation: &RelationKind,
) -> Vec<&'a ResolvedRelationship> {
    snapshot
        .resolved_relationships
        .iter()
        .filter(|rel| match direction {
            Direction::Upstream => &rel.target == focused && &rel.relation == relation,
            Direction::Downstream => &rel.source == focused && &rel.relation == relation,
        })
        .collect()
}

fn neighbor_display_label(node: &GraphNode, home: Option<&Path>) -> String {
    match node {
        GraphNode::AgentSession(s) => format!("{}:{}", s.harness_key, s.id.session_key),
        GraphNode::MuxSession(m) => format!("{}:{}", m.backend, m.native_id),
        GraphNode::Pin(p) => format!("pin:{}", p.display_name),
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
        GraphNode::Workspace(w) => w
            .name
            .clone()
            .unwrap_or_else(|| shorten_home(&w.root, home)),
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
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, ForgePrId,
        ForgePrNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        Provenance, RepoId, RepoNode, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole,
        SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    #[test]
    fn directional_verb_catalog_is_exhaustive_and_disambiguates_inverses() {
        // ADR 0074 §2: every RelationKind has a verb pair, and for
        // every asymmetric relation the two verbs differ. The
        // exhaustive match in `directional_verb` keeps this catalog
        // honest at compile time; the assertions below pin a few
        // anchor cases so a future verb rewrite doesn't accidentally
        // collapse the direction signal.
        let pairs = [
            (RelationKind::WorkspaceContainsRepo, "contains", "member of"),
            (RelationKind::LinkedToMux, "attached to", "attached session"),
            (RelationKind::ParentFork, "forked from", "forked by"),
            (RelationKind::ChildSession, "child of", "parent of"),
            (RelationKind::ParentSession, "parent of", "child of"),
            (
                RelationKind::MuxContainsProcess,
                "contains process",
                "in mux",
            ),
            (RelationKind::BranchHasForgePr, "has PR", "for branch"),
        ];
        for (rel, out, inn) in pairs {
            assert_eq!(directional_verb(&rel, Direction::Downstream), out);
            assert_eq!(directional_verb(&rel, Direction::Upstream), inn);
            assert_ne!(
                directional_verb(&rel, Direction::Downstream),
                directional_verb(&rel, Direction::Upstream),
                "asymmetric relation {rel:?} should have distinct verbs per direction",
            );
        }
        // `AssociatedWith` is the documented symmetric relation; the
        // ADR explicitly accepts the same verb in both directions
        // until a real operator confusion materializes.
        assert_eq!(
            directional_verb(&RelationKind::AssociatedWith, Direction::Downstream),
            directional_verb(&RelationKind::AssociatedWith, Direction::Upstream),
        );
    }

    #[test]
    fn flat_rows_splits_validated_and_other_zones() {
        // ADR 0074 §3: the validated zone holds every `Resolves` row
        // in a flat list; alternates, conflicts, and unresolved
        // stubs sit under one collapsible `Other` header below.
        // This test exercises the split via a multi-candidate
        // LinkedToMux setup: the resolver picks one winner, the
        // other candidate becomes an Other row.
        let mut snapshot = GraphSnapshot::empty();
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
        snapshot
            .nodes
            .push(agent("claude", "abc", Some("/x"), None));
        snapshot.nodes.push(mux("tmux", "primary", Some("/x")));
        snapshot.nodes.push(mux("tmux", "secondary", Some("/x")));
        let cwd_link = |id: &str, target: NodeId| {
            let mut l = link(id, session_id.clone(), target, RelationKind::LinkedToMux);
            l.confidence = Confidence::High;
            l.source_metadata.evidence = Some("exact_cwd_match".to_string());
            l
        };
        snapshot.candidate_links.push(cwd_link(
            "to-primary",
            NodeId::MuxSession(MuxSessionId::new("primary")),
        ));
        snapshot.candidate_links.push(cwd_link(
            "to-secondary",
            NodeId::MuxSession(MuxSessionId::new("secondary")),
        ));
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &session_id, Some(home().as_path()));
        let counts = view.relationship_counts();
        assert!(
            counts.validated >= 1,
            "at least one resolver winner expected: {counts:?}"
        );
        assert!(
            counts.other >= 1,
            "non-winning candidate should land in the Other zone: {counts:?}"
        );

        // Collapsed Other: only the validated rows + the header
        // appear; expanding adds the Other children.
        let collapsed = view.flat_rows(false, false);
        let validated_in_collapsed = collapsed
            .iter()
            .filter(|r| matches!(r, ExplorerRow::ValidatedLink { .. }))
            .count();
        let other_header_in_collapsed = collapsed
            .iter()
            .filter(|r| matches!(r, ExplorerRow::OtherHeader { .. }))
            .count();
        let other_children_in_collapsed = collapsed
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    ExplorerRow::OtherLink { .. } | ExplorerRow::OtherUnresolved { .. }
                )
            })
            .count();
        assert_eq!(validated_in_collapsed, counts.validated);
        assert_eq!(other_header_in_collapsed, 1);
        assert_eq!(
            other_children_in_collapsed, 0,
            "Other children should not appear while the zone is collapsed"
        );

        let expanded = view.flat_rows(true, false);
        let other_children_in_expanded = expanded
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    ExplorerRow::OtherLink { .. } | ExplorerRow::OtherUnresolved { .. }
                )
            })
            .count();
        assert_eq!(other_children_in_expanded, counts.other);
    }

    #[test]
    fn relationship_groups_sort_by_kind_then_verb_then_label() {
        // ADR 0074 §4: rows in the Related zone scan top-to-bottom
        // as kind → verb → neighbor label. The merged-group sort in
        // `sort_relationship_groups` is the source of truth; this
        // test pins the contract by checking the kind ordinals are
        // monotonically non-decreasing across the merged list.
        let mut snapshot = GraphSnapshot::empty();
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
        let workspace_id = NodeId::Workspace(WorkspaceId::new("/ws"));
        let repo_id = NodeId::Repo(RepoId::new("/r/.git"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        snapshot
            .nodes
            .push(agent("claude", "abc", Some("/r"), None));
        snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new("/ws"),
            root: "/ws".to_string(),
            provider: None,
            name: None,
        }));
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))));
        snapshot.nodes.push(mux("tmux", "editor", Some("/r")));
        snapshot.candidate_links.push(link(
            "to-workspace",
            session_id.clone(),
            workspace_id.clone(),
            RelationKind::AssociatedWith,
        ));
        snapshot.candidate_links.push(link(
            "to-repo",
            session_id.clone(),
            repo_id.clone(),
            RelationKind::AssociatedWith,
        ));
        snapshot.candidate_links.push(link(
            "to-mux",
            session_id.clone(),
            mux_id.clone(),
            RelationKind::LinkedToMux,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &session_id, Some(home().as_path()));
        let ordinals: Vec<usize> = view
            .relationships
            .groups
            .iter()
            .map(|g| {
                crate::tui::icons::NodeKind::from_snake_case(&g.neighbor_kind)
                    .map(|k| k.ordinal())
                    .unwrap_or(usize::MAX)
            })
            .collect();
        for pair in ordinals.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "kind ordinals should not decrease across the sorted group list: {ordinals:?}"
            );
        }
        // Workspace (ordinal 0) sorts before Repo (1) sorts before
        // MuxSession (4).
        let kinds: Vec<&str> = view
            .relationships
            .groups
            .iter()
            .map(|g| g.neighbor_kind.as_str())
            .collect();
        let ws = kinds.iter().position(|k| *k == "workspace");
        let repo = kinds.iter().position(|k| *k == "repo");
        let mux = kinds.iter().position(|k| *k == "mux_session");
        if let (Some(ws), Some(repo)) = (ws, repo) {
            assert!(ws < repo, "workspace should sort before repo: {kinds:?}");
        }
        if let (Some(repo), Some(mux)) = (repo, mux) {
            assert!(repo < mux, "repo should sort before mux: {kinds:?}");
        }
    }

    #[test]
    fn build_node_view_merges_directions_into_single_relationships() {
        // ADR 0074 pass 1: the data shape collapses to one
        // `relationships` field on NodeView, with each group
        // carrying its own direction. The `upstream()` /
        // `downstream()` helpers project filtered views off the
        // combined list for transitional consumers (passes 2 / 3
        // remove these helpers entirely). This test pins both
        // halves to prove the collapse + projection round-trip.
        let mut snapshot = GraphSnapshot::empty();
        let session_id = NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc"));
        snapshot
            .nodes
            .push(agent("claude", "abc", Some("/x"), None));
        snapshot.nodes.push(mux("tmux", "editor", Some("/x")));
        snapshot.candidate_links.push(link(
            "session->mux",
            session_id.clone(),
            NodeId::MuxSession(MuxSessionId::new("editor")),
            RelationKind::LinkedToMux,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &session_id, Some(home().as_path()));
        // Combined list carries the LinkedToMux group with
        // direction marked as Downstream (focus = source).
        assert_eq!(view.relationships.groups.len(), 1);
        assert_eq!(
            view.relationships.groups[0].direction,
            Direction::Downstream
        );
        assert_eq!(
            view.relationships.groups[0].relation,
            RelationKind::LinkedToMux
        );

        // Direction filtering on the combined list still works.
        let upstream_count = view
            .relationships
            .groups
            .iter()
            .filter(|g| g.direction == Direction::Upstream)
            .count();
        let downstream_count = view
            .relationships
            .groups
            .iter()
            .filter(|g| g.direction == Direction::Downstream)
            .count();
        assert_eq!(upstream_count, 0);
        assert_eq!(downstream_count, 1);
    }

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    /// Test helper that re-projects the combined `relationships`
    /// list back to a single-direction `RelationshipExplorer` so
    /// the legacy upstream/downstream assertions in this file keep
    /// reading naturally without recomputing the filter at every
    /// call site.
    fn filter_direction(view: &NodeView, direction: Direction) -> RelationshipExplorer {
        RelationshipExplorer {
            groups: view
                .relationships
                .groups
                .iter()
                .filter(|g| g.direction == direction)
                .cloned()
                .collect(),
        }
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
        let id = view
            .core_fields
            .iter()
            .find(|f| f.label == "id")
            .expect("id field");
        assert_eq!(id.value, "abc");
        assert!(view.relationships.groups.is_empty());
    }

    #[test]
    fn agent_session_core_id_shows_full_external_session_key() {
        let long_id = "ffffffff-1111-2222-3333-444444444444";
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("opencode", long_id, Some("/home/op/src/x"), None));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", long_id));

        let view = build(&snapshot, &target, Some(home().as_path()));
        let id = view
            .core_fields
            .iter()
            .find(|f| f.label == "id")
            .expect("id field");
        assert_eq!(id.value, long_id);
    }

    #[test]
    fn agent_session_title_is_not_labeled_as_alias_in_core_fields() {
        let long_title = "The conspectus TUI, fashioned after a long prompt, should remain a title";
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "codex",
            "abc",
            Some("/home/op/src/x"),
            Some(long_title),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

        let view = build(&snapshot, &target, Some(home().as_path()));
        let labels: Vec<&str> = view.core_fields.iter().map(|f| f.label).collect();
        assert_eq!(labels, vec!["id", "harness", "title", "cwd", "status"]);
        let title = view
            .core_fields
            .iter()
            .find(|f| f.label == "title")
            .expect("title field");
        assert!(title.value.contains('…'));
        assert_eq!(title.long_value.as_deref(), Some(long_title));
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
        let downstream = filter_direction(&view, Direction::Downstream);
        assert_eq!(downstream.groups.len(), 1);
        let group = &downstream.groups[0];
        assert_eq!(group.relation, RelationKind::LinkedToMux);
        assert_eq!(group.neighbor_kind, "mux_session");
        assert!(group.is_single());
        assert_eq!(group.links.len(), 1);
        assert!(group.links[0].resolved_winner);
        assert_eq!(group.links[0].edge_state, EdgeStateLabel::Resolves);
        // Preview carries mux core fields. The `id` field shows the
        // external mux name and `backend` carries the backend label
        // (e.g. `tmux`), mirroring the agent-session detail layout.
        let labels: Vec<&str> = group.links[0].preview.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec!["id", "backend", "cwd", "attached", "last_active"]
        );
        let id_field = group.links[0]
            .preview
            .iter()
            .find(|f| f.label == "id")
            .expect("id field");
        let backend_field = group.links[0]
            .preview
            .iter()
            .find(|f| f.label == "backend")
            .expect("backend field");
        assert_eq!(id_field.value, "work-claude");
        assert_eq!(backend_field.value, "tmux");
    }

    #[test]
    fn linked_to_mux_suppressed_slot_surfaces_as_no_winner_ambiguous_group() {
        // H-UI-006 (ADR 0077) retires the H-UI-007 candidate-fan-out
        // fallback: the resolver now preserves the suppressed
        // `LinkedToMux` slot with `selected_link_id = None` and the
        // candidate set rolled into `competing_link_ids`. The
        // explorer reads ambiguity directly off the slot now —
        // every candidate row drops into the Other zone, the group
        // is marked ambiguous, and no validated row exists.
        let mut snapshot = GraphSnapshot::empty();
        let cwd = Some("/home/op/src/x");
        snapshot
            .nodes
            .push(agent("claude-code", "focused", cwd, None));
        snapshot
            .nodes
            .push(agent("claude-code", "other", cwd, None));
        snapshot.nodes.push(mux("tmux", "project", cwd));
        snapshot.nodes.push(mux("tmux", "ambiguous", cwd));

        let focused_id =
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "focused"));
        let other_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "other"));
        let mux_a = NodeId::MuxSession(MuxSessionId::new("project"));
        let mux_b = NodeId::MuxSession(MuxSessionId::new("ambiguous"));

        let cwd_link = |id: &str, source: NodeId, target: NodeId| {
            let mut l = link(id, source, target, RelationKind::LinkedToMux);
            // `suppress_ambiguous_cwd_mux_links` keys off
            // `match_kind == exact_cwd_match` to recognize cwd
            // evidence; both `fields` and `evidence` are checked.
            l.source_metadata.evidence = Some("exact_cwd_match".to_string());
            l
        };

        snapshot
            .candidate_links
            .push(cwd_link("focused-a", focused_id.clone(), mux_a.clone()));
        snapshot
            .candidate_links
            .push(cwd_link("focused-b", focused_id.clone(), mux_b.clone()));
        snapshot
            .candidate_links
            .push(cwd_link("other-a", other_id.clone(), mux_a.clone()));
        snapshot
            .candidate_links
            .push(cwd_link("other-b", other_id.clone(), mux_b.clone()));

        let snapshot = resolve_snapshot(snapshot);

        // Precondition (ADR 0077): the resolver preserves the
        // `LinkedToMux` slot for the focused session but flips
        // `selected_link_id` to `None`. The slot survives so
        // downstream consumers can read ambiguity off the model.
        let focused_mux_slot = snapshot
            .resolved_relationships
            .iter()
            .find(|r| r.source == focused_id && r.relation == RelationKind::LinkedToMux)
            .expect("suppression preserves the LinkedToMux slot for the focused session");
        assert!(
            focused_mux_slot.selected_link_id.is_none(),
            "suppressed slot must carry no winner: {focused_mux_slot:?}",
        );
        assert!(
            focused_mux_slot.competing_link_ids.len() >= 2,
            "the candidate set rolls into competing_link_ids: {focused_mux_slot:?}",
        );

        let view = build(&snapshot, &focused_id, Some(home().as_path()));
        let group = filter_direction(&view, Direction::Downstream)
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::LinkedToMux)
            .expect("LinkedToMux group should render against the preserved slot")
            .clone();
        assert!(
            group.ambiguous,
            "no-winner slot must mark the group ambiguous",
        );
        assert!(
            group.links.len() >= 2,
            "both candidate targets should still appear as rows: {:?}",
            group.links,
        );
        assert!(
            group
                .links
                .iter()
                .all(|l| !matches!(l.edge_state, EdgeStateLabel::Resolves)),
            "no candidate is a winner inside a no-winner slot: {:?}",
            group.links,
        );
    }

    #[test]
    fn multi_process_candidates_flag_ambiguity() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/home/op/src/x"), None));
        snapshot
            .nodes
            .push(process("obs:1", 100, "/usr/bin/claude"));
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
        let upstream = filter_direction(&view, Direction::Upstream);
        assert_eq!(upstream.groups.len(), 2);
        let identifies = upstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::ProcessIdentifiesSession)
            .expect("identifies group");
        assert!(identifies.is_single());
        assert!(identifies.links[0].resolved_winner);
        assert_eq!(identifies.links[0].edge_state, EdgeStateLabel::Resolves);

        let candidates = upstream
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
    fn workspace_contains_multiple_repos_all_validated() {
        // Regression: the resolver keys `WorkspaceContainsRepo` by
        // `(source, relation, target)` so a workspace with three
        // distinct repo edges yields three independent winners.
        // The detail-pane explorer used to collapse them under a
        // single `resolved_for` lookup, leaving only one repo in the
        // validated zone and demoting the other two to the `Other`
        // chevron. Every per-target winner must land in the
        // validated zone with `EdgeStateLabel::Resolves`.
        let mut snapshot = GraphSnapshot::empty();
        let workspace_id = WorkspaceId::new("/home/op/work/multi");
        snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: "/home/op/work/multi".to_string(),
            provider: None,
            name: Some("multi".to_string()),
        }));
        let repos = ["/srv/git/a.git", "/srv/git/b.git", "/srv/git/c.git"];
        for common_dir in repos {
            snapshot.nodes.push(GraphNode::Repo(RepoNode {
                id: RepoId::new(common_dir),
                common_dir: common_dir.to_string(),
                source_paths: Vec::new(),
                remotes: Vec::new(),
            }));
        }
        let workspace = NodeId::Workspace(workspace_id);
        for (idx, common_dir) in repos.iter().enumerate() {
            snapshot.candidate_links.push(link(
                &format!("l{idx}"),
                workspace.clone(),
                NodeId::Repo(RepoId::new(*common_dir)),
                RelationKind::WorkspaceContainsRepo,
            ));
        }
        let snapshot = resolve_snapshot(snapshot);

        // Sanity: the resolver emitted one slot per repo.
        let workspace_slots = snapshot
            .resolved_relationships
            .iter()
            .filter(|r| r.source == workspace && r.relation == RelationKind::WorkspaceContainsRepo)
            .count();
        assert_eq!(
            workspace_slots, 3,
            "resolver should emit one WorkspaceContainsRepo slot per target repo",
        );

        let view = build(&snapshot, &workspace, Some(home().as_path()));
        let downstream = filter_direction(&view, Direction::Downstream);
        let group = downstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::WorkspaceContainsRepo)
            .expect("WorkspaceContainsRepo group");
        assert_eq!(group.links.len(), 3);
        for row in &group.links {
            assert!(
                row.resolved_winner,
                "every per-target winner must surface as resolved: {row:?}"
            );
            assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
        }
        assert!(
            !view.has_other_rows(),
            "no candidate should fall into the Other zone when every slot has a winner",
        );
        assert!(
            !group.ambiguous,
            "distinct per-target winners are not ambiguous",
        );
        let counts = view.relationship_counts();
        assert_eq!(counts.validated, 3);
        assert_eq!(counts.other, 0);
    }

    #[test]
    fn workspace_associated_with_multiple_repos_all_validated() {
        // Symmetric coverage for `AssociatedWith`: a workspace that
        // declares an association with several repos should show
        // every repo as validated rather than demoting all but one
        // to `Other`. `AssociatedWith` is also part of the
        // `multi_target_relation` set on the resolver side.
        let mut snapshot = GraphSnapshot::empty();
        let workspace_id = WorkspaceId::new("/home/op/work/assoc");
        snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: workspace_id.clone(),
            root: "/home/op/work/assoc".to_string(),
            provider: None,
            name: Some("assoc".to_string()),
        }));
        let repos = ["/srv/git/x.git", "/srv/git/y.git"];
        for common_dir in repos {
            snapshot.nodes.push(GraphNode::Repo(RepoNode {
                id: RepoId::new(common_dir),
                common_dir: common_dir.to_string(),
                source_paths: Vec::new(),
                remotes: Vec::new(),
            }));
        }
        let workspace = NodeId::Workspace(workspace_id);
        for (idx, common_dir) in repos.iter().enumerate() {
            snapshot.candidate_links.push(link(
                &format!("a{idx}"),
                workspace.clone(),
                NodeId::Repo(RepoId::new(*common_dir)),
                RelationKind::AssociatedWith,
            ));
        }
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &workspace, Some(home().as_path()));
        let downstream = filter_direction(&view, Direction::Downstream);
        let group = downstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::AssociatedWith)
            .expect("AssociatedWith downstream group");
        assert_eq!(group.links.len(), 2);
        for row in &group.links {
            assert!(row.resolved_winner);
            assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
        }
        assert!(!view.has_other_rows());
    }

    #[test]
    fn repo_associated_with_multiple_workspaces_all_validated() {
        // Mirror of the bug report from the workspace's vantage:
        // when a single repo participates in several workspaces via
        // `AssociatedWith`, focusing the *repo* should surface every
        // workspace as validated (upstream direction). Each
        // `(workspace, AssociatedWith, repo)` slot is owned by its
        // workspace, so resolved_relationships contain three
        // independent winners, all of which the repo's detail pane
        // observes upstream.
        let mut snapshot = GraphSnapshot::empty();
        let repo_id = RepoId::new("/srv/git/shared.git");
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: repo_id.clone(),
            common_dir: "/srv/git/shared.git".to_string(),
            source_paths: Vec::new(),
            remotes: Vec::new(),
        }));
        let workspace_roots = [
            "/home/op/work/one",
            "/home/op/work/two",
            "/home/op/work/three",
        ];
        for root in workspace_roots {
            snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
                id: WorkspaceId::new(root),
                root: root.to_string(),
                provider: None,
                name: None,
            }));
        }
        let repo = NodeId::Repo(repo_id);
        for (idx, root) in workspace_roots.iter().enumerate() {
            snapshot.candidate_links.push(link(
                &format!("w{idx}"),
                NodeId::Workspace(WorkspaceId::new(*root)),
                repo.clone(),
                RelationKind::AssociatedWith,
            ));
        }
        let snapshot = resolve_snapshot(snapshot);

        let view = build(&snapshot, &repo, Some(home().as_path()));
        let upstream = filter_direction(&view, Direction::Upstream);
        let group = upstream
            .groups
            .iter()
            .find(|g| g.relation == RelationKind::AssociatedWith)
            .expect("AssociatedWith upstream group on the repo");
        assert_eq!(group.links.len(), 3);
        for row in &group.links {
            assert!(row.resolved_winner);
            assert_eq!(row.edge_state, EdgeStateLabel::Resolves);
        }
        assert!(!view.has_other_rows());
    }

    #[test]
    fn child_session_group_multi_link_keeps_winner_first() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "parent", Some("/home/op/src/x"), None));
        snapshot.nodes.push(agent(
            "claude-code",
            "child-a",
            Some("/home/op/src/y"),
            None,
        ));
        snapshot.nodes.push(agent(
            "claude-code",
            "child-b",
            Some("/home/op/src/z"),
            None,
        ));
        let parent = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "parent"));
        let child_a = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child-a"));
        let child_b = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child-b"));
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
        let upstream = filter_direction(&view, Direction::Upstream);
        let group = upstream
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
        snapshot
            .nodes
            .push(agent("claude-code", "child", Some("/home/op/src/x"), None));
        let child = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "child"));
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
        let downstream = filter_direction(&view, Direction::Downstream);
        let group = downstream
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
        let long_command =
            "/usr/bin/claude --resume 7f3c2a917b8c4d556e6f7a8b9c0d1e2f3a4b5c6d --extra";
        snapshot.nodes.push(process("obs:1", 82310, long_command));
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
        let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
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
    fn short_node_label_renders_kind_short_tag_per_node() {
        // T8-038: every node kind should produce a `kind:short_tag`
        // label suitable for breadcrumb hops.
        assert_eq!(
            short_node_label(&agent(
                "claude-code",
                "session-abcdef0123456789",
                None,
                None,
            )),
            "session:23456789",
        );
        assert_eq!(short_node_label(&mux("tmux", "editor", None)), "mux:editor");
        assert_eq!(
            short_node_label(&process("obs:1", 82310, "/usr/bin/claude --resume aaa")),
            "proc:claude",
        );
        let pr = GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "octo", "repo", 7),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "repo".to_string(),
            number: 7,
            state: None,
            url: None,
            updated_epoch: None,
            is_draft: false,
        });
        assert_eq!(short_node_label(&pr), "pr:octo/repo#7");
        let repo = RepoNode {
            id: RepoId::new("/srv/git/conspectus.git"),
            common_dir: "/srv/git/conspectus.git".to_string(),
            source_paths: vec![],
            remotes: vec![],
        };
        assert_eq!(
            short_node_label(&GraphNode::Repo(repo)),
            "repo:conspectus.git"
        );
    }

    #[test]
    fn render_breadcrumb_chain_joins_hops_with_separator() {
        let theme = Theme::default();
        let hops = vec![
            breadcrumb_hop(
                "session:abc",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
            ),
            breadcrumb_hop(
                "mux:editor",
                NodeId::MuxSession(MuxSessionId::new("editor")),
            ),
            breadcrumb_hop(
                "proc:claude",
                NodeId::RuntimeProcess(RuntimeProcessId::new("proc:1")),
            ),
        ];
        let rendered = render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty");
        // Flatten the line back to plain text. Each hop renders as
        // `<glyph> <tag>` (no `kind:` prefix; the glyph carries
        // the kind identity now).
        let plain = breadcrumb_plain(&rendered);
        assert_eq!(
            plain,
            format!(
                "{} abc › {} editor › {} claude",
                NodeKind::AgentSession.default_glyph(),
                NodeKind::MuxSession.default_glyph(),
                NodeKind::RuntimeProcess.default_glyph(),
            ),
        );
    }

    #[test]
    fn render_breadcrumb_chain_uses_kind_color_per_glyph_span() {
        let theme = Theme::default();
        let hops = vec![
            breadcrumb_hop(
                "session:abc",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
            ),
            breadcrumb_hop(
                "mux:editor",
                NodeId::MuxSession(MuxSessionId::new("editor")),
            ),
        ];
        let rendered = render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty");
        let session_glyph = NodeKind::AgentSession.default_glyph();
        let mux_glyph = NodeKind::MuxSession.default_glyph();
        let session_span = rendered
            .spans
            .iter()
            .find(|s| s.content == session_glyph)
            .expect("session glyph span");
        let mux_span = rendered
            .spans
            .iter()
            .find(|s| s.content == mux_glyph)
            .expect("mux glyph span");
        assert_eq!(session_span.style.fg, Some(theme.node_agent_session));
        assert_eq!(mux_span.style.fg, Some(theme.node_mux_session));
    }

    #[test]
    fn render_breadcrumb_chain_returns_none_when_empty() {
        let theme = Theme::default();
        assert!(render_breadcrumb_chain(&[], &theme, 80).is_none());
    }

    #[test]
    fn render_breadcrumb_chain_elides_middle_when_too_long() {
        let theme = Theme::default();
        // Four hops; budget only fits `first … last`.
        let hops = vec![
            breadcrumb_hop(
                "session:abcdefgh",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "agent-1")),
            ),
            breadcrumb_hop(
                "mux:editor-east",
                NodeId::MuxSession(MuxSessionId::new("editor-east")),
            ),
            breadcrumb_hop(
                "proc:claude-helper",
                NodeId::RuntimeProcess(RuntimeProcessId::new("proc:helper")),
            ),
            breadcrumb_hop(
                "session:xyzlast",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "agent-2")),
            ),
        ];
        let plain =
            breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 40).expect("non-empty"));
        let session_glyph = NodeKind::AgentSession.default_glyph();
        assert!(
            plain.starts_with(&format!("{session_glyph} abcdefgh")),
            "expected leading first hop in {plain:?}",
        );
        assert!(
            plain.ends_with(&format!("{session_glyph} xyzlast")),
            "expected trailing last hop in {plain:?}",
        );
        assert!(plain.contains('…'), "expected elision marker in {plain:?}");
    }

    #[test]
    fn render_breadcrumb_chain_falls_back_to_last_hop_when_extremely_narrow() {
        let theme = Theme::default();
        let hops = vec![
            breadcrumb_hop(
                "session:abc",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc-1")),
            ),
            breadcrumb_hop(
                "mux:editor",
                NodeId::MuxSession(MuxSessionId::new("editor")),
            ),
            breadcrumb_hop(
                "proc:claude",
                NodeId::RuntimeProcess(RuntimeProcessId::new("proc:1")),
            ),
        ];
        // Budget only fits the last hop.
        let plain =
            breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 5).expect("non-empty"));
        let proc_glyph = NodeKind::RuntimeProcess.default_glyph();
        assert_eq!(plain, format!("{proc_glyph} claude"));
    }

    #[test]
    fn render_breadcrumb_chain_disambiguates_colliding_short_labels() {
        let theme = Theme::default();
        // Two `session:abc` hops should pick up a `·last4` tail
        // so the operator can tell which is which.
        let hops = vec![
            breadcrumb_hop(
                "session:abc",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "session-1234")),
            ),
            breadcrumb_hop(
                "mux:editor",
                NodeId::MuxSession(MuxSessionId::new("editor")),
            ),
            breadcrumb_hop(
                "session:abc",
                NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "session-5678")),
            ),
        ];
        let plain =
            breadcrumb_plain(&render_breadcrumb_chain(&hops, &theme, 80).expect("non-empty"));
        // Both colliding hops should carry a `·` disambiguator
        // (`abc·1234`, `abc·5678`).
        let session_segments: Vec<&str> =
            plain.split(" › ").filter(|s| s.contains("abc")).collect();
        assert_eq!(session_segments.len(), 2);
        assert!(
            session_segments.iter().all(|s| s.contains('·')),
            "colliding hops should be disambiguated: {plain}",
        );
    }

    fn breadcrumb_hop(short_label: &str, focused: NodeId) -> BreadcrumbHop {
        BreadcrumbHop {
            focused,
            short_label: short_label.to_string(),
            cursor_key: None,
            other_expanded: false,
            full_detail_expanded: false,
            left_pane_selection: None,
        }
    }

    fn breadcrumb_plain(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn cwd_owner_kind_resolves_to_checkout_workspace_then_repo() {
        // T8-039: when the cwd of a session matches a Checkout in
        // the snapshot, surface `checkout`; otherwise Workspace,
        // then Repo (`common_dir` or any `source_paths` entry).
        let mut snapshot = GraphSnapshot::empty();
        let repo_id = RepoId::new("/srv/git/conspectus.git");
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: repo_id.clone(),
            common_dir: "/srv/git/conspectus.git".to_string(),
            source_paths: vec!["/home/op/src/conspectus".to_string()],
            remotes: vec![],
        }));
        snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new("/home/op/atelier/demo"),
            root: "/home/op/atelier/demo".to_string(),
            provider: None,
            name: Some("demo".to_string()),
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, "/home/op/src/conspectus"),
            root: "/home/op/src/conspectus".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        // Checkout wins over Repo for the same path.
        assert_eq!(
            cwd_owner_kind(&snapshot, "/home/op/src/conspectus"),
            Some("checkout")
        );
        // Workspace beats Repo when no checkout matches.
        assert_eq!(
            cwd_owner_kind(&snapshot, "/home/op/atelier/demo"),
            Some("workspace")
        );
        // Repo by common_dir.
        assert_eq!(
            cwd_owner_kind(&snapshot, "/srv/git/conspectus.git"),
            Some("repo")
        );
        // No match anywhere → bare cwd.
        assert_eq!(cwd_owner_kind(&snapshot, "/tmp/scratch"), None);
        // Empty/whitespace inputs don't claim a match.
        assert_eq!(cwd_owner_kind(&snapshot, ""), None);
    }

    #[test]
    fn agent_session_cwd_field_carries_kind_chip_when_resolved() {
        // T8-039: the session's `cwd` field should pick up the
        // owning-node kind chip when the path resolves in the graph.
        let mut snapshot = GraphSnapshot::empty();
        let repo_id = RepoId::new("/srv/git/conspectus.git");
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: repo_id.clone(),
            common_dir: "/srv/git/conspectus.git".to_string(),
            source_paths: vec!["/home/op/src/conspectus".to_string()],
            remotes: vec![],
        }));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, "/home/op/src/conspectus"),
            root: "/home/op/src/conspectus".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot.nodes.push(agent(
            "claude-code",
            "abc",
            Some("/home/op/src/conspectus"),
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let view = build(&snapshot, &target, Some(home().as_path()));
        let cwd = view
            .core_fields
            .iter()
            .find(|f| f.label == "cwd")
            .expect("cwd field present");
        assert_eq!(cwd.kind_chip, Some("checkout"));
    }

    #[test]
    fn agent_session_cwd_has_no_kind_chip_when_path_does_not_resolve() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("claude-code", "abc", Some("/tmp/scratch"), None));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
        let view = build(&snapshot, &target, Some(home().as_path()));
        let cwd = view
            .core_fields
            .iter()
            .find(|f| f.label == "cwd")
            .expect("cwd field present");
        assert!(
            cwd.kind_chip.is_none(),
            "unresolved cwd should leave kind_chip empty: {cwd:?}",
        );
    }
}
