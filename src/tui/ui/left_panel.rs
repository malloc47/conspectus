//! Left panel: the row tree, its scrollbar, and per-row spans.

use super::*;

use crate::tui::icons::state;

/// Draw the left pane and return the window its tree drew, or `None`
/// when the tree is empty, so the window it had is kept for when rows
/// come back.
pub(super) fn draw_left_panel(
    app: &App,
    frame: &mut Frame<'_>,
    area: Rect,
) -> Option<ScrollWindow> {
    // The corners take a cell each from the top border.
    let title_width = usize::from(area.width.saturating_sub(2));
    let block = Block::default()
        .borders(Borders::ALL)
        .title(left_panel_title(app, title_width));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The Pinned view advertises legacy pins that still need
    // `pin migrate` on the last line of the pane.
    let inner = match pinned_banner(app) {
        Some(banner) if inner.height > 4 => {
            let lines: Vec<Line<'_>> = banner.split('\n').map(Line::from).collect();
            let rows = WrappedRows::new(&lines, usize::from(inner.width));
            let wrapped = u16::try_from(rows.len()).unwrap_or(u16::MAX).clamp(1, 3);
            let banner_area = Rect {
                y: inner.y + inner.height - wrapped,
                height: wrapped,
                ..inner
            };
            let widget =
                Paragraph::new(rows.into_rows()).style(Style::default().fg(app.theme().warning));
            frame.render_widget(widget, banner_area);
            Rect {
                height: inner.height - wrapped,
                ..inner
            }
        }
        _ => inner,
    };

    // The Search view says which remembered search it shows on its first
    // line.
    let inner = match search_header(app, inner.width.into()) {
        Some(header) if inner.height > 3 => {
            let header_area = Rect { height: 1, ..inner };
            frame.render_widget(Paragraph::new(header), header_area);
            Rect {
                y: inner.y + 1,
                height: inner.height - 1,
                ..inner
            }
        }
        _ => inner,
    };

    let visible = app.visible_rows();
    if visible.is_empty() {
        let placeholder: Vec<Line<'static>> =
            text::wrap(&empty_left_panel_text(app), usize::from(inner.width))
                .into_iter()
                .map(Line::from)
                .collect();
        let widget =
            Paragraph::new(placeholder).style(Style::default().add_modifier(app.theme().muted));
        frame.render_widget(widget, inner);
        return None;
    }

    let now = current_unix_epoch_for_render();
    let summaries = compute_group_summaries(app.tree());
    let align = GroupAlign::of_tree(app, &summaries);

    // Lay rows out against the width the paragraph really gets: the
    // scrollbar gutter is carved out of the pane when the rows overflow it.
    let gutter = ScrollGutter::layout(inner, visible.len());
    let row_width = usize::from(gutter.content().width);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());
    let mut selected_line: Option<usize> = None;
    for row in &visible {
        let is_selected = app.selection() == Some(&row.id);
        let summary = summaries.get(&row.id).copied();
        let primary = render_left_row(row, app, is_selected, row_width, now, summary, align);
        if is_selected && selected_line.is_none() {
            selected_line = Some(lines.len());
        }
        lines.push(primary);
    }

    // Each visible row is one line. The window starts where the last
    // frame left it, is measured again for this pane, and follows the
    // selection (its first row, when a row is listed twice).
    let mut window = app.left_window();
    window.resize(usize::from(inner.height), lines.len());
    if let Some(line) = selected_line {
        window.follow(line..line + 1);
    }
    let widget = Paragraph::new(lines).scroll(window.paragraph_scroll());
    frame.render_widget(widget, gutter.content());
    gutter.render(&window, frame.buffer_mut());
    Some(window)
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

/// Per-group aggregates surfaced as right-aligned chips on group
/// rows (Phase 7). Counts walk the full row tree (not just visible
/// rows) so a collapsed group still advertises what's inside.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GroupSummary {
    pub(super) agents: usize,
    pub(super) attached: usize,
    pub(super) ambiguous: usize,
    pub(super) unmuxed: usize,
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
                match session.mux_state {
                    MuxIndicator::Attached => summary.attached += 1,
                    MuxIndicator::Ambiguous { .. } => summary.ambiguous += 1,
                    MuxIndicator::Unmuxed => summary.unmuxed += 1,
                }
            }
        }
        out.insert(row.id.clone(), summary);
    }
    out
}

/// Append the body (label + secondary + cwd marker) spans for a
/// group row to `spans`. Extracted so the pre-pass that aligns
/// summary chips across group rows can measure body widths without
/// rebuilding the spans inside `render_left_row`.
pub(super) fn append_group_body_spans(
    spans: &mut Vec<Span<'static>>,
    group: &crate::tui::rows::GroupRow,
    theme: &Theme,
    target_label_width: usize,
    secondary_max_width: Option<usize>,
) {
    let label = compact_path_label(&group.display_path);
    let label_width = text::width(label.as_str());
    spans.push(span!(Modifier::BOLD; "{label}"));
    // Pad the label cell so the secondary content starts at the
    // same column across every visible group row. Skipped when the
    // label is already at or past the target.
    if label_width < target_label_width {
        spans.push(Span::raw(" ".repeat(target_label_width - label_width)));
    }
    let secondary = compact_path_secondary(&group.display_path);
    if !secondary.is_empty() {
        // Truncate the dim canonical path with a mid-string ellipsis
        // when a width budget is supplied and the natural width would
        // push the trailing summary chip block (count + ambiguity
        // glyph) past the right edge of the pane.
        let rendered = match secondary_max_width {
            Some(budget) if budget < text::width(secondary.as_str()) => {
                text::truncate_middle(&secondary, budget)
            }
            _ => secondary,
        };
        spans.push(span!(theme.muted; "  {rendered}"));
    }
    if group.is_launch_context {
        spans.push(span!(
            Style::default().fg(theme.cwd_mark()).add_modifier(theme.muted);
            "  (cwd)"
        ));
    }
}

/// Width of the group row body (the row prefix, label, secondary, and
/// cwd marker), with the label padded to `align.label_width`. Used to
/// align the summary-chip column across the tree's group rows; it
/// measures the prefix [`render_left_row`] draws, so the two agree.
/// Returns 0 for non-group rows so callers can fold every row through
/// the same width-max.
pub(super) fn group_row_body_width(
    row: &crate::tui::rows::Row,
    app: &App,
    align: &GroupAlign,
) -> usize {
    let RowKind::Group(group) = &row.kind else {
        return 0;
    };
    let mut spans = row_prefix_spans(row, app, align);
    append_group_body_spans(&mut spans, group, app.theme(), align.label_width, None);
    text::spans_width(&spans)
}

/// Width of just the label cell for a group row (excluding indent,
/// disclosure, secondary, and cwd marker). Used by the left-pane
/// pre-pass to compute the label column's max width before bodies
/// are rendered.
pub(super) fn group_row_label_width(row: &crate::tui::rows::Row) -> usize {
    let RowKind::Group(group) = &row.kind else {
        return 0;
    };
    text::width(compact_path_label(&group.display_path).as_str())
}

/// Append the right-aligned summary chips to a group row's span
/// list. Renders nothing when the group contains no sessions so
/// workspace-only ancestors stay quiet.
///
/// `count_width` right-pads the `(N)` count chip so single- and
/// double-digit counts (`(2)` vs `(72)`) anchor on the same column,
/// keeping any trailing ambiguity glyph aligned across rows.
///
/// ADR 0072: per-bucket muxed/unmuxed counts at the group level are
/// gone — the only secondary signal is a single `▲` (theme `warning`)
/// when at least one descendant session is in the `Ambiguous`
/// candidate-set state. The ambiguity catalog lives on the group's
/// detail pane (ADR 0071).
pub(super) fn append_group_summary_spans(
    spans: &mut Vec<Span<'static>>,
    summary: GroupSummary,
    theme: &Theme,
    count_width: usize,
) {
    if summary.agents == 0 {
        return;
    }
    let count = format!("({})", summary.agents);
    spans.push(span!(theme.muted; "  {count:>count_width$}"));
    if summary.ambiguous > 0 {
        spans.push(Span::raw("  "));
        spans.push(span!(
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
            "{}", marks::WARNING
        ));
    }
}

/// Column anchors for the rows of a tree, computed once per render
/// pass. Padding keys the secondary segment and summary chip block
/// onto consistent columns so adjacent group rows read like a
/// table; non-group rows ignore these widths. Every width is measured
/// over the whole tree, folded groups included (ADR 0131's rule), so
/// folding or unfolding a group never moves the columns of the others.
#[derive(Clone, Copy, Default)]
pub(super) struct GroupAlign {
    /// Max label width across the tree's group rows. Pads between the
    /// label and the secondary segment.
    pub(super) label_width: usize,
    /// Max body width (label-padded) across the group rows that get a
    /// summary chip block. Pads between the body and the `(N)` count
    /// plus optional `▲` ambiguity glyph (ADR 0072).
    pub(super) body_width: usize,
    /// Max `(N)` count chip width (including parens) across the group
    /// rows with sessions. Right-pads the count chip so any trailing
    /// ambiguity glyph anchors on the same column whether the count is
    /// `(2)` or `(72)`.
    pub(super) count_width: usize,
    /// Column widths for PR rows (ADR 0131).
    pub(super) pr: PrAlign,
    /// Column widths for the mixed-kind rows of the Pinned and History
    /// views.
    pub(super) mixed: MixedAlign,
    /// Whether the Sessions, Mux, PRs, and Search rows reserve the pin marker
    /// cell: once any row in the tree is pinned, every row holds its
    /// place so the columns don't move.
    pub(super) native_marker: bool,
    /// Whether a group header that stands for no entity keeps a blank
    /// kind glyph cell: once any header in the tree has a glyph, the
    /// glyph-less ones (`Ungrouped`, a PR remote group) hold its place
    /// so every header's label starts in the same column.
    pub(super) group_glyph: bool,
}

impl GroupAlign {
    /// The anchors for `app`'s tree, given each group's `summaries`.
    pub(super) fn of_tree(
        app: &App,
        summaries: &std::collections::HashMap<RowId, GroupSummary>,
    ) -> Self {
        let tree = app.tree();
        let mut align = Self {
            pr: PrAlign::of_tree(tree),
            mixed: MixedAlign::of_tree(tree, app.theme()),
            native_marker: native_marker_reserved(app),
            group_glyph: tree.rows.iter().any(
                |row| matches!(&row.kind, RowKind::Group(group) if group.primary_node.is_some()),
            ),
            ..Self::default()
        };
        align.label_width = tree
            .rows
            .iter()
            .map(group_row_label_width)
            .max()
            .unwrap_or(0);
        // Rows without a summary chip block don't take part, so an idle
        // workspace header doesn't push everyone else's counts right.
        for row in &tree.rows {
            let Some(summary) = summaries.get(&row.id).filter(|summary| summary.agents > 0) else {
                continue;
            };
            align.body_width = align.body_width.max(group_row_body_width(row, app, &align));
            align.count_width = align
                .count_width
                .max(format!("({})", summary.agents).chars().count());
        }
        align
    }
}

/// Whether the native views' pin marker cell is reserved: some row of
/// the tree stands for a pinned entity.
fn native_marker_reserved(app: &App) -> bool {
    if !matches!(
        app.active_view(),
        crate::tui::View::Sessions
            | crate::tui::View::Mux
            | crate::tui::View::Prs
            | crate::tui::View::Search
    ) {
        return false;
    }
    let Some(handle) = app.snapshot_handle() else {
        return false;
    };
    let markers = handle.pinned_nodes();
    !markers.is_empty()
        && app.tree().rows.iter().any(|row| {
            crate::tui::rows::row_node(&row.id).is_some_and(|node| markers.contains_key(node))
        })
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

/// Widest label a mixed-kind row shows before cutting it.
const MIXED_LABEL_MAX_WIDTH: usize = 28;
/// Widest status a mixed-kind row shows.
const MIXED_STATUS_MAX_WIDTH: usize = 9;

/// Column widths for the mixed-kind rows of a tree (ADR 0131's rule):
/// label, status, and age pad to the widest value in the whole tree, so
/// folding a group doesn't shift them. The content takes what is left.
#[derive(Clone, Copy, Default)]
pub(super) struct MixedAlign {
    label: usize,
    status: usize,
    /// Width of the pin marker cell, `0` when no row in the tree has a
    /// marker. Reserved on every row once any has one, so the columns
    /// hold still.
    marker: usize,
    /// Width of the chip cell, `0` when no row has a chip. Rows without
    /// one pad to it so the content starts in one column.
    chip: usize,
}

impl MixedAlign {
    pub(super) fn of_tree(tree: &crate::tui::rows::RowTree, theme: &Theme) -> Self {
        use crate::tui::rows::{MixedGrid, PinMarker};
        let mut align = Self::default();
        if tree.rows.iter().any(|row| {
            matches!(&row.kind, RowKind::Mixed(m)
                if m.grid == MixedGrid::Columns && m.marker != PinMarker::None)
        }) {
            align.marker = MARKER_CELL_WIDTH;
        }
        for row in &tree.rows {
            let RowKind::Mixed(mixed) = &row.kind else {
                continue;
            };
            if mixed.grid != MixedGrid::Columns {
                continue;
            }
            // The label cell runs from the row's own indent to a common
            // column, so nested rows line their status up with their
            // parents'.
            let label_width = text::width(mixed.label.as_str()).min(MIXED_LABEL_MAX_WIDTH);
            align.label = align
                .label
                .max(mixed_prefix_width(row.depth, align.marker) + label_width);
            align.status = align
                .status
                .max(text::width(mixed.status.as_str()).min(MIXED_STATUS_MAX_WIDTH));
            if let Some(chip) = &mixed.chip {
                align.chip = align
                    .chip
                    .max(text::width(mixed_chip_span(chip, theme).content.as_ref()));
            }
        }
        align
    }
}

/// Width of the pin marker cell: the glyph and its space.
const MARKER_CELL_WIDTH: usize = 2;

/// Cells before a mixed-kind row's label: indent, disclosure, the kind
/// glyph with its space, and the pin marker cell when the tree has one.
fn mixed_prefix_width(depth: u8, marker: usize) -> usize {
    usize::from(depth) * 2 + 4 + marker
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

/// The body of a mixed-kind row after its kind glyph. The Columns grid
/// is label, status word, age, then the chip and content, which fill
/// `budget` and are cut with an ellipsis. The Mux grid is the Mux
/// view's own (name, chip, age, state glyph), so a stopped mux lines up
/// with the running ones. Stopped, removed, missing, planned, and
/// unknown rows are dimmed whole.
pub(super) fn render_mixed_spans(
    mixed: &crate::tui::rows::MixedRow,
    theme: &Theme,
    align: MixedAlign,
    prefix_width: usize,
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
        let label = text::truncate_end(&mixed.label, MIXED_LABEL_MAX_WIDTH);
        let label_cell = align.label.saturating_sub(prefix_width);
        spans.push(span!(Modifier::BOLD; "{}", text::pad(&label, label_cell)));
        let status = text::truncate_end(status_word, MIXED_STATUS_MAX_WIDTH);
        spans.push(span!(status_style; "  {}", text::pad(&status, align.status)));
        spans.push(Span::raw("  "));
        spans.push(age_span(mixed.age.as_deref(), mixed.age_epoch, now, theme));
        // The chip cell has one width in the tree, so the content starts
        // in one column whether or not a row has a chip.
        if align.chip > 0 {
            let cell = 2 + align.chip;
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
    Style::default()
        .fg(theme.inert)
        .add_modifier(Modifier::ITALIC)
}

/// Whether `style` draws a filled chip (a harness or program badge),
/// whose fill is its background or its reversed foreground.
fn is_chip(style: Style) -> bool {
    style.bg.is_some() || style.add_modifier.contains(Modifier::REVERSED)
}

/// Restyle `spans` as a ghost. Chips keep their background and are
/// dimmed instead, so an agent still reads as an agent.
pub(super) fn ghostify(spans: &mut [Span<'static>], theme: &Theme) {
    for span in spans {
        span.style = if is_chip(span.style) {
            span.style.add_modifier(theme.muted | Modifier::ITALIC)
        } else {
            span.style
                .fg(theme.inert)
                .remove_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC)
        };
    }
}

/// The kind glyph of a mixed-kind row; PR glyphs take their state's hue.
fn mixed_glyph_span(mixed: &crate::tui::rows::MixedRow, theme: &Theme) -> Span<'static> {
    if mixed.node_kind == NodeKind::ForgePr {
        return pr_kind_glyph_span(mixed.pr_status, theme);
    }
    node_kind_glyph_span(mixed.node_kind, theme)
}

/// The cells every row starts with: indent, disclosure, kind glyph, and
/// pin marker. The group pre-pass measures headers with it, so it and
/// [`render_left_row`] agree on where each header's body starts.
fn row_prefix_spans(
    row: &crate::tui::rows::Row,
    app: &App,
    align: &GroupAlign,
) -> Vec<Span<'static>> {
    let theme = app.theme();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let flat_sessions = matches!(
        app.grouping(),
        crate::tui::Grouping::Sessions(SessionsGrouping::None)
    );
    if !(flat_sessions && matches!(row.kind, RowKind::AgentSession(_))) {
        spans.push(Span::raw(row_indent(row.depth)));
    }
    spans.push(disclosure_span(row, app));

    // ADR 0073: per-row node-kind glyph between the disclosure and
    // the row body. A group header for no entity has none, and leaves
    // the cell blank when a sibling header has one.
    match row_kind_glyph_span(&row.kind, theme) {
        Some(glyph) => spans.push(glyph),
        None if align.group_glyph && matches!(row.kind, RowKind::Group(_)) => {
            spans.push(Span::raw("  "));
        }
        None => {}
    }
    // Native views mark a pinned entity where it sits, with the marker
    // the Pinned view uses. Once any row is pinned the cell is reserved
    // on every row, so columns hold still.
    if align.native_marker
        && let Some(handle) = app.snapshot_handle()
    {
        let marker = crate::tui::rows::row_node(&row.id)
            .and_then(|node| handle.pinned_nodes().get(node).copied())
            .unwrap_or(crate::tui::rows::PinMarker::None);
        spans.push(pin_marker_span(marker, theme));
    }

    if let RowKind::Mixed(mixed) = &row.kind
        && mixed.grid == crate::tui::rows::MixedGrid::Columns
        && align.mixed.marker > 0
    {
        spans.push(pin_marker_span(mixed.marker, theme));
    }
    spans
}

/// Build the rendered line for a single visible row.
pub(super) fn render_left_row(
    row: &crate::tui::rows::Row,
    app: &App,
    is_selected: bool,
    width: usize,
    now: i64,
    group_summary: Option<GroupSummary>,
    align: GroupAlign,
) -> Line<'static> {
    let theme = app.theme();
    // Rows the operator just left in tmux keep their old values,
    // dimmed, with a spinner where the attach glyph goes, until the
    // background refresh lands (ADR 0108).
    let handoff_spinner = app
        .handoff()
        .filter(|_| app.row_is_frozen(row))
        .map(|handoff| spinner_glyph(handoff.started_at));
    let mut spans = row_prefix_spans(row, app, &align);

    match &row.kind {
        RowKind::Group(group) => {
            // Reserve fixed space on the right for the summary chip
            // block (`  (N)` plus optional `  ▲`) so the count and
            // ambiguity glyph stay visible at narrow widths. The dim
            // canonical path absorbs the slack via mid-string
            // truncation in `append_group_body_spans`.
            let summary_reservation = group_summary
                .filter(|s| s.agents > 0)
                .map_or(0, |_| align.count_width + 5);
            let head_width = text::spans_width(&spans);
            let label_cell_width = align.label_width.max(group_row_label_width(row));
            let secondary_budget = width
                .saturating_sub(head_width)
                .saturating_sub(label_cell_width)
                .saturating_sub(2) // "  " separator before the secondary path
                .saturating_sub(summary_reservation);
            append_group_body_spans(
                &mut spans,
                group,
                theme,
                align.label_width,
                Some(secondary_budget),
            );
            if let Some(summary) = group_summary {
                let body_target = align
                    .body_width
                    .min(width.saturating_sub(summary_reservation));
                let current_width = text::spans_width(&spans);
                if current_width < body_target {
                    spans.push(Span::raw(" ".repeat(body_target - current_width)));
                }
                append_group_summary_spans(&mut spans, summary, theme, align.count_width);
            }
        }
        RowKind::AgentSession(session) => {
            spans.extend(render_session_spans(session, theme, now, handoff_spinner));
            append_session_preview(&mut spans, session, width, theme);
        }
        RowKind::MuxSession(mux) => {
            let remaining = width.saturating_sub(text::spans_width(&spans));
            spans.extend(render_mux_session_spans(
                mux,
                theme,
                now,
                remaining,
                handoff_spinner,
            ));
        }
        RowKind::Pr(pr) => {
            let budget = width.saturating_sub(text::spans_width(&spans));
            spans.extend(render_pr_spans(pr, theme, align.pr, budget, now));
        }
        RowKind::Hit(hit) => {
            let budget = width.saturating_sub(text::spans_width(&spans));
            spans.extend(render_hit_spans(hit, theme, budget, now));
        }
        RowKind::Pane(pane) => {
            let budget = width.saturating_sub(text::spans_width(&spans));
            spans.extend(render_pane_spans(pane, theme, budget, now));
        }
        RowKind::PaneHit(hit) => {
            let budget = width.saturating_sub(text::spans_width(&spans));
            spans.extend(render_pane_hit_spans(hit, theme, budget, now));
        }
        RowKind::Mixed(mixed) => {
            let prefix_width = text::spans_width(&spans);
            let budget = width.saturating_sub(prefix_width);
            spans.extend(render_mixed_spans(
                mixed,
                theme,
                align.mixed,
                prefix_width,
                budget,
                now,
                handoff_spinner,
            ));
        }
    }

    if handoff_spinner.is_some() {
        for span in &mut spans {
            span.style = span.style.add_modifier(theme.muted);
        }
    }
    // A group (or any non-mixed row) standing for an entity that is
    // gone or unknown wears the ghost style like the mixed rows do.
    if !matches!(row.kind, RowKind::Mixed(_))
        && let Some(handle) = app.snapshot_handle()
        && let Some(node) = crate::tui::rows::row_node(&row.id)
        && handle.snapshot().presence(node).is_some()
    {
        ghostify(&mut spans, theme);
    }
    let mut line = Line::from(spans);
    if is_selected {
        let modifiers = if app.focus() == Focus::Left {
            theme.selection_active
        } else {
            theme.selection_inactive
        };
        let selection_style = Style::default().add_modifier(modifiers);
        line = line.style(selection_style);
        for span in &mut line.spans {
            span.style = if is_chip(span.style) {
                // A chip keeps the look it has on every other row, so it
                // still reads as a chip: the highlight marks the rest of
                // the row, and reversing a program chip's fill would
                // blend it into the highlight.
                span.style
                    .remove_modifier(modifiers.difference(span.style.add_modifier))
            } else {
                span.style.patch(selection_style)
            };
        }
    }
    line
}

/// `handoff_spinner` replaces the attach glyph while the row awaits
/// the refresh that follows a tmux hand-off (ADR 0108).
pub(super) fn render_session_spans(
    session: &AgentSessionRow,
    theme: &Theme,
    now: i64,
    handoff_spinner: Option<&'static str>,
) -> Vec<Span<'static>> {
    use crate::tui::widgets::badge::harness_badge;
    const SESSION_ID_COLUMN_WIDTH: usize = 8;
    const SESSION_DISPLAY_LABEL_WIDTH: usize = 32;

    let mut spans = Vec::new();
    // Lead with the harness-native session key rather than
    // Conspectus's internal short node id. Operators recognize the
    // external session id; the internal id is still accepted by
    // explicit node lookup commands.
    let session_id = text::pad(
        &text::clip(&session.session.session_key, SESSION_ID_COLUMN_WIDTH),
        SESSION_ID_COLUMN_WIDTH,
    );
    let id_style = Style::default().fg(theme.inert);
    spans.push(span!(id_style; "{session_id}  "));
    // The badge widget pads internally so every chip is the same
    // width regardless of label length; no external padding span
    // needed.
    spans.push(harness_badge(&session.harness_label, theme));
    spans.push(Span::raw("  "));
    spans.push(age_span(
        session.recency.as_deref(),
        session.activity_epoch,
        now,
        theme,
    ));
    spans.push(Span::raw("  "));
    if let Some(glyph) = handoff_spinner {
        spans.push(span!(Style::default().fg(theme.inert); "{glyph}"));
    } else {
        spans.push(mux_indicator_span(session.mux_state, theme));
    }
    // The row's tree label surfaces the operator-chosen
    // alias unconditionally and the harness-recorded title only when
    // the builder flagged this row for disambiguation. Right pane,
    // search index, and status hints still read `display_label`.
    if let Some(label) = session.tree_label().filter(|label| !label.is_empty()) {
        let label = text::truncate_end(label, SESSION_DISPLAY_LABEL_WIDTH);
        let style = if session
            .alias
            .as_deref()
            .is_some_and(|alias| !alias.is_empty())
        {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(span!(style; "  {label}"));
    }
    if let Some(mux) = session.gone_mux_name() {
        // An untitled session is already named after its mux, so the
        // chip only says that mux is gone.
        let named_after_it = session.tree_label() == Some(mux);
        spans.push(crate::tui::widgets::badge::was_in_chip(
            mux,
            named_after_it,
            theme,
        ));
    }
    if let Some(project) = session
        .project_display
        .as_deref()
        .filter(|project| !project.is_empty())
    {
        spans.push(span!(
            Style::default().fg(theme.inert);
            "  {}",
            text::fit(project, 16)
        ));
    }
    spans
}

pub(super) fn append_session_preview(
    spans: &mut Vec<Span<'static>>,
    session: &AgentSessionRow,
    width: usize,
    theme: &Theme,
) {
    if let Some(hit) = &session.content_hit {
        append_content_hit(spans, hit, width, theme);
        return;
    }
    let Some(preview) = session.preview.as_deref().filter(|p| !p.is_empty()) else {
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

pub(super) fn disclosure_span(row: &crate::tui::rows::Row, app: &App) -> Span<'static> {
    if !row.expandable {
        return Span::raw("  ");
    }
    let glyph = if app.is_expanded(&row.id) {
        marks::EXPANDED
    } else {
        marks::COLLAPSED
    };
    span!(Style::default().fg(app.theme().disclosure); "{glyph} ")
}

pub(super) fn row_indent(depth: u8) -> String {
    "  ".repeat(depth as usize)
}
