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
//! up. The Search view's hit, pane, and pane-hit rows are laid out from
//! the search modal's hit columns, so a hit reads alike in both.

mod columns;

use tuikit::empty_state::EmptyState;
use tuikit::selection;
use tuikit::tab_bar::TabBar;
use tuikit::tree::{RowCtx, RowState, TreeColumns, TreeView};

use super::*;

use crate::tui::icons::state;
use crate::tui::rows::Row;
use columns::{Col, LeftColumns, Schemas};

/// Draw the left pane and return the window its tree drew, or `None`
/// when the tree is empty, so the window it had is kept for when rows
/// come back.
pub(super) fn draw_left_panel(app: &App, buf: &mut Buffer, area: Rect) -> Option<ScrollWindow> {
    // The body is the pane less its border on each side.
    let inner_width = usize::from(area.width.saturating_sub(2));
    let states = app.tree_row_states();
    let columns = LeftColumns::of(app);
    let rows = LeftRows::of(app, &columns, &states);
    let line = |row: &Row, cx: &RowCtx<'_>| rows.line(row, cx);
    // The tree draws its pane in the framework's pane frame, which puts
    // the focus marker before the title and tints the border with focus.
    let mut view = TreeView::new(&app.tree().rows, app.tree_state(), line, app.theme())
        .focused(app.focus() == Focus::Left)
        .states(&states)
        .title(left_panel_title(app, PaneFrame::title_width(area.width)));
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
        view = view.empty(empty_left_panel(app));
    }
    view.render(area, buf)
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
/// by the framework.
struct LeftRows<'a> {
    columns: &'a LeftColumns<'a>,
    tree: TreeColumns<'a, Row, LeftColumns<'a>>,
}

impl<'a> LeftRows<'a> {
    /// The layout of `app`'s tree, whose rows have the `states` the App
    /// worked out (`App::tree_row_states`).
    fn of(app: &'a App, columns: &'a LeftColumns<'a>, states: &[RowState]) -> Self {
        let rows = &app.tree().rows;
        Self {
            columns,
            tree: TreeColumns::new(columns, rows, app.tree_state(), states, app.theme()),
        }
    }

    /// The producer the tree draws a row with: its cells at the width
    /// the context gives. The tree applies the row's looks.
    fn line(&self, row: &Row, cx: &RowCtx<'_>) -> Line<'static> {
        let theme = self.columns.theme;
        match &row.kind {
            RowKind::Mixed(mixed) if columns::stopped_mux(mixed) => self.tree.line(row, cx),
            // A Pinned or History row for something stopped, removed,
            // missing, planned, or unknown dims its cells from the label
            // on: in a list of mixed kinds its glyph is the only sign of
            // its kind, so the glyph keeps its color.
            RowKind::Mixed(mixed) if mixed.tone == crate::tui::rows::MixedTone::Dim => self
                .tree
                .line_with_look(row, cx, Col::MixedLabel, |spans| ghostify(spans, theme)),
            _ => self.tree.line(row, cx),
        }
    }
}

/// `rows` laid out as the left pane lays out a tree of only them, each at
/// `width` (the cells after the cursor gutter) with no cursor and the pane
/// focused, for a test of the cells a row draws: the producer's line, with
/// no gutter and no looks. `theme` and `now` are what a frame has; with a
/// `spinner`, every row awaits the hand-off and shows that frame.
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
                scrollbar: 0,
                theme,
            };
            left.line(row, &cx)
        })
        .collect()
}

/// `row` as the left pane draws it on its own in a body `width` cells
/// wide: its state as `app` works it out, its cells measured alone, the
/// cursor gutter before them, and its looks applied.
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
    };
    let cx = RowCtx {
        selected: is_selected,
        focused: app.focus() == Focus::Left,
        expanded: app.is_expanded(&row.id),
        ghost: states[0].ghost,
        pending: states[0].pending,
        mark: states[0].mark,
        width: width.saturating_sub(selection::GUTTER_WIDTH),
        scrollbar: 0,
        theme: app.theme(),
    };
    cx.row(left.line(row, &cx))
}

/// Render the left pane title as a lazydocker-style tab strip
/// showing every view, with the active one accented. Operators see
/// the available views at a glance instead of having to remember
/// the `1`–`6` accelerators or open the controls overlay.
///
/// `width` is the cells the pane's top border leaves for the title after
/// the focus marker the pane frame puts before it
/// ([`PaneFrame::title_width`]). The strip is the framework's
/// [`TabBar`]: it names every view when they fit; otherwise the inactive
/// names give way from the end of the strip, the active one stays, and
/// the strip ends with `…` and a free cell, so a narrow pane never cuts a
/// label in half. The frame adds the closing space when the strip leaves
/// a cell for it.
pub(super) fn left_panel_title(app: &App, width: usize) -> Line<'static> {
    let views = crate::tui::widgets::controls::VIEW_OPTIONS;
    let labels: Vec<&str> = views.iter().map(|&view| view_label(view)).collect();
    let active = views
        .iter()
        .position(|&view| view == app.active_view())
        .unwrap_or(0);
    TabBar::new(&labels, active, app.theme()).line(width)
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

/// What the left pane says when its tree has no rows: why it is empty
/// (still loading, a search that found nothing, a filter that hides
/// everything, a view with nothing in it) and, on the hint's row, what
/// would fill it.
pub(super) fn empty_left_panel(app: &App) -> EmptyState {
    if app.snapshot_handle().is_none() {
        return EmptyState::new("Loading discovery…");
    }
    if app.active_view() == crate::tui::View::Search {
        if !app.config().live_preview_enabled {
            return EmptyState::new(format!(
                "{}.",
                crate::tui::widgets::search::CONTENT_SEARCH_OFF
            ));
        }
        if app.viewed_search().is_none() {
            return EmptyState::new(
                "Press `s` (or Ctrl-/) to search the contents of sessions and mux panes.",
            );
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
        let reading =
            content.group == crate::viewer::search::Grouping::Flat && !content.hits().is_empty();
        let running = matches!(content.status, crate::tui::ContentStatus::Running(_));
        let searching = format!("Searching for \"{}\"…", content.text);
        let reading_text = format!("Reading the messages that match \"{}\"…", content.text);
        if app.active_view() == crate::tui::View::Search {
            return if running {
                EmptyState::new(searching).hint("Matches appear as they are found.")
            } else if reading {
                EmptyState::new(reading_text)
            } else {
                EmptyState::new(format!("Nothing matches \"{}\".", content.text))
                    .hint("Press `s` to search again, `<` for an earlier search.")
            };
        }
        return if running {
            EmptyState::new(searching)
                .hint("Sessions that match appear as they are found. Press `F` to clear.")
        } else if reading {
            EmptyState::new(reading_text).hint("Press `F` to clear.")
        } else {
            EmptyState::new(format!("No sessions match \"{}\".", content.text))
                .hint("Press `s` to search again, `F` to clear.")
        };
    }
    if !app.filter().is_empty() {
        // Filtered-zero case: a snapshot is loaded but the
        // active filter dropped every session. Distinguish from
        // "no sessions discovered" so the operator knows their
        // filter — not the world — is the reason.
        let chips = render_filter_chips(app.filter());
        return EmptyState::new(format!("No rows match `{chips}`."))
            .hint("Press `F` to clear filters, `f` to edit.");
    }
    if app.active_view() == crate::tui::View::Pinned {
        return EmptyState::new("No pins yet.").hint(format!(
            "Pin something with `{} pin add <target>`.",
            crate::invocation::invoked_name()
        ));
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
            return EmptyState::new(format!(
                "Every stopped or removed record is pinned ({pinned_gone}), so it is in the Pinned view."
            ))
            .hint("Clear `Unpinned only` in Controls (`f`) to list them here.");
        }
        return EmptyState::new(
            "Nothing has stopped. Entities Conspectus has seen stop or vanish are listed here.",
        )
        .hint("Press `.` for the menu (Clean up…).");
    }
    if app.active_view() == crate::tui::View::Prs {
        // Forge listings are reused for the forge interval, so `r`
        // alone won't show a PR opened a minute ago.
        return EmptyState::new("No pull requests discovered.")
            .hint("Press `Ctrl-R` to ask the forges again or `q` to quit.");
    }
    EmptyState::new("No sessions discovered.").hint("Press `r` to refresh or `q` to quit.")
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

/// Map a [`GroupRow`] to its node kind so the renderer can stamp the
/// row with the glyph slate (ADR 0073). Synthetic group buckets
/// without a backing node return `None` and the renderer skips the
/// glyph prefix for that row.
pub(super) fn group_node_kind(group: &GroupRow) -> Option<NodeKind> {
    group.primary_node.as_ref().map(NodeKind::from)
}

/// The node-kind glyph span for any [`RowKind`], or `None` when no graph
/// node kind backs the row (a synthetic group bucket, a matching turn or
/// pane line), so every row kind's glyph cell comes from one place.
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

#[cfg(test)]
#[path = "left_panel/hits_tests.rs"]
mod hits_tests;
