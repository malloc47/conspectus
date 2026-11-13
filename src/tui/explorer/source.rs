//! The graph as a source for the framework's explorer
//! (`tuikit::explorer`).
//!
//! The explorer keeps the right pane's cursor, fold, every-field switch,
//! drill frames, and armed `Backspace`; this adapter says what it shows.
//! A node is a [`NodeView`], built when the explorer loads it. Its rows
//! are two zones: the node's fields (a zone a lost cursor does not land
//! on, so a cursor that stood on a relationship row stays among them),
//! then, when the node has neighbors, the `Related` zone under its rule:
//! the validated rows, the `Other` header, and, while it is open, the
//! alternates, conflicts, and unresolved stubs under it. A row is keyed by
//! [`ExplorerRowKey`] and drawn as the grid row `explorer::grid` builds;
//! a frame keeps the left pane's selection, so `Backspace` restores both
//! panes together.
//!
//! The explorer opens on its first row, which is a field, not the first
//! relationship, so the Preview shows the node's own preview (the pane
//! capture, the transcript's tail) until `j` walks into the
//! relationships and the Preview turns to the neighbor and its edge.

use tuikit::explorer::{Entry, Explorer, ExplorerModel, ExplorerSource, FrameView, Zone};
use tuikit::frame::SectionRule;
use tuikit::glyphs::marks;
use tuikit::property_grid::PropertyGrid;

use super::{EdgeStateLabel, ExplorerInputs, ExplorerRow, ExplorerRowKey, NodeView, grid};
use crate::model::{GraphSnapshot, NodeId};
use crate::tui::rows::RowId;
use crate::tui::theme::Theme;

/// The explorer's model over the graph: nodes by id, rows by
/// [`ExplorerRowKey`], the view model's own rows, a node's [`NodeView`],
/// and the left-pane selection a frame keeps.
#[derive(Debug)]
pub struct Graph;

impl ExplorerModel for Graph {
    type Id = NodeId;
    type Key = ExplorerRowKey;
    type Row = ExplorerRow;
    type Content = NodeView;
    type Saved = Option<RowId>;
}

/// The right pane's explorer.
pub type GraphExplorer = Explorer<Graph>;

/// The app's data as the explorer reads it, built for each call.
pub struct GraphSource<'a> {
    /// The snapshot nodes are built from.
    pub snapshot: &'a GraphSnapshot,
    /// The home directory paths are shortened against.
    pub home: Option<std::path::PathBuf>,
    /// The theme the rows draw in.
    pub theme: &'a Theme,
    /// Whether link rows show their edge meta (`E`).
    pub edge_meta: bool,
    /// The left pane's selection, which a frame keeps.
    pub left: Option<&'a RowId>,
}

impl ExplorerSource for GraphSource<'_> {
    type Model = Graph;

    fn load(&self, id: &NodeId) -> Option<NodeView> {
        super::build_node_view(ExplorerInputs {
            snapshot: self.snapshot,
            target: id,
            home: self.home.as_deref(),
            now: Some(crate::tui::rows::tree_current_unix_epoch()),
        })
    }

    fn id(&self, view: &NodeView) -> NodeId {
        view.focused.clone()
    }

    fn zones(&self, view: &NodeView, frame: &FrameView<ExplorerRowKey>) -> Vec<Zone<Graph>> {
        zones(view, frame, self.theme, self.edge_meta)
    }

    fn follow(&self, view: &NodeView, entry: &Entry<Graph>) -> Option<NodeId> {
        view.drill_target(&entry.host)
    }

    fn tag(&self, view: &NodeView) -> String {
        view.short_label.clone()
    }

    fn save(&self) -> Option<RowId> {
        self.left.cloned()
    }

    fn grid(&self) -> PropertyGrid {
        grid::grid(self.theme)
    }
}

/// The zones of `view` as `frame` shows it: the fields, then, when the
/// node has neighbors, the `Related` zone under its rule.
pub(crate) fn zones(
    view: &NodeView,
    frame: &FrameView<ExplorerRowKey>,
    theme: &Theme,
    edge_meta: bool,
) -> Vec<Zone<Graph>> {
    let mut fields = Vec::new();
    let mut related = Vec::new();
    for row in rows(view, frame) {
        let drawn = grid::grid_row(view, &row, theme, edge_meta);
        let entry = Entry::new(row.key(view), drawn, row.clone());
        match row {
            ExplorerRow::NodeField { .. } => fields.push(entry),
            ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherHeader { .. } => {
                related.push(entry);
            }
            ExplorerRow::OtherLink { .. } | ExplorerRow::OtherUnresolved { .. } => {
                related.push(entry.under(ExplorerRowKey::OtherHeader));
            }
        }
    }
    let mut zones = vec![Zone::new(fields)];
    if !view.relationships.groups.is_empty() {
        zones.push(Zone::new(related).heading(related_rule(view)).lands());
    }
    zones
}

/// The rows the explorer lists for `view` as `frame` shows it, top to
/// bottom: the fields (every field with Expanded Node Detail on, the
/// summary otherwise), every validated link, and, when the node has any,
/// the `Other` header with, while it is open, the alternates, conflicts,
/// and unresolved stubs. The groups are sorted (ADR 0074 §4), so the
/// rows read in a stable kind, verb, label order; a node with no `Other`
/// rows carries no dangling header.
fn rows(view: &NodeView, frame: &FrameView<ExplorerRowKey>) -> Vec<ExplorerRow> {
    let mut rows: Vec<ExplorerRow> = view
        .fields(frame.full)
        .iter()
        .enumerate()
        .map(|(index, field)| ExplorerRow::NodeField {
            index,
            field: field.clone(),
        })
        .collect();
    let groups = &view.relationships.groups;
    let resolves = |state: &EdgeStateLabel| matches!(state, EdgeStateLabel::Resolves);
    for (group_index, group) in groups.iter().enumerate() {
        for (link_index, link) in group.links.iter().enumerate() {
            if resolves(&link.edge_state) {
                rows.push(ExplorerRow::ValidatedLink {
                    group_index,
                    link_index,
                });
            }
        }
    }
    if view.has_other_rows() {
        let expanded = frame.is_open(&ExplorerRowKey::OtherHeader);
        rows.push(ExplorerRow::OtherHeader { expanded });
        if expanded {
            for (group_index, group) in groups.iter().enumerate() {
                for (link_index, link) in group.links.iter().enumerate() {
                    if !resolves(&link.edge_state) {
                        rows.push(ExplorerRow::OtherLink {
                            group_index,
                            link_index,
                        });
                    }
                }
                for unresolved_index in 0..group.unresolved.len() {
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

/// The `Related` rule over the link rows, its summary counting the
/// validated rows and what the Other zone holds (ADR 0074 §5). The
/// summary comes first, so the chip stays in the `Preview` chip's column
/// however long the counts grow.
fn related_rule(view: &NodeView) -> SectionRule {
    let counts = view.relationship_counts();
    let mut summary = format!("{} validated", counts.validated);
    if counts.other > 0 {
        summary.push_str(&format!(" · {} other", counts.other));
        if counts.ambiguous > 0 {
            summary.push_str(&format!(" · {} {}", counts.ambiguous, marks::WARNING));
        }
        if counts.unresolved > 0 {
            summary.push_str(&format!(" · {} {}", counts.unresolved, marks::NO_VALUE));
        }
    }
    SectionRule::new("Related").summary(summary).summary_first()
}
