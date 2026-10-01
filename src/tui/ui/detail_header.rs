//! Detail header: sectioned field rows above the preview (ADR 0033).

use super::*;

/// Natural height of the right-panel header (one row per field plus
/// section dividers), clamped so the preview zone keeps room for at
/// least the separator and two body rows.
///
/// The Paragraph widget wraps long field values onto extra terminal
/// rows; the budget has to count those wrapped rows or sections
/// below the wrap get clipped. Pre-`H-RIGHT-WRAP` this function
/// assumed one terminal row per field, which produced a visible
/// "Session section vanishes" bug on muxes whose `native_id` was
/// long enough to wrap the `name` row.
pub(super) fn header_zone_height(
    detail: &NodeDetail,
    expand_linked: bool,
    extra_mux_rows: &[HeaderField],
    panel_height: u16,
    panel_width: u16,
) -> u16 {
    let natural = count_detail_lines(
        detail.kind,
        &detail.header_fields,
        extra_mux_rows,
        expand_linked,
        panel_width,
        0,
    );
    let natural = u16::try_from(natural).unwrap_or(u16::MAX);
    let max = panel_height.saturating_sub(3);
    natural.min(max).max(3)
}

/// Mirrors [`emit_detail_section_lines`] so the layout budget tracks
/// exactly what the renderer will emit — section dividers, wrap-aware
/// field rows, runtime extras, and (when expanded) the recursive
/// sub-detail with its own dividers and indent. Keeping the two
/// functions structurally identical is how the wrap-aware fix avoids
/// re-introducing the "Session row clipped" class of bug.
pub(super) fn count_detail_lines(
    kind: NodeKind,
    fields: &[HeaderField],
    extra_mux_rows: &[HeaderField],
    expand_linked: bool,
    panel_width: u16,
    indent: usize,
) -> usize {
    let sections = crate::tui::detail::group_fields_into_sections(kind, fields);
    let mut total = 0usize;
    let mut first = true;
    for section in &sections {
        if !first {
            total += 1;
        }
        first = false;
        for field in &section.fields {
            total += header_field_line_count(field, panel_width, indent);
            if expand_linked
                && let Some(sub_kind) = field.expanded_kind
                && !field.expanded_fields.is_empty()
            {
                total += count_detail_lines(
                    sub_kind,
                    &field.expanded_fields,
                    &[],
                    false,
                    panel_width,
                    indent + 2,
                );
            }
        }
        if section.kind == SectionKind::Mux {
            for field in extra_mux_rows {
                total += header_field_line_count(field, panel_width, indent);
            }
        }
    }
    total
}

/// Approximate the number of terminal rows a header field rendered
/// through [`render_header_field`] will occupy under `Paragraph::wrap`.
/// The renderer produces a 10-cell label column; the inline-expansion
/// path shifts that whole block right by `indent` cells so the
/// effective row width is `panel_width - indent`.
///
/// `Paragraph::wrap` word-wraps at whitespace, and the label's
/// right-padding is whitespace. So when the value can't share the
/// label row, the wrap pushes the value to its own line, costing one
/// extra terminal row beyond the simple `ceil((label + value) /
/// effective_width)` formula. The original wrap-aware fix used the
/// simple formula and still under-counted by one in the line-wrap
/// case, which clipped the Session section's first row by one row
/// off the bottom of the header zone.
pub(super) fn header_field_line_count(
    field: &HeaderField,
    panel_width: u16,
    indent: usize,
) -> usize {
    const LABEL_WIDTH: usize = 10;
    let effective_width = (panel_width as usize).saturating_sub(indent);
    if effective_width == 0 {
        return 1;
    }
    let value_width = UnicodeWidthStr::width(field.value.as_str());
    let annotation_width = field
        .annotation
        .map_or(0, |annotation| 1 + UnicodeWidthStr::width(annotation));
    let total = LABEL_WIDTH + value_width + annotation_width;
    if total <= effective_width {
        return 1;
    }
    // Value doesn't share the label row: budget one line for the
    // label plus however many wrap lines the value + annotation need
    // on their own.
    let value_lines = (value_width + annotation_width)
        .div_ceil(effective_width)
        .max(1);
    1 + value_lines
}

pub(super) fn draw_detail_header(
    detail: &NodeDetail,
    expand_linked: bool,
    extra_mux_rows: &[HeaderField],
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
) {
    let lines = emit_detail_section_lines(
        detail.kind,
        &detail.header_fields,
        extra_mux_rows,
        expand_linked,
        area.width as usize,
        0,
        theme,
    );
    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

/// Render a flat header-field list as the section-divided line
/// stream the right-panel header expects. Shared between the
/// top-level [`draw_detail_header`] entry point and the inline
/// expansion path (`expand_linked`) so a linked entity's expanded
/// view renders byte-for-byte like its standalone detail — same
/// label widths, same colorization, same section dividers — only
/// shifted right by `indent` cells per `H-RIGHT-EXPAND-UNIFY`.
pub(super) fn emit_detail_section_lines(
    kind: NodeKind,
    fields: &[HeaderField],
    extra_mux_rows: &[HeaderField],
    expand_linked: bool,
    panel_width: usize,
    indent: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let sections = crate::tui::detail::group_fields_into_sections(kind, fields);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut first = true;
    let divider_width = panel_width.saturating_sub(indent);
    for section in &sections {
        if !first {
            lines.push(section_divider_line(section.kind, divider_width, theme));
        }
        first = false;
        for field in &section.fields {
            lines.push(render_header_field(field, section.kind, theme));
            if expand_linked
                && let Some(sub_kind) = field.expanded_kind
                && !field.expanded_fields.is_empty()
            {
                let sub_lines = emit_detail_section_lines(
                    sub_kind,
                    &field.expanded_fields,
                    &[],
                    false,
                    panel_width,
                    indent + 2,
                    theme,
                );
                lines.extend(sub_lines);
            }
        }
        // The Mux section absorbs runtime-only rows derived from
        // the per-mux preview cache (capture freshness). They sit
        // below the static fields so the source-of-truth `mux` row
        // stays first.
        if section.kind == SectionKind::Mux {
            for field in extra_mux_rows {
                lines.push(render_header_field(field, section.kind, theme));
            }
        }
    }
    if indent > 0 {
        let indent_span = Span::raw(" ".repeat(indent));
        for line in &mut lines {
            line.spans.insert(0, indent_span.clone());
        }
    }
    lines
}

/// Build runtime-only rows for the Mux section: capture freshness
/// from the per-mux preview cache. Returned as `HeaderField`s so
/// they render through the same colorization path as the static
/// fields. Empty when no attach target resolves or no capture has
/// landed yet.
pub(super) fn mux_runtime_rows(app: &App) -> Vec<HeaderField> {
    let Ok(target) = resolve_attach_target(app) else {
        return Vec::new();
    };
    let Some(entry) = app.mux_preview(&target.mux) else {
        return Vec::new();
    };
    let Some(captured) = entry.captured_at else {
        return Vec::new();
    };
    let elapsed = format_elapsed(captured.elapsed().as_secs());
    vec![HeaderField {
        label: "captured",
        value: format!("{elapsed} ago"),
        placeholder: false,
        annotation: None,
        target: None,
        expanded_kind: None,
        expanded_fields: Vec::new(),
    }]
}

/// Short-form elapsed-duration formatter used by the runtime Mux
/// rows. Mirrors the recency formatter in `rows/mod.rs` but takes
/// a `Duration::as_secs` payload rather than an epoch delta so the
/// preview cache's `Instant::elapsed` value plugs in directly.
pub(super) fn format_elapsed(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    format!("{}d", hours / 24)
}

/// Right-anchored labeled rule used between detail sections. Wraps
/// [`chip_divider_line`] with the section's static label.
pub(super) fn section_divider_line(
    kind: SectionKind,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    chip_divider_line(kind.label(), None, width, theme, ChipAnchor::Left)
}

/// Build a right-anchored divider with a filled-chip label and an
/// optional dim suffix between the chip and the trailing rule.
/// Used by the section dividers in the detail header and by the
/// preview divider that separates the header from the preview
/// body. The suffix surface lets the preview divider keep its
/// captured-time / pane-target context inline without breaking the
/// uniform chip treatment.
/// Anchor side for a zone-header chip. `Left` keeps the chip near the
/// start of the line with any aggregate summary trailing it (used by
/// Node and Preview, which have no summary). `Right` flips the order
/// so the aggregate renders left of the chip and the chip anchors
/// flush right — keeps the bold zone label easy to scan
/// vertically when Upstream / Downstream summaries grow.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum ChipAnchor {
    Left,
    Right,
}

pub(super) fn chip_divider_line(
    label: &str,
    suffix: Option<&str>,
    width: usize,
    theme: &Theme,
    anchor: ChipAnchor,
) -> Line<'static> {
    let chip_text = format!(" {label} ");
    let chip_width = chip_text.chars().count();
    let suffix_text = suffix.map(|s| format!(" {s} ")).unwrap_or_default();
    let suffix_width = suffix_text.chars().count();
    let trailing_rule = 2;
    let leading_rule = width.saturating_sub(chip_width + suffix_width + trailing_rule);
    let chip_span = span!(
        Style::default().fg(theme.panel_focus_accent).add_modifier(theme.badge);
        "{chip_text}"
    );
    let suffix_span = (suffix_width > 0)
        .then(|| span!(Style::default().fg(theme.secondary_text); "{suffix_text}"));
    let mut spans = Vec::with_capacity(4);
    if leading_rule > 0 {
        spans.push(span!(theme.divider; "{}", "─".repeat(leading_rule)));
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
    spans.push(span!(theme.divider; "{}", "─".repeat(trailing_rule)));
    Line::from(spans)
}

pub(super) fn render_header_field(
    field: &HeaderField,
    section: SectionKind,
    theme: &Theme,
) -> Line<'static> {
    let label = span!(Modifier::BOLD; "{:<10}", field.label);
    let value_style = field_value_style(section, field, theme);
    let mut spans = vec![label, span!(value_style; "{}", field.value.clone())];
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

/// Per-(section, field-label) colorization per ADR 0033's mapping.
/// Placeholders always inherit `theme.placeholder` regardless of
/// section so the dim treatment stays consistent across the pane.
pub(super) fn field_value_style(section: SectionKind, field: &HeaderField, theme: &Theme) -> Style {
    if field.placeholder {
        return Style::default().add_modifier(theme.placeholder);
    }
    match (section, field.label) {
        (SectionKind::Session, "id") => Style::default().fg(theme.link_id),
        (SectionKind::Session, "cwd") => Style::default().fg(theme.cwd_mark),
        (SectionKind::Session, "title") => Style::default().add_modifier(Modifier::BOLD),
        (SectionKind::Mux, "name") => Style::default().fg(theme.link_id),
        (SectionKind::Mux, "mux") => Style::default().fg(theme.link_id),
        (SectionKind::Lineage, "lineage") => Style::default().fg(theme.link_id),
        (SectionKind::Pr, "pr") => pr_value_style(&field.value, theme),
        _ => Style::default(),
    }
}

/// Coarse PR-state coloring: scan the value for one of the known
/// state tokens and route through the ADR 0022 palette. Falls back
/// to default-fg when no token matches so unfamiliar provider
/// states still render legibly.
pub(super) fn pr_value_style(value: &str, theme: &Theme) -> Style {
    if value.contains("(merged)") {
        Style::default().fg(theme.pr_merged)
    } else if value.contains("(closed)") {
        Style::default().fg(theme.pr_closed)
    } else if value.contains("draft") {
        Style::default().fg(theme.pr_draft)
    } else if value.contains("(open)") {
        Style::default().fg(theme.pr_open)
    } else {
        Style::default()
    }
}
