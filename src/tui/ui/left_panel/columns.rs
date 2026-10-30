//! The left pane's columns: one schema per kind of row, and the cells
//! each row gives under them.
//!
//! The framework's `columns` module measures every row of the tree once a
//! frame and lays each row out by one narrow-width policy; this module
//! says what the columns are (`docs/tui-columns.md` is their reference)
//! and what each cell shows. Rows of different kinds share the prefix
//! keys, so the pin marker cell is reserved on every row of a tree once
//! any row has a marker, and a blank kind glyph cell on every group
//! header once any header has a glyph.

use std::collections::HashMap;

use tuikit::chip;
use tuikit::columns::{Cell, Cells, Column, ColumnSchema};
use tuikit::tree::{self, RowColumns, RowCtx, RowMark};

use super::*;
use crate::tui::rows::{MixedRow, PinMarker, Row};
use crate::tui::widgets::badge::{harness_badge, hits_chip, was_in_chip};

/// The key of a left-pane column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Col {
    /// `▶`/`▼` on a row with children: every row.
    Disclosure,
    /// What the row is: every row that stands for an entity.
    Glyph,
    /// A group header's kind glyph, blank on a header for no entity while
    /// another header has one.
    HeaderGlyph,
    /// The pin marker of the Sessions, Mux, PRs, and Search rows.
    Marker,
    /// The pin marker a Pinned or History row carries.
    MixedMarker,
    /// A group header's label, padded to the widest in the tree.
    HeaderLabel,
    /// A group header's path or members, cut in the middle; its width
    /// lines the counts up.
    Detail,
    /// A group header's session count.
    Count,
    /// A group header's ambiguous-link warning.
    Warning,
    /// A session's harness key.
    Key,
    /// A session's harness chip.
    Chip,
    /// A session's age.
    Age,
    /// A session's mux link glyph.
    State,
    /// A session's alias, title, or mux name.
    Label,
    /// The `was in <mux>` note.
    WasIn,
    /// A flat session's project.
    Project,
    /// The `N hits` note of a content search.
    Hits,
    /// A session's last message or best snippet.
    Preview,
    /// A Pinned or History row's label, measured for its own layout.
    MixedLabel,
    /// A Pinned or History row's status word, measured for its own layout.
    MixedStatus,
    /// A Pinned or History row's chip, measured for its own layout.
    MixedChip,
}

/// The widest label a session row shows.
const SESSION_LABEL_CELLS: usize = 32;
/// The cells a session row gives the harness key, cut with no mark: the
/// key is an identifier whose first cells tell sessions apart.
const SESSION_KEY_CELLS: usize = 8;
/// The cells a flat session row gives its project.
const PROJECT_CELLS: usize = 16;
/// The fewest cells a session label narrows to before it leaves.
const LABEL_FLOOR: usize = 8;
/// The fewest cells a header's path narrows to before it leaves.
const DETAIL_FLOOR: usize = 8;
/// The widest label a Pinned or History row shows.
pub(super) const MIXED_LABEL_CELLS: usize = 28;
/// The widest status word a Pinned or History row shows.
pub(super) const MIXED_STATUS_CELLS: usize = 9;

/// The cells every row starts with: the disclosure, the kind glyph, and
/// the pin marker. Each cell carries its own trailing space.
fn prefix(glyph: Option<Col>, marker: Col) -> Vec<Column<Col>> {
    let mut columns = vec![Column::fixed(Col::Disclosure, 2).gap(0)];
    match glyph {
        Some(Col::HeaderGlyph) => columns.push(Column::cap(Col::HeaderGlyph, 2).gap(0)),
        Some(key) => columns.push(Column::fixed(key, 2).gap(0)),
        None => {}
    }
    columns.push(Column::cap(marker, 2).gap(0));
    columns
}

/// The schemas of the left pane's rows, built once per frame (a chip's
/// width comes from the theme).
pub(super) struct Schemas {
    header: ColumnSchema<Col>,
    session: ColumnSchema<Col>,
    /// The prefix alone, for a row whose body is drawn on its own.
    entity: ColumnSchema<Col>,
    /// The prefix alone for a row with no kind glyph (a matching turn,
    /// a matching pane line).
    unglyphed: ColumnSchema<Col>,
    /// The prefix alone for a Pinned or History row.
    mixed: ColumnSchema<Col>,
}

impl Schemas {
    pub(super) fn new(theme: &Theme) -> Self {
        let mut header = prefix(Some(Col::HeaderGlyph), Col::Marker);
        header.extend([
            Column::cap(Col::HeaderLabel, usize::MAX),
            // The detail spaces the counts: a nested header's is narrower
            // by its indent, so every count in the tree is in one column.
            Column::cap(Col::Detail, usize::MAX)
                .ragged()
                .absorbs_lead()
                .shrinks(0, DETAIL_FLOOR)
                .drops(0),
            Column::cap(Col::Count, usize::MAX).right(),
            Column::cap(Col::Warning, 1),
        ]);
        let mut session = prefix(Some(Col::Glyph), Col::Marker);
        session.extend([
            Column::fixed(Col::Key, SESSION_KEY_CELLS).drops(1),
            Column::fixed(Col::Chip, chip::width(theme)).drops(2),
            Column::fixed(Col::Age, tuikit::age::CELL_WIDTH).drops(3),
            Column::fixed(Col::State, 1),
            Column::hug(Col::Label, SESSION_LABEL_CELLS)
                .shrinks(4, LABEL_FLOOR)
                .drops(4),
            Column::hug(Col::WasIn, usize::MAX).drops(1),
            Column::fixed(Col::Project, PROJECT_CELLS)
                .shrinks(2, LABEL_FLOOR)
                .drops(2),
            Column::hug(Col::Hits, usize::MAX).drops(3),
            Column::fill(Col::Preview).drops(0),
        ]);
        Self {
            header: ColumnSchema::new(header),
            session: ColumnSchema::new(session),
            entity: ColumnSchema::new(prefix(Some(Col::Glyph), Col::Marker)),
            unglyphed: ColumnSchema::new(prefix(None, Col::Marker)),
            mixed: ColumnSchema::new(prefix(Some(Col::Glyph), Col::MixedMarker)),
        }
    }
}

/// The left pane's rows as the framework lays them out: each row's schema
/// and cells, with what every row of a frame shares (the theme, the
/// clock, the hand-off's spinner frame, each group's counts).
pub(super) struct LeftColumns<'a> {
    pub(super) theme: &'a Theme,
    pub(super) now: i64,
    /// The spinner frame a row awaiting the hand-off shows.
    pub(super) spinner: Option<&'static str>,
    pub(super) summaries: HashMap<RowId, GroupSummary>,
    pub(super) schemas: Schemas,
}

impl RowColumns<Row> for LeftColumns<'_> {
    type Key = Col;

    fn schema(&self, row: &Row) -> &ColumnSchema<Col> {
        match &row.kind {
            RowKind::Group(_) => &self.schemas.header,
            RowKind::AgentSession(_) => &self.schemas.session,
            RowKind::Mixed(mixed) if mixed.state.is_none() => &self.schemas.mixed,
            RowKind::Hit(_) | RowKind::PaneHit(_) => &self.schemas.unglyphed,
            RowKind::MuxSession(_) | RowKind::Mixed(_) | RowKind::Pr(_) | RowKind::Pane(_) => {
                &self.schemas.entity
            }
        }
    }

    fn cells(&self, row: &Row, cx: &RowCtx<'_>) -> Cells<Col> {
        let theme = self.theme;
        let mut cells = vec![(
            Col::Disclosure,
            tree::disclosure(row.expandable, cx.expanded, theme),
        )];
        match &row.kind {
            RowKind::Group(group) => {
                if let Some(kind) = group_node_kind(group) {
                    cells.push((
                        Col::HeaderGlyph,
                        Cell::spans(vec![node_kind_glyph_span(kind, theme)]),
                    ));
                }
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                let summary = self.summaries.get(&row.id).copied().unwrap_or_default();
                cells.extend(self.header_cells(group, summary));
            }
            RowKind::AgentSession(session) => {
                cells.push((Col::Glyph, glyph_cell(NodeKind::AgentSession, theme)));
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.session_cells(session, cx.pending));
            }
            RowKind::Mixed(mixed) if mixed.state.is_none() => {
                cells.push((
                    Col::Glyph,
                    Cell::spans(vec![mixed_glyph_span(mixed, theme)]),
                ));
                cells.push((Col::MixedMarker, mixed_marker_cell(mixed, theme)));
                cells.extend(mixed_measures(mixed, theme));
            }
            kind => {
                if let Some(glyph) = row_kind_glyph_span(kind, theme) {
                    cells.push((Col::Glyph, Cell::spans(vec![glyph])));
                }
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
            }
        }
        cells
    }
}

impl LeftColumns<'_> {
    /// A group header's cells after its prefix: the bold label, the path
    /// or members (with `(cwd)` on the launch directory's group), the
    /// count of sessions beneath it, and `▲` when one of them has an
    /// ambiguous mux link.
    fn header_cells(&self, group: &GroupRow, summary: GroupSummary) -> Cells<Col> {
        let theme = self.theme;
        let label = compact_path_label(&group.display_path);
        let secondary = compact_path_secondary(&group.display_path);
        let cwd_style = Style::default()
            .fg(theme.cwd_mark())
            .add_modifier(theme.muted);
        let detail = match (secondary.is_empty(), group.is_launch_context) {
            (true, true) => Cell::text("(cwd)", cwd_style),
            (true, false) => Cell::empty(),
            (false, true) => {
                Cell::secondary(secondary, theme).tail(vec![Span::styled("  (cwd)", cwd_style)])
            }
            (false, false) => Cell::secondary(secondary, theme),
        };
        let mut cells = vec![
            (
                Col::HeaderLabel,
                Cell::text(label, Style::default().add_modifier(Modifier::BOLD)),
            ),
            (Col::Detail, detail),
        ];
        // A header for a group with no sessions under it stays quiet.
        if summary.agents > 0 {
            cells.push((
                Col::Count,
                Cell::text(
                    format!("({})", summary.agents),
                    Style::default().add_modifier(theme.muted),
                ),
            ));
            if summary.ambiguous > 0 {
                cells.push((
                    Col::Warning,
                    Cell::text(
                        marks::WARNING,
                        Style::default()
                            .fg(theme.warning)
                            .add_modifier(Modifier::BOLD),
                    ),
                ));
            }
        }
        cells
    }

    /// A session row's cells after its prefix: the harness key, the
    /// harness chip, the age, the mux link glyph (a spinner while the row
    /// awaits the hand-off), and then the label, the `was in` note, the
    /// project in flat grouping, and the last message or what a content
    /// search found.
    fn session_cells(&self, session: &AgentSessionRow, pending: bool) -> Cells<Col> {
        let theme = self.theme;
        let secondary = Style::default().fg(theme.inert);
        let state = match self.spinner.filter(|_| pending) {
            Some(glyph) => Cell::text(glyph, secondary),
            None => Cell::spans(vec![mux_indicator_span(session.mux_state, theme)]),
        };
        let mut cells = vec![
            (
                Col::Key,
                Cell::text(&session.session.session_key, secondary).clip(),
            ),
            (
                Col::Chip,
                Cell::chip(harness_badge(&session.harness_label, theme)),
            ),
            (
                Col::Age,
                age_cell(session.recency.as_deref(), session.activity_epoch, self.now),
            ),
            (Col::State, state),
        ];
        // The alias shows always and bold; a title only when it tells the
        // session apart from another in its group.
        if let Some(label) = session.tree_label().filter(|label| !label.is_empty()) {
            let aliased = session
                .alias
                .as_deref()
                .is_some_and(|alias| !alias.is_empty());
            let style = if aliased {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            cells.push((Col::Label, Cell::text(label, style)));
        }
        if let Some(mux) = session.gone_mux_name() {
            // An untitled session is already named after its mux, so the
            // note only says that mux is gone.
            let named_after_it = session.tree_label() == Some(mux);
            cells.push((
                Col::WasIn,
                Cell::chip(was_in_chip(mux, named_after_it, theme)),
            ));
        }
        if let Some(project) = session
            .project_display
            .as_deref()
            .filter(|project| !project.is_empty())
        {
            cells.push((Col::Project, Cell::text(project, secondary)));
        }
        match &session.content_hit {
            Some(hit) => {
                cells.push((Col::Hits, Cell::chip(hits_chip(hit.count, theme))));
                cells.push((
                    Col::Preview,
                    snippet_cell(&hit.snippet, &hit.highlight, theme),
                ));
            }
            None => {
                if let Some(preview) = session.preview.as_deref().filter(|p| !p.is_empty()) {
                    cells.push((Col::Preview, preview_cell(preview, theme)));
                }
            }
        }
        cells
    }
}

/// The pin marker cell: the filled pin glyph for a pin on a running
/// entity, its hollow twin in the ghost look for one that isn't, blank
/// when the row is not pinned.
fn marker_cell(mark: RowMark, theme: &Theme) -> Cell {
    match PinMarker::from(mark) {
        PinMarker::None => Cell::empty(),
        marker => Cell::spans(vec![pin_marker_span(marker, theme)]),
    }
}

/// A Pinned or History row's own marker cell.
fn mixed_marker_cell(mixed: &MixedRow, theme: &Theme) -> Cell {
    marker_cell(mixed.marker.into(), theme)
}

/// A kind glyph with its trailing space.
fn glyph_cell(kind: NodeKind, theme: &Theme) -> Cell {
    Cell::spans(vec![node_kind_glyph_span(kind, theme)])
}

/// The age cell: the framework's, tinted by how recent `epoch` is at
/// `now`, or the no-value mark when the age is unknown.
fn age_cell(age: Option<&str>, epoch: Option<i64>, now: i64) -> Cell {
    Cell::age(
        age.map(str::to_string),
        Age::of(epoch, Some(now)).map(Age::recency),
    )
}

/// A last-message preview: inert italics, cut at the end.
fn preview_cell(preview: &str, theme: &Theme) -> Cell {
    Cell::text(
        preview,
        Style::default()
            .fg(theme.inert)
            .add_modifier(Modifier::ITALIC),
    )
}

/// What a content search found: the best snippet, cut around its match,
/// the match in the theme's match look.
fn snippet_cell(snippet: &str, highlight: &std::ops::Range<usize>, theme: &Theme) -> Cell {
    Cell::snippet(
        snippet,
        highlight.clone(),
        Style::default()
            .fg(theme.inert)
            .add_modifier(Modifier::ITALIC),
        theme.match_style(),
    )
    .dropping(crate::tui::widgets::search::MARKDOWN_TICKS)
}

/// What a Pinned or History row's own layout measures: its label and
/// status word as it cuts them, and its chip.
fn mixed_measures(mixed: &MixedRow, theme: &Theme) -> Cells<Col> {
    let mut cells = vec![
        (
            Col::MixedLabel,
            Cell::text(
                text::truncate_end(&mixed.label, MIXED_LABEL_CELLS),
                Style::default(),
            ),
        ),
        (
            Col::MixedStatus,
            Cell::text(
                text::truncate_end(&mixed.status, MIXED_STATUS_CELLS),
                Style::default(),
            ),
        ),
    ];
    if let Some(chip) = &mixed.chip {
        cells.push((Col::MixedChip, Cell::chip(mixed_chip_span(chip, theme))));
    }
    cells
}
