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
use crate::tui::rows::{
    EntityState, HitRow, MixedRow, MixedTone, PaneHitRow, PaneRow, PinMarker, Row,
};
use crate::tui::widgets::badge::{
    command_badge, harness_badge, hits_chip, no_agent_chip, was_in_chip,
};
use crate::tui::widgets::search::{
    Hit, HitChip, HitCol, HitShape, hit_columns, hits_tag, line_tag, snippet_cell, turn_tag,
};

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
    /// A group header's label, padded so it ends in one column at every
    /// depth.
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
    /// A mux's name.
    MuxName,
    /// The chips of a mux's agents, its program, or `no agent`.
    MuxChips,
    /// A mux's age.
    MuxAge,
    /// A mux's client glyph, or a stopped mux's state.
    MuxState,
    /// The `N hits` note on a mux whose agent a content search matched.
    MuxHits,
    /// A mux's one agent's last message or best snippet, or a stopped
    /// mux's directory.
    MuxContent,
    /// A PR's label: the forge repo, unless its group names it, and the
    /// number, which stays whole.
    PrLabel,
    /// Whether a PR has a local branch.
    PrTracking,
    /// A PR's age.
    PrAge,
    /// A PR's state glyph.
    PrState,
    /// A PR's local branch.
    PrBranch,
    /// A PR's title.
    PrTitle,
    /// A Pinned or History row's label: it ends in one column at every
    /// depth, so nested rows line their status up with their parents'.
    MixedLabel,
    /// A Pinned or History row's state word.
    MixedStatus,
    /// A Pinned or History row's age.
    MixedAge,
    /// A Pinned or History row's chip, one width across the tree.
    MixedChip,
    /// What a Pinned or History row says after its chip.
    MixedContent,
    /// A column of the Search view's matching turns, panes, and pane
    /// lines: the search modal's hit columns.
    Hit(HitCol),
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
/// The mux name column, by the cells left for it and the columns after
/// it: a wider pane gives names more room.
const MUX_NAME_STEPS: &[(usize, usize)] = &[(105, 30), (73, 24), (39, 20), (0, 18)];
/// The fewest cells a mux name narrows to.
const MUX_NAME_FLOOR: usize = 10;
/// The widest branch a PR row shows, so one long branch cannot crowd out
/// the title. The branch is content, not a lined-up column: only PRs with
/// a local branch have one, so a column would sit mostly blank.
const PR_BRANCH_CELLS: usize = 24;
/// The widest label a Pinned or History row shows.
const MIXED_LABEL_CELLS: usize = 28;
/// The widest status word a Pinned or History row shows.
const MIXED_STATUS_CELLS: usize = 9;

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
    /// A mux row, running or stopped.
    mux: ColumnSchema<Col>,
    pr: ColumnSchema<Col>,
    /// A Pinned or History row.
    mixed: ColumnSchema<Col>,
    /// A pane whose scrollback matched, where a session row would be.
    pane: ColumnSchema<Col>,
    /// A matching turn or pane line that names its session or pane (the
    /// Messages listing), with no kind glyph.
    hit: ColumnSchema<Col>,
    /// A matching turn or pane line under the row that names its session
    /// or pane.
    nested_hit: ColumnSchema<Col>,
}

impl Schemas {
    pub(super) fn new(theme: &Theme) -> Self {
        let mut header = prefix(Some(Col::HeaderGlyph), Col::Marker);
        header.extend([
            // The label ends in one column at every depth (a nested
            // header's is narrower by its indent), so the path after it
            // starts where its parent's does, and the path spaces the
            // counts, so every count in the tree is in one column.
            Column::cap(Col::HeaderLabel, usize::MAX).absorbs_lead(),
            Column::cap(Col::Detail, usize::MAX)
                .ragged()
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
        // A mux's chips are up to two of its agents' and a count of the
        // rest. When the row is narrow they narrow first (to one chip and
        // the count), then the name narrows, then the age leaves, then
        // the chips.
        let chip = chip::width(theme);
        let more = text::width(" +9");
        let mut mux = prefix(Some(Col::Glyph), Col::Marker);
        mux.extend([
            Column::steps(Col::MuxName, MUX_NAME_STEPS).shrinks(2, MUX_NAME_FLOOR),
            Column::cap(Col::MuxChips, 2 * chip + 1 + more)
                .shrinks(1, chip + more)
                .drops(4),
            Column::fixed(Col::MuxAge, tuikit::age::CELL_WIDTH).drops(3),
            Column::fixed(Col::MuxState, 1),
            Column::hug(Col::MuxHits, usize::MAX).drops(1),
            Column::fill(Col::MuxContent).drops(0),
        ]);
        // A PR's label is as wide as the widest in the tree, so the
        // columns after it line up across groups. When the row is narrow
        // the title leaves first, then the branch narrows and leaves; then
        // the label narrows (its number stays whole), the age leaves, and
        // the tracking mark leaves. The state stays.
        let mut pr = prefix(Some(Col::Glyph), Col::Marker);
        pr.extend([
            Column::cap(Col::PrLabel, usize::MAX)
                .gap(1)
                .shrinks(1, LABEL_FLOOR),
            Column::fixed(Col::PrTracking, 1).drops(3),
            Column::fixed(Col::PrAge, tuikit::age::CELL_WIDTH).drops(2),
            Column::fixed(Col::PrState, 1),
            Column::hug(Col::PrBranch, PR_BRANCH_CELLS)
                .shrinks(1, LABEL_FLOOR)
                .drops(1),
            Column::fill(Col::PrTitle).drops(0),
        ]);
        // A Pinned or History row lines every cell up, its label at every
        // depth. When the row is narrow the chip leaves first, then the
        // content; then the label narrows and the age leaves. The state
        // word stays.
        let mut mixed = prefix(Some(Col::Glyph), Col::MixedMarker);
        mixed.extend([
            Column::cap(Col::MixedLabel, MIXED_LABEL_CELLS)
                .absorbs_lead()
                .shrinks(2, LABEL_FLOOR),
            Column::cap(Col::MixedStatus, MIXED_STATUS_CELLS),
            Column::fixed(Col::MixedAge, tuikit::age::CELL_WIDTH).drops(3),
            Column::cap(Col::MixedChip, usize::MAX).drops(0),
            Column::fill(Col::MixedContent).drops(1),
        ]);
        // A search hit is laid out as the search modal lays its results out:
        // the chip or command, the name, the hit count or position, the
        // age, and the snippet, with the modal's widths and the order they
        // give way in. The row's kind glyph and place say whether it is a
        // session's or a pane's, so it has no source word.
        let hit = |prefix: Vec<Column<Col>>, shape| {
            let mut columns = prefix;
            columns.extend(hit_columns(Col::Hit, chip, shape));
            ColumnSchema::new(columns)
        };
        Self {
            header: ColumnSchema::new(header),
            session: ColumnSchema::new(session),
            mux: ColumnSchema::new(mux),
            pr: ColumnSchema::new(pr),
            mixed: ColumnSchema::new(mixed),
            pane: hit(prefix(Some(Col::Glyph), Col::Marker), HitShape::Named),
            hit: hit(prefix(None, Col::Marker), HitShape::Named),
            nested_hit: hit(prefix(None, Col::Marker), HitShape::Nested),
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
            RowKind::MuxSession(_) => &self.schemas.mux,
            RowKind::Mixed(mixed) if stopped_mux(mixed) => &self.schemas.mux,
            RowKind::Mixed(_) => &self.schemas.mixed,
            RowKind::Pr(_) => &self.schemas.pr,
            RowKind::Pane(_) => &self.schemas.pane,
            RowKind::Hit(HitRow {
                origin: Some(_), ..
            })
            | RowKind::PaneHit(PaneHitRow {
                origin: Some(_), ..
            }) => &self.schemas.hit,
            RowKind::Hit(_) | RowKind::PaneHit(_) => &self.schemas.nested_hit,
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
            RowKind::MuxSession(mux) => {
                cells.push((Col::Glyph, glyph_cell(NodeKind::MuxSession, theme)));
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.mux_cells(mux, cx.pending));
            }
            RowKind::Pr(pr) => {
                cells.push((
                    Col::Glyph,
                    Cell::spans(vec![pr_kind_glyph_span(Some(pr.status()), theme)]),
                ));
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.pr_cells(pr, cx.pending));
            }
            RowKind::Mixed(mixed) if stopped_mux(mixed) => {
                cells.push((Col::Glyph, glyph_cell(NodeKind::MuxSession, theme)));
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.stopped_mux_cells(mixed, cx.pending));
            }
            RowKind::Mixed(mixed) => {
                cells.push((
                    Col::Glyph,
                    Cell::spans(vec![mixed_glyph_span(mixed, theme)]),
                ));
                cells.push((Col::MixedMarker, mixed_marker_cell(mixed, theme)));
                cells.extend(self.mixed_cells(mixed, cx.pending));
            }
            RowKind::Pane(pane) => {
                // A pane is a part of a mux and wears its glyph.
                if let Some(glyph) = row_kind_glyph_span(&row.kind, theme) {
                    cells.push((Col::Glyph, Cell::spans(vec![glyph])));
                }
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.pane_cells(pane));
            }
            // A matching turn or pane line is drawn without a kind glyph.
            RowKind::Hit(hit) => {
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.hit_cells(hit));
            }
            RowKind::PaneHit(hit) => {
                cells.push((Col::Marker, marker_cell(cx.mark, theme)));
                cells.extend(self.pane_hit_cells(hit));
            }
        }
        cells
    }
}

impl LeftColumns<'_> {
    /// How recent an age that counts from `epoch` is now, which tints it.
    fn recency(&self, epoch: Option<i64>) -> Option<tuikit::age::Recency> {
        Age::of(epoch, Some(self.now)).map(Age::recency)
    }

    /// A matching turn's cells after its prefix: the turn and its kind,
    /// when it was said, and the snippet; on a row of the Messages listing,
    /// first the session's chip and name.
    fn hit_cells(&self, hit: &HitRow) -> Cells<Col> {
        let origin = hit.origin.as_ref();
        Hit {
            source: "session",
            chip: origin.map(|origin| HitChip::Harness(&origin.harness_label)),
            label: origin.map_or("", |origin| origin.label.as_str()),
            tag: turn_tag(hit.turn_index, hit.kind),
            age: Some(&hit.age),
            recency: self.recency(hit.age_epoch),
            snippet: &hit.snippet,
            highlight: hit.highlight.clone(),
        }
        .cells(Col::Hit, self.theme)
    }

    /// A pane row's cells after its prefix: what the pane runs, the mux's
    /// name and the pane's place in it, the hit count, how long ago the
    /// scrollback was read, and the newest matching line.
    fn pane_cells(&self, pane: &PaneRow) -> Cells<Col> {
        let label = format!("{} {}", pane.mux_label, pane.location);
        Hit {
            source: "pane",
            chip: Some(HitChip::Command(pane.command.as_deref().unwrap_or(""))),
            label: &label,
            tag: hits_tag(pane.count),
            age: Some(&pane.age),
            recency: self.recency(pane.age_epoch),
            snippet: &pane.snippet,
            highlight: pane.highlight.clone(),
        }
        .cells(Col::Hit, self.theme)
    }

    /// A matching pane line's cells after its prefix: its line, how long
    /// ago the scrollback was read, and the line; on a row of the Messages
    /// listing, first what the pane runs and the mux's name and the pane's
    /// place.
    fn pane_hit_cells(&self, hit: &PaneHitRow) -> Cells<Col> {
        let origin = hit.origin.as_deref();
        Hit {
            source: "pane",
            chip: origin.map(|_| HitChip::Command(hit.pane.command.as_deref().unwrap_or(""))),
            label: origin.unwrap_or(""),
            tag: line_tag(hit.line),
            age: Some(&hit.age),
            recency: self.recency(hit.age_epoch),
            snippet: &hit.snippet,
            highlight: hit.highlight.clone(),
        }
        .cells(Col::Hit, self.theme)
    }
}

impl LeftColumns<'_> {
    /// A group header's cells after its prefix: the bold label, the path
    /// or members (with `(cwd)` on the launch directory's group), the
    /// count of sessions beneath it, and `▲` when one of them has an
    /// ambiguous mux link.
    fn header_cells(&self, group: &GroupRow, summary: GroupSummary) -> Cells<Col> {
        let theme = self.theme;
        let cwd_style = Style::default()
            .fg(theme.cwd_mark())
            .add_modifier(theme.muted);
        let detail = match (group.detail.is_empty(), group.is_launch_context) {
            (true, true) => Cell::text("(cwd)", cwd_style),
            (true, false) => Cell::empty(),
            (false, true) => {
                Cell::secondary(&group.detail, theme).tail(vec![Span::styled("  (cwd)", cwd_style)])
            }
            (false, false) => Cell::secondary(&group.detail, theme),
        };
        let mut cells = vec![
            (
                Col::HeaderLabel,
                Cell::text(&group.label, Style::default().add_modifier(Modifier::BOLD)),
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

impl LeftColumns<'_> {
    /// A mux row's cells after its prefix: the name, the chips of its
    /// agents (or of the harness or program its pane runs, or `no
    /// agent`), its own age, whether a client is attached (a spinner while
    /// the row awaits the hand-off), and its one agent's last message or
    /// what a content search found in it.
    fn mux_cells(&self, mux: &MuxSessionRow, pending: bool) -> Cells<Col> {
        let theme = self.theme;
        let chips = if mux.agent_labels.is_empty() {
            let chip = match (&mux.program_harness, &mux.program) {
                (Some(harness), _) => harness_badge(harness, theme),
                (None, Some(program)) => command_badge(program, theme),
                (None, None) => no_agent_chip(theme),
            };
            vec![chip]
        } else {
            mux.agent_labels
                .iter()
                .map(|label| harness_badge(label, theme))
                .collect()
        };
        let state = self.state_cell(pending, || match mux.client_attached {
            Some(true) => (state::LINKED, Style::default().fg(theme.mux_attached())),
            Some(false) => (state::UNLINKED, Style::default().add_modifier(theme.muted)),
            None => (state::UNKNOWN, Style::default().add_modifier(theme.muted)),
        });
        let mut cells = vec![
            (
                Col::MuxName,
                Cell::text(
                    compact_mux_native_id(&mux.native_id),
                    Style::default().fg(theme.link),
                ),
            ),
            (
                Col::MuxChips,
                Cell::chips(chips, Style::default().fg(theme.inert)),
            ),
            (
                Col::MuxAge,
                age_cell(mux.recency.as_deref(), mux.activity_epoch, self.now),
            ),
            (Col::MuxState, state),
        ];
        match &mux.content_hit {
            Some(hit) => {
                cells.push((Col::MuxHits, Cell::chip(hits_chip(hit.count, theme))));
                cells.push((
                    Col::MuxContent,
                    snippet_cell(&hit.snippet, &hit.highlight, theme),
                ));
            }
            None => {
                if let Some(preview) = mux
                    .single_session_preview
                    .as_deref()
                    .filter(|preview| !preview.is_empty())
                {
                    cells.push((Col::MuxContent, preview_cell(preview, theme)));
                }
            }
        }
        cells
    }

    /// A PR row's cells after its prefix: the label (bold), whether it has
    /// a local branch, its age, its state glyph (in the style its kind
    /// glyph shares), then the local branch, dim, and the title, gray.
    fn pr_cells(&self, pr: &PrRow, pending: bool) -> Cells<Col> {
        let theme = self.theme;
        let bold = Style::default().add_modifier(Modifier::BOLD);
        // Under a group that names the forge repo the row reads `#N`;
        // elsewhere the repo comes first and is cut before the number is.
        let label = match &pr.forge_repo {
            Some(repo) => Cell::text(repo, bold).tail(vec![Span::styled(pr.number(), bold)]),
            None => Cell::text(pr.number(), bold),
        };
        let mut cells = vec![
            (Col::PrLabel, label),
            (
                Col::PrTracking,
                Cell::spans(vec![pr_tracking_span(pr, theme)]),
            ),
            (
                Col::PrAge,
                age_cell(pr.updated_recency.as_deref(), pr.updated_epoch, self.now),
            ),
            (
                Col::PrState,
                self.state_cell(pending, || pr.status().mark(theme)),
            ),
        ];
        if let Some(branch) = pr.branch_name.as_deref() {
            cells.push((
                Col::PrBranch,
                Cell::text(branch, Style::default().add_modifier(theme.muted)),
            ));
        }
        if let Some(title) = pr.title.as_deref() {
            cells.push((
                Col::PrTitle,
                Cell::text(title, Style::default().fg(theme.inert)),
            ));
        }
        cells
    }

    /// A row's state cell: the glyph and style `glyph` gives, or the
    /// hand-off's spinner frame while the row awaits the refresh that
    /// follows it (ADR 0108), since its old state may no longer hold.
    fn state_cell(&self, pending: bool, glyph: impl FnOnce() -> (&'static str, Style)) -> Cell {
        let spinner = self.spinner.filter(|_| pending);
        spinner.map_or_else(
            || {
                let (glyph, style) = glyph();
                Cell::text(glyph, style)
            },
            |frame| Cell::text(frame, Style::default().fg(self.theme.inert)),
        )
    }

    /// A Pinned or History row's cells after its prefix: the bold label,
    /// the state word (a spinner while the row awaits the hand-off, as wide
    /// as the word, so its column holds still), the age, the chip, and the
    /// content. A row for something stopped, removed, missing, planned, or
    /// unknown is dimmed from its label on when it is drawn.
    fn mixed_cells(&self, mixed: &MixedRow, pending: bool) -> Cells<Col> {
        let theme = self.theme;
        // A pull request's word takes the style of its state glyph, so the
        // two read as one signal; other live words share the attached
        // color.
        let status_style = match (mixed.tone, mixed.pr_status) {
            (MixedTone::Dim, _) => Style::default(),
            (MixedTone::Normal, Some(status)) => status.mark(theme).1,
            (MixedTone::Normal, None) => Style::default().fg(theme.mux_attached()),
        };
        let word = text::truncate_end(&mixed.status, MIXED_STATUS_CELLS);
        let status = match self.spinner.filter(|_| pending) {
            Some(glyph) => text::pad(glyph, text::width(&word)),
            None => word,
        };
        let mut cells = vec![
            (
                Col::MixedLabel,
                Cell::text(&mixed.label, Style::default().add_modifier(Modifier::BOLD)),
            ),
            (Col::MixedStatus, Cell::text(status, status_style)),
            (
                Col::MixedAge,
                age_cell(mixed.age.as_deref(), mixed.age_epoch, self.now),
            ),
        ];
        if let Some(chip) = &mixed.chip {
            cells.push((Col::MixedChip, Cell::chip(mixed_chip_span(chip, theme))));
        }
        cells.push((
            Col::MixedContent,
            Cell::text(&mixed.content, Style::default().fg(theme.inert)),
        ));
        cells
    }

    /// A stopped mux's cells, in the running ones' columns: the name, the
    /// harness it last ran (or `no agent`), the time since it was last
    /// seen running, its state glyph (a spinner while the row awaits the
    /// hand-off), and the directory it ran in. The tree draws the row as a
    /// ghost.
    fn stopped_mux_cells(&self, mixed: &MixedRow, pending: bool) -> Cells<Col> {
        let theme = self.theme;
        let chip = mixed
            .chip
            .as_ref()
            .map_or_else(|| no_agent_chip(theme), |chip| mixed_chip_span(chip, theme));
        let state = self.state_cell(pending, || match mixed.state {
            Some(EntityState::Stopped) => (state::STOPPED, ghost_style(theme)),
            Some(EntityState::Linked) => (state::LINKED, Style::default().fg(theme.mux_attached())),
            Some(EntityState::Unlinked) => {
                (state::UNLINKED, Style::default().add_modifier(theme.muted))
            }
            Some(EntityState::Unknown) | None => {
                (state::UNKNOWN, Style::default().add_modifier(theme.muted))
            }
        });
        vec![
            (
                Col::MuxName,
                Cell::text(&mixed.label, Style::default().fg(theme.link)),
            ),
            (
                Col::MuxChips,
                Cell::chips(vec![chip], Style::default().fg(theme.inert)),
            ),
            (
                Col::MuxAge,
                age_cell(mixed.age.as_deref(), mixed.age_epoch, self.now),
            ),
            (Col::MuxState, state),
            (
                Col::MuxContent,
                Cell::text(&mixed.content, Style::default().fg(theme.inert)),
            ),
        ]
    }
}

/// Whether a mixed-kind row is a stopped mux in the Mux view: a row that
/// shows its state as a glyph, as a view of one kind of entity does, is
/// drawn in that entity's columns.
pub(super) fn stopped_mux(mixed: &MixedRow) -> bool {
    mixed.state.is_some() && mixed.node_kind == NodeKind::MuxSession
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
