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
//! - Header reports `N agents · M mux` from the current snapshot
//!   counts; the `updated Ns ago` slot is filled by P8-007's
//!   follow-on once the runtime threads `loaded_at_epoch` through
//!   the reducer.
//! - Sessions row inline preview lights up for the selected row and
//!   for the N most-recent visible session rows
//!   (`[tui].inline_preview_rows`, default 3).
//! - Empty/loading frames render minimal copy when no row tree is
//!   loaded yet; richer empty/error states land in the P8-007
//!   follow-on after P8-008's full data adapter ships.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::model::{GraphNode, GraphSnapshot};
use crate::tui::View;
use crate::tui::app::{App, Focus};
use crate::tui::detail::{HeaderField, NodeDetail};
use crate::tui::rows::{AgentSessionRow, MuxCandidateRow, MuxIndicator, RowId, RowKind};

const INLINE_PREVIEW_DEFAULT: usize = 3;
const SELECTED_BG: Color = Color::Indexed(238);

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
    let title = format!("Conspectus · {view_label} ─ {agents} agents · {mux} mux",);
    let widget = Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(widget, area);
}

fn draw_status_bar(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let hints = "j/k move · Enter expand · Tab focus · J/K scroll preview · r refresh · q quit";
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
    let split = Layout::default()
        .direction(Direction::Horizontal)
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
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Min(3),
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
/// Today: the selected agent session's `last_message_preview` from
/// the row tree, or an empty-state message otherwise. The pane-
/// capture and transcript-tail surfaces wire in later
/// (P8-009 / P8-012c).
fn preview_text_for_selection(app: &App) -> String {
    let Some(selection) = app.selection() else {
        return String::new();
    };
    let selected_row = app.tree().rows.iter().find(|r| &r.id == selection);
    let Some(row) = selected_row else {
        return String::new();
    };
    match &row.kind {
        RowKind::AgentSession(session) => session
            .preview
            .clone()
            .unwrap_or_else(|| "no preview available".to_string()),
        _ => match app.config().live_preview_enabled {
            true => "preview lands once the mux capture adapter (P8-009) wires in".to_string(),
            false => "preview disabled (--no-live-preview)".to_string(),
        },
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
}
