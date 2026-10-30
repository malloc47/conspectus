//! Right panel: the graph explorer (ADR 0074) and its title.

use tuikit::breadcrumb::Breadcrumb;
use tuikit::frame::SectionRule;
use tuikit::master_detail::StackedMasterDetail;
use tuikit::property_grid::{GridLines, PropertyGrid, PropertyRow};

use crate::tui::explorer::grid::{self as explorer_grid, field_row};

use super::*;

/// Rows kept for the Preview zone so a full Related list cannot collapse
/// it to one or two lines. On a pane too short for it beside the
/// explorer zone's three rows and the divider, the Preview gets what
/// those leave.
const MIN_PREVIEW_HEIGHT: u16 = 6;

/// The right pane's stack: the explorer zone, the `Preview` rule, and
/// the Preview.
const LAYOUT: StackedMasterDetail = StackedMasterDetail::new().min_detail(MIN_PREVIEW_HEIGHT);

/// Draw the right pane and return the window its explorer zone drew and
/// the Preview's scroll region as drawn, `None` for a zone the pane did
/// not draw (or a Preview whose content has not loaded).
pub(super) fn draw_right_panel(
    app: &mut App,
    frame: &mut Frame<'_>,
    area: Rect,
) -> (Option<ScrollWindow>, Option<ScrollRegion>) {
    // The corners take a cell each from the top border.
    let block = Block::default()
        .borders(Borders::ALL)
        .title(right_panel_title(
            app,
            usize::from(area.width.saturating_sub(2)),
        ));
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

    // The explorer's lines are laid out at the layout's width, a column
    // short of the pane, so the `Related` rule still fits on one row when
    // the scrollbar takes that column.
    let mut list = {
        let state = app.explorer().expect("checked above");
        render_explorer_lines(
            state,
            StackedMasterDetail::layout_width(inner),
            app.theme(),
            app.edge_meta_visible(),
            app.focus() == Focus::Right,
        )
    };
    // A selected hit row says which turn it is and shows what matched.
    let hit_lines = selected_hit_lines(app);
    if !hit_lines.is_empty() {
        list.cursor_line = list.cursor_line.map(|index| index + hit_lines.len());
        list.lines.splice(0..0, hit_lines);
    }
    // The zone's window starts where the last frame left it.
    let stacked = LAYOUT.render(
        inner,
        frame.buffer_mut(),
        list,
        preview_divider_line(usize::from(inner.width), app.theme()),
        app.explorer_window(),
    );
    // A transcript preview leaves a column for its scrollbar.
    app.prepare_preview(stacked.detail.width.saturating_sub(1).max(1));
    let state = app.explorer().expect("checked above");
    let preview = draw_explorer_preview(app, state, frame, stacked.detail);
    // A Preview still waiting for its content draws a placeholder, not
    // the rows its offset counts, so it keeps the offset until the
    // content has loaded and a frame shows it.
    let loaded = crate::tui::preview::preview_source(app)
        .is_none_or(|source| app.previews().get(&source).is_some());
    (Some(stacked.window), loaded.then_some(preview))
}

/// Render the right pane title with the kind of node currently
/// being inspected (`session`, `mux`, `pr`, …) so the operator can
/// tell what they're looking at without re-reading the body.
/// Falls back to `detail` while no selection is resolved.
///
/// `width` is the cells the pane's top border leaves for the title.
pub(super) fn right_panel_title(app: &App, width: usize) -> Line<'static> {
    let label = right_panel_kind_label(app);
    let theme = app.theme();
    let mut spans = vec![Span::raw(" "), focus_marker_span(app, Focus::Right)];
    // ADR 0073 §3: the right-panel node header is `<glyph> <label>`
    // with the glyph in the node-kind color and the label bold. The
    // glyph is suppressed when there is no resolved selection
    // (fallback "detail" label) so a placeholder pane does not stamp
    // a misleading kind cue.
    if let Some(node_kind) = right_panel_node_kind(app) {
        let style = node_kind_style(node_kind, theme);
        spans.push(span!(Style::default().fg(style.color); "{} ", style.glyph));
    }
    spans.push(span!(Modifier::BOLD; "{label}"));
    // A drilled pane shows the chain that led here between ` ◄ ` and
    // ` · depth N`, so even a chain that gave way says how deep the
    // pane is. The chain takes the cells the rest of the title leaves
    // and gives way hop by hop to fit. The trailing pad, which keeps the
    // title off the corner, gives way before any hop: it is drawn only
    // when the chain leaves a cell for it.
    let mut pad = true;
    if let Some(state) = app.explorer()
        && !state.breadcrumb.is_empty()
    {
        let inert = Style::default().fg(theme.inert);
        let back = span!(inert; " {} ", marks::BACK);
        let depth = span!(inert; " · depth {}", state.breadcrumb.len());
        let room = width.saturating_sub(text::spans_width(&spans) + back.width() + depth.width());
        let hops = crate::tui::explorer::breadcrumb_hops(&state.breadcrumb, theme);
        if let Some(chain) = Breadcrumb::new(&hops, theme).line(room) {
            pad = chain.width() < room;
            spans.push(back);
            spans.extend(chain.spans);
            spans.push(depth);
        }
    }
    if pad {
        spans.push(Span::raw(" "));
    }
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

/// Render the related-entities layout (ADR 0074) as a property grid: the
/// focused node's fields, then, when it has neighbors, the `Related`
/// rule and one link row per neighbor. Validated rows (resolver winners)
/// sit flush under the rule; alternates, conflicts, and unresolved stubs
/// collapse under a single `Other` header below the fold. Each link row
/// reads as `<verb column> <kind-glyph> <neighbor_label>`; direction
/// flows from the verb, not from section placement.
pub(super) fn render_explorer_lines(
    state: &crate::tui::app::ExplorerState,
    width: usize,
    theme: &Theme,
    show_edge_meta: bool,
    focused: bool,
) -> GridLines {
    use crate::tui::explorer::ExplorerRow;
    let view = &state.view;
    let rows = state.rows();
    let fields = rows
        .iter()
        .take_while(|row| matches!(row, ExplorerRow::NodeField { .. }))
        .count();
    let push = |grid: &mut PropertyGrid, rows: &[ExplorerRow]| {
        for row in rows {
            grid.push(explorer_grid::grid_row(view, row, theme, show_edge_meta));
        }
    };
    let mut grid = explorer_grid::grid(theme);
    push(&mut grid, &rows[..fields]);
    if !view.relationships.groups.is_empty() {
        grid.push_line(related_rule(view, width, theme));
        push(&mut grid, &rows[fields..]);
    }
    grid.lines(width, Some(state.cursor), focused, theme)
}

/// The `Related` rule over the link rows, its summary counting the
/// validated rows and what the Other zone holds (ADR 0074 §5). The
/// summary comes first, so the chip stays in the `Preview` chip's column
/// however long the counts grow.
fn related_rule(
    view: &crate::tui::explorer::NodeView,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
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
    SectionRule::new("Related")
        .summary(summary)
        .summary_first()
        .line(width, theme)
}

/// Render the Preview zone body for the currently-selected explorer
/// row. A Related row leads with its neighbor, `edge`, and `evidence`
/// lines (ADR 0107), then the neighbor's own preview (ADR 0112 §3), or
/// its core fields when it has none. Other rows show the focused
/// node's preview through `draw_detail_preview`.
/// Draw the Preview for the explorer cursor's row and return its scroll
/// region as the body drew it.
pub(super) fn draw_explorer_preview(
    app: &App,
    state: &crate::tui::app::ExplorerState,
    frame: &mut Frame<'_>,
    area: Rect,
) -> ScrollRegion {
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
    let receding = Style::default().add_modifier(theme.muted);
    // Under a bold heading that names it, the neighbor's fields and its
    // edge read as the explorer's own fields do: a label column, then
    // the value.
    let heading = |what: &str, name: &str| {
        Line::from(span!(Modifier::BOLD; "  {}{name}", text::pad(what, 12)))
    };
    let mut grid = explorer_grid::grid(theme);
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
            grid.push_line(heading("neighbor", neighbor_label));
            neighbor_source = crate::tui::preview::node_source(app, neighbor_id);
            if neighbor_source.is_none() {
                match sourceless_node_text(app, neighbor_id) {
                    Some(text) => neighbor_text = Some(text),
                    None => {
                        for field in fields {
                            grid.push(field_row(field, theme));
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
            grid.push(
                PropertyRow::new("edge", edge_value).style(Style::default().fg(theme.warning)),
            );
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
                grid.push(PropertyRow::new(label, value).style(receding));
            }
        }
        RowPreview::Unresolved {
            node_type,
            evidence,
            provenance,
            confidence,
            state: link_state,
        } => {
            grid.push_line(heading("unresolved", node_type));
            for (label, value) in [
                ("harness_key", evidence.harness_key.as_deref()),
                ("native_id", evidence.native_id.as_deref()),
                ("state_scope", evidence.state_scope.as_deref()),
                ("path", evidence.path.as_deref()),
            ] {
                if let Some(value) = value {
                    grid.push(PropertyRow::new(label, value));
                }
            }
            grid.push(
                PropertyRow::new(
                    "edge",
                    format!(
                        "{} · {} · {}",
                        provenance.snake_case(),
                        confidence.snake_case(),
                        link_state.snake_case()
                    ),
                )
                .style(receding),
            );
        }
    }
    let mut lines = grid
        .lines(usize::from(area.width), None, false, theme)
        .lines;
    let view = if let Some(source) = neighbor_source {
        lines.push(Line::raw(""));
        let body_height = rows_under(&lines, area.width, area.height as usize);
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
