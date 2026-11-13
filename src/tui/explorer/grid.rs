//! The explorer's rows as rows of the framework's property grid
//! (`tuikit::property_grid`).
//!
//! The view model says what the pane lists; this adapter says how each
//! row reads in the grid. A field is a property row, cut where the field
//! says, with its kind glyph. A neighbor is a link row: flush under the
//! `Related` rule in the primary tone when the resolver picked it, and
//! indented under the `Other` header otherwise, in the quiet tone for an
//! alternate and the alert tone (led by `▲`) for a conflict, both from
//! their `[tui.theme]` roles. An unresolved stub is an inert link row,
//! its evidence in place of a label. The Other header is a disclosure
//! row with its counts.
//!
//! The pane draws these rows, and Enter and `o` ask the same rows what
//! they mean (`Row::activate`, `Row::inspect`), so a copy or an inspect
//! acts on exactly the row the pane shows.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::glyphs::marks;
use tuikit::property_grid::{
    Cut, DisclosureRow, LinkRow, PropertyGrid, PropertyRow, Row, Tone, Tones,
};

use super::{
    CoreField, EdgeStateLabel, ExplorerRow, NodeView, RelationshipCounts, RelationshipGroup,
    RelationshipLink, UnresolvedRow, directional_verb,
};
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::theme::Theme;

/// The cells between the cursor gutter and the verb of a row under the
/// Other header, so the zone reads as the header's.
const OTHER_INDENT: usize = 4;

/// A grid laid out as the explorer lays its rows out: the framework's
/// label and verb columns, the cut hint naming the key that opens a
/// value whole, and the edge tones in their theme roles.
pub(crate) fn grid(theme: &Theme) -> PropertyGrid {
    PropertyGrid::new()
        .inspect_key(crate::tui::keymap::INSPECT_KEY.to_string())
        .tones(tones(theme))
}

/// The link tones: the resolver's pick in the `link` color, an alternate
/// in `edge_alt_of`, a conflict in `edge_conflict` and bold, an
/// unresolved stub receding.
pub(crate) fn tones(theme: &Theme) -> Tones {
    Tones::new(theme)
        .with(Tone::Quiet, Style::default().fg(theme.edge_alt_of()))
        .with(
            Tone::Alert,
            Style::default()
                .fg(theme.edge_conflict())
                .add_modifier(Modifier::BOLD),
        )
}

/// Per-kind glyph chip used to surface the graph node kind next to a
/// value or beside a relationship-explorer neighbor. ADR 0073 §3
/// replaces the prior dim `[kind]` text with a 1-cell glyph in the
/// node-kind color so the chip carries identity at a glance. A missing
/// kind (the neighbor isn't in the snapshot) falls back to a dim `?` so
/// the chip slot stays visible without misleading the operator.
///
/// `ForgePr`'s glyph takes the `pr_open` hue here because the
/// chip-rendering surfaces do not carry PR state down; the row tree
/// colors PR glyphs by state directly off the row.
pub(crate) fn kind_chip_span(kind: Option<NodeKind>, theme: &Theme) -> Span<'static> {
    let Some(node_kind) = kind else {
        return Span::styled("?", Style::default().add_modifier(theme.muted));
    };
    let style = node_kind_style(node_kind, theme);
    Span::styled(style.glyph.clone(), Style::default().fg(style.color))
}

/// A field as a property row: its label and whole value, cut where the
/// field says, receding when it is a placeholder, its kind glyph beside
/// the label, and its annotation.
pub(crate) fn field_row(field: &CoreField, theme: &Theme) -> PropertyRow {
    let mut row = PropertyRow::new(field.label, field.value.clone());
    if let Some(cells) = field.cut_at {
        row = row.cut(Cut::End, cells);
    }
    if field.placeholder {
        row = row.placeholder();
    }
    if field.kind_chip.is_some() {
        row = row.glyph(kind_chip_span(field.kind_chip, theme));
    }
    if let Some(annotation) = field.annotation {
        row = row.annotation(annotation);
    }
    row
}

/// The edge meta `E` shows after a related row's label: the link's
/// provenance, confidence, and state.
fn edge_meta(row: LinkRow, show: bool, parts: [&'static str; 3]) -> LinkRow {
    if show {
        row.meta(parts.join(marks::SPACED_SEPARATOR))
    } else {
        row
    }
}

/// A neighbor as a link row: the directional verb, the neighbor's kind
/// glyph and label, and, with edge meta on, the representative link's
/// provenance, confidence, and state.
fn neighbor_row(
    group: &RelationshipGroup,
    link: &RelationshipLink,
    theme: &Theme,
    show_edge_meta: bool,
) -> LinkRow {
    let row = LinkRow::new(
        directional_verb(&group.relation, group.direction),
        link.neighbor_label.clone(),
    )
    .glyph(kind_chip_span(link.neighbor_kind, theme));
    edge_meta(
        row,
        show_edge_meta,
        [
            link.provenance.snake_case(),
            link.confidence.snake_case(),
            link.state.snake_case(),
        ],
    )
}

/// A validated-zone row (ADR 0074 §3): a resolver winner, flush under
/// the `Related` rule in the primary tone.
pub(crate) fn validated_row(
    group: &RelationshipGroup,
    link: &RelationshipLink,
    theme: &Theme,
    show_edge_meta: bool,
) -> LinkRow {
    neighbor_row(group, link, theme, show_edge_meta)
}

/// An Other-zone link row (ADR 0075): indented under the header, a
/// conflict in the alert tone (its `▲` the signal that survives
/// `NO_COLOR`) and an alternate in the quiet one. A `Resolves` row never
/// lands here; it would read as an alternate.
pub(crate) fn other_row(
    group: &RelationshipGroup,
    link: &RelationshipLink,
    theme: &Theme,
    show_edge_meta: bool,
) -> LinkRow {
    let tone = match link.edge_state {
        EdgeStateLabel::Conflict => Tone::Alert,
        EdgeStateLabel::AltOf(_) | EdgeStateLabel::Resolves => Tone::Quiet,
    };
    neighbor_row(group, link, theme, show_edge_meta)
        .indent(OTHER_INDENT)
        .tone(tone)
}

/// An unresolved-evidence row: an inert link under the Other header whose
/// far end is unknown, what the evidence says of it in the label's
/// place, and, with edge meta on, the evidence's provenance, confidence,
/// and state, as a neighbor row shows its link's.
pub(crate) fn unresolved_row(
    group: &RelationshipGroup,
    row: &UnresolvedRow,
    show_edge_meta: bool,
) -> LinkRow {
    let link = LinkRow::new(
        directional_verb(&group.relation, group.direction),
        unresolved_label(row),
    )
    .indent(OTHER_INDENT)
    .tone(Tone::Inert);
    edge_meta(
        link,
        show_edge_meta,
        [
            row.provenance.snake_case(),
            row.confidence.snake_case(),
            row.state.snake_case(),
        ],
    )
}

/// The Other zone's header: `▶ Other  (N · K ▲ · L —)` (ADR 0074 §3).
pub(crate) fn other_header(expanded: bool, counts: RelationshipCounts) -> DisclosureRow {
    DisclosureRow::new("Other", expanded)
        .count(counts.other)
        .tally(counts.ambiguous, marks::WARNING)
        .tally(counts.unresolved, marks::NO_VALUE)
}

/// What an unresolved stub says about the thing it could not find: its
/// native id, harness, and path, or its node type when the evidence
/// names none of them.
pub(crate) fn unresolved_label(row: &UnresolvedRow) -> String {
    let parts: Vec<&str> = [
        row.evidence.native_id.as_deref(),
        row.evidence.harness_key.as_deref(),
        row.evidence.path.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        row.node_type.clone()
    } else {
        parts.join(marks::SPACED_SEPARATOR)
    }
}

/// `row` of `view` as a grid row. A row whose link has left the view
/// (the view model never names one) is an empty inert link, so the rows
/// stay one for one with the cursor's.
pub(crate) fn grid_row(
    view: &NodeView,
    row: &ExplorerRow,
    theme: &Theme,
    show_edge_meta: bool,
) -> Row {
    let link = |group_index: usize, link_index: usize| {
        let group = view.relationships.groups.get(group_index)?;
        Some((group, group.links.get(link_index)?))
    };
    let gone = || LinkRow::default().tone(Tone::Inert).into();
    match row {
        ExplorerRow::NodeField { field, .. } => field_row(field, theme).into(),
        ExplorerRow::ValidatedLink {
            group_index,
            link_index,
        } => link(*group_index, *link_index).map_or_else(gone, |(group, link)| {
            validated_row(group, link, theme, show_edge_meta).into()
        }),
        ExplorerRow::OtherHeader { expanded } => {
            other_header(*expanded, view.relationship_counts()).into()
        }
        ExplorerRow::OtherLink {
            group_index,
            link_index,
        } => link(*group_index, *link_index).map_or_else(gone, |(group, link)| {
            other_row(group, link, theme, show_edge_meta).into()
        }),
        ExplorerRow::OtherUnresolved {
            group_index,
            unresolved_index,
        } => view
            .relationships
            .groups
            .get(*group_index)
            .and_then(|group| Some((group, group.unresolved.get(*unresolved_index)?)))
            .map_or_else(gone, |(group, unresolved)| {
                unresolved_row(group, unresolved, show_edge_meta).into()
            }),
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
