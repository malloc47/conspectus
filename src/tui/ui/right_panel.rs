//! Right panel: the graph explorer (ADR 0074) and its title.

use super::*;

pub(super) fn draw_right_panel(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(right_panel_title(app, area.width as usize));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(detail) = app.detail() else {
        let widget = Paragraph::new(empty_right_panel_text(app))
            .style(Style::default().add_modifier(app.theme().placeholder));
        frame.render_widget(widget, inner);
        return;
    };

    // When the graph explorer state is available, render
    // the new mockup layout (Node + Upstream + Downstream sections
    // with cursor highlight) on top. Falls back to the legacy
    // section-grouped detail when the explorer state isn't ready
    // yet (race during the first SetData).
    if app.explorer().is_some() {
        // Reserve a usable minimum for the preview zone so a full
        // Related list cannot collapse the preview to 1–2 lines.
        // Below this floor on very small terminals the layout
        // falls back to the prior behavior (preview keeps at least
        // two rows after the divider).
        const MIN_PREVIEW_HEIGHT: u16 = 6;

        // `app` is `&mut` here so scroll
        // reconciliation lives on the App fields directly rather
        // than in `Cell`s. We derive all state-dependent lines +
        // wrap counts through an immutable borrow of the explorer
        // state, drop that borrow, mutate the scroll offset, then
        // re-borrow the state for the preview render below.
        //
        // Render the chip dividers (`Related`, `Other`) at the
        // narrower content width that `scrollbar_layout` will give
        // the paragraph once it reserves a gutter for the
        // scrollbar. Sizing the chip line to `inner.width` instead
        // would force the divider to overflow by one character at
        // the gutter boundary — it wraps to the next row and every
        // cursor row below it lands one line lower than the
        // per_line_rows math expects, leaving the cursor visible
        // off the bottom of the viewport.
        let content_width = inner.width.saturating_sub(1).max(1) as usize;
        let ExplorerRender { lines, cursor_line } = {
            let state = app.explorer().expect("checked above");
            render_explorer_lines(state, content_width, app.theme(), app.edge_meta_visible())
        };
        // Account for Paragraph wrap: any logical line whose
        // displayed width exceeds the pane width consumes extra
        // terminal rows. Without the wrap-aware row count, the
        // header sizing and the cursor-position math both
        // misread how much vertical space each line consumes.
        //
        // The paragraph renders at `content_area.width = inner.width
        // - 1` once `scrollbar_layout` reserves a gutter for the
        // scrollbar (it does for any overflowing content). Compute
        // wrap counts at that narrower width so a line that fits at
        // the full inner width but spills at the content width —
        // the `Related` chip divider is the canonical case, sized
        // to fit "exactly" the pane — gets the row count the
        // paragraph actually paints. When content fits in the
        // viewport and the gutter isn't reserved, this overcounts
        // by at most one row per long line; the worst case is a
        // tiny bit of slack in `header_height`, which is harmless.
        let pane_width = inner.width.saturating_sub(1).max(1) as usize;
        let per_line_rows: Vec<usize> = lines
            .iter()
            .map(|line| {
                let width = line
                    .spans
                    .iter()
                    .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                    .sum::<usize>()
                    .max(1);
                width.div_ceil(pane_width).max(1)
            })
            .collect();
        let wrapped_rows: usize = per_line_rows.iter().sum();
        // Post-wrap row span of the cursor in `lines`. The first
        // row is the sum of wrap rows for everything above the
        // cursor's logical line; the last row is the first row
        // plus the cursor line's own wrap count minus one. Using
        // both keeps a wrapped cursor line fully visible — the
        // first row drives "scroll up if cursor moves above the
        // top," the last row drives "scroll down if cursor moves
        // past the bottom."
        let (cursor_first_row, cursor_last_row) = cursor_line.map_or((0, 0), |idx| {
            let first = per_line_rows.iter().take(idx).sum::<usize>();
            let height = per_line_rows.get(idx).copied().unwrap_or(1).max(1);
            (first, first + height - 1)
        });
        let preview_floor = MIN_PREVIEW_HEIGHT.min(inner.height.saturating_sub(4));
        let max_header_height = inner
            .height
            .saturating_sub(preview_floor.saturating_add(1))
            .max(3);
        // +1 safety margin: the per-line `div_ceil` count assumes
        // the renderer packs each line tight to the right edge, but
        // Paragraph wraps on word boundaries and a long unbroken
        // token can push the actual row count one above the
        // estimate. Without the slack the last explorer line
        // (typically a single-link composite or the trailing Down-
        // stream row) gets clipped when the Node zone carries a
        // very long value.
        let header_height = u16::try_from(wrapped_rows)
            .unwrap_or(u16::MAX)
            .saturating_add(1)
            .min(max_header_height)
            .max(3);
        let split = vertical![==header_height, ==1, >=0].split(inner);
        // The reducer owns scroll reconciliation.
        // Draw measures the post-wrap cursor row span (that math
        // needs the widget-rendered lines) and dispatches the
        // measurement as a Msg; the reducer runs the same offset
        // math it used to run in the renderer and updates
        // `explorer_scroll`. Draw then reads the pre-computed
        // offset as a pure getter.
        app.update(Msg::ExplorerViewportChanged {
            cursor_first_row,
            cursor_last_row,
            viewport_height: split[0].height,
        });
        let scroll = app.explorer_scroll();
        let (explorer_content_area, explorer_scrollbar_area) =
            scrollbar_layout(split[0], wrapped_rows);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            explorer_content_area,
        );
        if let Some(area) = explorer_scrollbar_area {
            render_vertical_scrollbar(frame, area, wrapped_rows, scroll as usize);
        }
        frame.render_widget(
            Paragraph::new(preview_divider_line(
                app,
                split[1].width as usize,
                app.theme(),
            )),
            split[1],
        );
        // Re-fetch the explorer state for the preview render — the
        // adjust_explorer_scroll call above needed a mutable
        // borrow of `app`, which required dropping the earlier
        // immutable state borrow.
        let state = app.explorer().expect("checked above");
        draw_explorer_preview(app, state, frame, split[2]);
        return;
    }

    let mux_runtime = mux_runtime_rows(app);
    // Give the header exactly the height its fields need
    // (title + blank + one row per HeaderField), clamped
    // so the preview zone keeps a minimum of two rows.
    let header_h = header_zone_height(
        detail,
        app.detail_links_expanded(),
        &mux_runtime,
        inner.height,
        inner.width,
    );
    let split = vertical![==header_h, ==1, >=0].split(inner);

    draw_detail_header(
        detail,
        app.detail_links_expanded(),
        &mux_runtime,
        frame,
        split[0],
        app.theme(),
    );
    frame.render_widget(
        Paragraph::new(preview_divider_line(
            app,
            split[1].width as usize,
            app.theme(),
        )),
        split[1],
    );
    draw_detail_preview(app, detail, frame, split[2]);
}

/// Render the right pane title with the kind of node currently
/// being inspected (`session`, `mux`, `pr`, …) so the operator can
/// tell what they're looking at without re-reading the body.
/// Falls back to `detail` while no selection is resolved.
pub(super) fn right_panel_title(app: &App, width: usize) -> Line<'static> {
    let label = right_panel_kind_label(app);
    let mut spans = vec![Span::raw(" "), focus_marker_span(app, Focus::Right)];
    // ADR 0073 §3: the right-panel node header is `<glyph> <label>`
    // with the glyph in the node-kind color and the label bold. The
    // glyph is suppressed when there is no resolved selection
    // (fallback "detail" label) so a placeholder pane does not stamp
    // a misleading kind cue.
    if let Some(detail) = app.detail() {
        let node_kind = detail.kind;
        let style = node_kind_style(node_kind, app.theme());
        let color = if matches!(node_kind, NodeKind::ForgePr) {
            app.theme().pr_open
        } else {
            style.color
        };
        spans.push(span!(Style::default().fg(color); "{} ", style.glyph));
    }
    spans.push(span!(Modifier::BOLD; "{label}"));
    // Render the full drilldown chain in short `kind:tag`
    // form so the operator can see depth at a glance, with elision
    // (`first … last`) when the chain exceeds the title's available
    // width. The breadcrumb glyph `◀` plus the trailing depth marker
    // anchor the chain so even an elided rendering still conveys
    // where the explorer is.
    if let Some(state) = app.explorer()
        && !state.breadcrumb.is_empty()
    {
        // Budget for the chain itself: total title width minus the
        // fixed-cost spans we already pushed (focus marker + label +
        // " ◀ " prefix + " · depth N" suffix + the leading/trailing
        // padding spaces).
        let depth_suffix = format!(" · depth {}", state.breadcrumb.len());
        // ADR 0073 §3 prefix glyph: `<glyph> ` between the focus
        // marker and the bold label takes 2 cells when the detail
        // pane resolves to a known node kind.
        let kind_glyph_width = if app.detail().is_some() { 2 } else { 0 };
        let fixed_width = 1
            + focus_marker_width(app, Focus::Right)
            + kind_glyph_width
            + label.chars().count()
            + " ◀ ".chars().count()
            + depth_suffix.chars().count()
            + 1;
        let available = width.saturating_sub(fixed_width);
        if let Some(chain) = crate::tui::explorer::render_breadcrumb_chain(
            &state.breadcrumb,
            app.theme(),
            available.max(8),
        ) {
            // The leading ` ◀ ` and trailing `depth N` chrome stays
            // in `secondary_text`; the chain itself carries its own
            // per-hop styling (kind glyph in kind color, tag in
            // secondary_text) so the spans are pushed verbatim.
            spans.push(span!(Style::default().fg(app.theme().secondary_text); " ◀ "));
            spans.extend(chain.spans);
            spans.push(span!(Style::default().fg(app.theme().secondary_text); "{depth_suffix}"));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

pub(super) fn right_panel_kind_label(app: &App) -> &'static str {
    let Some(detail) = app.detail() else {
        return "detail";
    };
    match detail.kind {
        NodeKind::AgentSession => "session",
        NodeKind::MuxSession => "mux",
        NodeKind::ForgePr => "pr",
        NodeKind::Fork => "fork",
        NodeKind::Repo => "repo",
        NodeKind::Checkout => "checkout",
        NodeKind::Workspace => "workspace",
        NodeKind::Branch => "branch",
        NodeKind::Pin => "pin",
        NodeKind::RuntimeProcess => "detail",
    }
}

/// Output of [`render_explorer_lines`]: the rendered lines plus the
/// index in `lines` of the cursor's row (when one of the rendered
/// rows is selected). Used by [`draw_right_panel`] to compute the
/// post-wrap scroll offset that keeps the cursor in view.
pub(super) struct ExplorerRender {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) cursor_line: Option<usize>,
}

/// Render the related-entities layout (ADR 0074). Validated rows
/// (resolver winners) sit in a flat list under one `Related` chip
/// divider; alternates, conflicts, and unresolved stubs collapse
/// under a single `Other` header below the fold. Each row reads as
/// `<verb-column 22w> <kind-glyph> <neighbor_label>`; direction
/// flows from the verb, not from section placement.
pub(super) fn render_explorer_lines(
    state: &crate::tui::app::ExplorerState,
    width: usize,
    theme: &Theme,
    show_edge_meta: bool,
) -> ExplorerRender {
    use crate::tui::explorer::ExplorerRow;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut cursor_line: Option<usize> = None;
    let rows = state.rows();
    let cursor = state.cursor;
    let view = &state.view;

    for (idx, field) in view.fields(state.full_detail_expanded).iter().enumerate() {
        let flat_index = rows
            .iter()
            .position(|row| matches!(row, ExplorerRow::NodeField { index, .. } if *index == idx));
        let highlight = flat_index == Some(cursor);
        if highlight {
            cursor_line = Some(lines.len());
        }
        lines.push(render_node_field_line(field, highlight, theme));
    }

    if view.relationships.groups.is_empty() {
        return ExplorerRender { lines, cursor_line };
    }

    let counts = view.relationship_counts();
    let mut summary = format!("{} validated", counts.validated);
    if counts.other > 0 {
        summary.push_str(&format!(" · {} other", counts.other));
        if counts.ambiguous > 0 {
            summary.push_str(&format!(" · {} ⚠", counts.ambiguous));
        }
        if counts.unresolved > 0 {
            summary.push_str(&format!(" · {} —", counts.unresolved));
        }
    }
    lines.push(chip_divider_line(
        "Related",
        Some(&summary),
        width,
        theme,
        ChipAnchor::Right,
    ));

    for (row_index, row) in rows.iter().enumerate() {
        let highlight = row_index == cursor;
        let pushed = match row {
            ExplorerRow::NodeField { .. } => false,
            ExplorerRow::ValidatedLink {
                group_index,
                link_index,
            } => {
                if let Some(group) = view.relationships.groups.get(*group_index)
                    && let Some(link) = group.links.get(*link_index)
                {
                    lines.push(render_validated_link_line(
                        group,
                        link,
                        highlight,
                        theme,
                        show_edge_meta,
                        width,
                    ));
                    true
                } else {
                    false
                }
            }
            ExplorerRow::OtherHeader { expanded } => {
                lines.push(render_other_header_line(
                    *expanded,
                    counts.other,
                    counts.ambiguous,
                    counts.unresolved,
                    highlight,
                    theme,
                ));
                true
            }
            ExplorerRow::OtherLink {
                group_index,
                link_index,
            } => {
                if let Some(group) = view.relationships.groups.get(*group_index)
                    && let Some(link) = group.links.get(*link_index)
                {
                    lines.push(render_other_link_line(
                        group,
                        link,
                        highlight,
                        theme,
                        show_edge_meta,
                        width,
                    ));
                    true
                } else {
                    false
                }
            }
            ExplorerRow::OtherUnresolved {
                group_index,
                unresolved_index,
            } => {
                if let Some(group) = view.relationships.groups.get(*group_index)
                    && let Some(unresolved) = group.unresolved.get(*unresolved_index)
                {
                    lines.push(render_other_unresolved_line(
                        group, unresolved, highlight, theme,
                    ));
                    true
                } else {
                    false
                }
            }
        };
        if highlight && pushed {
            cursor_line = Some(lines.len() - 1);
        }
    }
    ExplorerRender { lines, cursor_line }
}

pub(super) fn render_node_field_line(
    field: &crate::tui::explorer::CoreField,
    highlight: bool,
    theme: &Theme,
) -> Line<'static> {
    let mut style = if field.placeholder {
        Style::default().add_modifier(theme.placeholder)
    } else {
        Style::default()
    };
    if highlight {
        style = style.add_modifier(Modifier::REVERSED);
    }
    let label = span!(Modifier::BOLD; "  {:<14}", field.label);
    let mut spans = vec![label];
    // ADR 0073 §3: render the kind-chip glyph before the value so the
    // chip stays anchored beside the label even when the value wraps
    // onto a new line in a narrow pane (otherwise the chip strands at
    // the wrap tail and reads as a stray symbol).
    if let Some(kind) = field.kind_chip {
        spans.push(kind_chip_span(Some(kind), theme));
        spans.push(Span::raw(" "));
    }
    spans.push(span!(style; "{}", field.value.clone()));
    if field.long_value.is_some() {
        spans.push(span!(theme.placeholder; "  (truncated · o)"));
    }
    if let Some(annotation) = field.annotation {
        spans.push(Span::raw(" "));
        spans.push(span!(
            Style::default().fg(theme.warning);
            "{}",
            annotation
        ));
    }
    Line::from(spans)
}

/// Per-kind glyph chip used to surface the graph node kind next to a
/// value or beside a relationship-explorer neighbor. ADR
/// 0073 §3 replaces the prior dim `[kind]` text with a 1-cell glyph
/// in the node-kind color so the chip carries identity at a glance.
/// A missing kind (the neighbor isn't in the snapshot) falls back to
/// a dim `?` so the chip slot stays visible without misleading the
/// operator.
///
/// `ForgePr`'s glyph color reuses `theme.pr_open` here because the
/// chip-rendering surfaces do not carry PR state down — the row
/// tree's [`forge_pr_glyph_span`] handles state-aware coloring
/// directly off the row.
pub(super) fn kind_chip_span(kind: Option<NodeKind>, theme: &Theme) -> Span<'static> {
    let Some(node_kind) = kind else {
        return span!(theme.placeholder; "?");
    };
    let style = node_kind_style(node_kind, theme);
    let color = if matches!(node_kind, NodeKind::ForgePr) {
        theme.pr_open
    } else {
        style.color
    };
    span!(Style::default().fg(color); "{}", style.glyph)
}

/// Width of the verb column in row lines (ADR 0074 §3 mockup). The
/// verb takes the place section labels (`Upstream`, `Downstream`)
/// used to hold; direction reads off the verb itself.
pub(super) const VERB_COLUMN_WIDTH: usize = 22;

/// Render a validated zone row: `<verb 22w> <glyph> <neighbor_label>`
/// (ADR 0074 §3, ADR 0075). The prior `★` resolver-winner marker
/// is dropped — every validated row is a resolver winner by
/// construction, so the marker no longer earns its column.
pub(super) fn render_validated_link_line(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
    width: usize,
) -> Line<'static> {
    render_related_row(
        group,
        link,
        highlight,
        theme,
        show_edge_meta,
        width,
        "  ",
        None,
        Style::default()
            .fg(theme.link_id)
            .add_modifier(Modifier::BOLD),
    )
}

/// Render an Other-zone link row (ADR 0075). The row prefix +
/// label color encode the `EdgeStateLabel` so an operator skimming
/// the zone can tell at a glance which rows are alternates and
/// which are conflicts:
///
/// - `AltOf(_)` → 6-space indent, no prefix glyph, label in
///   `theme.edge_alt_of`. Quiet — these are candidates the
///   resolver considered but didn't pick.
/// - `Conflict` → `⚠ ` prefix in `theme.edge_conflict`, label in
///   `theme.edge_conflict` + BOLD. The `⚠` is the cross-mode
///   anchor (survives `NO_COLOR`); it also lines up vocabulary
///   with the Other header's `K ⚠` summary count.
/// - `Resolves` is unreachable here — Resolves rows live in the
///   validated zone — but the renderer falls back to AltOf styling
///   defensively in case a future change routes one through.
pub(super) fn render_other_link_line(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
    width: usize,
) -> Line<'static> {
    use crate::tui::explorer::EdgeStateLabel;
    let (indent, prefix_span, label_style) = match link.edge_state {
        EdgeStateLabel::Conflict => {
            let conflict_style = Style::default()
                .fg(theme.edge_conflict)
                .add_modifier(Modifier::BOLD);
            let prefix = span!(conflict_style; "⚠ ");
            ("    ", Some(prefix), conflict_style)
        }
        EdgeStateLabel::AltOf(_) | EdgeStateLabel::Resolves => {
            ("      ", None, Style::default().fg(theme.edge_alt_of))
        }
    };
    render_related_row(
        group,
        link,
        highlight,
        theme,
        show_edge_meta,
        width,
        indent,
        prefix_span,
        label_style,
    )
}

/// Narrowest label budget worth middle-truncating to; below this the
/// optional edge-meta suffix gives up its room instead.
pub(super) const MIN_RELATED_LABEL_WIDTH: usize = 8;

/// Render one related-entities row. The neighbor label is
/// middle-truncated to the space left after the indent, verb column,
/// and kind glyph: the explorer paragraph word-wraps, so an unbroken
/// path that overflows would otherwise drop onto the next line and
/// leave this row visually empty.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_related_row(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
    width: usize,
    indent: &str,
    prefix: Option<Span<'static>>,
    label_style_base: Style,
) -> Line<'static> {
    let verb = crate::tui::explorer::directional_verb(&group.relation, group.direction);
    let verb_text = format!("{indent}{verb:<VERB_COLUMN_WIDTH$} ");
    let mut row_style = Style::default();
    let mut label_style = label_style_base;
    if highlight {
        row_style = row_style.add_modifier(Modifier::REVERSED);
        label_style = label_style.add_modifier(Modifier::REVERSED);
    }
    let mut kind_chip = kind_chip_span(link.neighbor_kind, theme);
    if highlight {
        kind_chip.style = kind_chip.style.add_modifier(Modifier::REVERSED);
    }
    let edge_meta = show_edge_meta.then(|| {
        format!(
            "  · {} · {} · {}",
            link.provenance.snake_case(),
            link.confidence.snake_case(),
            link.state.snake_case(),
        )
    });
    let prefix_width = prefix
        .as_ref()
        .map_or(0, |p| UnicodeWidthStr::width(p.content.as_ref()));
    // verb column + glyph + separating space
    let fixed = prefix_width + UnicodeWidthStr::width(verb_text.as_str()) + 2;
    let meta_width = edge_meta.as_deref().map_or(0, UnicodeWidthStr::width);
    let with_meta = width.saturating_sub(fixed + meta_width);
    let label_budget = if with_meta >= MIN_RELATED_LABEL_WIDTH {
        with_meta
    } else {
        width.saturating_sub(fixed).max(1)
    };
    let label = truncate_to_width_middle(&link.neighbor_label, label_budget);

    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some(mut prefix) = prefix {
        if highlight {
            prefix.style = prefix.style.add_modifier(Modifier::REVERSED);
        }
        spans.push(prefix);
    }
    spans.push(span!(row_style; "{verb_text}"));
    spans.push(kind_chip);
    spans.push(Span::raw(" "));
    spans.push(span!(label_style; "{label}"));
    if let Some(trailing) = edge_meta {
        spans.push(span!(label_style; "{trailing}"));
    }
    Line::from(spans)
}

/// `▶ Other (N · K ⚠ · L —)` / `▼ Other (N · K ⚠ · L —)` row that
/// gates the alternates / conflicts / unresolved zone (ADR 0074 §3).
pub(super) fn render_other_header_line(
    expanded: bool,
    other_count: usize,
    ambiguous_count: usize,
    unresolved_count: usize,
    highlight: bool,
    theme: &Theme,
) -> Line<'static> {
    let glyph = if expanded { "▼" } else { "▶" };
    let mut suffix = format!("({other_count}");
    if ambiguous_count > 0 {
        suffix.push_str(&format!(" · {ambiguous_count} ⚠"));
    }
    if unresolved_count > 0 {
        suffix.push_str(&format!(" · {unresolved_count} —"));
    }
    suffix.push(')');
    let text = format!("  {glyph} Other  {suffix}");
    let mut style = Style::default()
        .fg(theme.secondary_text)
        .add_modifier(Modifier::BOLD);
    if highlight {
        style = style.add_modifier(Modifier::REVERSED);
    }
    Line::from(span!(style; "{text}"))
}

/// Render an Other-zone unresolved-evidence row. Shape mirrors
/// [`render_other_link_line`] but the right side carries the
/// evidence summary instead of a neighbor id.
pub(super) fn render_other_unresolved_line(
    group: &crate::tui::explorer::RelationshipGroup,
    row: &crate::tui::explorer::UnresolvedRow,
    highlight: bool,
    theme: &Theme,
) -> Line<'static> {
    let verb = crate::tui::explorer::directional_verb(&group.relation, group.direction);
    let detail = unresolved_evidence_summary(row);
    let text = format!("      {verb:<VERB_COLUMN_WIDTH$} — {detail}",);
    let mut style = Style::default().add_modifier(theme.placeholder);
    if highlight {
        style = style.add_modifier(Modifier::REVERSED);
    }
    Line::from(span!(style; "{text}"))
}

pub(super) fn unresolved_evidence_summary(row: &crate::tui::explorer::UnresolvedRow) -> String {
    let mut parts = Vec::new();
    if let Some(native) = &row.evidence.native_id {
        parts.push(native.clone());
    }
    if let Some(harness) = &row.evidence.harness_key {
        parts.push(harness.clone());
    }
    if let Some(path) = &row.evidence.path {
        parts.push(path.clone());
    }
    if parts.is_empty() {
        parts.push(row.node_type.clone());
    }
    format!(
        "{}  ·  {} · {}",
        parts.join(" · "),
        row.provenance.snake_case(),
        row.confidence.snake_case(),
    )
}

/// Render the Preview zone body for the currently-selected explorer
/// row. Mirrors `draw_detail_preview` but draws the neighbor's core
/// summary plus an `edge` row instead of the live mux capture.
pub(super) fn draw_explorer_preview(
    app: &App,
    state: &crate::tui::app::ExplorerState,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    use crate::tui::explorer::RowPreview;
    let Some(row) = state.selected_row() else {
        // Fall back to the legacy preview body for rows that don't
        // carry a neighbor (node fields land here).
        if let Some(detail) = app.detail() {
            draw_detail_preview(app, detail, frame, area);
        }
        return;
    };
    let Some(preview) = state.view.row_preview(&row) else {
        // No neighbor for this row — defer to the legacy preview
        // body (live mux capture, message preview, etc.).
        if let Some(detail) = app.detail() {
            draw_detail_preview(app, detail, frame, area);
        }
        return;
    };
    let theme = app.theme();
    let mut lines: Vec<Line<'static>> = Vec::new();
    match preview {
        RowPreview::Link {
            neighbor_label,
            fields,
            provenance,
            confidence,
            state: link_state,
            edge_state,
            evidence,
        } => {
            lines.push(Line::from(span!(
                Modifier::BOLD;
                "  neighbor    {neighbor_label}"
            )));
            for field in fields {
                lines.push(render_node_field_line(field, false, theme));
            }
            let edge_value = format!(
                "{} · {} · {}   ·   {}",
                provenance.snake_case(),
                confidence.snake_case(),
                link_state.snake_case(),
                edge_state.snake_case(),
            );
            lines.push(Line::from(vec![
                span!(Modifier::BOLD; "  {:<14}", "edge"),
                span!(Style::default().fg(theme.warning); "{edge_value}"),
            ]));
            // One line per backing link so agreeing producers are
            // visible without each taking a row in the Related list.
            // The evidence kind names the producer more precisely
            // than its adapter, so it takes the slot when present.
            for (idx, item) in evidence.iter().enumerate() {
                let label = if idx == 0 { "evidence" } else { "" };
                let source = item
                    .evidence
                    .as_deref()
                    .or_else(|| Some(item.adapter.as_str()).filter(|a| !a.is_empty()));
                let value = source
                    .into_iter()
                    .chain([item.provenance.snake_case(), item.confidence.snake_case()])
                    .collect::<Vec<_>>()
                    .join(" · ");
                lines.push(Line::from(vec![
                    span!(Modifier::BOLD; "  {label:<14}"),
                    span!(theme.placeholder; "{value}"),
                ]));
            }
        }
        RowPreview::Unresolved {
            node_type,
            evidence,
            provenance,
            confidence,
            state: link_state,
        } => {
            lines.push(Line::from(span!(
                Modifier::BOLD;
                "  unresolved  {node_type}"
            )));
            for (label, value) in [
                ("harness_key", evidence.harness_key.as_deref()),
                ("native_id", evidence.native_id.as_deref()),
                ("state_scope", evidence.state_scope.as_deref()),
                ("path", evidence.path.as_deref()),
            ] {
                if let Some(value) = value {
                    lines.push(Line::from(vec![
                        span!(Modifier::BOLD; "  {label:<14}"),
                        Span::raw(value.to_string()),
                    ]));
                }
            }
            lines.push(Line::from(vec![
                span!(Modifier::BOLD; "  {:<14}", "edge"),
                span!(
                    theme.placeholder;
                    "{} · {} · {}",
                    provenance.snake_case(),
                    confidence.snake_case(),
                    link_state.snake_case()
                ),
            ]));
        }
    }
    let total_rows = wrapped_line_count(&lines, area.width);
    let (content_area, scrollbar_area) = scrollbar_layout(area, total_rows);
    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll(), 0));
    frame.render_widget(widget, content_area);
    if let Some(sb_area) = scrollbar_area {
        render_vertical_scrollbar(frame, sb_area, total_rows, app.preview_scroll() as usize);
    }
}

pub(super) fn empty_right_panel_text(app: &App) -> &'static str {
    if app.snapshot_handle().is_none() {
        "Loading…"
    } else {
        "Select a row to view its detail."
    }
}
