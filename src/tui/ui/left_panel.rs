//! Left panel: the row tree in the framework's `TreeView`, and the
//! cells it is drawn from.
//!
//! The framework draws the pane (its title, the Search header, the
//! Pinned banner, the placeholder, the window over the showing rows, the
//! scrollbar), measures and lays out every row's columns by one
//! narrow-width policy, and gives each row its looks (the hand-off's
//! pending look, the ghost look, the selection highlight). This module
//! says what the columns are and what each cell shows ([`columns`]);
//! [`LeftRows::line`] is the producer. What only the snapshot and the
//! hand-off can say about a row (ghost, pending, pin marker) is worked
//! out by the App before the frame, so laying a row out looks nothing
//! up. The PR, Pinned, History, and Search hit and pane rows still draw
//! their own cells after the prefix the framework lays out.

mod columns;

use tuikit::selection;
use tuikit::tree::{RowCtx, RowState, TreeColumns, TreeView};

use super::*;

use crate::tui::icons::state;
use crate::tui::rows::Row;
use columns::{Col, LeftColumns, Schemas};

/// Draw the left pane and return the window its tree drew, or `None`
/// when the tree is empty, so the window it had is kept for when rows
/// come back.
pub(super) fn draw_left_panel(
    app: &App,
    frame: &mut Frame<'_>,
    area: Rect,
) -> Option<ScrollWindow> {
    // The corners take a cell each from the top border; the body is as
    // wide.
    let inner_width = usize::from(area.width.saturating_sub(2));
    let states = app.tree_row_states();
    let columns = LeftColumns::of(app);
    let rows = LeftRows::of(app, &columns, &states);
    let line = |row: &Row, cx: &RowCtx<'_>| rows.line(row, cx);
    let mut view = TreeView::new(&app.tree().rows, app.tree_state(), line, app.theme())
        .focused(app.focus() == Focus::Left)
        .states(&states)
        .title(left_panel_title(app, inner_width));
    // The Pinned view advertises legacy pins that still need
    // `pin migrate` on the last line of the pane.
    if let Some(banner) = pinned_banner(app) {
        let lines = banner
            .split('\n')
            .map(|line| Line::from(line.to_string()))
            .collect();
        view = view.footer(lines, Style::default().fg(app.theme().warning));
    }
    // The Search view says which remembered search it shows on its first
    // line.
    if let Some(header) = search_header(app, inner_width) {
        view = view.header(header);
    }
    // A tree with rows always shows its first, so the placeholder's copy
    // is worked out only when there are none.
    if app.tree().rows.is_empty() {
        view = view.empty(empty_left_panel_text(app));
    }
    view.render(area, frame.buffer_mut())
}

impl<'a> LeftColumns<'a> {
    /// The columns of `app`'s tree for this frame: its theme, the clock
    /// the ages read against, the hand-off's spinner frame, and each
    /// group's counts.
    pub(super) fn of(app: &'a App) -> Self {
        Self {
            theme: app.theme(),
            now: current_unix_epoch_for_render(),
            spinner: app
                .handoff()
                .map(|handoff| spinner_glyph(handoff.started_at)),
            summaries: compute_group_summaries(app.tree()),
            schemas: Schemas::new(app.theme()),
        }
    }
}

/// The left pane's rows laid out for one frame: every row measured once
/// by the framework, and the PR column widths the PR rows still work out
/// themselves.
struct LeftRows<'a> {
    columns: &'a LeftColumns<'a>,
    tree: TreeColumns<'a, Row, LeftColumns<'a>>,
    pr: PrAlign,
}

impl<'a> LeftRows<'a> {
    /// The layout of `app`'s tree, whose rows have the `states` the App
    /// worked out (`App::tree_row_states`).
    fn of(app: &'a App, columns: &'a LeftColumns<'a>, states: &[RowState]) -> Self {
        let rows = &app.tree().rows;
        Self {
            columns,
            tree: TreeColumns::new(columns, rows, app.tree_state(), states, app.theme()),
            pr: PrAlign::of_tree(app.tree()),
        }
    }

    /// The producer the tree draws a row with: its cells at the width
    /// the context gives. The tree applies the row's looks.
    fn line(&self, row: &Row, cx: &RowCtx<'_>) -> Line<'static> {
        let LeftColumns {
            theme,
            now,
            spinner,
            ..
        } = *self.columns;
        // Rows the operator just left in tmux keep their old values,
        // dimmed, with a spinner where their state goes, until the
        // background refresh lands (ADR 0108).
        let spinner = spinner.filter(|_| cx.pending);
        match &row.kind {
            RowKind::Group(_) | RowKind::AgentSession(_) => self.tree.line(row, cx),
            RowKind::MuxSession(mux) => self.tree.line_then(row, cx, |left| {
                render_mux_session_spans(mux, theme, now, left, spinner)
            }),
            RowKind::Pr(pr) => self.tree.line_then(row, cx, |left| {
                render_pr_spans(pr, theme, self.pr, left, now)
            }),
            RowKind::Hit(hit) => self
                .tree
                .line_then(row, cx, |left| render_hit_spans(hit, theme, left, now)),
            RowKind::Pane(pane) => self
                .tree
                .line_then(row, cx, |left| render_pane_spans(pane, theme, left, now)),
            RowKind::PaneHit(hit) => self
                .tree
                .line_then(row, cx, |left| render_pane_hit_spans(hit, theme, left, now)),
            RowKind::Mixed(mixed) => {
                let widths = self.mixed_widths(row);
                self.tree.line_then(row, cx, |left| {
                    render_mixed_spans(mixed, theme, widths, left, now, spinner)
                })
            }
        }
    }

    /// The column widths a Pinned or History row lays its own cells out
    /// with, from the one measure pass: the label cell runs from the row's
    /// indent to a common column, so nested rows line their status up with
    /// their parents'.
    fn mixed_widths(&self, row: &Row) -> MixedWidths {
        let measure = self.tree.measure();
        MixedWidths {
            label: measure
                .widest_with_lead(Col::MixedLabel)
                .saturating_sub(self.tree.lead(row)),
            status: measure.widest(Col::MixedStatus),
            chip: measure.widest(Col::MixedChip),
        }
    }
}

/// `rows` laid out as the left pane lays out a tree of only them, each at
/// `width` with no cursor and the pane focused, for a test of the cells a
/// row draws. `theme` and `now` are what a frame has; with a `spinner`,
/// every row awaits the hand-off and shows that frame.
#[cfg(test)]
pub(super) fn lay_out(
    rows: &[Row],
    theme: &Theme,
    now: i64,
    width: usize,
    spinner: Option<&'static str>,
) -> Vec<Line<'static>> {
    let tree = crate::tui::rows::RowTree {
        rows: rows.to_vec(),
        ..crate::tui::rows::RowTree::default()
    };
    let columns = LeftColumns {
        theme,
        now,
        spinner,
        summaries: compute_group_summaries(&tree),
        schemas: Schemas::new(theme),
    };
    let state = RowState {
        pending: spinner.is_some(),
        ..RowState::default()
    };
    let states = vec![state; rows.len()];
    let tree_state = tuikit::tree::TreeState::new();
    let left = LeftRows {
        columns: &columns,
        tree: TreeColumns::new(&columns, rows, &tree_state, &states, theme),
        pr: PrAlign::of_tree(&tree),
    };
    rows.iter()
        .map(|row| {
            let cx = RowCtx {
                selected: false,
                focused: true,
                expanded: false,
                ghost: false,
                pending: state.pending,
                mark: state.mark,
                width,
                theme,
            };
            left.line(row, &cx)
        })
        .collect()
}

/// `row` as the left pane draws it on its own, its looks applied: its
/// state as `app` works it out, its cells measured alone, at `width`.
#[cfg(test)]
pub(super) fn render_left_row(
    row: &Row,
    app: &App,
    is_selected: bool,
    width: usize,
) -> Line<'static> {
    let columns = LeftColumns::of(app);
    let states = [app.tree_row_state(row)];
    let rows = std::slice::from_ref(row);
    let tree_state = tuikit::tree::TreeState::new();
    let left = LeftRows {
        columns: &columns,
        tree: TreeColumns::new(&columns, rows, &tree_state, &states, app.theme()),
        pr: PrAlign::of_tree(app.tree()),
    };
    let cx = RowCtx {
        selected: is_selected,
        focused: app.focus() == Focus::Left,
        expanded: app.is_expanded(&row.id),
        ghost: states[0].ghost,
        pending: states[0].pending,
        mark: states[0].mark,
        width,
        theme: app.theme(),
    };
    let mut line = left.line(row, &cx);
    cx.apply_looks(&mut line.spans);
    line
}

/// How wide a Pinned or History row's own cells are.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MixedWidths {
    /// The label cell, from the end of the row's prefix.
    pub(super) label: usize,
    /// The status word.
    pub(super) status: usize,
    /// The chip; 0 when no row has one.
    pub(super) chip: usize,
}

/// Render the left pane title as a lazydocker-style tab strip
/// showing every view, with the active one accented. Operators see
/// the available views at a glance instead of having to remember
/// the `1`–`6` accelerators or open the controls overlay.
///
/// `width` is the cells the pane's top border leaves for the title.
/// The strip names every view when they fit; otherwise it names fewer
/// and ends with `…` (see [`tab_strip_fit`]), so a narrow pane never
/// cuts a label in half.
pub(super) fn left_panel_title(app: &App, width: usize) -> Line<'static> {
    let theme = app.theme();
    let active = app.active_view();
    let lead = vec![Span::raw(" "), focus_marker_span(app, Focus::Left)];
    let lead_width: usize = lead.iter().map(Span::width).sum();
    let views = crate::tui::widgets::controls::VIEW_OPTIONS;
    let labels: Vec<&str> = views.iter().map(|&view| view_label(view)).collect();
    let active_index = views.iter().position(|&view| view == active).unwrap_or(0);
    let fit = tab_strip_fit(&labels, active_index, width.saturating_sub(lead_width));
    let dim = Style::default().fg(theme.inert);
    let mut spans = lead;
    for (position, &index) in fit.shown.iter().enumerate() {
        if position > 0 {
            spans.push(Span::styled(TAB_SEPARATOR, dim));
        }
        let label = fit
            .active_label
            .as_deref()
            .filter(|_| index == active_index);
        let label = label.unwrap_or(labels[index]).to_string();
        if index == active_index {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(label, dim));
        }
    }
    if fit.shortened {
        spans.push(Span::styled(TAB_STRIP_MORE, dim));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Between two tab labels.
const TAB_SEPARATOR: &str = " · ";

/// Ends a tab strip that leaves views out.
const TAB_STRIP_MORE: &str = " …";

/// Which tabs a strip of `labels` shows in `room` cells.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TabStripFit {
    /// Indexes into the labels, in strip order.
    pub(super) shown: Vec<usize>,
    /// Some views are left out, so the strip ends with `…`.
    pub(super) shortened: bool,
    /// The active label cut to fit, when not even it fits whole.
    pub(super) active_label: Option<String>,
}

/// Fit a tab strip into `room` cells, the trailing pad not counted.
///
/// Every label shows when they all fit. Otherwise the active label
/// stays, the inactive ones give way from the end of the strip, and the
/// strip ends with ` …` and a pad so the operator can tell it is
/// shortened. Only when the active label alone is too wide is it cut,
/// with its own `…`. A label is never cut in the middle by the border.
pub(super) fn tab_strip_fit(labels: &[&str], active: usize, room: usize) -> TabStripFit {
    let separator = text::width(TAB_SEPARATOR);
    let span_of = |shown: &[usize]| -> usize {
        shown
            .iter()
            .map(|&index| text::width(labels[index]))
            .sum::<usize>()
            + separator * shown.len().saturating_sub(1)
    };
    let every: Vec<usize> = (0..labels.len()).collect();
    if span_of(&every) <= room {
        return TabStripFit {
            shown: every,
            shortened: false,
            active_label: None,
        };
    }
    let active = active.min(labels.len().saturating_sub(1));
    // The marker and a pad after it, so the `…` never touches the corner.
    let budget = room.saturating_sub(text::width(TAB_STRIP_MORE) + 1);
    let mut shown = vec![active];
    for index in (0..labels.len()).filter(|&index| index != active) {
        let mut wider = shown.clone();
        wider.push(index);
        wider.sort_unstable();
        if span_of(&wider) > budget {
            break;
        }
        shown = wider;
    }
    let active_label = (span_of(&shown) > budget).then(|| text::truncate_end(labels[active], room));
    TabStripFit {
        shortened: active_label.is_none(),
        shown,
        active_label,
    }
}

/// The one-line notice under the Pinned view's rows when legacy pins
/// are still being read from config files.
pub(super) fn pinned_banner(app: &App) -> Option<String> {
    if app.active_view() != crate::tui::View::Pinned {
        return None;
    }
    app.snapshot_handle()?
        .snapshot()
        .diagnostics
        .iter()
        .find_map(crate::model::Diagnostic::advice)
}

/// The Search view's first line: which remembered search is shown, where
/// it sits among them, what it searched and found, and the keys that page
/// through the others. `None` outside the Search view or without a search.
pub(super) fn search_header(app: &App, width: usize) -> Option<Line<'static>> {
    use crate::tui::widgets::search::Reach;
    if app.active_view() != crate::tui::View::Search {
        return None;
    }
    let (position, total) = app.search_history().position()?;
    let entry = app.search_history().viewed()?;
    let search = &entry.search;
    let theme = app.theme();
    let query = text::truncate_end(&search.text, 28);
    let reach = match search.reach {
        Reach::Current => "Current",
        Reach::All => "All",
    };
    let mut note = search.status_note(std::time::Instant::now());
    if entry.stopped {
        note.push_str(" · stopped");
    }
    let dim = Style::default().add_modifier(theme.muted);
    let filtering = if app.viewed_is_filter() {
        " · filtering"
    } else {
        ""
    };
    let lead = format!("search {position}/{total} \"{query}\"");
    // What follows the query gives way when the pane is narrow.
    let room = width.saturating_sub(text::width(lead.as_str()) + text::width(filtering));
    let detail = text::truncate_end(&format!(" · {reach} · {note}"), room);
    let mut spans = vec![
        span!(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
            "search {position}/{total}"),
        span!(Modifier::BOLD; " \"{query}\""),
        span!(dim; "{detail}"),
    ];
    if !filtering.is_empty() {
        spans.push(span!(Style::default().fg(theme.accent); "{filtering}"));
    }
    if total > 1 {
        let hint = "< older  > newer";
        let used = text::spans_width(&spans);
        let gap = width.saturating_sub(used + text::width(hint));
        if gap >= 2 {
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(span!(dim; "{hint}"));
        }
    }
    Some(Line::from(spans))
}

pub(super) fn empty_left_panel_text(app: &App) -> String {
    if app.snapshot_handle().is_none() {
        return "Loading discovery…".to_string();
    }
    if app.active_view() == crate::tui::View::Search {
        if !app.config().live_preview_enabled {
            return format!("{}.", crate::tui::widgets::search::CONTENT_SEARCH_OFF);
        }
        if app.viewed_search().is_none() {
            return "Press `s` (or Ctrl-/) to search the contents of sessions and mux panes."
                .to_string();
        }
    }
    let shown = if app.active_view() == crate::tui::View::Search {
        app.viewed_search()
    } else {
        app.content()
    };
    if let Some(content) = shown
        && !matches!(
            app.active_view(),
            crate::tui::View::Pinned | crate::tui::View::History
        )
    {
        if app.active_view() == crate::tui::View::Search {
            return match content.status {
                crate::tui::ContentStatus::Running(_) => format!(
                    "Searching for \"{}\"…\nMatches appear as they are found.",
                    content.text
                ),
                crate::tui::ContentStatus::Done(_)
                    if content.group == crate::viewer::search::Grouping::Flat
                        && !content.hits().is_empty() =>
                {
                    format!("Reading the messages that match \"{}\"…", content.text)
                }
                crate::tui::ContentStatus::Done(_) => format!(
                    "Nothing matches \"{}\".\nPress `s` to search again, `<` for an earlier search.",
                    content.text
                ),
            };
        }
        return match content.status {
            crate::tui::ContentStatus::Running(_) => format!(
                "Searching for \"{}\"…\nSessions that match appear as they are found. Press `F` to clear.",
                content.text
            ),
            crate::tui::ContentStatus::Done(_)
                if content.group == crate::viewer::search::Grouping::Flat
                    && !content.hits().is_empty() =>
            {
                format!(
                    "Reading the messages that match \"{}\"…\nPress `F` to clear.",
                    content.text
                )
            }
            crate::tui::ContentStatus::Done(_) => format!(
                "No sessions match \"{}\".\nPress `s` to search again, `F` to clear.",
                content.text
            ),
        };
    }
    if !app.filter().is_empty() {
        // Filtered-zero case: a snapshot is loaded but the
        // active filter dropped every session. Distinguish from
        // "no sessions discovered" so the operator knows their
        // filter — not the world — is the reason.
        let chips = render_filter_chips(app.filter());
        return format!("No rows match `{chips}`.\nPress `F` to clear filters, `f` to edit.");
    }
    if app.active_view() == crate::tui::View::Pinned {
        return format!(
            "No pins yet.\nPin something with `{} pin add <target>`.",
            crate::invocation::invoked_name()
        );
    }
    if app.active_view() == crate::tui::View::History {
        let pinned_gone = app.snapshot_handle().map_or(0, |handle| {
            let snapshot = handle.snapshot();
            crate::tui::rows::history::gone_records(snapshot, None)
                .iter()
                .filter(|entry| {
                    !crate::tui::rows::history::pinned_by(snapshot, &entry.meta.id).is_empty()
                })
                .count()
        });
        if pinned_gone > 0 && !app.filter().show_pinned_history {
            return format!(
                "Every stopped or removed record is pinned ({pinned_gone}), so it is in the Pinned view.\nClear `Unpinned only` in Controls (`f`) to list them here."
            );
        }
        return "Nothing has stopped. Entities Conspectus has seen stop or vanish are listed here.\nPress `.` for the menu (Clean up…)."
            .to_string();
    }
    if app.active_view() == crate::tui::View::Prs {
        // Forge listings are reused for the forge interval, so `r`
        // alone won't show a PR opened a minute ago.
        return "No pull requests discovered.\nPress `Ctrl-R` to ask the forges again or `q` to quit."
            .to_string();
    }
    "No sessions discovered.\nPress `r` to refresh or `q` to quit.".to_string()
}

/// What a group header counts: the sessions beneath it, and how many of
/// them have an ambiguous mux link. Counts walk the full row tree (not
/// just the rows that show) so a collapsed group still says what is
/// inside.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GroupSummary {
    pub(super) agents: usize,
    pub(super) ambiguous: usize,
}

/// Walk the row tree once and compute the [`GroupSummary`] for every
/// group row. Each group's summary aggregates every `AgentSession`
/// row that sits below it in the flat tree (continuous depth >
/// group depth) until the next sibling-or-shallower row.
pub(super) fn compute_group_summaries(
    tree: &crate::tui::rows::RowTree,
) -> std::collections::HashMap<RowId, GroupSummary> {
    let mut out = std::collections::HashMap::new();
    let rows = &tree.rows;
    for (i, row) in rows.iter().enumerate() {
        if !matches!(row.kind, RowKind::Group(_)) {
            continue;
        }
        let parent_depth = row.depth;
        let mut summary = GroupSummary::default();
        for child in &rows[i + 1..] {
            if child.depth <= parent_depth {
                break;
            }
            if let RowKind::AgentSession(session) = &child.kind {
                summary.agents += 1;
                if matches!(session.mux_state, MuxIndicator::Ambiguous { .. }) {
                    summary.ambiguous += 1;
                }
            }
        }
        out.insert(row.id.clone(), summary);
    }
    out
}

/// Widest branch name a PR row shows before truncating it, so one long
/// branch can't crowd out the title. The branch is content, not a
/// column: only PRs with a local branch have one, so a column would sit
/// mostly blank.
const PR_BRANCH_MAX_WIDTH: usize = 24;

/// Column widths for the PR rows of a tree, so the number column lines
/// up across groups. Measured over the whole tree, not just the visible
/// rows, so folding a group doesn't shift the column. The age and state
/// cells have fixed widths, and the content (branch, then title) takes
/// whatever width is left.
#[derive(Clone, Copy, Default)]
pub(super) struct PrAlign {
    label: usize,
}

impl PrAlign {
    pub(super) fn of_tree(tree: &crate::tui::rows::RowTree) -> Self {
        let mut align = Self::default();
        for row in &tree.rows {
            let RowKind::Pr(pr) = &row.kind else {
                continue;
            };
            align.label = align.label.max(text::width(pr.label.as_str()));
        }
        align
    }
}

/// The pin marker cell of a row, the same in every view: the filled pin
/// glyph for a pin on a running entity, and its hollow twin in the ghost
/// color for a pin on one that is stopped, removed, missing, or not
/// started yet. Two cells, the glyph and a space.
pub(super) fn pin_marker_span(marker: crate::tui::rows::PinMarker, theme: &Theme) -> Span<'static> {
    use crate::tui::rows::PinMarker;
    match marker {
        PinMarker::None => Span::raw("  "),
        PinMarker::Running => node_kind_glyph_span(NodeKind::Pin, theme),
        PinMarker::Stopped => span!(ghost_style(theme); "{} ", state::PIN_STOPPED),
    }
}

/// The chip of a mixed-kind row: the agent's color for a registered
/// harness, the neutral program chip otherwise.
fn mixed_chip_span(chip: &crate::tui::rows::MixedChip, theme: &Theme) -> Span<'static> {
    use crate::tui::widgets::badge::{command_badge, harness_badge};
    if chip.agent {
        harness_badge(&chip.label, theme)
    } else {
        command_badge(&chip.label, theme)
    }
}

/// The age cell every row draws: the framework's age cell, four cells
/// right-aligned, `—` when the age is unknown, and tinted by how recent
/// `epoch` is at `now`, so a fresh row reads fresh wherever it appears.
/// The label was written when the row was built.
pub(super) fn age_span(
    age: Option<&str>,
    epoch: Option<i64>,
    now: i64,
    theme: &Theme,
) -> Span<'static> {
    let recency = Age::of(epoch, Some(now)).map(Age::recency);
    tuikit::age::cell(age, recency, tuikit::age::CELL_WIDTH, theme)
}

/// The body of a mixed-kind row after its prefix. The Columns grid is
/// label, status word, age, then the chip and content, which fill
/// `budget` and are cut with an ellipsis, at the `widths` the tree's
/// measure pass found. The Mux grid is the Mux view's own (name, chip,
/// age, state glyph), so a stopped mux lines up with the running ones.
/// Stopped, removed, missing, planned, and unknown rows are dimmed whole.
pub(super) fn render_mixed_spans(
    mixed: &crate::tui::rows::MixedRow,
    theme: &Theme,
    widths: MixedWidths,
    budget: usize,
    now: i64,
    handoff_spinner: Option<&'static str>,
) -> Vec<Span<'static>> {
    use crate::tui::rows::{EntityState, MixedGrid, MixedTone};
    let mut spans: Vec<Span<'static>> = Vec::new();
    if mixed.grid == MixedGrid::Mux {
        // Same columns as `render_mux_session_spans`: the name in the
        // link color, the chip, the age, then the state glyph.
        let label_width = mux_label_column_width(budget);
        spans.push(span!(
            Style::default().fg(theme.link);
            "{}",
            text::fit(&mixed.label, label_width)
        ));
        spans.push(Span::raw("  "));
        match &mixed.chip {
            Some(chip) => spans.push(mixed_chip_span(chip, theme)),
            None => spans.push(crate::tui::widgets::badge::no_agent_chip(theme)),
        }
        spans.push(Span::raw("  "));
        spans.push(age_span(mixed.age.as_deref(), mixed.age_epoch, now, theme));
        spans.push(Span::raw("  "));
        spans.push(match handoff_spinner {
            Some(glyph) => span!(Style::default().fg(theme.inert); "{glyph}"),
            None => match mixed.state {
                Some(EntityState::Stopped) => span!(ghost_style(theme); "{}", state::STOPPED),
                Some(EntityState::Linked) => {
                    span!(Style::default().fg(theme.mux_attached()); "{}", state::LINKED)
                }
                Some(EntityState::Unlinked) => span!(theme.muted; "{}", state::UNLINKED),
                Some(EntityState::Unknown) | None => span!(theme.muted; "{}", state::UNKNOWN),
            },
        });
        let room = budget.saturating_sub(text::spans_width(&spans));
        if room > 4 && !mixed.content.is_empty() {
            let content = text::truncate_end(&mixed.content, room - 2);
            spans.push(Span::styled(
                format!("  {content}"),
                Style::default().fg(theme.inert),
            ));
        }
    } else {
        // A row awaiting the refresh after an action shows a spinner
        // where its status word goes.
        let status_word = handoff_spinner.map_or(mixed.status.as_str(), |glyph| glyph);
        // A pull request's word takes the style of its state glyph, so
        // the two read as one signal; other live words share the
        // attached color.
        let status_style = match (mixed.tone, mixed.pr_status) {
            (MixedTone::Dim, _) => Style::default(),
            (MixedTone::Normal, Some(status)) => status.mark(theme).1,
            (MixedTone::Normal, None) => Style::default().fg(theme.mux_attached()),
        };
        let label = text::truncate_end(&mixed.label, columns::MIXED_LABEL_CELLS);
        spans.push(span!(Modifier::BOLD; "{}", text::pad(&label, widths.label)));
        let status = text::truncate_end(status_word, columns::MIXED_STATUS_CELLS);
        spans.push(span!(status_style; "  {}", text::pad(&status, widths.status)));
        spans.push(Span::raw("  "));
        spans.push(age_span(mixed.age.as_deref(), mixed.age_epoch, now, theme));
        // The chip cell has one width in the tree, so the content starts
        // in one column whether or not a row has a chip.
        if widths.chip > 0 {
            let cell = 2 + widths.chip;
            let fits = budget.saturating_sub(text::spans_width(&spans)) > cell + 2;
            match (&mixed.chip, fits) {
                (Some(chip), true) => {
                    spans.push(Span::raw("  "));
                    spans.push(mixed_chip_span(chip, theme));
                }
                (None, true) => spans.push(Span::raw(" ".repeat(cell))),
                _ => {}
            }
        }
        let room = budget.saturating_sub(text::spans_width(&spans));
        if room > 2 && !mixed.content.is_empty() {
            let content = text::truncate_end(&mixed.content, room - 2);
            spans.push(Span::styled(
                format!("  {content}"),
                Style::default().fg(theme.inert),
            ));
        }
    }
    if mixed.tone == MixedTone::Dim {
        ghostify(&mut spans, theme);
    }
    spans
}

/// The ghost style: the theme's `inert` color in italics. Gone entities
/// and pins that aren't running share it in every view.
pub(super) fn ghost_style(theme: &Theme) -> Style {
    selection::ghost_style(theme)
}

/// Restyle `spans` as a ghost. Chips keep their fill and recede
/// instead, so an agent still reads as an agent.
pub(super) fn ghostify(spans: &mut [Span<'static>], theme: &Theme) {
    selection::ghost(spans, theme);
}

/// The kind glyph of a mixed-kind row; PR glyphs take their state's hue.
fn mixed_glyph_span(mixed: &crate::tui::rows::MixedRow, theme: &Theme) -> Span<'static> {
    if mixed.node_kind == NodeKind::ForgePr {
        return pr_kind_glyph_span(mixed.pr_status, theme);
    }
    node_kind_glyph_span(mixed.node_kind, theme)
}

/// `handoff_spinner` replaces the attached glyph while the row awaits
/// the refresh that follows a tmux hand-off (ADR 0108).
pub(super) fn render_mux_session_spans(
    mux: &MuxSessionRow,
    theme: &Theme,
    now: i64,
    width: usize,
    handoff_spinner: Option<&'static str>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if width == 0 {
        return spans;
    }

    // Column order mirrors the agent-session row so muxed and
    // session rows scan as a single visual rhythm:
    //   name · harness chip · recency · attached glyph · preview
    // The mux name drops the backend prefix (e.g. `tmux:`) since
    // every row in the view is the same backend and the prefix only
    // steals horizontal space.
    let label = compact_mux_native_id(&mux.native_id);
    let label_width = mux_label_column_width(width);
    let label_style = Style::default().fg(theme.link);
    spans.push(span!(
        label_style;
        "{}",
        text::fit(&label, label_width)
    ));

    if width <= text::spans_width(&spans) + 4 {
        return text::fit_spans(spans, width);
    }

    spans.push(Span::raw("  "));
    append_mux_agent_labels(&mut spans, mux, theme, width);

    if width <= text::spans_width(&spans) + 4 {
        return text::fit_spans(spans, width);
    }

    spans.push(Span::raw("  "));
    spans.push(age_span(
        mux.recency.as_deref(),
        mux.activity_epoch,
        now,
        theme,
    ));

    spans.push(Span::raw("  "));
    if let Some(glyph) = handoff_spinner {
        spans.push(span!(Style::default().fg(theme.inert); "{glyph}"));
    } else {
        match mux.client_attached {
            Some(true) => {
                spans.push(span!(Style::default().fg(theme.mux_attached()); "{}", state::LINKED));
            }
            Some(false) => {
                spans.push(span!(theme.muted; "{}", state::UNLINKED));
            }
            None => {
                spans.push(span!(theme.muted; "{}", state::UNKNOWN));
            }
        }
    }

    append_mux_single_session_preview(&mut spans, mux, theme, width);
    text::fit_spans(spans, width)
}

pub(super) fn append_mux_agent_labels(
    spans: &mut Vec<Span<'static>>,
    mux: &MuxSessionRow,
    theme: &Theme,
    width: usize,
) {
    use crate::tui::widgets::badge::{command_badge, harness_badge, no_agent_chip};

    if mux.agent_labels.is_empty() {
        // A pane running a harness whose session isn't attributed to this
        // mux still gets that harness's badge and color; other programs
        // get the neutral program chip.
        match (&mux.program_harness, &mux.program) {
            (Some(harness), _) => spans.push(harness_badge(harness, theme)),
            (None, Some(program)) => spans.push(command_badge(program, theme)),
            (None, None) => spans.push(no_agent_chip(theme)),
        }
        return;
    }

    let max_labels = if width >= 76 { 2 } else { 1 };
    for (index, label) in mux.agent_labels.iter().take(max_labels).enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(harness_badge(label, theme));
    }

    let hidden_labels = mux.agent_labels.len().saturating_sub(max_labels);
    if hidden_labels > 0 {
        spans.push(span!(
            Style::default().fg(theme.inert);
            " +{hidden_labels}"
        ));
    }
}

pub(super) fn mux_label_column_width(width: usize) -> usize {
    match width {
        0..=38 => width.saturating_sub(14).clamp(10, 18),
        39..=72 => 20,
        73..=104 => 24,
        _ => 30,
    }
}

pub(super) fn append_mux_single_session_preview(
    spans: &mut Vec<Span<'static>>,
    mux: &MuxSessionRow,
    theme: &Theme,
    width: usize,
) {
    if let Some(hit) = &mux.content_hit {
        append_content_hit(spans, hit, width, theme);
        return;
    }
    let Some(preview) = mux
        .single_session_preview
        .as_deref()
        .filter(|p| !p.is_empty())
    else {
        return;
    };
    let used = text::spans_width(spans);
    if width <= used + 8 {
        return;
    }
    spans.push(Span::raw("  "));
    spans.push(span!(
        Style::default().fg(theme.inert).add_modifier(Modifier::ITALIC);
        "{}",
        text::truncate_end(preview, width - used - 2)
    ));
}

/// What a content search found in a row's session, in the slot a
/// message preview would take: a dim `3 hits` chip, then the best
/// snippet with the match picked out.
fn append_content_hit(
    spans: &mut Vec<Span<'static>>,
    hit: &crate::tui::rows::ContentHitCell,
    width: usize,
    theme: &Theme,
) {
    spans.push(Span::raw("  "));
    spans.push(crate::tui::widgets::badge::hits_chip(hit.count, theme));
    let used = text::spans_width(spans);
    if width <= used + 8 {
        return;
    }
    spans.push(Span::raw("  "));
    append_snippet(spans, &hit.snippet, &hit.highlight, width - used - 2, theme);
}

/// How many cells a flat row gives the session's name.
const FLAT_LABEL_WIDTH: usize = 12;

/// The columns of a matching turn under its session: who said it, what
/// kind of turn it is, when, and the snippet with the match picked out.
fn render_hit_spans(
    hit: &crate::tui::rows::HitRow,
    theme: &Theme,
    budget: usize,
    now: i64,
) -> Vec<Span<'static>> {
    let secondary = Style::default().fg(theme.inert);
    let mut spans = Vec::new();
    if let Some(origin) = &hit.origin {
        // A flat row names its session: the harness chip and the name,
        // padded so the columns after them line up down the list.
        spans.push(crate::tui::widgets::badge::harness_badge(
            &origin.harness_label,
            theme,
        ));
        let name = text::fit(&origin.label, FLAT_LABEL_WIDTH);
        spans.push(span!(Modifier::BOLD; " {name}  "));
    }
    spans.extend([
        span!(secondary; "turn {:<4}", hit.turn_index + 1),
        span!(secondary; "  {:<6}", hit.role),
        span!(theme.muted; "  {:<11}", hit.kind),
        Span::raw("  "),
        age_span(Some(&hit.age), hit.age_epoch, now, theme),
    ]);
    let used = text::spans_width(&spans);
    if budget > used + 8 {
        spans.push(Span::raw("  "));
        append_snippet(
            &mut spans,
            &hit.snippet,
            &hit.highlight,
            budget - used - 2,
            theme,
        );
    }
    spans
}

/// How many cells a pane row gives the mux's name.
const PANE_LABEL_WIDTH: usize = 16;

/// A pane whose scrollback matched, where a session row would be: the
/// mux's name, the pane's place in it, what it runs, how long ago it was
/// read, then the hit count and the newest matching line.
fn render_pane_spans(
    pane: &crate::tui::rows::PaneRow,
    theme: &Theme,
    budget: usize,
    now: i64,
) -> Vec<Span<'static>> {
    let secondary = Style::default().fg(theme.inert);
    let name = text::fit(&pane.mux_label, PANE_LABEL_WIDTH);
    let command = text::fit(pane.command.as_deref().unwrap_or(""), 8);
    let mut spans = vec![
        span!(Modifier::BOLD; "{name}"),
        span!(secondary; "  {:<5}", pane.location),
        span!(theme.muted; "  {command}"),
        Span::raw("  "),
        age_span(Some(&pane.age), pane.age_epoch, now, theme),
    ];
    let cell = crate::tui::rows::ContentHitCell {
        count: u32::try_from(pane.count).unwrap_or(u32::MAX),
        snippet: pane.snippet.clone(),
        highlight: pane.highlight.clone(),
    };
    append_content_hit(&mut spans, &cell, budget, theme);
    spans
}

/// A matching line of a pane's scrollback: its line number, how long ago
/// the scrollback was read, and the line with the match picked out. In
/// the flat listing it is prefixed with the mux's name and the pane's
/// place, padded so the cells after line up.
fn render_pane_hit_spans(
    hit: &crate::tui::rows::PaneHitRow,
    theme: &Theme,
    budget: usize,
    now: i64,
) -> Vec<Span<'static>> {
    let secondary = Style::default().fg(theme.inert);
    let mut spans = Vec::new();
    if let Some(origin) = &hit.origin {
        let name = text::fit(origin, PANE_LABEL_WIDTH);
        spans.push(span!(Modifier::BOLD; "{name}  "));
    }
    spans.extend([
        span!(secondary; "line {:<5}", hit.line),
        Span::raw("  "),
        age_span(Some(&hit.age), hit.age_epoch, now, theme),
    ]);
    let used = text::spans_width(&spans);
    if budget > used + 8 {
        spans.push(Span::raw("  "));
        append_snippet(
            &mut spans,
            &hit.snippet,
            &hit.highlight,
            budget - used - 2,
            theme,
        );
    }
    spans
}

/// `text` in the preview style with the byte range `highlight` picked
/// out in the theme's match style, cropped around the match to `budget`
/// cells so the match is always on screen.
fn append_snippet(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    highlight: &std::ops::Range<usize>,
    budget: usize,
    theme: &Theme,
) {
    let ticks = crate::tui::widgets::search::MARKDOWN_TICKS;
    let fitted = text::fit_snippet(text, highlight.clone(), budget, ticks);
    let plain = Style::default()
        .fg(theme.inert)
        .add_modifier(Modifier::ITALIC);
    let painted = text::paint(
        &[Span::styled(fitted.text, plain)],
        fitted.highlight,
        |_| theme.match_style(),
    );
    spans.extend(painted.into_iter().filter(|span| !span.content.is_empty()));
}

/// Per-row node-kind glyph span (ADR 0073). Renders the slate glyph
/// styled with the matching `theme.node_*` color, followed by a
/// trailing space so the next span sits at a predictable offset. The
/// span occupies `glyph_width + 1` cells; callers that compute
/// column budgets fold the result through [`spans_width`] and the
/// downstream width math automatically subtracts the new cells.
///
/// `ForgePr` is the one kind whose color depends on PR state; callers
/// render PR glyphs through [`pr_kind_glyph_span`] instead so the hue
/// follows the state.
pub(super) fn node_kind_glyph_span(kind: NodeKind, theme: &Theme) -> Span<'static> {
    let style = node_kind_style(kind, theme);
    span!(Style::default().fg(style.color); "{} ", style.glyph)
}

/// The PR kind glyph in the style of the PR's state glyph, so the two
/// never disagree. A row that doesn't know the state (a pin whose PR
/// isn't listed) uses the open hue, as every other PR glyph drawn
/// without a state does.
pub(super) fn pr_kind_glyph_span(status: Option<PrStatus>, theme: &Theme) -> Span<'static> {
    let glyph = node_kind_style(NodeKind::ForgePr, theme).glyph;
    let style = status.map_or_else(
        || Style::default().fg(theme.pr_open()),
        |status| status.mark(theme).1,
    );
    span!(style; "{glyph} ")
}

/// The PR row's local-branch marker (ADR 0129): the Branch glyph in the
/// branch color when the PR resolves to a local branch, a dim hollow
/// `◌` when it doesn't. One cell either way.
pub(super) fn pr_tracking_span(pr: &PrRow, theme: &Theme) -> Span<'static> {
    if pr.tracked {
        let style = node_kind_style(NodeKind::Branch, theme);
        span!(Style::default().fg(style.color); "{}", style.glyph)
    } else {
        span!(theme.muted; "{}", state::UNTRACKED)
    }
}

/// The body of a PR row after its kind glyph: number, local branch
/// marker, age, state glyph (the same order as the Sessions and Mux rows
/// put their age and state), then the content: the local branch, dim,
/// and the title, gray. The number aligns to `align`; the content fills
/// `budget`, the cells left in the row, and is cut with an ellipsis.
pub(super) fn render_pr_spans(
    pr: &PrRow,
    theme: &Theme,
    align: PrAlign,
    budget: usize,
    now: i64,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(span!(Modifier::BOLD; "{}", text::pad(&pr.label, align.label)));
    spans.push(Span::raw(" "));
    spans.push(pr_tracking_span(pr, theme));
    spans.push(Span::raw("  "));
    spans.push(age_span(
        pr.updated_recency.as_deref(),
        pr.updated_epoch,
        now,
        theme,
    ));
    spans.push(Span::raw("  "));
    let (glyph, style) = pr.status().mark(theme);
    spans.push(span!(style; "{glyph}"));
    // The content: the local branch when there is one, then the title.
    // Two cells separate each part from what comes before it.
    let mut room = budget.saturating_sub(text::spans_width(&spans));
    let mut push_content = |spans: &mut Vec<Span<'static>>, style: Style, text: &str| {
        if room <= 2 {
            return;
        }
        let text = text::truncate_end(text, room - 2);
        room = room.saturating_sub(text::width(text.as_str()) + 2);
        spans.push(Span::styled(format!("  {text}"), style));
    };
    if let Some(branch) = pr.branch_name.as_deref() {
        let branch = text::truncate_end(branch, PR_BRANCH_MAX_WIDTH);
        push_content(
            &mut spans,
            Style::default().add_modifier(theme.muted),
            &branch,
        );
    }
    if let Some(title) = pr.title.as_deref() {
        push_content(&mut spans, Style::default().fg(theme.inert), title);
    }
    spans
}

/// Map a [`GroupRow`] to its node kind so the renderer can stamp the
/// row with the glyph slate (ADR 0073). Synthetic group buckets
/// without a backing node return `None` and the renderer skips the
/// glyph prefix for that row.
pub(super) fn group_node_kind(group: &GroupRow) -> Option<NodeKind> {
    group.primary_node.as_ref().map(NodeKind::from)
}

/// Compute the per-row node-kind glyph span for any [`RowKind`], or
/// `None` when the row already carries an identity signal (the
/// colored harness pill on `AgentSession`) or when no graph node
/// kind backs it (synthetic group buckets).
/// Folds the per-row dispatch the `render_left_row` body and the
/// `group_row_body_width` pre-pass both rely on through a single
/// helper so the two paths agree on row widths.
pub(super) fn row_kind_glyph_span(kind: &RowKind, theme: &Theme) -> Option<Span<'static>> {
    let node_kind = match kind {
        RowKind::Group(group) => group_node_kind(group)?,
        // A session wears its kind glyph in every view, so it reads
        // as a session wherever it appears; the harness pill follows it.
        RowKind::AgentSession(_) => NodeKind::AgentSession,
        RowKind::MuxSession(_) => NodeKind::MuxSession,
        RowKind::Pr(pr) => return Some(pr_kind_glyph_span(Some(pr.status()), theme)),
        RowKind::Mixed(mixed) => return Some(mixed_glyph_span(mixed, theme)),
        // A matching turn is drawn under its session without a glyph.
        RowKind::Hit(_) | RowKind::PaneHit(_) => return None,
        // A pane is a part of a mux and wears its glyph.
        RowKind::Pane(_) => NodeKind::MuxSession,
    };
    Some(node_kind_glyph_span(node_kind, theme))
}

pub(super) fn mux_indicator_span(indicator: MuxIndicator, theme: &Theme) -> Span<'static> {
    // The row chip is attachable-binary. The linked glyph answers "yes,
    // there is a definitive mux here that `Enter`/`a` will attach to."
    // Both `Ambiguous` and `Unmuxed` fail that test and share the
    // unlinked glyph — ambiguity surfaces only on the enclosing group
    // row.
    match indicator {
        MuxIndicator::Attached => {
            span!(Style::default().fg(theme.mux_attached()); "{}", state::LINKED)
        }
        MuxIndicator::Ambiguous { .. } | MuxIndicator::Unmuxed => {
            span!(theme.muted; "{}", state::UNLINKED)
        }
    }
}
