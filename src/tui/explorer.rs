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
    /// Current unix epoch for relative ages (`4m ago`). `None` renders
    /// timestamps as unknown.
    pub now: Option<i64>,
}

/// Build the explorer view model for the given node id. Returns
/// `None` when the node isn't in the snapshot.
pub fn build_node_view(inputs: ExplorerInputs<'_>) -> Option<NodeView> {
    let node = inputs.snapshot.find_node(inputs.target)?;
    let id = node.id();
    let kind_label = kind_label(node);
    let title_line = title_line(inputs.snapshot, node);
    let short_id = node_short_id(&id);
    let core_fields = core_fields(inputs.snapshot, node, inputs.home, inputs.now);
    let all_fields = all_fields(inputs.snapshot, node, inputs.home, inputs.now);
    let upstream = build_explorer(
        inputs.snapshot,
        &id,
        Direction::Upstream,
        inputs.home,
        inputs.now,
    );
    let downstream = build_explorer(
        inputs.snapshot,
        &id,
        Direction::Downstream,
        inputs.home,
        inputs.now,
    );
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
    /// `Self::upstream_groups` / `Self::downstream_groups`
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
            .map_or(usize::MAX, super::icons::NodeKind::ordinal);
        let ord_b = crate::tui::icons::NodeKind::from_snake_case(&b.neighbor_kind)
            .map_or(usize::MAX, super::icons::NodeKind::ordinal);
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
        .map_or("", |l| l.neighbor_label.as_str())
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
/// transitional convenience via `NodeView::upstream_groups` /
/// `NodeView::downstream_groups` until passes 2 and 3 refactor the
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
    /// `crate::tui::detail::preferred_link`.
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
                .map_or(hop.short_label.as_str(), |(_, tag)| tag);
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

fn core_fields(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
    now: Option<i64>,
) -> Vec<CoreField> {
    match node {
        GraphNode::Repo(r) => repo_core(r, home),
        GraphNode::Checkout(c) => checkout_core(c, home),
        GraphNode::Workspace(w) => workspace_core(w, home),
        GraphNode::AgentSession(s) => agent_session_core(snapshot, s, home, now),
        GraphNode::MuxSession(m) => mux_session_core(snapshot, m, home, now),
        GraphNode::Pin(p) => pin_core(p, home),
        GraphNode::RuntimeProcess(p) => runtime_process_core(p, home, now),
        GraphNode::Branch(b) => branch_core(b),
        GraphNode::Fork(f) => fork_core(f),
        GraphNode::ForgePr(pr) => forge_pr_core(pr, now),
    }
}

fn all_fields(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
    now: Option<i64>,
) -> Vec<CoreField> {
    let mut fields = core_fields(snapshot, node, home, now);
    let extras = extra_fields(snapshot, node, home, now);
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
    now: Option<i64>,
) -> Vec<CoreField> {
    match node {
        GraphNode::AgentSession(s) => agent_session_extras(snapshot, s, home),
        GraphNode::MuxSession(m) => mux_session_extras(m, now),
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
    now: Option<i64>,
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
    fields.push(CoreField::plain("status", session_status(s, now)));
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
    now: Option<i64>,
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
        Some(epoch) => CoreField::plain("last_active", relative_epoch(epoch, now)),
        None => CoreField::placeholder("last_active", "—"),
    });
    fields
}

fn mux_session_extras(m: &MuxSessionNode, now: Option<i64>) -> Vec<CoreField> {
    let mut fields = Vec::new();
    if let Some(epoch) = m.created_epoch {
        fields.push(CoreField::plain("created", relative_epoch(epoch, now)));
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

fn runtime_process_core(
    p: &RuntimeProcessNode,
    _home: Option<&Path>,
    now: Option<i64>,
) -> Vec<CoreField> {
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
        Some(epoch) => CoreField::plain("observed", relative_epoch(epoch, now)),
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

fn forge_pr_core(pr: &ForgePrNode, now: Option<i64>) -> Vec<CoreField> {
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
        Some(epoch) => CoreField::plain("updated", relative_epoch(epoch, now)),
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

fn session_status(s: &AgentSessionNode, now: Option<i64>) -> String {
    match s.last_active_epoch {
        Some(epoch) => format!("active · last {}", relative_epoch(epoch, now)),
        None => "—".to_string(),
    }
}

fn relative_epoch(epoch: i64, now: Option<i64>) -> String {
    crate::tui::rows::format_recency(now, Some(epoch))
        .map_or_else(|| "—".to_string(), |age| format!("{age} ago"))
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
    now: Option<i64>,
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
                .find_node(id)
                .map_or("unknown", kind_label)
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
        .map(|builder| finalize_group(snapshot, focused, direction, builder, home, now))
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
    now: Option<i64>,
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
    // The resolver keeps an ambiguous slot alive with
    // `selected_link_id = None`, so a no-winner slot is itself the
    // ambiguity signal, even when `competing_link_ids` is empty.
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
            let neighbor_node = snapshot.find_node(&neighbor_id);
            let neighbor_kind_label = neighbor_node.map_or("unknown", kind_label);
            let neighbor_label = neighbor_node.map_or_else(
                || format!("{neighbor_id}"),
                |n| neighbor_display_label(n, home),
            );
            let preview = neighbor_node
                .map(|n| core_fields(snapshot, n, home, now))
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
#[path = "explorer_tests.rs"]
mod tests;
