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

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::model::{GraphNode, GraphSnapshot, MuxSessionId, NodeId};
use crate::tui::View;
use crate::tui::actions::resolve_attach_target;
use crate::tui::app::{App, Focus};
use crate::tui::detail::{HeaderField, NodeDetail};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, MuxCandidateRow, MuxIndicator, RowId, RowKind, format_recency,
};

const INLINE_PREVIEW_DEFAULT: usize = 3;
const SELECTED_BG: Color = Color::Indexed(238);
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
    let hints =
        "j/k move · Enter expand · a attach · Tab focus · J/K scroll preview · r refresh · q quit";
    let scope = match app.focus() {
        Focus::Left => "[left]",
        Focus::Right => "[right]",
    };
    let widget = Paragraph::new(format!("{hints}  {scope}"))
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
    for row in &visible {
        let is_selected = app.selection() == Some(&row.id);
        let primary = render_left_row(row, app, is_selected);
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

    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
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
                group.display_path.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        }
        RowKind::AgentSession(session) => spans.extend(render_session_spans(session)),
        RowKind::AgentSessionMuxCandidate(candidate) => {
            spans.extend(render_candidate_spans(candidate))
        }
    }

    let mut line = Line::from(spans);
    if is_selected && app.focus() == Focus::Left {
        line = line.style(
            Style::default()
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        );
    } else if is_selected {
        line = line.style(Style::default().add_modifier(Modifier::BOLD));
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
    spans.push(Span::raw(candidate.mux_label.clone()));
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
        Paragraph::new("─ preview ─").style(Style::default().add_modifier(Modifier::DIM)),
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
    let preview_text = preview_text_for_selection(app);
    let widget = Paragraph::new(preview_text)
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
fn preview_text_for_selection(app: &App) -> String {
    let Some(selection) = app.selection() else {
        return String::new();
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return String::new();
    };
    let live_preview = app.config().live_preview_enabled;
    match &row.kind {
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => {
                mux_preview_text(app, live_preview)
            }
            MuxIndicator::Unmuxed => session
                .preview
                .clone()
                .unwrap_or_else(|| "no preview available".to_string()),
        },
        RowKind::AgentSessionMuxCandidate(_) => mux_preview_text(app, live_preview),
        _ => match selection {
            RowId::Group(NodeId::MuxSession(_)) => mux_preview_text(app, live_preview),
            _ => {
                if live_preview {
                    "no preview for this row".to_string()
                } else {
                    "preview disabled (--no-live-preview)".to_string()
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
fn mux_preview_text(app: &App, live_preview: bool) -> String {
    if !live_preview {
        return "preview disabled (--no-live-preview)".to_string();
    }
    let Some(target) = resolve_attach_target(app).ok().map(|t| t.mux) else {
        return "no mux target for this row".to_string();
    };
    format_preview_for_mux(app, &target)
}

fn format_preview_for_mux(app: &App, mux: &MuxSessionId) -> String {
    match app.mux_preview(mux) {
        Some(entry) => match &entry.content {
            PreviewContent::Text(text) if text.is_empty() => "(empty pane)".to_string(),
            PreviewContent::Text(text) => text.clone(),
            PreviewContent::NoTarget => "tmux target not found — try `r` to refresh".to_string(),
            PreviewContent::Unavailable(reason) => {
                format!("tmux unavailable: {reason}")
            }
            PreviewContent::Failed(message) => message.clone(),
            PreviewContent::Unsupported => {
                "preview unavailable (runner does not implement capture)".to_string()
            }
        },
        None => "loading mux preview…".to_string(),
    }
}

fn panel_focus_style(app: &App, focus: Focus) -> Style {
    if app.focus() == focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
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
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
        });
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

        let area = Rect::new(0, 0, 80, 24);
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
        let area = Rect::new(0, 0, 80, 24);
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
        let area = Rect::new(0, 0, 80, 24);
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
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        config.live_preview_enabled = false;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: Arc::new(snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
        });
        app.update(Msg::NavDown); // jump from repo group → session row

        let area = Rect::new(0, 0, 80, 24);
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
}
