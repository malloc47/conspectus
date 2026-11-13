//! Right panel: the graph explorer (ADR 0074) and its title.

use tuikit::breadcrumb::Breadcrumb;
use tuikit::empty_state::EmptyState;
use tuikit::master_detail::StackedMasterDetail;
use tuikit::property_grid::PropertyRow;

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
    buf: &mut Buffer,
    area: Rect,
) -> (Option<ScrollWindow>, Option<ScrollRegion>) {
    // The framework's pane frame puts the focus marker before the title
    // and tints the border while the pane has focus.
    let inner = PaneFrame::new(app.theme())
        .title(right_panel_title(app, PaneFrame::title_width(area.width)))
        .focused(app.focus() == Focus::Right)
        .render(area, buf);

    // The explorer's lines are laid out at the layout's width, a column
    // short of the pane, so the `Related` rule still fits on one row when
    // the scrollbar takes that column. Validated rows (resolver winners)
    // sit flush under the `Related` rule; alternates, conflicts, and
    // unresolved stubs collapse under a single `Other` header below them
    // (ADR 0074).
    let lines = app
        .explorer()
        .zip(app.graph_source())
        .map(|(explorer, source)| {
            explorer.lines(
                &source,
                StackedMasterDetail::layout_width(inner),
                app.focus() == Focus::Right,
                app.theme(),
            )
        });
    // No explorer means the selection names no node (or nothing has
    // loaded yet): show the placeholder instead of the explorer.
    let Some(mut list) = lines else {
        empty_right_panel(app).render(inner, buf, app.theme());
        return (None, None);
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
        buf,
        list,
        preview_divider_line(usize::from(inner.width), app.theme()),
        app.explorer_window(),
    );
    // A transcript preview leaves a column for its scrollbar.
    app.prepare_preview(stacked.detail.width.saturating_sub(1).max(1));
    let preview = draw_explorer_preview(app, buf, stacked.detail);
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
/// `width` is the cells the pane's top border leaves for the title after
/// the focus marker the pane frame puts before it
/// ([`PaneFrame::title_width`]); the frame adds the closing space when
/// the title leaves a cell for it.
pub(super) fn right_panel_title(app: &App, width: usize) -> Line<'static> {
    let label = right_panel_kind_label(app);
    let theme = app.theme();
    let mut spans = Vec::new();
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
    // and gives way hop by hop to fit. The frame's closing space, which
    // keeps the title off the corner, gives way before any hop: it is
    // drawn only when the chain leaves a cell for it.
    if let Some(explorer) = app.explorer()
        && !explorer.frames().is_empty()
    {
        let inert = Style::default().fg(theme.inert);
        let back = span!(inert; " {} ", marks::BACK);
        let depth = span!(inert; " · depth {}", explorer.frames().len());
        let room = width.saturating_sub(text::spans_width(&spans) + back.width() + depth.width());
        let hops = crate::tui::explorer::breadcrumb_hops(explorer.frames(), theme);
        if let Some(chain) = Breadcrumb::new(&hops, theme).line(room) {
            spans.push(back);
            spans.extend(chain.spans);
            spans.push(depth);
        }
    }
    Line::from(spans)
}

/// The kind of node the explorer is focused on, `None` while no
/// selection resolves to a node.
fn right_panel_node_kind(app: &App) -> Option<NodeKind> {
    app.explorer().map(|explorer| explorer.content().kind)
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

/// Render the Preview zone body for the currently-selected explorer
/// row. A Related row leads with its neighbor, `edge`, and `evidence`
/// lines (ADR 0107), then the neighbor's own preview (ADR 0112 §3), or
/// its core fields when it has none. Other rows show the focused
/// node's preview through `draw_detail_preview`.
/// Draw the Preview for the explorer cursor's row and return its scroll
/// region as the body drew it.
pub(super) fn draw_explorer_preview(app: &App, buf: &mut Buffer, area: Rect) -> ScrollRegion {
    use crate::tui::explorer::RowPreview;
    let (Some(explorer), Some(row)) = (app.explorer(), app.explorer_cursor_row()) else {
        // Rows that don't carry a neighbor (node fields land here)
        // show the focused node's own preview body.
        return draw_detail_preview(app, buf, area);
    };
    let Some(preview) = explorer.content().row_preview(&row) else {
        // No neighbor for this row — show the focused node's own
        // preview body (live mux capture, message preview, etc.).
        return draw_detail_preview(app, buf, area);
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
    draw_preview_view(app, view, buf, area)
}

/// What the right pane says when the selection names no node: that the
/// graph is still loading, or that a row needs selecting.
pub(super) fn empty_right_panel(app: &App) -> EmptyState {
    if app.snapshot_handle().is_none() {
        EmptyState::new("Loading…")
    } else {
        EmptyState::new("Select a row to view its detail.")
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
