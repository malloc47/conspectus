//! Ratatui draw.
//!
//! v1 render per `docs/tui-sessions-mockup.md`: a single status-bar
//! footer, a header line, and a two-panel body with the row tree on
//! the left and the selected node's detail + preview on the right.
//!
//! Locked decisions reflected here:
//!
//! - `~`-shortened paths are produced upstream in the row tree and
//!   detail view-models; the renderer just lays them out.
//! - Mux glyphs `◉` / `◐` / `◯` with color carrying the primary
//!   signal (green attached / yellow ambiguous / dim un-muxed).
//! - Header reports `updated Ns ago · N agents · M mux`; the
//!   freshness slot uses [`format_recency`] over
//!   `App::loaded_at_epoch`.
//! - Sessions row inline preview lights up for the selected row and
//!   for the N most-recent visible session rows
//!   (`[tui].inline_preview_rows`, default 3).
//! - Body switches from side-by-side to a vertical stack when the
//!   terminal is narrower than ~100 columns (T8-004).
//! - Muxed-session right-panel preview shows the
//!   `--no-live-preview` banner when live extras are suppressed,
//!   while inline tree previews remain (locked decision).
//! - Empty/loading frames render minimal copy when no row tree is
//!   loaded yet; the rest of the error/empty matrix lands with
//!   `T8-003`.

use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::model::{GraphNode, GraphSnapshot, MuxSessionId, NodeId};
use crate::tui::View;
use crate::tui::actions::{attach_disabled_reason, resolve_attach_target, target_label};
use crate::tui::app::{App, Focus};
use crate::tui::detail::{HeaderField, NodeDetail};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, MuxCandidateRow, MuxIndicator, RowId, RowKind, format_recency,
};

const INLINE_PREVIEW_DEFAULT: usize = 3;
const SELECTED_BG: Color = Color::Indexed(238);
const SELECTED_INACTIVE_BG: Color = Color::Indexed(236);
const PREVIEW_HEADER_FG: Color = Color::Indexed(244);
/// Terminal width threshold below which the body switches from a
/// side-by-side split to a vertical stack (left-on-top per the
/// phase-08 layout note).
const NARROW_LAYOUT_THRESHOLD: u16 = 100;

/// Render one frame. Pure with respect to `app`; the runtime calls
/// this on every loop iteration.
pub fn draw(app: &App, frame: &mut Frame<'_>) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(app, frame, layout[0]);
    draw_body(app, frame, layout[1]);
    draw_status_bar(app, frame, layout[2]);
}

// -----------------------------------------------------------------------------
// Header / status bar
// -----------------------------------------------------------------------------

fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let view_label = view_label(app.config().default_view);
    let (agents, mux) = snapshot_counts(app.snapshot().map(|s| s.as_ref()));
    let freshness = header_freshness(app);
    let title = format!("Conspectus · {view_label} ─ {freshness}{agents} agents · {mux} mux",);
    let widget = Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(widget, area);
}

/// Build the `updated Ns ago · ` slice of the header, or an empty
/// string when no snapshot has loaded yet. The trailing separator
/// is part of the returned slice so callers don't have to special-
/// case the empty form.
fn header_freshness(app: &App) -> String {
    let Some(loaded_at) = app.loaded_at_epoch() else {
        return String::new();
    };
    let now = current_unix_epoch_for_render();
    match format_recency(Some(now), Some(loaded_at)) {
        Some(label) => format!("updated {label} ago · "),
        None => String::new(),
    }
}

#[cfg(not(test))]
fn current_unix_epoch_for_render() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}

/// Test override: a fixed clock so snapshot tests stay
/// deterministic without monkey-patching the system clock.
#[cfg(test)]
fn current_unix_epoch_for_render() -> i64 {
    test_clock::now()
}

#[cfg(test)]
pub(crate) mod test_clock {
    use std::cell::Cell;
    thread_local! {
        static NOW: Cell<i64> = const { Cell::new(0) };
    }

    pub fn set(value: i64) {
        NOW.with(|cell| cell.set(value));
    }

    pub fn now() -> i64 {
        NOW.with(|cell| cell.get())
    }
}

fn draw_status_bar(app: &App, frame: &mut Frame<'_>, area: Rect) {
    if let Some(message) = app.status_message() {
        let widget = Paragraph::new(message.to_string()).style(Style::default().fg(Color::Yellow));
        frame.render_widget(widget, area);
        return;
    }
    let hints = contextual_status_text(app);
    let scope = match app.focus() {
        Focus::Left => "[left]",
        Focus::Right => "[right]",
    };
    let widget = Paragraph::new(format!("{scope} {hints}"))
        .style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(widget, area);
}

fn view_label(view: View) -> &'static str {
    match view {
        View::Sessions => "sessions",
        View::Mux => "mux",
        View::Union => "union",
        View::Prs => "prs",
        View::Forks => "forks",
    }
}

fn snapshot_counts(snapshot: Option<&GraphSnapshot>) -> (usize, usize) {
    let Some(snapshot) = snapshot else {
        return (0, 0);
    };
    let mut agents = 0;
    let mut mux = 0;
    for node in &snapshot.nodes {
        match node {
            GraphNode::AgentSession(_) => agents += 1,
            GraphNode::MuxSession(_) => mux += 1,
            _ => {}
        }
    }
    (agents, mux)
}

// -----------------------------------------------------------------------------
// Body: left tree + right detail
// -----------------------------------------------------------------------------

fn draw_body(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let direction = if area.width < NARROW_LAYOUT_THRESHOLD {
        Direction::Vertical
    } else {
        Direction::Horizontal
    };
    let split = Layout::default()
        .direction(direction)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    draw_left_panel(app, frame, split[0]);
    draw_right_panel(app, frame, split[1]);
}

fn draw_left_panel(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(left_panel_title(app))
        .style(panel_focus_style(app, Focus::Left));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_rows();
    if visible.is_empty() {
        let placeholder = empty_left_panel_text(app);
        let widget = Paragraph::new(placeholder)
            .wrap(Wrap { trim: false })
            .style(Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(widget, inner);
        return;
    }

    let inline_preview_ids = inline_preview_session_ids(app, INLINE_PREVIEW_DEFAULT);
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());
    // The selected row may render as one (primary) or two (primary
    // + inline preview) lines. Track the line index of the primary
    // line so we can scroll to keep it visible.
    let mut selected_primary_line: Option<usize> = None;
    for row in &visible {
        let is_selected = app.selection() == Some(&row.id);
        let primary = render_left_row(row, app, is_selected);
        if is_selected {
            selected_primary_line = Some(lines.len());
        }
        lines.push(primary);

        // Inline preview line below the row when applicable.
        if let RowKind::AgentSession(session) = &row.kind
            && session.preview.is_some()
            && (is_selected || inline_preview_ids.contains(&row.id))
        {
            let preview_text = session.preview.clone().unwrap_or_default();
            let indent = row_indent(row.depth + 1);
            lines.push(Line::from(vec![
                Span::raw(indent),
                Span::styled(
                    truncate_to_width(&preview_text, inner.width.saturating_sub(4) as usize),
                    Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
                ),
            ]));
        }
    }

    // Pick the line range the selected row needs (primary + the
    // following preview line if there is one) and ask the App to
    // adjust the per-frame scroll offset so it stays visible.
    let scroll = if let Some(line_idx) = selected_primary_line {
        // If the inline preview line follows, target the preview
        // line — that ensures the renderer scrolls enough to show
        // both the row and its inline preview together.
        let target = if line_idx + 1 < lines.len()
            && app
                .selection()
                .and_then(|sel| visible.iter().find(|r| &r.id == sel))
                .is_some_and(|r| matches!(&r.kind, RowKind::AgentSession(s) if s.preview.is_some()))
        {
            line_idx + 1
        } else {
            line_idx
        };
        app.adjust_left_scroll(target, inner.height)
    } else {
        0
    };

    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(widget, inner);
}

fn left_panel_title(app: &App) -> Line<'static> {
    let view_label = view_label(app.config().default_view);
    Line::from(vec![
        Span::raw(" "),
        Span::styled(view_label, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" "),
    ])
}

fn empty_left_panel_text(app: &App) -> &'static str {
    if app.snapshot().is_none() {
        "Loading discovery…"
    } else {
        "No sessions discovered.\nPress `r` to refresh or `q` to quit."
    }
}

/// Build the rendered line for a single visible row.
fn render_left_row(row: &crate::tui::rows::Row, app: &App, is_selected: bool) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(row_indent(row.depth)));
    spans.push(Span::raw(disclosure_glyph(row, app)));

    match &row.kind {
        RowKind::Group(group) => {
            spans.push(Span::styled(
                compact_path_label(&group.display_path),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            let secondary = compact_path_secondary(&group.display_path);
            if !secondary.is_empty() {
                spans.push(Span::styled(
                    format!("  {secondary}"),
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            if group.is_launch_context {
                spans.push(Span::styled(
                    "  (cwd)".to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                ));
            }
        }
        RowKind::AgentSession(session) => spans.extend(render_session_spans(session)),
        RowKind::AgentSessionMuxCandidate(candidate) => {
            spans.extend(render_candidate_spans(candidate))
        }
    }

    let mut line = Line::from(spans);
    if is_selected {
        let bg = if app.focus() == Focus::Left {
            SELECTED_BG
        } else {
            SELECTED_INACTIVE_BG
        };
        line = line.style(Style::default().bg(bg).add_modifier(Modifier::BOLD));
    }
    line
}

fn render_session_spans(session: &AgentSessionRow) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    spans.push(Span::raw(format!("{}  ", session.short_id)));
    spans.push(Span::styled(
        format!("{:8}", session.harness_label),
        Style::default().fg(harness_color(&session.harness_label)),
    ));
    spans.push(Span::raw("  "));
    let recency = session.recency.clone().unwrap_or_else(|| "—".to_string());
    spans.push(Span::styled(
        format!("{recency:>4}"),
        Style::default().add_modifier(Modifier::DIM),
    ));
    spans.push(Span::raw("  "));
    spans.push(mux_indicator_span(session.mux_state));
    spans
}

fn render_candidate_spans(candidate: &MuxCandidateRow) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let glyph = if candidate.is_preferred {
        Span::styled("◉ ", Style::default().fg(Color::Green))
    } else {
        Span::styled("◯ ", Style::default().add_modifier(Modifier::DIM))
    };
    spans.push(glyph);
    spans.push(Span::raw(compact_mux_label(&candidate.mux_label)));
    if candidate.is_preferred {
        spans.push(Span::styled(
            "  (preferred)".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    spans
}

fn mux_indicator_span(state: MuxIndicator) -> Span<'static> {
    match state {
        MuxIndicator::Attached => Span::styled("◉", Style::default().fg(Color::Green)),
        MuxIndicator::Ambiguous { .. } => Span::styled("◐", Style::default().fg(Color::Yellow)),
        MuxIndicator::Unmuxed => Span::styled("◯", Style::default().add_modifier(Modifier::DIM)),
    }
}

fn harness_color(label: &str) -> Color {
    // Per H-TBL-014: each harness gets a distinct hue. We pick from
    // a small palette here; downstream story can converge with the
    // table renderer's color map (T8-002).
    match label {
        "claude" => Color::Magenta,
        "codex" => Color::Cyan,
        "opencode" => Color::Green,
        _ => Color::White,
    }
}

fn disclosure_glyph(row: &crate::tui::rows::Row, app: &App) -> &'static str {
    if !row.expandable {
        return "  ";
    }
    if app.is_expanded(&row.id) {
        "▼ "
    } else {
        "▶ "
    }
}

fn row_indent(depth: u8) -> String {
    "  ".repeat(depth as usize)
}

fn truncate_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(width);
    let mut budget = width;
    for ch in text.chars() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_w > budget {
            out.push('…');
            return out;
        }
        out.push(ch);
        budget -= ch_w;
    }
    out
}

fn compact_path_label(path: &str) -> String {
    if path == "Ungrouped" {
        return path.to_string();
    }
    let trimmed = path.trim_end_matches('/');
    if trimmed == "~" {
        return "~".to_string();
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

fn compact_path_secondary(path: &str) -> String {
    if path == "Ungrouped" {
        return String::new();
    }
    let label = compact_path_label(path);
    if label == path {
        String::new()
    } else {
        path.to_string()
    }
}

fn compact_mux_label(label: &str) -> String {
    let Some((backend, native)) = label.split_once(':') else {
        return label.to_string();
    };
    let short_native = if native.chars().count() > 36 {
        let head: String = native.chars().take(28).collect();
        let tail: String = native
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("{head}…{tail}")
    } else {
        native.to_string()
    };
    format!("{backend}:{short_native}")
}

/// Compute the row ids that should render an inline preview line
/// under their row. v1 rule: the selected row + the top `n` visible
/// agent session rows by recency. Today recency is `None` for every
/// session, so this collapses to "the first n visible session rows".
fn inline_preview_session_ids(app: &App, n: usize) -> std::collections::BTreeSet<RowId> {
    let mut sessions: Vec<&crate::tui::rows::Row> = app
        .visible_rows()
        .into_iter()
        .filter(|r| matches!(r.kind, RowKind::AgentSession(_)))
        .collect();
    sessions.sort_by(|a, b| {
        let recency = |row: &&crate::tui::rows::Row| match &row.kind {
            RowKind::AgentSession(s) => s.activity_epoch.unwrap_or(i64::MIN),
            _ => i64::MIN,
        };
        recency(b).cmp(&recency(a))
    });
    sessions.into_iter().take(n).map(|r| r.id.clone()).collect()
}

// -----------------------------------------------------------------------------
// Right panel
// -----------------------------------------------------------------------------

fn draw_right_panel(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" detail ")
        .style(panel_focus_style(app, Focus::Right));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(detail) = app.detail() else {
        let widget = Paragraph::new(empty_right_panel_text(app))
            .style(Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(widget, inner);
        return;
    };

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            // Give the header exactly the height its fields need
            // (title + blank + one row per HeaderField), clamped
            // so the preview zone keeps a minimum of two rows.
            Constraint::Length(header_zone_height(detail, inner.height)),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    draw_detail_header(detail, frame, split[0]);
    frame.render_widget(
        Paragraph::new(preview_separator(app)).style(
            Style::default()
                .fg(PREVIEW_HEADER_FG)
                .add_modifier(Modifier::DIM),
        ),
        split[1],
    );
    draw_detail_preview(app, detail, frame, split[2]);
}

fn empty_right_panel_text(app: &App) -> &'static str {
    if app.snapshot().is_none() {
        "Loading…"
    } else {
        "Select a row to view its detail."
    }
}

/// Natural height of the right-panel header (title + blank + one
/// row per field), clamped so the preview zone keeps room for at
/// least the separator and two body rows.
fn header_zone_height(detail: &NodeDetail, panel_height: u16) -> u16 {
    let natural = (detail.header_fields.len() + 2) as u16;
    let max = panel_height.saturating_sub(3);
    natural.min(max).max(3)
}

fn draw_detail_header(detail: &NodeDetail, frame: &mut Frame<'_>, area: Rect) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        detail.title_line.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));
    for field in &detail.header_fields {
        lines.push(render_header_field(field));
    }
    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_header_field(field: &HeaderField) -> Line<'static> {
    let label = Span::styled(
        format!("{:<10}", field.label),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let value_style = if field.placeholder {
        Style::default().add_modifier(Modifier::DIM)
    } else {
        Style::default()
    };
    let mut spans = vec![label, Span::styled(field.value.clone(), value_style)];
    if let Some(annotation) = field.annotation {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            annotation.to_string(),
            Style::default().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
}

fn draw_detail_preview(app: &App, _detail: &NodeDetail, frame: &mut Frame<'_>, area: Rect) {
    let preview = preview_text_for_selection(app, area.height as usize);
    let widget = Paragraph::new(preview)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll(), 0));
    frame.render_widget(widget, area);
}

/// Source the preview body from whatever the selection points at.
/// Mirrors the locked decision in `docs/tui-sessions-mockup.md`:
///
/// - Un-muxed agent session: render the graph-resident
///   `last_message_preview`. `--no-live-preview` does **not**
///   suppress this — only live extras (pane capture + transcript-
///   tail) are gated.
/// - Muxed agent session: the renderer would normally show a tmux
///   pane capture; until `P8-009` wires that in, we show an
///   "incoming" placeholder. With `--no-live-preview`, the
///   placeholder switches to the privacy banner instead.
/// - Other rows: pane capture or fork-detail enrichment wires in
///   per `P8-009` / `P8-012b`. Gated by `--no-live-preview`.
fn preview_text_for_selection(app: &App, height: usize) -> Text<'static> {
    let Some(selection) = app.selection() else {
        return Text::raw("");
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return Text::raw("");
    };
    let live_preview = app.config().live_preview_enabled;
    match &row.kind {
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => {
                mux_preview_text(app, live_preview, height)
            }
            MuxIndicator::Unmuxed => Text::raw(
                session
                    .preview
                    .clone()
                    .unwrap_or_else(|| "no preview available".to_string()),
            ),
        },
        RowKind::AgentSessionMuxCandidate(_) => mux_preview_text(app, live_preview, height),
        _ => match selection {
            RowId::Group(NodeId::MuxSession(_)) => mux_preview_text(app, live_preview, height),
            _ => {
                if live_preview {
                    Text::raw("no preview for this row")
                } else {
                    Text::raw("preview disabled (--no-live-preview)")
                }
            }
        },
    }
}

/// Compose the preview body for a row whose preview source is a
/// tmux pane capture. Pulls from the per-mux cache; if no cache
/// entry exists yet (the runtime hasn't refreshed for this
/// selection), shows a "loading" placeholder. With
/// `--no-live-preview`, swaps in the privacy banner instead.
fn mux_preview_text(app: &App, live_preview: bool, height: usize) -> Text<'static> {
    if !live_preview {
        return Text::raw("preview disabled (--no-live-preview)");
    }
    let Some(target) = resolve_attach_target(app).ok().map(|t| t.mux) else {
        return Text::raw("no mux target for this row");
    };
    format_preview_for_mux(app, &target, height, app.config().color)
}

fn format_preview_for_mux(
    app: &App,
    mux: &MuxSessionId,
    height: usize,
    color: bool,
) -> Text<'static> {
    match app.mux_preview(mux) {
        Some(entry) => match &entry.content {
            PreviewContent::Text(text) if text.is_empty() => Text::raw("(empty pane)"),
            PreviewContent::Text(text) => {
                let cropped = crop_bottom_lines(text, height.max(1));
                render_captured_pane(&cropped, color)
            }
            PreviewContent::NoTarget => Text::raw("tmux target not found — try `r` to refresh"),
            PreviewContent::Unavailable(reason) => Text::raw(format!("tmux unavailable: {reason}")),
            PreviewContent::Failed(message) => Text::raw(message.clone()),
            PreviewContent::Unsupported => {
                Text::raw("preview unavailable (runner does not implement capture)")
            }
        },
        None => Text::raw("loading mux preview…"),
    }
}

/// Translate a raw `tmux capture-pane -e` payload into styled
/// `Text` for the preview pane. ADR 0025: `ansi-to-tui` does the
/// CSI/SGR parsing; we keep ownership of the colour-disabled
/// path and the malformed-input fallback.
fn render_captured_pane(text: &str, color: bool) -> Text<'static> {
    if !color {
        // `Text::to_string` flattens spans back to plain bytes,
        // which is the natural way to strip styling regardless of
        // how the upstream parser would group the escape bytes.
        // On a parse error there's nothing to strip — return as is.
        return match text.into_text() {
            Ok(parsed) => Text::raw(parsed.to_string()),
            Err(_) => Text::raw(text.to_string()),
        };
    }
    text.into_text()
        .unwrap_or_else(|_| Text::raw(text.to_string()))
}

fn panel_focus_style(app: &App, focus: Focus) -> Style {
    if app.focus() == focus {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

fn contextual_status_text(app: &App) -> String {
    let focus_hint = match app.focus() {
        Focus::Left => "j/k move · Enter expand",
        // When the right pane has focus, j/k are remapped to
        // preview scroll (T8-011 behavioral) — surface that so
        // operators know `Tab` changed what those keys do.
        Focus::Right => "j/k scroll preview",
    };
    let action_hint = match resolve_attach_target(app) {
        Ok(target) => {
            let label = app
                .snapshot()
                .map(|snapshot| target_label(snapshot.as_ref(), &target))
                .unwrap_or_else(|| format!("{}:{}", target.backend, target.native_id));
            match selected_mux_state(app) {
                Some(MuxIndicator::Ambiguous { .. }) => {
                    format!("a attach preferred {label} · m choose")
                }
                _ => format!("a attach {}", compact_mux_label(&label)),
            }
        }
        Err(reason) => attach_disabled_reason(&reason),
    };
    format!("{focus_hint} · {action_hint} · Tab focus · r refresh · q quit")
}

fn selected_mux_state(app: &App) -> Option<MuxIndicator> {
    let selection = app.selection()?;
    let row = app.tree().rows.iter().find(|row| &row.id == selection)?;
    match &row.kind {
        RowKind::AgentSession(session) => Some(session.mux_state),
        _ => None,
    }
}

fn preview_separator(app: &App) -> String {
    let Some(target) = resolve_attach_target(app).ok() else {
        return "─ preview ─".to_string();
    };
    let label = compact_mux_label(&format!("{}:{}", target.backend, target.native_id));
    let freshness = app
        .mux_preview(&target.mux)
        .and_then(|entry| entry.captured_at)
        .map(|captured| {
            format!(
                " · captured {}",
                format_elapsed(captured.elapsed().as_secs())
            )
        });
    match freshness {
        Some(freshness) => format!("─ preview · {label}{freshness} ago ─"),
        None => format!("─ preview · {label} ─"),
    }
}

fn format_elapsed(seconds: u64) -> String {
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

fn crop_bottom_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

// -----------------------------------------------------------------------------
// Snapshot helpers (test-only)
// -----------------------------------------------------------------------------

/// Render the entire UI for `app` into a fresh `Buffer` at the given
/// terminal area. Exposed for snapshot tests so they can assert on
/// the rendered shape without a real TTY.
#[cfg(test)]
pub fn render_to_buffer(app: &App, area: Rect) -> ratatui::buffer::Buffer {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("test terminal");
    terminal
        .draw(|frame| draw(app, frame))
        .expect("draw on test backend");
    terminal.backend().buffer().clone()
}

/// Convert a buffer to a newline-joined string. Used by snapshot
/// tests; styling is dropped — we only assert on the characters
/// the operator would see.
#[cfg(test)]
pub fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let width = buffer.area.width as usize;
    let mut out = String::with_capacity((width + 1) * buffer.area.height as usize);
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot, RepoId, RepoNode, WorktreeId,
        WorktreeNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::SessionsGrouping;
    use crate::tui::app::Msg;
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
    use crate::tui::{RunConfig, View};
    use std::sync::Arc;

    fn seeded_app() -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
            root: "/home/op/src/proj".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("codex", "/state", "abc"),
                harness_key: "codex".to_string(),
                cwd: Some("/home/op/src/proj".to_string()),
                title: Some("Phase 8 walkthrough".to_string()),
                last_message_preview: Some("could you give me a bit more context?".to_string()),
            }));
        let snapshot = resolve_snapshot(snapshot);

        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    fn muxed_app(native_id: &str, capture: Option<&str>) -> App {
        use crate::model::{
            Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
            Provenance, RelationKind,
        };

        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
            root: "/home/op/src/proj".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("codex", "/state", "abc"),
                harness_key: "codex".to_string(),
                cwd: Some("/home/op/src/proj".to_string()),
                title: None,
                last_message_preview: Some("stale msg".to_string()),
            }));
        let mux_graph_id = MuxSessionId::new(native_id);
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_graph_id.clone(),
            backend: "tmux".to_string(),
            native_id: native_id.to_string(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(mux_graph_id.clone());
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session_id,
            target: LinkEndpoint::Node { id: mux_id },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.update(Msg::NavDown);
        if let Some(capture) = capture {
            app.update(Msg::SetMuxPreview {
                mux: mux_graph_id,
                content: PreviewContent::Text(capture.to_string()),
            });
        }
        app
    }

    #[test]
    fn render_at_default_size_shows_header_tree_and_detail() {
        let mut app = seeded_app();
        // Auto-selection lands on the first visible row (the repo
        // group); step down once so the right panel shows the
        // session's detail, which is what the operator-facing
        // assertions below cover.
        app.update(Msg::NavDown);

        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        assert!(
            text.contains("sessions"),
            "header view label missing: {text}"
        );
        assert!(text.contains("1 agents"), "agent count missing: {text}");
        assert!(
            text.contains("~/src/proj"),
            "shortened path missing: {text}"
        );
        assert!(
            text.contains("codex"),
            "session harness label missing: {text}"
        );
        assert!(
            text.contains("could you give me a bit more"),
            "inline preview missing: {text}"
        );
        assert!(
            text.contains("Phase 8 walkthrough"),
            "right-panel title row missing: {text}"
        );
        assert!(text.contains("─ preview ─"), "preview separator missing");
    }

    #[test]
    fn empty_app_renders_loading_placeholder() {
        let app = App::new(RunConfig::defaults());
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("Loading"),
            "expected loading placeholder: {text}"
        );
    }

    #[test]
    fn header_shows_updated_ns_ago_when_clock_is_ahead_of_load_epoch() {
        let app = seeded_app();
        // seeded_app sets loaded_at_epoch = 1_700_000_000.
        // Advance the rendering clock 12s to assert the freshness slot.
        test_clock::set(1_700_000_012);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("updated 12s ago"),
            "expected updated-ago slot, got: {text}"
        );
    }

    #[test]
    fn narrow_terminal_stacks_the_two_panels_vertically() {
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        // Width 60 is below NARROW_LAYOUT_THRESHOLD.
        let area = Rect::new(0, 0, 60, 30);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        // In the stacked layout, only one panel border occupies
        // each row at any given column. The header still appears
        // at the top, and the right-panel content ("Phase 8
        // walkthrough" title) sits *below* the row tree content
        // ("~/src/proj") rather than beside it. Assert that
        // ordering.
        let proj_line = text
            .lines()
            .position(|l| l.contains("~/src/proj"))
            .expect("project path line present");
        let title_line = text
            .lines()
            .position(|l| l.contains("Phase 8 walkthrough"))
            .expect("right-panel title present");
        assert!(
            title_line > proj_line,
            "right panel should be below left in narrow mode (title={title_line}, proj={proj_line})"
        );
    }

    #[test]
    fn no_live_preview_muxed_session_shows_privacy_banner() {
        // Build a session that's muxed (has one LinkedToMux candidate),
        // load with --no-live-preview, and ensure the preview block
        // shows the privacy banner rather than the graph snippet.
        use crate::model::{
            Confidence, GraphLink, GraphNode, LinkEndpoint, LinkState, MuxSessionId,
            MuxSessionNode, NodeId, Provenance, RelationKind,
        };

        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
            root: "/home/op/src/proj".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("codex", "/state", "abc"),
                harness_key: "codex".to_string(),
                cwd: Some("/home/op/src/proj".to_string()),
                title: None,
                last_message_preview: Some("stale msg".to_string()),
            }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".to_string(),
            native_id: "editor".to_string(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: session_id,
            target: LinkEndpoint::Node { id: mux_id },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        config.live_preview_enabled = false;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.update(Msg::NavDown); // jump from repo group → session row

        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("preview disabled"),
            "expected --no-live-preview banner in right panel, got: {text}"
        );
        // Per the locked mockup decision, `--no-live-preview` does
        // NOT suppress inline previews in the row tree — only live
        // extras (pane capture + transcript-tail) in the right
        // panel. The graph-resident `stale msg` is allowed to
        // remain in the inline-preview line below the row.
    }

    #[test]
    fn right_focus_keeps_selected_row_highlighted_and_changes_status_scope() {
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        app.update(Msg::CycleFocus);

        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("j/k scroll preview"),
            "right focus status hint missing: {text}"
        );
        assert!(
            text.contains("[right]"),
            "right focus marker missing: {text}"
        );

        let selected_has_inactive_bg = (0..buffer.area.height).any(|y| {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.contains("codex")
                && (0..buffer.area.width)
                    .any(|x| buffer[(x, y)].style().bg == Some(SELECTED_INACTIVE_BG))
        });
        assert!(
            selected_has_inactive_bg,
            "selected row should remain highlighted when right pane has focus"
        );
    }

    #[test]
    fn contextual_status_reports_unmuxed_attach_reason() {
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        let area = Rect::new(0, 0, 100, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("attach: session is not attached to any mux"),
            "expected contextual attach-disabled reason: {text}"
        );
    }

    #[test]
    fn mux_preview_renders_compact_header_and_bottom_cropped_capture() {
        let mut app = muxed_app(
            "agentdeck_conspectus-very-long-session-name-with-suffix_12345678",
            Some("line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7"),
        );
        app.update(Msg::ScrollPreviewBy(1));
        app.update(Msg::ScrollPreviewBy(-1));

        let area = Rect::new(0, 0, 100, 14);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("preview · tmux:agentdeck_conspectus-very-lo"),
            "expected compact mux preview header: {text}"
        );
        assert!(
            preview_separator(&app).contains("captured"),
            "freshness label missing from full preview separator"
        );
        assert!(
            !text.contains("line 1"),
            "oldest capture lines should be cropped out: {text}"
        );
        assert!(
            text.contains("line 7"),
            "latest capture line should remain visible: {text}"
        );
    }

    #[test]
    fn compact_path_helpers_keep_primary_label_short() {
        assert_eq!(compact_path_label("~/src/conspectus"), "conspectus");
        assert_eq!(
            compact_path_secondary("~/src/conspectus"),
            "~/src/conspectus"
        );
        assert_eq!(compact_path_label("Ungrouped"), "Ungrouped");
        assert_eq!(compact_path_secondary("Ungrouped"), "");
    }

    #[test]
    fn crop_bottom_lines_keeps_latest_lines() {
        assert_eq!(crop_bottom_lines("a\nb\nc\nd", 2), "c\nd");
        assert_eq!(crop_bottom_lines("a\nb", 3), "a\nb");
    }

    #[test]
    fn render_captured_pane_with_color_parses_ansi_into_styled_spans() {
        // ESC[31m makes "red", ESC[0m resets.
        let raw = "\x1b[31mred\x1b[0m  plain";
        let text = render_captured_pane(raw, true);
        // Flattened content matches the visible characters.
        assert_eq!(text.to_string(), "red  plain");
        // The first line's first span carries red foreground style.
        let first_line = text.lines.first().expect("at least one line");
        let first_span = first_line.spans.first().expect("at least one span");
        assert_eq!(first_span.content, "red");
        assert_eq!(
            first_span.style.fg,
            Some(ratatui::style::Color::Red),
            "expected red fg on the red span"
        );
    }

    #[test]
    fn render_captured_pane_without_color_strips_styling() {
        let raw = "\x1b[31mred\x1b[0m  plain";
        let text = render_captured_pane(raw, false);
        assert_eq!(text.to_string(), "red  plain");
        // With colour disabled every span should land styled
        // identically to a plain `Text::raw` — i.e. default fg.
        for line in &text.lines {
            for span in &line.spans {
                assert_eq!(
                    span.style.fg, None,
                    "expected colour to be stripped, got span={span:?}"
                );
            }
        }
    }

    #[test]
    fn render_captured_pane_falls_back_to_plain_text_on_malformed_input() {
        // Lone ESC byte — ansi-to-tui should either parse harmlessly
        // or fail; either way `render_captured_pane` returns
        // something printable rather than panicking.
        let raw = "before\x1bafter";
        let text = render_captured_pane(raw, true);
        let flattened = text.to_string();
        // The visible characters either side of the rogue ESC
        // must survive — operators don't lose pane content to a
        // single bad byte.
        assert!(
            flattened.contains("before"),
            "expected 'before' in output, got: {flattened:?}"
        );
        assert!(
            flattened.contains("after"),
            "expected 'after' in output, got: {flattened:?}"
        );
    }

    #[test]
    fn left_panel_scrolls_to_keep_selected_row_visible_past_viewport() {
        // Build a snapshot with one repo and twenty sessions so
        // the rendered tree spills well past a small viewport.
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
            root: "/home/op/src/proj".to_string(),
            git_dir: None,
            current_branch: None,
        }));
        for i in 0..20 {
            snapshot
                .nodes
                .push(GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                    harness_key: "codex".to_string(),
                    cwd: Some("/home/op/src/proj".to_string()),
                    title: None,
                    last_message_preview: None,
                }));
        }
        let snapshot = crate::resolve::resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        // Jump to the last visible row — it lives well below the
        // viewport for a 10-tall window.
        app.update(Msg::End);

        // Render into a narrow 80x12 window — body is 10 tall after
        // header/status bars. The selected session's short id must
        // appear in the rendered buffer.
        let area = Rect::new(0, 0, 80, 12);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        // The last row should be visible. Confirm via the short
        // id of the last session pushed (s19).
        let visible = app.visible_rows();
        let last_id = match &visible.last().unwrap().kind {
            RowKind::AgentSession(s) => s.short_id.clone(),
            other => panic!("expected last row to be a session, got {other:?}"),
        };
        assert!(
            text.contains(&last_id),
            "selected row's short id ({last_id}) should be visible after End; got:\n{text}"
        );

        // The first row (the repo group) should now be scrolled
        // off the top.
        assert!(
            !text.contains("~/src/proj"),
            "top-of-tree group should be scrolled away when selection is at End; got:\n{text}"
        );
    }
}
