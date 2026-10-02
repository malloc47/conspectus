//! Left panel: the row tree, its scrollbar, and per-row spans.

use super::*;

pub(super) fn draw_left_panel(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(left_panel_title(app));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_rows();
    if visible.is_empty() {
        let placeholder = empty_left_panel_text(app);
        let widget = Paragraph::new(placeholder)
            .wrap(Wrap { trim: false })
            .style(Style::default().add_modifier(app.theme().placeholder));
        frame.render_widget(widget, inner);
        return;
    }

    let now = current_unix_epoch_for_render();
    let summaries = compute_group_summaries(app.tree());

    // Pre-pass over visible group rows. We compute two column
    // anchors so adjacent group rows read like a table:
    //   1. `label_width` — the widest label across every visible
    //      group row. Pads between the label and the secondary
    //      segment so the secondary column starts at the same
    //      column on every row.
    //   2. `body_width` — the widest body (with #1's padding
    //      already applied) across the rows that will get a
    //      summary chip block. Rows without a chip block don't
    //      participate so an idle workspace header doesn't push
    //      everyone else's chips right.
    let mut align = GroupAlign::default();
    for row in &visible {
        let label_width = group_row_label_width(row);
        if label_width > align.label_width {
            align.label_width = label_width;
        }
    }
    for row in &visible {
        let Some(summary) = summaries.get(&row.id).copied() else {
            continue;
        };
        if summary.agents == 0 {
            continue;
        }
        let body_width = group_row_body_width(row, app, align.label_width);
        if body_width > align.body_width {
            align.body_width = body_width;
        }
        let count_width = format!("({})", summary.agents).chars().count();
        if count_width > align.count_width {
            align.count_width = count_width;
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());
    let mut selected_primary_line: Option<usize> = None;
    for row in &visible {
        let is_selected = app.selection() == Some(&row.id);
        let summary = summaries.get(&row.id).copied();
        let primary = render_left_row(
            row,
            app,
            is_selected,
            inner.width as usize,
            now,
            summary,
            align,
        );
        if is_selected {
            selected_primary_line = Some(lines.len());
        }
        lines.push(primary);
    }

    // The reducer owns scroll reconciliation.
    // Dispatch the post-layout viewport height and read the
    // pre-computed offset as a pure getter. `selected_primary_line`
    // stays computed above as an assertion anchor — the reducer
    // arm derives the same index from
    // `visible_rows().position(...)` internally.
    let _ = selected_primary_line;
    app.update(Msg::LeftViewportChanged {
        viewport_height: inner.height,
    });
    let scroll = app.left_scroll();

    let total_lines = lines.len();
    let (content_area, scrollbar_area) = scrollbar_layout(inner, total_lines);
    let widget = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(widget, content_area);
    if let Some(area) = scrollbar_area {
        render_vertical_scrollbar(frame, area, total_lines, scroll as usize);
    }
}

/// Reserve a 1-column gutter on the right edge of `area` for the
/// vertical scrollbar when `content_length > area.height` (ADR 0076).
/// Returns `(content_area, Some(scrollbar_area))` for the scrolled
/// case so the paragraph body and scrollbar widget render into
/// disjoint regions — no overpaint, no inherited selection-row
/// `REVERSED` modifier bleeding through the scrollbar glyphs.
/// Returns `(area, None)` when content fits in the viewport
/// (fade-on-fit).
pub(super) fn scrollbar_layout(area: Rect, content_length: usize) -> (Rect, Option<Rect>) {
    if content_length == 0 || content_length <= area.height as usize || area.width < 2 {
        return (area, None);
    }
    let content = Rect {
        x: area.x,
        y: area.y,
        width: area.width - 1,
        height: area.height,
    };
    let scrollbar = Rect {
        x: area.x + area.width - 1,
        y: area.y,
        width: 1,
        height: area.height,
    };
    (content, Some(scrollbar))
}

/// Render a vertical scrollbar into the gutter `area` returned by
/// [`scrollbar_layout`]. `content_length` is the total wrapped-line
/// count of the buffered content; `position` is the renderer's
/// scroll offset (top of the visible window).
///
/// The ratatui `Scrollbar` widget treats `position` as the index
/// of the topmost visible item in a model where you can keep
/// scrolling until only one item is at the top — so its max
/// position is `content_length - 1`. Our scroll offset only goes
/// up to `content_length - viewport_height` (last item flush with
/// the viewport bottom), which mapped to a thumb that stopped at
/// the middle of the track even when the operator had scrolled all
/// the way down. Pass an effective `content_length = content -
/// viewport + 1` so the widget's max-position matches our max
/// scroll offset and "scrolled to the bottom" reads as
/// "thumb at the bottom."
///
/// `.remove_modifier(Modifier::REVERSED)` patches each scrollbar
/// cell with `sub_modifier = REVERSED` so the bar does not inherit
/// the selection highlight from whatever was painted there before.
pub(super) fn render_vertical_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    content_length: usize,
    position: usize,
) {
    let viewport = area.height as usize;
    if viewport == 0 || content_length <= viewport {
        return;
    }
    let effective_content = content_length - viewport + 1;
    let position = position.min(effective_content - 1);
    let mut state = ScrollbarState::new(effective_content)
        .position(position)
        .viewport_content_length(viewport);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .style(Style::default().remove_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// Render the left pane title as a lazydocker-style tab strip
/// showing every view, with the active one accented. Operators see
/// the available views at a glance instead of having to remember
/// the `1`–`5` accelerators or open the controls overlay.
pub(super) fn left_panel_title(app: &App) -> Line<'static> {
    let theme = app.theme();
    let active = app.active_view();
    let mut spans = vec![Span::raw(" "), focus_marker_span(app, Focus::Left)];
    let mut first = true;
    for &view in crate::tui::widgets::controls::VIEW_OPTIONS {
        if !first {
            spans.push(span!(Style::default().fg(theme.secondary_text); " · "));
        }
        first = false;
        let label = view_label(view);
        if view == active {
            spans.push(span!(
                Style::default().fg(theme.panel_focus_accent).add_modifier(Modifier::BOLD);
                "{label}"
            ));
        } else {
            spans.push(span!(Style::default().fg(theme.secondary_text); "{label}"));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

pub(super) fn empty_left_panel_text(app: &App) -> String {
    if app.snapshot_handle().is_none() {
        return "Loading discovery…".to_string();
    }
    if !app.filter().is_empty() {
        // Filtered-zero case: a snapshot is loaded but the
        // active filter dropped every session. Distinguish from
        // "no sessions discovered" so the operator knows their
        // filter — not the world — is the reason.
        let chips = render_filter_chips(app.filter());
        return format!("No rows match `{chips}`.\nPress `F` to clear filters, `f` to edit.");
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
    let label_width = UnicodeWidthStr::width(label.as_str());
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
            Some(budget) if budget < UnicodeWidthStr::width(secondary.as_str()) => {
                truncate_to_width_middle(&secondary, budget)
            }
            _ => secondary,
        };
        spans.push(span!(theme.placeholder; "  {rendered}"));
    }
    if group.is_launch_context {
        spans.push(span!(
            Style::default().fg(theme.cwd_mark).add_modifier(theme.placeholder);
            "  (cwd)"
        ));
    }
}

/// Width of the group row body (indent + disclosure + label +
/// secondary + cwd marker), padded to `target_label_width` between
/// the label and the secondary segment. Used by the left-pane
/// renderer to align the summary-chip column across visible group
/// rows. Returns 0 for non-group rows so callers can fold every
/// visible row through the same width-max.
pub(super) fn group_row_body_width(
    row: &crate::tui::rows::Row,
    app: &App,
    target_label_width: usize,
) -> usize {
    let RowKind::Group(group) = &row.kind else {
        return 0;
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let flat_sessions = matches!(
        app.grouping(),
        crate::tui::Grouping::Sessions(SessionsGrouping::None)
    );
    if !(flat_sessions && matches!(row.kind, RowKind::AgentSession(_))) {
        spans.push(Span::raw(row_indent(row.depth)));
    }
    spans.push(disclosure_span(row, app));
    // Mirror `render_left_row`'s ADR 0073 prefix glyph so the body
    // width the pre-pass measures matches the body width the row
    // renderer actually produces.
    if let Some(glyph) = row_kind_glyph_span(&row.kind, app.theme()) {
        spans.push(glyph);
    }
    append_group_body_spans(&mut spans, group, app.theme(), target_label_width, None);
    spans_width(&spans)
}

/// Width of just the label cell for a group row (excluding indent,
/// disclosure, secondary, and cwd marker). Used by the left-pane
/// pre-pass to compute the label column's max width before bodies
/// are rendered.
pub(super) fn group_row_label_width(row: &crate::tui::rows::Row) -> usize {
    let RowKind::Group(group) = &row.kind else {
        return 0;
    };
    UnicodeWidthStr::width(compact_path_label(&group.display_path).as_str())
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
/// gone — the only secondary signal is a single `⚠` (theme `warning`)
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
    spans.push(span!(theme.placeholder; "  {count:>count_width$}"));
    if summary.ambiguous > 0 {
        spans.push(Span::raw("  "));
        spans.push(span!(
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
            "⚠"
        ));
    }
}

/// Column anchors for group-row alignment computed once per render
/// pass. Padding keys the secondary segment and summary chip block
/// onto consistent columns so adjacent group rows read like a
/// table; non-group rows ignore these widths.
#[derive(Clone, Copy, Default)]
pub(super) struct GroupAlign {
    /// Max label width across visible group rows. Pads between the
    /// label and the secondary segment.
    pub(super) label_width: usize,
    /// Max body width (label-padded) across visible group rows that
    /// will get a summary chip block. Pads between the body and the
    /// `(N)` count plus optional `⚠` ambiguity glyph (ADR 0072).
    pub(super) body_width: usize,
    /// Max `(N)` count chip width (including parens) across visible
    /// group rows with sessions. Right-pads the count chip so any
    /// trailing ambiguity glyph anchors on the same column whether
    /// the count is `(2)` or `(72)`.
    pub(super) count_width: usize,
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
        .filter(|_| app.row_awaits_handoff(&row.kind))
        .map(|handoff| spinner_glyph(handoff.started_at));
    let mut spans: Vec<Span<'static>> = Vec::new();
    let flat_sessions = matches!(
        app.grouping(),
        crate::tui::Grouping::Sessions(SessionsGrouping::None)
    );
    if flat_sessions && matches!(row.kind, RowKind::AgentSession(_)) {
        spans.push(disclosure_span(row, app));
    } else {
        spans.push(Span::raw(row_indent(row.depth)));
        spans.push(disclosure_span(row, app));
    }

    // ADR 0073: per-row node-kind glyph between the disclosure and
    // the row body. Skipped for `Pin` rows (no graph NodeKind — the
    // 📌 sentinel is the identity) and for synthetic group buckets
    // without a backing node.
    if let Some(glyph) = row_kind_glyph_span(&row.kind, theme) {
        spans.push(glyph);
    }

    match &row.kind {
        RowKind::Group(group) => {
            // Reserve fixed space on the right for the summary chip
            // block (`  (N)` plus optional `  ⚠`) so the count and
            // ambiguity glyph stay visible at narrow widths. The dim
            // canonical path absorbs the slack via mid-string
            // truncation in `append_group_body_spans`.
            let summary_reservation = group_summary
                .filter(|s| s.agents > 0)
                .map_or(0, |_| align.count_width + 5);
            let head_width = spans_width(&spans);
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
                let current_width = spans_width(&spans);
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
        RowKind::AgentSessionMuxCandidate(candidate) => {
            spans.extend(render_candidate_spans(candidate, theme));
        }
        RowKind::MuxSession(mux) => {
            let remaining = width.saturating_sub(spans_width(&spans));
            spans.extend(render_mux_session_spans(
                mux,
                theme,
                now,
                remaining,
                handoff_spinner,
            ));
        }
        RowKind::Pr(pr) => {
            spans.push(span!(Modifier::BOLD; "{}", pr.repo_display.clone()));
            if let Some(state) = pr.state.as_deref() {
                let style = match state {
                    "open" => Style::default().fg(theme.mux_attached),
                    "closed" | "merged" => Style::default().add_modifier(theme.placeholder),
                    _ => Style::default().fg(theme.secondary_text),
                };
                spans.push(span!(style; "  {state}"));
            }
            if pr.is_draft {
                spans.push(span!(theme.placeholder; "  draft"));
            }
            if let Some(branch) = &pr.branch_name {
                spans.push(span!(theme.placeholder; "  {branch}"));
            }
            if let Some(updated) = &pr.updated_recency {
                spans.push(span!(Style::default().fg(theme.secondary_text); "  {updated}"));
            }
            spans.push(span!(theme.placeholder; "  ({})", pr.attached_count));
        }
        RowKind::Fork(fork) => {
            spans.push(span!(Modifier::BOLD; "{}", fork.fork_label.clone()));
            if let Some(parent) = &fork.parent_label {
                spans.push(span!(
                    Style::default().fg(theme.secondary_text);
                    "  parent:{parent}"
                ));
            }
            if let Some(scope) = &fork.scope {
                spans.push(span!(theme.placeholder; "  {scope}"));
            }
            spans.push(span!(theme.placeholder; "  ({})", fork.child_count));
        }
        RowKind::Pin(pin) => {
            // Pinned, but unbound — dim "📌" marker + display name.
            // Reuses the `placeholder` modifier to keep the row
            // visibly distinct without a dedicated Theme key.
            spans.push(span!(theme.placeholder; "📌  "));
            spans.push(span!(Modifier::BOLD; "{}", pin.display_name.clone()));
            spans.push(span!(
                theme.placeholder;
                "  ({} · {} · {} · {})",
                pin.state_label,
                pin.harness_label,
                pin.cwd_display,
                pin.mux_label
            ));
        }
        RowKind::Repo(repo) => {
            spans.extend(render_repo_spans(repo, theme));
        }
    }

    if handoff_spinner.is_some() {
        for span in &mut spans {
            span.style = span.style.add_modifier(theme.placeholder);
        }
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
            span.style = span.style.patch(selection_style);
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
    let placeholder = row_primary_node_is_pin(&session.primary_node);
    // Lead with the harness-native session key rather than
    // Conspectus's internal short node id. Operators recognize the
    // external session id; the internal id is still accepted by
    // explicit node lookup commands.
    let session_id =
        truncate_to_width_no_marker(&session.session.session_key, SESSION_ID_COLUMN_WIDTH);
    let id_style = if placeholder {
        Style::default()
            .fg(theme.secondary_text)
            .add_modifier(theme.placeholder)
    } else {
        Style::default().fg(theme.secondary_text)
    };
    spans.push(span!(id_style; "{session_id}  "));
    // The badge widget pads internally so every chip is the same
    // width regardless of label length; no external padding span
    // needed.
    spans.push(harness_badge(&session.harness_label, theme));
    spans.push(Span::raw("  "));
    let recency = session.recency.clone().unwrap_or_else(|| "—".to_string());
    let recency_style = recency_bucket(Some(now), session.activity_epoch).map_or_else(
        || Style::default().add_modifier(theme.placeholder),
        |bucket| bucket.style(theme),
    );
    spans.push(span!(recency_style; "{recency:>4}"));
    spans.push(Span::raw("  "));
    if let Some(glyph) = handoff_spinner {
        spans.push(span!(Style::default().fg(theme.secondary_text); "{glyph}"));
    } else if placeholder {
        // Mirror the mux view: an unbound-pin session has no live mux
        // to attach to, so the attached-glyph column shows the same
        // dotted-circle marker as the mux-view placeholder row.
        spans.push(span!(Style::default().fg(theme.pin_placeholder); "◌"));
    } else {
        spans.push(mux_indicator_span(session.mux_state, theme));
    }
    if session.pin_id.is_some() {
        // ADR 0057 bound-pin marker. Reuses `placeholder` rather
        // than a dedicated theme key.
        spans.push(span!(theme.placeholder; "  📌"));
    }
    // The row's tree label surfaces the operator-chosen
    // alias unconditionally and the harness-recorded title only when
    // the builder flagged this row for disambiguation. Right pane,
    // search index, and status hints still read `display_label`.
    if let Some(label) = session.tree_label().filter(|label| !label.is_empty()) {
        let label = truncate_to_width_strict(label, SESSION_DISPLAY_LABEL_WIDTH);
        let style = if placeholder {
            Style::default().add_modifier(theme.placeholder | Modifier::BOLD)
        } else if session
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
    if let Some(project) = session
        .project_display
        .as_deref()
        .filter(|project| !project.is_empty())
    {
        spans.push(span!(
            Style::default().fg(theme.secondary_text);
            "  {:<16}",
            truncate_to_width(project, 16)
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
    let Some(preview) = session.preview.as_deref().filter(|p| !p.is_empty()) else {
        return;
    };
    let used = spans_width(spans);
    if width <= used + 8 {
        return;
    }
    spans.push(Span::raw("  "));
    spans.push(span!(
        Style::default().fg(theme.secondary_text).add_modifier(Modifier::ITALIC);
        "{}",
        truncate_to_width(preview, width - used - 2)
    ));
}

/// Render a `RepoRow` so workspace members in the left pane scan as
/// the same visual rhythm as session / mux rows: short id column +
/// `repo` chip + bold display name + dim canonical path. Mirrors
/// `render_session_spans`'s column shape without the recency / mux
/// glyph columns (repos have no activity state of their own; the
/// row's purpose is identity and navigation).
pub(super) fn render_repo_spans(
    repo: &crate::tui::rows::RepoRow,
    theme: &Theme,
) -> Vec<Span<'static>> {
    const SHORT_ID_COLUMN_WIDTH: usize = 8;
    const KIND_BADGE_WIDTH: usize = 6; // ` repo `
    let mut spans = Vec::new();
    let short_id = truncate_to_width_no_marker(&repo.short_id, SHORT_ID_COLUMN_WIDTH);
    spans.push(span!(
        Style::default().fg(theme.secondary_text);
        "{short_id:<SHORT_ID_COLUMN_WIDTH$}  "
    ));
    spans.push(span!(
        Style::default().fg(theme.secondary_text).add_modifier(theme.badge);
        "{:<KIND_BADGE_WIDTH$}",
        " repo "
    ));
    spans.push(Span::raw("  "));
    spans.push(span!(Modifier::BOLD; "{}", repo.display_name.clone()));
    if let Some(path) = repo
        .canonical_path
        .as_deref()
        .filter(|p| !p.is_empty() && *p != repo.display_name)
    {
        spans.push(span!(
            Style::default().fg(theme.secondary_text).add_modifier(Modifier::DIM);
            "  {path}"
        ));
    }
    spans
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
    let placeholder = row_primary_node_is_pin(&mux.primary_node);

    // Column order mirrors the agent-session row so muxed and
    // session rows scan as a single visual rhythm:
    //   name · harness chip · recency · attached glyph · preview
    // The mux name drops the backend prefix (e.g. `tmux:`) since
    // every row in the view is the same backend and the prefix only
    // steals horizontal space.
    let label = compact_mux_native_id(&mux.native_id);
    let label_width = mux_label_column_width(width);
    let label_style = if placeholder {
        Style::default()
            .fg(theme.link_id)
            .add_modifier(theme.placeholder)
    } else {
        Style::default().fg(theme.link_id)
    };
    spans.push(span!(
        label_style;
        "{}",
        pad_to_width(truncate_to_width_strict(&label, label_width), label_width)
    ));

    if width <= spans_width(&spans) + 4 {
        return fit_spans_to_width(spans, width);
    }

    spans.push(Span::raw("  "));
    append_mux_agent_labels(&mut spans, mux, theme, width);

    if width <= spans_width(&spans) + 4 {
        return fit_spans_to_width(spans, width);
    }

    let recency = mux.recency.clone().unwrap_or_else(|| "—".to_string());
    let recency_style = recency_bucket(Some(now), mux.activity_epoch).map_or_else(
        || Style::default().add_modifier(theme.placeholder),
        |bucket| bucket.style(theme),
    );
    spans.push(Span::raw("  "));
    spans.push(span!(
        recency_style;
        "{:>4}",
        truncate_to_width_strict(&recency, 4)
    ));

    spans.push(Span::raw("  "));
    if let Some(glyph) = handoff_spinner {
        spans.push(span!(Style::default().fg(theme.secondary_text); "{glyph}"));
    } else if placeholder {
        // Single-char colored glyph keeps the attached-glyph column
        // in rhythm with ◉/◯/? on real rows. The dotted circle
        // (U+25CC) reads as "phantom / not currently live"; the
        // `pin_placeholder` color (default yellow) keeps it from
        // being confused with the dim `◯` unmuxed glyph.
        spans.push(span!(Style::default().fg(theme.pin_placeholder); "◌"));
    } else {
        match mux.client_attached {
            Some(true) => {
                spans.push(span!(Style::default().fg(theme.mux_attached); "◉"));
            }
            Some(false) => {
                spans.push(span!(theme.mux_unmuxed; "◯"));
            }
            None => {
                spans.push(span!(theme.placeholder; "?"));
            }
        }
    }
    if mux.ambiguous_count > 0 {
        spans.push(Span::raw(" "));
        spans.push(span!(Style::default().fg(theme.mux_ambiguous); "◐"));
    }
    if mux.pin_id.is_some() {
        // Bound-pin marker on the mux row. Same glyph the agent-
        // session row uses (ui.rs:1027) so pin-bound muxes scan
        // the same way pin-bound sessions do.
        spans.push(span!(theme.placeholder; "  📌"));
    }

    append_mux_single_session_preview(&mut spans, mux, theme, width);
    fit_spans_to_width(spans, width)
}

pub(super) fn row_primary_node_is_pin(node: &NodeId) -> bool {
    matches!(node, NodeId::Pin(_))
}

pub(super) fn append_mux_agent_labels(
    spans: &mut Vec<Span<'static>>,
    mux: &MuxSessionRow,
    theme: &Theme,
    width: usize,
) {
    use crate::tui::widgets::badge::{command_badge, harness_badge};

    if mux.agent_labels.is_empty() {
        // A pane running a harness whose session isn't attributed to this
        // mux still gets that harness's badge and color; other programs
        // get the neutral program chip.
        match (&mux.program_harness, &mux.program) {
            (Some(harness), _) => spans.push(harness_badge(harness, theme)),
            (None, Some(program)) => spans.push(command_badge(program, theme)),
            (None, None) => spans.push(span!(theme.placeholder; " no agent ")),
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
            Style::default().fg(theme.secondary_text);
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
    let Some(preview) = mux
        .single_session_preview
        .as_deref()
        .filter(|p| !p.is_empty())
    else {
        return;
    };
    let used = spans_width(spans);
    if width <= used + 8 {
        return;
    }
    spans.push(Span::raw("  "));
    spans.push(span!(
        Style::default().fg(theme.secondary_text).add_modifier(Modifier::ITALIC);
        "{}",
        truncate_to_width(preview, width - used - 2)
    ));
}

pub(super) fn spans_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

pub(super) fn render_candidate_spans(
    candidate: &MuxCandidateRow,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let glyph = if candidate.is_preferred {
        span!(Style::default().fg(theme.mux_attached); "◉ ")
    } else {
        span!(theme.mux_unmuxed; "◯ ")
    };
    spans.push(glyph);
    spans.push(Span::raw(compact_mux_label(&candidate.mux_label)));
    if candidate.is_preferred {
        spans.push(span!(theme.placeholder; "  (preferred)"));
    }
    spans
}

/// Per-row node-kind glyph span (ADR 0073). Renders the slate glyph
/// styled with the matching `theme.node_*` color, followed by a
/// trailing space so the next span sits at a predictable offset. The
/// span occupies `glyph_width + 1` cells; callers that compute
/// column budgets fold the result through [`spans_width`] and the
/// downstream width math automatically subtracts the new cells.
///
/// `ForgePr` is the one kind whose color depends on PR state; callers
/// render PR rows through [`forge_pr_glyph_span`] instead so the hue
/// follows `theme.pr_*`.
pub(super) fn node_kind_glyph_span(kind: NodeKind, theme: &Theme) -> Span<'static> {
    let style = node_kind_style(kind, theme);
    span!(Style::default().fg(style.color); "{} ", style.glyph)
}

/// Variant of [`node_kind_glyph_span`] for PR rows: emits the
/// `NodeKind::ForgePr` glyph styled with the appropriate `theme.pr_*`
/// color based on PR state. `is_draft` overrides the state-based hue
/// because the draft flag is independent of the open/closed/merged
/// label in our model.
pub(super) fn forge_pr_glyph_span(pr: &PrRow, theme: &Theme) -> Span<'static> {
    let style = node_kind_style(NodeKind::ForgePr, theme);
    let color = if pr.is_draft {
        theme.pr_draft
    } else {
        match pr.state.as_deref() {
            Some("open") => theme.pr_open,
            Some("closed") => theme.pr_closed,
            Some("merged") => theme.pr_merged,
            _ => theme.pr_open,
        }
    };
    span!(Style::default().fg(color); "{} ", style.glyph)
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
        // ADR 0073 amendment (2026-06): AgentSession rows already
        // carry the colored harness pill as their identity. Stacking
        // a separate `●` glyph next to it doubled the signal in
        // practice without adding information. The pill stands alone
        // in row contexts. `NodeKind::AgentSession`'s glyph + color
        // remain defined for detail-pane and explorer surfaces
        // where no pill is rendered.
        RowKind::AgentSession(_) => return None,
        RowKind::AgentSessionMuxCandidate(_) => NodeKind::MuxSession,
        RowKind::MuxSession(_) => NodeKind::MuxSession,
        RowKind::Pr(pr) => return Some(forge_pr_glyph_span(pr, theme)),
        RowKind::Fork(_) => NodeKind::Fork,
        RowKind::Pin(_) => NodeKind::Pin,
        RowKind::Repo(_) => NodeKind::Repo,
    };
    Some(node_kind_glyph_span(node_kind, theme))
}

pub(super) fn mux_indicator_span(state: MuxIndicator, theme: &Theme) -> Span<'static> {
    // ADR 0072: the row chip is attachable-binary. `◉` answers "yes,
    // there is a definitive mux here that `Enter`/`a` will attach to."
    // Both `Ambiguous` and `Unmuxed` fail that test and share the `◯`
    // glyph — ambiguity surfaces only on the enclosing group row.
    match state {
        MuxIndicator::Attached => span!(Style::default().fg(theme.mux_attached); "◉"),
        MuxIndicator::Ambiguous { .. } | MuxIndicator::Unmuxed => {
            span!(theme.mux_unmuxed; "◯")
        }
    }
}

pub(super) fn disclosure_span(row: &crate::tui::rows::Row, app: &App) -> Span<'static> {
    if !row.expandable {
        return Span::raw("  ");
    }
    let glyph = if app.is_expanded(&row.id) {
        "▼ "
    } else {
        "▶ "
    };
    span!(Style::default().fg(app.theme().disclosure); "{glyph}")
}

pub(super) fn row_indent(depth: u8) -> String {
    "  ".repeat(depth as usize)
}
