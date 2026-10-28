//! Right panel: the graph explorer (ADR 0074) and its title.

use super::*;

/// Rows reserved for the Preview zone so a full Related list cannot
/// collapse it to one or two lines. Below this floor on very small
/// terminals the layout falls back to a smaller preview (at least two
/// rows after the divider).
const MIN_PREVIEW_HEIGHT: u16 = 6;

/// Draw the right pane and return the windows its explorer zone and its
/// Preview drew, `None` for a zone the pane did not draw (or a Preview
/// whose content has not loaded).
pub(super) fn draw_right_panel(
    app: &mut App,
    frame: &mut Frame<'_>,
    area: Rect,
) -> (Option<ScrollWindow>, Option<ScrollWindow>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(right_panel_title(app, area.width as usize));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // No explorer means the selection names no node (or nothing has
    // loaded yet): show the placeholder instead of the explorer.
    if app.explorer().is_none() {
        let widget = Paragraph::new(empty_right_panel_text(app))
            .style(Style::default().add_modifier(app.theme().muted));
        frame.render_widget(widget, inner);
        return (None, None);
    }

    // Render the chip dividers (`Related`, `Other`) at the
    // narrower content width the scrollbar gutter leaves the
    // paragraph once it reserves a column for the scrollbar.
    // Sizing the chip line to `inner.width` instead would force
    // the divider to overflow by one character at the gutter
    // boundary: it wraps to the next row and every cursor row
    // below it lands one line lower than the row count expects,
    // leaving the cursor visible off the bottom of the viewport.
    let content_width = inner.width.saturating_sub(1).max(1) as usize;
    let ExplorerRender {
        mut lines,
        mut cursor_line,
    } = {
        let state = app.explorer().expect("checked above");
        render_explorer_lines(
            state,
            content_width,
            app.theme(),
            app.edge_meta_visible(),
            app.focus() == Focus::Right,
        )
    };
    // A selected hit row says which turn it is and shows what matched.
    let hit_lines = selected_hit_lines(app);
    if !hit_lines.is_empty() {
        cursor_line = cursor_line.map(|index| index + hit_lines.len());
        lines.splice(0..0, hit_lines);
    }
    // Wrap every logical line here and draw these rows as they are, so
    // the header sizing and the cursor-position math count exactly the
    // rows the paragraph paints. The rows wrap at the full width when
    // they fit the zone, and at one column less when they overflow it,
    // since the scrollbar gutter then takes a column.
    let preview_floor = MIN_PREVIEW_HEIGHT.min(inner.height.saturating_sub(4));
    let max_header_height = inner
        .height
        .saturating_sub(preview_floor.saturating_add(1))
        .max(3);
    let rows = WrappedRows::fit(
        &lines,
        usize::from(inner.width),
        usize::from(max_header_height),
    );
    let header_height = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .min(max_header_height)
        .max(3);
    let split = vertical![==header_height, ==1, >=0].split(inner);
    // The window starts where the last frame left it, is measured again
    // for the zone, and follows every row the cursor's line took, so a
    // wrapped cursor line is shown whole; with no cursor, the first row.
    let mut window = app.explorer_window();
    window.resize(usize::from(split[0].height), rows.len());
    window.follow(cursor_line.map_or(0..1, |idx| rows.rows_of(idx)));
    let gutter = ScrollGutter::layout(split[0], rows.len());
    frame.render_widget(
        Paragraph::new(rows.into_rows()).scroll(window.paragraph_scroll()),
        gutter.content(),
    );
    gutter.render(&window, frame.buffer_mut());
    frame.render_widget(
        Paragraph::new(preview_divider_line(
            app,
            split[1].width as usize,
            app.theme(),
        )),
        split[1],
    );
    // A transcript preview leaves a column for its scrollbar.
    app.prepare_preview(split[2].width.saturating_sub(1).max(1));
    let state = app.explorer().expect("checked above");
    let preview = draw_explorer_preview(app, state, frame, split[2]);
    // A Preview still waiting for its content draws a placeholder, not
    // the rows its offset counts, so it keeps the offset until the
    // content has loaded and a frame shows it.
    let loaded = crate::tui::preview::preview_source(app)
        .is_none_or(|source| app.previews().get(&source).is_some());
    (Some(window), loaded.then_some(preview))
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
    if let Some(node_kind) = right_panel_node_kind(app) {
        let style = node_kind_style(node_kind, app.theme());
        spans.push(span!(Style::default().fg(style.color); "{} ", style.glyph));
    }
    spans.push(span!(Modifier::BOLD; "{label}"));
    // Render the full drilldown chain in short `kind:tag`
    // form so the operator can see depth at a glance, with elision
    // (`first … last`) when the chain exceeds the title's available
    // width. The breadcrumb glyph `◄` plus the trailing depth marker
    // anchor the chain so even an elided rendering still conveys
    // where the explorer is.
    if let Some(state) = app.explorer()
        && !state.breadcrumb.is_empty()
    {
        // Budget for the chain itself: total title width minus the
        // fixed-cost spans we already pushed (focus marker + label +
        // back-mark prefix + " · depth N" suffix + the leading/trailing
        // padding spaces).
        let depth_suffix = format!(" · depth {}", state.breadcrumb.len());
        let back_prefix = format!(" {} ", marks::BACK);
        // ADR 0073 §3 prefix glyph: `<glyph> ` between the focus
        // marker and the bold label takes 2 cells whenever the pane
        // resolves to a known node kind, which a live explorer does.
        let kind_glyph_width = 2;
        let fixed_width = 1
            + focus_marker_width(app, Focus::Right)
            + kind_glyph_width
            + text::width(label)
            + text::width(&back_prefix)
            + text::width(&depth_suffix)
            + 1;
        let available = width.saturating_sub(fixed_width);
        if let Some(chain) = crate::tui::explorer::render_breadcrumb_chain(
            &state.breadcrumb,
            app.theme(),
            available.max(8),
        ) {
            // The leading ` ◄ ` and trailing `depth N` chrome stays
            // `inert`; the chain itself carries its own per-hop
            // styling (kind glyph in kind color, tag `inert`) so the
            // spans are pushed verbatim.
            spans.push(span!(Style::default().fg(app.theme().inert); "{back_prefix}"));
            spans.extend(chain.spans);
            spans.push(span!(Style::default().fg(app.theme().inert); "{depth_suffix}"));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// The kind of node the explorer is focused on, `None` while no
/// selection resolves to a node.
fn right_panel_node_kind(app: &App) -> Option<NodeKind> {
    app.explorer().map(|state| state.view.kind)
}

pub(super) fn right_panel_kind_label(app: &App) -> &'static str {
    let Some(node_kind) = right_panel_node_kind(app) else {
        return "detail";
    };
    match node_kind {
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

/// Where a zone divider puts its summary relative to its chip. The line
/// is a rule, then the chip and summary, then a short closing rule.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum ChipAnchor {
    /// The summary follows the chip. Preview has no summary, so its chip
    /// ends the line.
    Left,
    /// The summary precedes the chip, which sits flush against the
    /// closing rule. That keeps the bold zone label easy to scan
    /// vertically as the summary changes length.
    Right,
}

/// A zone divider: a dim rule that fills the width, a filled-chip label
/// (`" label "`, accent colour with the badge modifier), an optional dim
/// summary beside the chip, and a two-cell closing rule. When the chip
/// and summary leave no room, the leading rule drops out.
pub(super) fn chip_divider_line(
    label: &str,
    suffix: Option<&str>,
    width: usize,
    theme: &Theme,
    anchor: ChipAnchor,
) -> Line<'static> {
    let chip_text = format!(" {label} ");
    let chip_width = text::width(&chip_text);
    let suffix_text = suffix.map(|s| format!(" {s} ")).unwrap_or_default();
    let suffix_width = text::width(&suffix_text);
    let trailing_rule = 2;
    let leading_rule = width.saturating_sub(chip_width + suffix_width + trailing_rule);
    let chip_span = span!(
        Style::default().fg(theme.accent).add_modifier(theme.badge);
        "{chip_text}"
    );
    let suffix_span =
        (suffix_width > 0).then(|| span!(Style::default().fg(theme.inert); "{suffix_text}"));
    let mut spans = Vec::with_capacity(4);
    if leading_rule > 0 {
        spans.push(span!(theme.muted; "{}", marks::RULE.repeat(leading_rule)));
    }
    match anchor {
        ChipAnchor::Left => {
            spans.push(chip_span);
            if let Some(span) = suffix_span {
                spans.push(span);
            }
        }
        ChipAnchor::Right => {
            if let Some(span) = suffix_span {
                spans.push(span);
            }
            spans.push(chip_span);
        }
    }
    spans.push(span!(theme.muted; "{}", marks::RULE.repeat(trailing_rule)));
    Line::from(spans)
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
    focused: bool,
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
        let at_cursor = flat_index == Some(cursor);
        // The cursor row still drives scrolling when the pane is
        // unfocused, but only the focused pane draws a highlight.
        let highlight = at_cursor && focused;
        if at_cursor {
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
            summary.push_str(&format!(" · {} {}", counts.ambiguous, marks::WARNING));
        }
        if counts.unresolved > 0 {
            summary.push_str(&format!(" · {} {}", counts.unresolved, marks::NO_VALUE));
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
        let at_cursor = row_index == cursor;
        let highlight = at_cursor && focused;
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
        if at_cursor && pushed {
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
        Style::default().add_modifier(theme.muted)
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
        spans.push(span!(theme.muted; "  (truncated · o)"));
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
/// `ForgePr`'s glyph takes the `pr_open` hue here because the
/// chip-rendering surfaces do not carry PR state down — the row
/// tree's [`pr_kind_glyph_span`] handles state-aware coloring
/// directly off the row.
pub(super) fn kind_chip_span(kind: Option<NodeKind>, theme: &Theme) -> Span<'static> {
    let Some(node_kind) = kind else {
        return span!(theme.muted; "?");
    };
    let style = node_kind_style(node_kind, theme);
    span!(Style::default().fg(style.color); "{}", style.glyph)
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
        Style::default().fg(theme.link).add_modifier(Modifier::BOLD),
    )
}

/// Render an Other-zone link row (ADR 0075). The row prefix +
/// label color encode the `EdgeStateLabel` so an operator skimming
/// the zone can tell at a glance which rows are alternates and
/// which are conflicts:
///
/// - `AltOf(_)` → 6-space indent, no prefix glyph, label in
///   `theme.edge_alt_of()`. Quiet — these are candidates the
///   resolver considered but didn't pick.
/// - `Conflict` → `▲ ` prefix in `theme.edge_conflict()`, label in
///   `theme.edge_conflict()` + BOLD. The `▲` is the cross-mode
///   anchor (survives `NO_COLOR`); it also lines up vocabulary
///   with the Other header's `K ▲` summary count.
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
                .fg(theme.edge_conflict())
                .add_modifier(Modifier::BOLD);
            let prefix = span!(conflict_style; "{} ", marks::WARNING);
            ("    ", Some(prefix), conflict_style)
        }
        EdgeStateLabel::AltOf(_) | EdgeStateLabel::Resolves => {
            ("      ", None, Style::default().fg(theme.edge_alt_of()))
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
        .map_or(0, |p| text::width(p.content.as_ref()));
    // verb column + glyph + separating space
    let fixed = prefix_width + text::width(verb_text.as_str()) + 2;
    let meta_width = edge_meta.as_deref().map_or(0, text::width);
    let with_meta = width.saturating_sub(fixed + meta_width);
    let label_budget = if with_meta >= MIN_RELATED_LABEL_WIDTH {
        with_meta
    } else {
        width.saturating_sub(fixed).max(1)
    };
    let label = text::truncate_middle(&link.neighbor_label, label_budget);

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

/// `► Other (N · K ▲ · L —)` / `▼ Other (N · K ▲ · L —)` row that
/// gates the alternates / conflicts / unresolved zone (ADR 0074 §3).
pub(super) fn render_other_header_line(
    expanded: bool,
    other_count: usize,
    ambiguous_count: usize,
    unresolved_count: usize,
    highlight: bool,
    theme: &Theme,
) -> Line<'static> {
    let glyph = if expanded {
        marks::EXPANDED
    } else {
        marks::COLLAPSED
    };
    let mut suffix = format!("({other_count}");
    if ambiguous_count > 0 {
        suffix.push_str(&format!(" · {ambiguous_count} {}", marks::WARNING));
    }
    if unresolved_count > 0 {
        suffix.push_str(&format!(" · {unresolved_count} {}", marks::NO_VALUE));
    }
    suffix.push(')');
    let text = format!("  {glyph} Other  {suffix}");
    let mut style = Style::default()
        .fg(theme.inert)
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
    let mut style = Style::default().add_modifier(theme.muted);
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
/// row. A Related row leads with its neighbor, `edge`, and `evidence`
/// lines (ADR 0107), then the neighbor's own preview (ADR 0112 §3), or
/// its core fields when it has none. Other rows show the focused
/// node's preview through `draw_detail_preview`.
/// Draw the Preview for the explorer cursor's row and return the window
/// its body drew.
pub(super) fn draw_explorer_preview(
    app: &App,
    state: &crate::tui::app::ExplorerState,
    frame: &mut Frame<'_>,
    area: Rect,
) -> ScrollWindow {
    use crate::tui::explorer::RowPreview;
    let Some(row) = state.selected_row() else {
        // Rows that don't carry a neighbor (node fields land here)
        // show the focused node's own preview body.
        return draw_detail_preview(app, frame, area);
    };
    let Some(preview) = state.view.row_preview(&row) else {
        // No neighbor for this row — show the focused node's own
        // preview body (live mux capture, message preview, etc.).
        return draw_detail_preview(app, frame, area);
    };
    let theme = app.theme();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut neighbor_source = None;
    let mut neighbor_text = None;
    match preview {
        RowPreview::Link {
            neighbor_id,
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
            neighbor_source = crate::tui::preview::node_source(app, neighbor_id);
            if neighbor_source.is_none() {
                match sourceless_node_text(app, neighbor_id) {
                    Some(text) => neighbor_text = Some(text),
                    None => {
                        for field in fields {
                            lines.push(render_node_field_line(field, false, theme));
                        }
                    }
                }
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
                    span!(theme.muted; "{value}"),
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
                    theme.muted;
                    "{} · {} · {}",
                    provenance.snake_case(),
                    confidence.snake_case(),
                    link_state.snake_case()
                ),
            ]));
        }
    }
    let view = if let Some(source) = neighbor_source {
        lines.push(Line::raw(""));
        let header_rows = WrappedRows::new(&lines, usize::from(area.width)).len();
        let body_height = (area.height as usize).saturating_sub(header_rows).max(1);
        source_view(app, &source, area.width, body_height).below(Text::from(lines))
    } else {
        let mut text = Text::from(lines);
        if let Some(neighbor_text) = neighbor_text {
            text.lines.push(Line::raw(""));
            text.extend(neighbor_text);
        }
        PreviewView::text(text)
    };
    draw_preview_view(app, view, frame, area)
}

pub(super) fn empty_right_panel_text(app: &App) -> &'static str {
    if app.snapshot_handle().is_none() {
        "Loading…"
    } else {
        "Select a row to view its detail."
    }
}

/// The block shown above the node detail when a hit row is selected:
/// who said it, what kind of turn it is, when, and the matching text
/// with the match picked out.
fn selected_hit_lines(app: &App) -> Vec<Line<'static>> {
    let Some(selection) = app.selection() else {
        return Vec::new();
    };
    let Some(row) = app.tree().rows.iter().find(|row| &row.id == selection) else {
        return Vec::new();
    };
    let RowKind::Hit(hit) = &row.kind else {
        return Vec::new();
    };
    let theme = app.theme();
    let secondary = Style::default().fg(theme.inert);
    let header = Line::from(vec![
        Span::styled("match", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "  turn {} · {} · {} · {}",
                hit.turn_index + 1,
                hit.role,
                hit.kind,
                hit.age
            ),
            secondary,
        ),
    ]);
    let snippet = text::paint(
        &[Span::raw(hit.snippet.as_str())],
        Some(hit.highlight.clone()).filter(|range| range.end <= hit.snippet.len()),
        |base| base.patch(theme.match_style()),
    );
    vec![header, Line::from(snippet), Line::default()]
}
