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
//! - Sessions use spare horizontal space after the mux indicator
//!   for a dim one-line last-message preview.
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
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::SessionsGrouping;
use crate::tui::Theme;
use crate::tui::View;
use crate::tui::actions::{attach_disabled_reason, resolve_attach_target, target_label};
use crate::tui::app::{App, Focus, GraphDb};
use crate::tui::detail::{HeaderField, NodeDetail, SectionKind};
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::preview::PreviewContent;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxCandidateRow, MuxIndicator, MuxSessionRow, PrRow, RowId, RowKind,
    format_recency, recency_bucket,
};

/// Terminal width threshold below which the body switches from a
/// side-by-side split to a vertical stack (left-on-top per the
/// phase-08 layout note).
pub(super) const NARROW_LAYOUT_THRESHOLD: u16 = 100;

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
    draw_controls_overlay(app, frame, area);
    draw_pins_overlay(app, frame, area);
    draw_search_overlay(app, frame, area);
    draw_help_overlay(app, frame, area);
    draw_rename_overlay(app, frame, area);
    draw_value_modal(app, frame, area);
    draw_toast(app, frame, area);
}

fn draw_toast(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.toast() else {
        return;
    };
    use crate::tui::widgets::toast::ToastWidget;
    frame.render_widget(ToastWidget::new(state, app.theme()), area);
}

fn draw_value_modal(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.value_modal() else {
        return;
    };
    use crate::tui::widgets::value_modal::ValueModalWidget;
    frame.render_widget(ValueModalWidget::new(state, app.theme()), area);
}

fn draw_help_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.help_overlay() else {
        return;
    };
    use crate::tui::widgets::help::HelpOverlayWidget;
    frame.render_widget(HelpOverlayWidget::new(state, app.theme()), area);
}

fn draw_search_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.search_overlay() else {
        return;
    };
    use crate::tui::search::items_from_rows;
    use crate::tui::widgets::search::SearchOverlayWidget;
    // Recompute items from the live visible row tree each frame so
    // the search overlay's label lookup never lags behind a
    // refresh. The trade is cheap (visible_rows is already
    // materialized; items_from_rows just clones a few strings per
    // row).
    let visible: Vec<crate::tui::rows::Row> = app.visible_rows().into_iter().cloned().collect();
    let items = items_from_rows(&visible);
    let widget = SearchOverlayWidget::new(state, &items, app.theme());
    frame.render_widget(widget, area);
}

fn draw_rename_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.rename_overlay() else {
        return;
    };
    use crate::tui::widgets::input::TextInputWidget;
    let widget = TextInputWidget::new(state);
    frame.render_widget(widget, area);
}

fn draw_controls_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.controls_overlay() else {
        return;
    };
    use crate::tui::widgets::controls::ControlsOverlayWidget;
    let widget = ControlsOverlayWidget::new(state, app.controls_context());
    frame.render_widget(widget, area);
}

fn draw_pins_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.pins_overlay() else {
        return;
    };
    use crate::tui::widgets::pins::PinsOverlayWidget;
    let widget = PinsOverlayWidget::new(state);
    frame.render_widget(widget, area);
}

// -----------------------------------------------------------------------------
// Header / status bar
// -----------------------------------------------------------------------------

fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let theme = app.theme();
    let view_label = view_label(app.config().default_view);
    let (agents_total, mux_total) = snapshot_counts(app.graph_db());
    let visible_sessions = visible_agent_session_count(app);
    let freshness = header_freshness(app);
    let agent_cell = format_count_with_filtered(visible_sessions, agents_total);

    // Identity prefix is always rendered in bold; chips after it
    // carry their own colors and stay independent of the prefix
    // style so theme overrides land cleanly.
    let prefix =
        format!("Conspectus · {view_label} · {freshness}{agent_cell} agents · {mux_total} mux");
    let prefix_width = prefix.chars().count();
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        prefix,
        Style::default().add_modifier(Modifier::BOLD),
    )];

    // Append per-harness and per-mux-state chips when the terminal
    // has the room. Drops chip labels first (counts only) and then
    // skips chips entirely when even the counts would overflow, so
    // the prefix above stays legible at every width.
    let counts = HeaderCounts::from_app(app);
    let budget = (area.width as usize).saturating_sub(prefix_width);
    append_header_chips(&mut spans, &counts, theme, budget);

    let widget = Paragraph::new(Line::from(spans));
    frame.render_widget(widget, area);
}

/// Per-harness and per-mux-state row aggregates used by the dense
/// header. Walks the currently-visible row tree so the chips
/// reflect "what's on screen" rather than raw discovery — matches
/// the existing `visible_sessions of total` rule on the agent
/// count.
#[derive(Default, Debug, Clone)]
struct HeaderCounts {
    by_harness: Vec<(String, usize)>,
    mux_attached: usize,
    mux_ambiguous: usize,
    mux_unmuxed: usize,
}

impl HeaderCounts {
    fn from_app(app: &App) -> Self {
        use std::collections::BTreeMap;
        let mut by_harness: BTreeMap<String, usize> = BTreeMap::new();
        let mut counts = HeaderCounts::default();
        for row in &app.tree().rows {
            if let RowKind::AgentSession(session) = &row.kind {
                *by_harness.entry(session.harness_label.clone()).or_default() += 1;
                match session.mux_state {
                    MuxIndicator::Attached => counts.mux_attached += 1,
                    MuxIndicator::Ambiguous { .. } => counts.mux_ambiguous += 1,
                    MuxIndicator::Unmuxed => counts.mux_unmuxed += 1,
                }
            }
        }
        counts.by_harness = by_harness.into_iter().collect();
        counts
    }
}

/// Width of one harness chip (` <label> ` + ` <count>`). Mirrors the
/// badge widget's contract so the layout math stays in step.
fn harness_chip_width(label: &str, count: usize) -> usize {
    crate::tui::widgets::badge::harness_badge_width(label) + 1 + count_digits(count)
}

fn count_digits(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        let mut n = value;
        let mut d = 0;
        while n > 0 {
            n /= 10;
            d += 1;
        }
        d
    }
}

/// Mux chips are `<glyph> <count>` with the glyph colored from the
/// theme. Always 3 visible cells per chip (1 glyph + 1 space + 1-2
/// digit count); we underestimate digit width as 1 for layout math
/// since the difference is at most one cell per chip.
const MUX_CHIP_BASE_WIDTH: usize = 3;
const CHIP_SEPARATOR: &str = "  ";
const SECTION_SEPARATOR: &str = "  ·  ";

fn append_header_chips(
    spans: &mut Vec<Span<'static>>,
    counts: &HeaderCounts,
    theme: &Theme,
    budget: usize,
) {
    use crate::tui::widgets::badge::harness_badge;
    if counts.by_harness.is_empty()
        && counts.mux_attached == 0
        && counts.mux_ambiguous == 0
        && counts.mux_unmuxed == 0
    {
        return;
    }

    let harness_section_width: usize = counts
        .by_harness
        .iter()
        .map(|(label, n)| harness_chip_width(label, *n))
        .sum::<usize>()
        + counts.by_harness.len().saturating_sub(1) * CHIP_SEPARATOR.len();

    let mux_section_width = MUX_CHIP_BASE_WIDTH * 3 + CHIP_SEPARATOR.len() * 2;

    let want = SECTION_SEPARATOR.len()
        + harness_section_width
        + SECTION_SEPARATOR.len()
        + mux_section_width;

    if budget < SECTION_SEPARATOR.len() + mux_section_width {
        // Not enough room for even the mux chip section; bail out
        // and keep the bare prefix.
        return;
    }

    let include_harness = budget >= want;

    spans.push(Span::raw(SECTION_SEPARATOR));

    if include_harness {
        let mut first = true;
        for (label, count) in &counts.by_harness {
            if !first {
                spans.push(Span::raw(CHIP_SEPARATOR));
            }
            first = false;
            spans.push(harness_badge(label, theme));
            spans.push(Span::raw(format!(" {count}")));
        }
        spans.push(Span::raw(SECTION_SEPARATOR));
    }

    // Mux chip section: one chip per state with the theme-colored glyph.
    spans.push(Span::styled(
        "◉".to_string(),
        Style::default().fg(theme.mux_attached),
    ));
    spans.push(Span::raw(format!(" {}", counts.mux_attached)));
    spans.push(Span::raw(CHIP_SEPARATOR));
    spans.push(Span::styled(
        "◐".to_string(),
        Style::default().fg(theme.mux_ambiguous),
    ));
    spans.push(Span::raw(format!(" {}", counts.mux_ambiguous)));
    spans.push(Span::raw(CHIP_SEPARATOR));
    spans.push(Span::styled(
        "◯".to_string(),
        Style::default().add_modifier(theme.mux_unmuxed),
    ));
    spans.push(Span::raw(format!(" {}", counts.mux_unmuxed)));
}

/// Render the header's agents count. When a filter is active and the
/// visible row count differs from the snapshot's total, format as
/// `<filtered> of <total>` per ADR 0031; otherwise keep the bare
/// count so unfiltered runs render exactly as before.
fn format_count_with_filtered(visible: usize, total: usize) -> String {
    if visible == total {
        total.to_string()
    } else {
        format!("{visible} of {total}")
    }
}

/// Count agent-session rows currently in the row tree. Mirrors the
/// "filtered count" the operator sees in the left panel, since the
/// row-tree builder is the authority on what's visible after
/// filters apply.
fn visible_agent_session_count(app: &App) -> usize {
    app.tree()
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .count()
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
    let theme = app.theme();
    if let Some(message) = app.status_message() {
        let widget = Paragraph::new(message.to_string()).style(Style::default().fg(theme.warning));
        frame.render_widget(widget, area);
        return;
    }

    let stale = app.refresh_failure().is_some();

    let hints = contextual_status_text(app);
    let scope = match app.focus() {
        Focus::Left => "[left]",
        Focus::Right => "[right]",
    };
    let settings = render_view_state_chips(app);

    let mut spans = vec![
        Span::styled(
            format!("{scope} "),
            Style::default().add_modifier(theme.placeholder),
        ),
        Span::styled(settings, Style::default().fg(theme.cwd_mark)),
        Span::styled(
            format!(" · {hints}"),
            Style::default().add_modifier(theme.placeholder),
        ),
    ];

    if stale {
        spans.push(Span::styled(
            "  stale",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }

    push_provider_chips(app, theme, &mut spans);

    let widget = Paragraph::new(Line::from(spans));
    frame.render_widget(widget, area);
}

/// Append right-side provider status chips to `spans` from
/// `App::provider_status`.
fn push_provider_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let status = app.provider_status();
    let mut leading_space = false;

    let mut push_chip = |label: &str, style: Style| {
        if !leading_space {
            spans.push(Span::raw("  "));
            leading_space = true;
        }
        spans.push(Span::styled(label.to_string(), style));
    };

    if status.tmux_disabled {
        push_chip("tmux:off", Style::default().add_modifier(theme.placeholder));
    } else if let Some(false) = status.tmux_available {
        let reason = status.tmux_reason.as_deref().unwrap_or("unavailable");
        push_chip(
            &format!("tmux:{reason}"),
            Style::default().fg(theme.warning),
        );
    }

    if status.forge_disabled {
        push_chip(
            "forge:off",
            Style::default().add_modifier(theme.placeholder),
        );
    } else if let Some(false) = status.forge_available {
        let reason = status.forge_reason.as_deref().unwrap_or("error");
        push_chip(&format!("gh:{reason}"), Style::default().fg(theme.warning));
    }
}

/// Render the active view state in a compact, always-visible form
/// so grouping / filtering / sorting are discoverable without
/// opening the controls overlay.
fn render_view_state_chips(app: &App) -> String {
    let filter = render_filter_chips(app.filter());
    let filter = if filter.is_empty() {
        "all".to_string()
    } else {
        filter
    };
    format!(
        "group:{} · filter:{} · sort:{}",
        app.grouping().as_str(),
        filter,
        sort_chip_label(app.sort())
    )
}

fn sort_chip_label(sort: crate::tui::Sort) -> &'static str {
    match sort {
        crate::tui::Sort::Hierarchy => "hierarchy",
        crate::tui::Sort::Recency => "recency",
    }
}

/// Render the active filter as a compact chip-strip:
/// `harness:claude,codex · max-age:7d · mux:unmuxed`. Returns the
/// empty string when no constraints are active so callers can
/// short-circuit the separator. Per ADR 0031 the chip order is
/// stable across runs (harness → max-age → mux-state) so the
/// operator builds muscle memory for where each predicate lives.
fn render_filter_chips(filter: &crate::filter::RowFilter) -> String {
    if !filter.has_narrowing_predicates() {
        return String::new();
    }
    let mut chips: Vec<String> = Vec::new();
    if let Some(harness) = &filter.harness {
        let values = harness.values();
        if !values.is_empty() {
            chips.push(format!("harness:{}", truncate_chip_list(values, 3)));
        }
    }
    if let Some(max_age) = filter.max_age {
        chips.push(format!(
            "max-age:{}",
            crate::tui::widgets::controls::format_duration_for_input(max_age)
        ));
    }
    if let Some(mux_state) = &filter.mux_state {
        let labels: Vec<String> = mux_state
            .values()
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        if !labels.is_empty() {
            chips.push(format!("mux:{}", truncate_chip_list(&labels, 3)));
        }
    }
    chips.join(" · ")
}

/// Comma-join up to `cap` values, replacing the tail with `+N more`
/// when the list is longer. Keeps the status bar compact when an
/// operator selects every harness or mux state.
fn truncate_chip_list(values: &[String], cap: usize) -> String {
    if values.len() <= cap {
        return values.join(",");
    }
    let head: Vec<&str> = values.iter().take(cap).map(|s| s.as_str()).collect();
    let extra = values.len() - cap;
    format!("{}+{extra} more", head.join(","))
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

fn snapshot_counts(database: Option<&GraphDb>) -> (usize, usize) {
    let Some(database) = database else {
        return (0, 0);
    };
    let agents = database
        .conn()
        .query_row("SELECT COUNT(*) FROM node_agent_sessions", [], |row| {
            row.get::<_, i64>(0)
        })
        .ok()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
    let mux = database
        .conn()
        .query_row("SELECT COUNT(*) FROM node_mux_sessions", [], |row| {
            row.get::<_, i64>(0)
        })
        .ok()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
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

    let scroll = if let Some(line_idx) = selected_primary_line {
        app.adjust_left_scroll(line_idx, inner.height)
    } else {
        0
    };

    let widget = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(widget, inner);
}

/// Render the left pane title as a lazydocker-style tab strip
/// showing every view, with the active one accented. Operators see
/// the available views at a glance instead of having to remember
/// the `1`–`5` accelerators or open the controls overlay.
fn left_panel_title(app: &App) -> Line<'static> {
    let theme = app.theme();
    let active = app.config().default_view;
    let mut spans = vec![Span::raw(" "), focus_marker_span(app, Focus::Left)];
    let mut first = true;
    for &view in crate::tui::widgets::controls::VIEW_OPTIONS {
        if !first {
            spans.push(Span::styled(
                " · ",
                Style::default().fg(theme.secondary_text),
            ));
        }
        first = false;
        let label = view_label(view);
        if view == active {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(theme.panel_focus_accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().fg(theme.secondary_text),
            ));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Render the right pane title with the kind of node currently
/// being inspected (`session`, `mux`, `pr`, …) so the operator can
/// tell what they're looking at without re-reading the body.
/// Falls back to `detail` while no selection is resolved.
fn right_panel_title(app: &App, width: usize) -> Line<'static> {
    let label = right_panel_kind_label(app);
    let mut spans = vec![Span::raw(" "), focus_marker_span(app, Focus::Right)];
    // ADR 0073 §3: the right-panel node header is `<glyph> <label>`
    // with the glyph in the node-kind color and the label bold. The
    // glyph is suppressed when there is no resolved selection
    // (fallback "detail" label) so a placeholder pane does not stamp
    // a misleading kind cue.
    if let Some(detail) = app.detail()
        && let Some(node_kind) = NodeKind::from_snake_case(detail.kind_label)
    {
        let style = node_kind_style(node_kind, app.theme());
        let color = if matches!(node_kind, NodeKind::ForgePr) {
            app.theme().pr_open
        } else {
            style.color
        };
        spans.push(Span::styled(
            format!("{} ", style.glyph),
            Style::default().fg(color),
        ));
    }
    spans.push(Span::styled(
        label,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    // T8-038: render the full drilldown chain in short `kind:tag`
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
        let kind_glyph_width = app
            .detail()
            .and_then(|d| NodeKind::from_snake_case(d.kind_label))
            .map(|_| 2)
            .unwrap_or(0);
        let fixed_width = 1
            + focus_marker_width(app, Focus::Right)
            + kind_glyph_width
            + label.chars().count()
            + " ◀ ".chars().count()
            + depth_suffix.chars().count()
            + 1;
        let available = width.saturating_sub(fixed_width);
        if let Some(chain) =
            crate::tui::explorer::render_breadcrumb_chain(&state.breadcrumb, available.max(8))
        {
            spans.push(Span::styled(
                format!(" ◀ {chain}"),
                Style::default().fg(app.theme().secondary_text),
            ));
            spans.push(Span::styled(
                depth_suffix,
                Style::default().fg(app.theme().secondary_text),
            ));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn focus_marker_width(app: &App, focus: Focus) -> usize {
    focus_marker_span(app, focus).content.chars().count()
}

fn right_panel_kind_label(app: &App) -> &'static str {
    let Some(detail) = app.detail() else {
        return "detail";
    };
    match detail.kind_label {
        "agent_session" => "session",
        "mux_session" => "mux",
        "forge_pr" => "pr",
        "fork" => "fork",
        "repo" => "repo",
        "checkout" => "checkout",
        "workspace" => "workspace",
        "branch" => "branch",
        _ => "detail",
    }
}

/// `▸ ` when the given panel has focus, two-space pad otherwise.
/// Keeps title widths consistent across focus states so the tab
/// strip and right-pane label sit at the same offset before and
/// after `Tab`.
fn focus_marker_span(app: &App, panel: Focus) -> Span<'static> {
    let focused = app.focus() == panel;
    Span::styled(
        if focused { "▸ " } else { "  " }.to_string(),
        Style::default().fg(app.theme().panel_focus_accent),
    )
}

fn empty_left_panel_text(app: &App) -> String {
    if app.graph_db().is_none() {
        return "Loading discovery…".to_string();
    }
    if !app.filter().is_empty() {
        // Filtered-zero case (F8-012): a snapshot is loaded but the
        // active filter dropped every session. Distinguish from
        // "no sessions discovered" so the operator knows their
        // filter — not the world — is the reason.
        let chips = render_filter_chips(app.filter());
        return format!("No rows match `{chips}`.\nPress `F` to clear filters, `v` to edit.");
    }
    "No sessions discovered.\nPress `r` to refresh or `q` to quit.".to_string()
}

/// Per-group aggregates surfaced as right-aligned chips on group
/// rows (Phase 7). Counts walk the full row tree (not just visible
/// rows) so a collapsed group still advertises what's inside.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
struct GroupSummary {
    agents: usize,
    attached: usize,
    ambiguous: usize,
    unmuxed: usize,
}

/// Walk the row tree once and compute the [`GroupSummary`] for every
/// group row. Each group's summary aggregates every `AgentSession`
/// row that sits below it in the flat tree (continuous depth >
/// group depth) until the next sibling-or-shallower row.
fn compute_group_summaries(
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
fn append_group_body_spans(
    spans: &mut Vec<Span<'static>>,
    group: &crate::tui::rows::GroupRow,
    theme: &Theme,
    target_label_width: usize,
) {
    let label = compact_path_label(&group.display_path);
    let label_width = UnicodeWidthStr::width(label.as_str());
    spans.push(Span::styled(
        label,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    // Pad the label cell so the secondary content starts at the
    // same column across every visible group row. Skipped when the
    // label is already at or past the target.
    if label_width < target_label_width {
        spans.push(Span::raw(" ".repeat(target_label_width - label_width)));
    }
    let secondary = compact_path_secondary(&group.display_path);
    if !secondary.is_empty() {
        spans.push(Span::styled(
            format!("  {secondary}"),
            Style::default().add_modifier(theme.placeholder),
        ));
    }
    if group.is_launch_context {
        spans.push(Span::styled(
            "  (cwd)".to_string(),
            Style::default()
                .fg(theme.cwd_mark)
                .add_modifier(theme.placeholder),
        ));
    }
}

/// Width of the group row body (indent + disclosure + label +
/// secondary + cwd marker), padded to `target_label_width` between
/// the label and the secondary segment. Used by the left-pane
/// renderer to align the summary-chip column across visible group
/// rows. Returns 0 for non-group rows so callers can fold every
/// visible row through the same width-max.
fn group_row_body_width(
    row: &crate::tui::rows::Row,
    app: &App,
    target_label_width: usize,
) -> usize {
    let RowKind::Group(group) = &row.kind else {
        return 0;
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let flat_sessions = matches!(app.config().sessions_grouping, SessionsGrouping::None);
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
    append_group_body_spans(&mut spans, group, app.theme(), target_label_width);
    spans_width(&spans)
}

/// Width of just the label cell for a group row (excluding indent,
/// disclosure, secondary, and cwd marker). Used by the left-pane
/// pre-pass to compute the label column's max width before bodies
/// are rendered.
fn group_row_label_width(row: &crate::tui::rows::Row) -> usize {
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
fn append_group_summary_spans(
    spans: &mut Vec<Span<'static>>,
    summary: GroupSummary,
    theme: &Theme,
    count_width: usize,
) {
    if summary.agents == 0 {
        return;
    }
    let count = format!("({})", summary.agents);
    spans.push(Span::styled(
        format!("  {count:>count_width$}"),
        Style::default().add_modifier(theme.placeholder),
    ));
    if summary.ambiguous > 0 {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "⚠",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }
}

/// Column anchors for group-row alignment computed once per render
/// pass. Padding keys the secondary segment and summary chip block
/// onto consistent columns so adjacent group rows read like a
/// table; non-group rows ignore these widths.
#[derive(Clone, Copy, Default)]
struct GroupAlign {
    /// Max label width across visible group rows. Pads between the
    /// label and the secondary segment.
    label_width: usize,
    /// Max body width (label-padded) across visible group rows that
    /// will get a summary chip block. Pads between the body and the
    /// `(N)` count plus optional `⚠` ambiguity glyph (ADR 0072).
    body_width: usize,
    /// Max `(N)` count chip width (including parens) across visible
    /// group rows with sessions. Right-pads the count chip so any
    /// trailing ambiguity glyph anchors on the same column whether
    /// the count is `(2)` or `(72)`.
    count_width: usize,
}

/// Build the rendered line for a single visible row.
fn render_left_row(
    row: &crate::tui::rows::Row,
    app: &App,
    is_selected: bool,
    width: usize,
    now: i64,
    group_summary: Option<GroupSummary>,
    align: GroupAlign,
) -> Line<'static> {
    let theme = app.theme();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let flat_sessions = matches!(app.config().sessions_grouping, SessionsGrouping::None);
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
            append_group_body_spans(&mut spans, group, theme, align.label_width);
            if let Some(summary) = group_summary {
                let current_width = spans_width(&spans);
                if current_width < align.body_width {
                    spans.push(Span::raw(" ".repeat(align.body_width - current_width)));
                }
                append_group_summary_spans(&mut spans, summary, theme, align.count_width);
            }
        }
        RowKind::AgentSession(session) => {
            spans.extend(render_session_spans(session, theme, now));
            append_session_preview(&mut spans, session, width, theme);
        }
        RowKind::AgentSessionMuxCandidate(candidate) => {
            spans.extend(render_candidate_spans(candidate, theme))
        }
        RowKind::MuxSession(mux) => {
            let remaining = width.saturating_sub(spans_width(&spans));
            spans.extend(render_mux_session_spans(mux, theme, now, remaining));
        }
        RowKind::Pr(pr) => {
            spans.push(Span::styled(
                pr.repo_display.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            if let Some(state) = pr.state.as_deref() {
                let style = match state {
                    "open" => Style::default().fg(theme.mux_attached),
                    "closed" | "merged" => Style::default().add_modifier(theme.placeholder),
                    _ => Style::default().fg(theme.secondary_text),
                };
                spans.push(Span::styled(format!("  {state}"), style));
            }
            if pr.is_draft {
                spans.push(Span::styled(
                    "  draft",
                    Style::default().add_modifier(theme.placeholder),
                ));
            }
            if let Some(branch) = &pr.branch_name {
                spans.push(Span::styled(
                    format!("  {branch}"),
                    Style::default().add_modifier(theme.placeholder),
                ));
            }
            if let Some(updated) = &pr.updated_recency {
                spans.push(Span::styled(
                    format!("  {updated}"),
                    Style::default().fg(theme.secondary_text),
                ));
            }
            spans.push(Span::styled(
                format!("  ({})", pr.attached_count),
                Style::default().add_modifier(theme.placeholder),
            ));
        }
        RowKind::Fork(fork) => {
            spans.push(Span::styled(
                fork.fork_label.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            if let Some(parent) = &fork.parent_label {
                spans.push(Span::styled(
                    format!("  parent:{parent}"),
                    Style::default().fg(theme.secondary_text),
                ));
            }
            if let Some(scope) = &fork.scope {
                spans.push(Span::styled(
                    format!("  {scope}"),
                    Style::default().add_modifier(theme.placeholder),
                ));
            }
            spans.push(Span::styled(
                format!("  ({})", fork.child_count),
                Style::default().add_modifier(theme.placeholder),
            ));
        }
        RowKind::Pin(pin) => {
            // Pinned, but unbound — dim "📌" marker + display name.
            // Final glyph + theme entry land alongside the rest of
            // H-PIN-016's TUI polish; for the v1 slice we reuse the
            // existing `placeholder` modifier to keep the row visibly
            // distinct without inventing a new Theme key.
            spans.push(Span::styled(
                "📌  ".to_string(),
                Style::default().add_modifier(theme.placeholder),
            ));
            spans.push(Span::styled(
                pin.display_name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(
                    "  ({} · {} · {} · {})",
                    pin.state_label, pin.harness_label, pin.cwd_display, pin.mux_label
                ),
                Style::default().add_modifier(theme.placeholder),
            ));
        }
        RowKind::Repo(repo) => {
            spans.extend(render_repo_spans(repo, theme));
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

fn render_session_spans(session: &AgentSessionRow, theme: &Theme, now: i64) -> Vec<Span<'static>> {
    use crate::tui::widgets::badge::harness_badge;
    const SESSION_ID_COLUMN_WIDTH: usize = 8;
    const SESSION_DISPLAY_LABEL_WIDTH: usize = 32;

    let mut spans = Vec::new();
    // Lead with the harness-native session key rather than
    // Conspectus's internal short node id. Operators recognize the
    // external session id; the internal id is still accepted by
    // explicit node lookup commands.
    let session_id =
        truncate_to_width_no_marker(&session.session.session_key, SESSION_ID_COLUMN_WIDTH);
    spans.push(Span::styled(
        format!("{session_id}  "),
        Style::default().fg(theme.secondary_text),
    ));
    // The badge widget pads internally so every chip is the same
    // width regardless of label length; no external padding span
    // needed.
    spans.push(harness_badge(&session.harness_label, theme));
    spans.push(Span::raw("  "));
    let recency = session.recency.clone().unwrap_or_else(|| "—".to_string());
    let recency_style = recency_bucket(Some(now), session.activity_epoch)
        .map(|bucket| bucket.style(theme))
        .unwrap_or_else(|| Style::default().add_modifier(theme.placeholder));
    spans.push(Span::styled(format!("{recency:>4}"), recency_style));
    spans.push(Span::raw("  "));
    spans.push(mux_indicator_span(session.mux_state, theme));
    if session.pin_id.is_some() {
        // ADR 0057 bound-pin marker. Glyph + theme entry are
        // finalized alongside the rest of the H-PIN-016 styling
        // polish; for the v1 slice we reuse `placeholder` so the
        // marker reads without depending on a new theme key.
        spans.push(Span::styled(
            "  📌",
            Style::default().add_modifier(theme.placeholder),
        ));
    }
    if let Some(label) = session.display_label().filter(|label| !label.is_empty()) {
        let label = truncate_to_width_strict(label, SESSION_DISPLAY_LABEL_WIDTH);
        let style = if session
            .alias
            .as_deref()
            .is_some_and(|alias| !alias.is_empty())
        {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(format!("  {label}"), style));
    }
    if let Some(project) = session
        .project_display
        .as_deref()
        .filter(|project| !project.is_empty())
    {
        spans.push(Span::styled(
            format!("  {:<16}", truncate_to_width(project, 16)),
            Style::default().fg(theme.secondary_text),
        ));
    }
    spans
}

fn append_session_preview(
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
    spans.push(Span::styled(
        truncate_to_width(preview, width - used - 2),
        Style::default()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::ITALIC),
    ));
}

/// Render a `RepoRow` so workspace members in the left pane scan as
/// the same visual rhythm as session / mux rows: short id column +
/// `repo` chip + bold display name + dim canonical path. Mirrors
/// `render_session_spans`'s column shape without the recency / mux
/// glyph columns (repos have no activity state of their own; the
/// row's purpose is identity and navigation).
fn render_repo_spans(repo: &crate::tui::rows::RepoRow, theme: &Theme) -> Vec<Span<'static>> {
    const SHORT_ID_COLUMN_WIDTH: usize = 8;
    const KIND_BADGE_WIDTH: usize = 6; // ` repo `
    let mut spans = Vec::new();
    let short_id = truncate_to_width_no_marker(&repo.short_id, SHORT_ID_COLUMN_WIDTH);
    spans.push(Span::styled(
        format!("{short_id:<SHORT_ID_COLUMN_WIDTH$}  "),
        Style::default().fg(theme.secondary_text),
    ));
    spans.push(Span::styled(
        format!("{:<KIND_BADGE_WIDTH$}", " repo "),
        Style::default()
            .fg(theme.secondary_text)
            .add_modifier(theme.badge),
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        repo.display_name.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    if let Some(path) = repo
        .canonical_path
        .as_deref()
        .filter(|p| !p.is_empty() && *p != repo.display_name)
    {
        spans.push(Span::styled(
            format!("  {path}"),
            Style::default()
                .fg(theme.secondary_text)
                .add_modifier(Modifier::DIM),
        ));
    }
    spans
}

fn render_mux_session_spans(
    mux: &MuxSessionRow,
    theme: &Theme,
    now: i64,
    width: usize,
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
    spans.push(Span::styled(
        pad_to_width(truncate_to_width_strict(&label, label_width), label_width),
        Style::default().fg(theme.link_id),
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
    let recency_style = recency_bucket(Some(now), mux.activity_epoch)
        .map(|bucket| bucket.style(theme))
        .unwrap_or_else(|| Style::default().add_modifier(theme.placeholder));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{:>4}", truncate_to_width_strict(&recency, 4)),
        recency_style,
    ));

    spans.push(Span::raw("  "));
    match mux.client_attached {
        Some(true) => {
            spans.push(Span::styled("◉", Style::default().fg(theme.mux_attached)));
        }
        Some(false) => {
            spans.push(Span::styled(
                "◯",
                Style::default().add_modifier(theme.mux_unmuxed),
            ));
        }
        None => {
            spans.push(Span::styled(
                "?",
                Style::default().add_modifier(theme.placeholder),
            ));
        }
    }
    if mux.ambiguous_count > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("◐", Style::default().fg(theme.mux_ambiguous)));
    }
    if mux.pin_id.is_some() {
        // Bound-pin marker on the mux row. Same glyph the agent-
        // session row uses (ui.rs:1027) so pin-bound muxes scan
        // the same way pin-bound sessions do.
        spans.push(Span::styled(
            "  📌",
            Style::default().add_modifier(theme.placeholder),
        ));
    }

    append_mux_single_session_preview(&mut spans, mux, theme, width);
    fit_spans_to_width(spans, width)
}

fn append_mux_agent_labels(
    spans: &mut Vec<Span<'static>>,
    mux: &MuxSessionRow,
    theme: &Theme,
    width: usize,
) {
    use crate::tui::widgets::badge::harness_badge;

    if mux.agent_labels.is_empty() {
        spans.push(Span::styled(
            " no agent ".to_string(),
            Style::default().add_modifier(theme.placeholder),
        ));
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
        spans.push(Span::styled(
            format!(" +{hidden_labels}"),
            Style::default().fg(theme.secondary_text),
        ));
    }
}

fn mux_label_column_width(width: usize) -> usize {
    match width {
        0..=38 => width.saturating_sub(14).clamp(10, 18),
        39..=72 => 20,
        73..=104 => 24,
        _ => 30,
    }
}

fn append_mux_single_session_preview(
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
    spans.push(Span::styled(
        truncate_to_width(preview, width - used - 2),
        Style::default()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::ITALIC),
    ));
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn render_candidate_spans(candidate: &MuxCandidateRow, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let glyph = if candidate.is_preferred {
        Span::styled("◉ ", Style::default().fg(theme.mux_attached))
    } else {
        Span::styled("◯ ", Style::default().add_modifier(theme.mux_unmuxed))
    };
    spans.push(glyph);
    spans.push(Span::raw(compact_mux_label(&candidate.mux_label)));
    if candidate.is_preferred {
        spans.push(Span::styled(
            "  (preferred)".to_string(),
            Style::default().add_modifier(theme.placeholder),
        ));
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
fn node_kind_glyph_span(kind: NodeKind, theme: &Theme) -> Span<'static> {
    let style = node_kind_style(kind, theme);
    Span::styled(
        format!("{} ", style.glyph),
        Style::default().fg(style.color),
    )
}

/// Variant of [`node_kind_glyph_span`] for PR rows: emits the
/// `NodeKind::ForgePr` glyph styled with the appropriate `theme.pr_*`
/// color based on PR state. `is_draft` overrides the state-based hue
/// because the draft flag is independent of the open/closed/merged
/// label in our model.
fn forge_pr_glyph_span(pr: &PrRow, theme: &Theme) -> Span<'static> {
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
    Span::styled(format!("{} ", style.glyph), Style::default().fg(color))
}

/// Map a [`GroupRow`] to its node kind so the renderer can stamp the
/// row with the glyph slate (ADR 0073). Synthetic group buckets
/// without a backing node return `None` and the renderer skips the
/// glyph prefix for that row.
fn group_node_kind(group: &GroupRow) -> Option<NodeKind> {
    group.primary_node.as_ref().map(NodeKind::from)
}

/// Compute the per-row node-kind glyph span for any [`RowKind`], or
/// `None` when the row already carries an identity signal (the
/// colored harness pill on `AgentSession`) or when no graph node
/// kind backs it (synthetic group buckets, sentinel `Pin` rows).
/// Folds the per-row dispatch the `render_left_row` body and the
/// `group_row_body_width` pre-pass both rely on through a single
/// helper so the two paths agree on row widths.
fn row_kind_glyph_span(kind: &RowKind, theme: &Theme) -> Option<Span<'static>> {
    let node_kind = match kind {
        RowKind::Group(group) => group_node_kind(group)?,
        // ADR 0073 amendment (2026-06): AgentSession rows already
        // carry the colored harness pill as their identity. Stacking
        // a separate `●` glyph next to it doubled the signal in
        // practice without adding information. The pill stands alone
        // in row contexts. `NodeKind::AgentSession`'s glyph + color
        // remain defined for detail-pane and explorer surfaces
        // (H-VIS-004) where no pill is rendered.
        RowKind::AgentSession(_) => return None,
        RowKind::AgentSessionMuxCandidate(_) => NodeKind::MuxSession,
        RowKind::MuxSession(_) => NodeKind::MuxSession,
        RowKind::Pr(pr) => return Some(forge_pr_glyph_span(pr, theme)),
        RowKind::Fork(_) => NodeKind::Fork,
        RowKind::Pin(_) => return None,
        RowKind::Repo(_) => NodeKind::Repo,
    };
    Some(node_kind_glyph_span(node_kind, theme))
}

fn mux_indicator_span(state: MuxIndicator, theme: &Theme) -> Span<'static> {
    // ADR 0072: the row chip is attachable-binary. `◉` answers "yes,
    // there is a definitive mux here that `Enter`/`a` will attach to."
    // Both `Ambiguous` and `Unmuxed` fail that test and share the `◯`
    // glyph — ambiguity surfaces only on the enclosing group row.
    match state {
        MuxIndicator::Attached => Span::styled("◉", Style::default().fg(theme.mux_attached)),
        MuxIndicator::Ambiguous { .. } | MuxIndicator::Unmuxed => {
            Span::styled("◯", Style::default().add_modifier(theme.mux_unmuxed))
        }
    }
}

fn disclosure_span(row: &crate::tui::rows::Row, app: &App) -> Span<'static> {
    if !row.expandable {
        return Span::raw("  ");
    }
    let glyph = if app.is_expanded(&row.id) {
        "▼ "
    } else {
        "▶ "
    };
    Span::styled(glyph, Style::default().fg(app.theme().disclosure))
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

fn truncate_to_width_strict(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut out = String::with_capacity(width);
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width - 1 {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

fn truncate_to_width_no_marker(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut out = String::with_capacity(width);
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out
}

fn pad_to_width(mut text: String, width: usize) -> String {
    let used = UnicodeWidthStr::width(text.as_str());
    if used < width {
        text.push_str(&" ".repeat(width - used));
    }
    text
}

fn fit_spans_to_width(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let mut used = 0;
    let mut out = Vec::with_capacity(spans.len());
    for span in spans.drain(..) {
        let span_width = UnicodeWidthStr::width(span.content.as_ref());
        if used + span_width <= width {
            used += span_width;
            out.push(span);
            continue;
        }
        let remaining = width.saturating_sub(used);
        if remaining > 0 {
            let style = span.style;
            out.push(Span::styled(
                truncate_to_width_strict(span.content.as_ref(), remaining),
                style,
            ));
        }
        break;
    }
    out
}

fn compact_path_label(path: &str) -> String {
    if path == "Ungrouped" {
        return path.to_string();
    }
    // Workspace group rows render via `format_workspace_display`,
    // which joins `<label>  <members>  (<provider>)` with double-
    // space separators. Treat the first segment as the bold label
    // so workspace headers don't bold the member list or provider
    // chip (which the eye reads as metadata, parallel to a repo's
    // CWD path).
    if let Some((label, _)) = path.split_once("  ") {
        return label.to_string();
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
    // Workspace-shape: everything after the first `  ` is the
    // non-bold member-list / provider segment. Render it as-is so
    // the eye still picks out the segment separators
    // `format_workspace_display` inserted.
    if let Some((_, rest)) = path.split_once("  ") {
        return rest.to_string();
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
        return compact_mux_native_id(label);
    };
    format!("{backend}:{}", compact_mux_native_id(native))
}

/// Head…tail truncation for long mux native ids. Long pane/session
/// ids would otherwise dominate the row; the head/tail shape keeps
/// both ends recognizable at a glance.
fn compact_mux_native_id(native: &str) -> String {
    if native.chars().count() <= 36 {
        return native.to_string();
    }
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
}

// -----------------------------------------------------------------------------
// Right panel
// -----------------------------------------------------------------------------

fn draw_right_panel(app: &App, frame: &mut Frame<'_>, area: Rect) {
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

    // T8-029: when the graph explorer state is available, render
    // the new mockup layout (Node + Upstream + Downstream sections
    // with cursor highlight) on top. Falls back to the legacy
    // section-grouped detail when the explorer state isn't ready
    // yet (race during the first SetData).
    if let Some(state) = app.explorer() {
        let lines = render_explorer_lines(
            state,
            inner.width as usize,
            app.theme(),
            app.edge_meta_visible(),
        );
        // Account for Paragraph wrap: any logical line whose
        // displayed width exceeds the pane width consumes extra
        // terminal rows. Without the wrap-aware estimate the
        // Upstream / Downstream sections get clipped when the Node
        // zone carries a long path or native id.
        let wrapped_rows: usize = lines
            .iter()
            .map(|line| {
                let width = line
                    .spans
                    .iter()
                    .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                    .sum::<usize>()
                    .max(1);
                width.div_ceil(inner.width.max(1) as usize).max(1)
            })
            .sum();
        // +1 safety margin: the per-line `div_ceil` count assumes
        // the renderer packs each line tight to the right edge, but
        // Paragraph wraps on word boundaries and a long unbroken
        // token can push the actual row count one above the
        // estimate. Without the slack the last explorer line
        // (typically a single-link composite or the trailing Down-
        // stream row) gets clipped when the Node zone carries a
        // very long value.
        let header_height = (wrapped_rows as u16)
            .saturating_add(1)
            .min(inner.height.saturating_sub(3))
            .max(3);
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), split[0]);
        frame.render_widget(
            Paragraph::new(preview_divider_line(
                app,
                split[1].width as usize,
                app.theme(),
            )),
            split[1],
        );
        draw_explorer_preview(app, state, frame, split[2]);
        return;
    }

    let mux_runtime = mux_runtime_rows(app);
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            // Give the header exactly the height its fields need
            // (title + blank + one row per HeaderField), clamped
            // so the preview zone keeps a minimum of two rows.
            Constraint::Length(header_zone_height(
                detail,
                app.detail_links_expanded(),
                &mux_runtime,
                inner.height,
                inner.width,
            )),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

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

/// Render the related-entities layout (ADR 0074). Validated rows
/// (resolver winners) sit in a flat list under one `Related` chip
/// divider; alternates, conflicts, and unresolved stubs collapse
/// under a single `Other` header below the fold. Each row reads as
/// `<verb-column 22w> <kind-glyph> <neighbor_label>`; direction
/// flows from the verb, not from section placement.
fn render_explorer_lines(
    state: &crate::tui::app::ExplorerState,
    width: usize,
    theme: &Theme,
    show_edge_meta: bool,
) -> Vec<Line<'static>> {
    use crate::tui::explorer::ExplorerRow;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let rows = state.rows();
    let cursor = state.cursor;
    let view = &state.view;

    for (idx, field) in view.fields(state.full_detail_expanded).iter().enumerate() {
        let flat_index = rows
            .iter()
            .position(|row| matches!(row, ExplorerRow::NodeField { index, .. } if *index == idx));
        let highlight = flat_index == Some(cursor);
        lines.push(render_node_field_line(field, highlight, theme));
    }

    if view.relationships.groups.is_empty() {
        return lines;
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
        match row {
            ExplorerRow::NodeField { .. } => {}
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
                    ));
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
                    ));
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
                }
            }
        }
    }
    lines
}

fn render_node_field_line(
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
    let label = Span::styled(
        format!("  {:<14}", field.label),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let mut spans = vec![label, Span::styled(field.value.clone(), style)];
    if let Some(kind) = field.kind_chip {
        spans.push(Span::raw(" "));
        spans.push(kind_chip_span(kind, theme));
    }
    if field.long_value.is_some() {
        spans.push(Span::styled(
            "  (truncated · o)".to_string(),
            Style::default().add_modifier(theme.placeholder),
        ));
    }
    if let Some(annotation) = field.annotation {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            annotation.to_string(),
            Style::default().fg(theme.warning),
        ));
    }
    Line::from(spans)
}

/// Per-kind glyph chip used to surface the graph node kind next to a
/// value (T8-039) or beside a relationship-explorer neighbor. ADR
/// 0073 §3 replaces the prior dim `[kind]` text with a 1-cell glyph
/// in the node-kind color so the chip carries identity at a glance.
/// Unknown kind strings (none in the slate) fall back to a dim `?`
/// so the chip slot stays visible without misleading the operator.
///
/// `ForgePr`'s glyph color reuses `theme.pr_open` here because the
/// chip-rendering surfaces do not carry PR state down — the row
/// tree's [`forge_pr_glyph_span`] handles state-aware coloring
/// directly off the row.
fn kind_chip_span(kind: &str, theme: &Theme) -> Span<'static> {
    let Some(node_kind) = NodeKind::from_snake_case(kind) else {
        return Span::styled(
            "?".to_string(),
            Style::default().add_modifier(theme.placeholder),
        );
    };
    let style = node_kind_style(node_kind, theme);
    let color = if matches!(node_kind, NodeKind::ForgePr) {
        theme.pr_open
    } else {
        style.color
    };
    Span::styled(style.glyph, Style::default().fg(color))
}

/// Width of the verb column in row lines (ADR 0074 §3 mockup). The
/// verb takes the place section labels (`Upstream`, `Downstream`)
/// used to hold; direction reads off the verb itself.
const VERB_COLUMN_WIDTH: usize = 22;

/// Render a validated zone row: `<verb 22w> <glyph> <neighbor_label>`
/// (ADR 0074 §3, ADR 0075). The prior `★` resolver-winner marker
/// is dropped — every validated row is a resolver winner by
/// construction, so the marker no longer earns its column.
fn render_validated_link_line(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
) -> Line<'static> {
    render_related_row(
        group,
        link,
        highlight,
        theme,
        show_edge_meta,
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
fn render_other_link_line(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
) -> Line<'static> {
    use crate::tui::explorer::EdgeStateLabel;
    let (indent, prefix_span, label_style) = match link.edge_state {
        EdgeStateLabel::Conflict => {
            let prefix = Span::styled(
                "⚠ ".to_string(),
                Style::default()
                    .fg(theme.edge_conflict)
                    .add_modifier(Modifier::BOLD),
            );
            (
                "    ",
                Some(prefix),
                Style::default()
                    .fg(theme.edge_conflict)
                    .add_modifier(Modifier::BOLD),
            )
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
        indent,
        prefix_span,
        label_style,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_related_row(
    group: &crate::tui::explorer::RelationshipGroup,
    link: &crate::tui::explorer::RelationshipLink,
    highlight: bool,
    theme: &Theme,
    show_edge_meta: bool,
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
    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some(mut prefix) = prefix {
        if highlight {
            prefix.style = prefix.style.add_modifier(Modifier::REVERSED);
        }
        spans.push(prefix);
    }
    spans.push(Span::styled(verb_text, row_style));
    spans.push(kind_chip);
    spans.push(Span::raw(" "));
    spans.push(Span::styled(link.neighbor_label.clone(), label_style));
    if show_edge_meta {
        let trailing = format!(
            "  · {} · {} · {}",
            link.provenance.snake_case(),
            link.confidence.snake_case(),
            link.state.snake_case(),
        );
        spans.push(Span::styled(trailing, label_style));
    }
    Line::from(spans)
}

/// `▶ Other (N · K ⚠ · L —)` / `▼ Other (N · K ⚠ · L —)` row that
/// gates the alternates / conflicts / unresolved zone (ADR 0074 §3).
fn render_other_header_line(
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
    Line::from(Span::styled(text, style))
}

/// Render an Other-zone unresolved-evidence row. Shape mirrors
/// [`render_other_link_line`] but the right side carries the
/// evidence summary instead of a neighbor id.
fn render_other_unresolved_line(
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
    Line::from(Span::styled(text, style))
}

fn unresolved_evidence_summary(row: &crate::tui::explorer::UnresolvedRow) -> String {
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
fn draw_explorer_preview(
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
        } => {
            lines.push(Line::from(Span::styled(
                format!("  neighbor    {neighbor_label}"),
                Style::default().add_modifier(Modifier::BOLD),
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
                Span::styled(
                    format!("  {:<14}", "edge"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(edge_value, Style::default().fg(theme.warning)),
            ]));
        }
        RowPreview::Unresolved {
            node_type,
            evidence,
            provenance,
            confidence,
            state: link_state,
        } => {
            lines.push(Line::from(Span::styled(
                format!("  unresolved  {node_type}"),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for (label, value) in [
                ("harness_key", evidence.harness_key.as_deref()),
                ("native_id", evidence.native_id.as_deref()),
                ("state_scope", evidence.state_scope.as_deref()),
                ("path", evidence.path.as_deref()),
            ] {
                if let Some(value) = value {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {:<14}", label),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(value.to_string()),
                    ]));
                }
            }
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", "edge"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "{} · {} · {}",
                        provenance.snake_case(),
                        confidence.snake_case(),
                        link_state.snake_case(),
                    ),
                    Style::default().add_modifier(theme.placeholder),
                ),
            ]));
        }
    }
    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll(), 0));
    frame.render_widget(widget, area);
}

fn empty_right_panel_text(app: &App) -> &'static str {
    if app.graph_db().is_none() {
        "Loading…"
    } else {
        "Select a row to view its detail."
    }
}

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
fn header_zone_height(
    detail: &NodeDetail,
    expand_linked: bool,
    extra_mux_rows: &[HeaderField],
    panel_height: u16,
    panel_width: u16,
) -> u16 {
    let natural = count_detail_lines(
        detail.kind_label,
        &detail.header_fields,
        extra_mux_rows,
        expand_linked,
        panel_width,
        0,
    ) as u16;
    let max = panel_height.saturating_sub(3);
    natural.min(max).max(3)
}

/// Mirrors [`emit_detail_section_lines`] so the layout budget tracks
/// exactly what the renderer will emit — section dividers, wrap-aware
/// field rows, runtime extras, and (when expanded) the recursive
/// sub-detail with its own dividers and indent. Keeping the two
/// functions structurally identical is how the wrap-aware fix avoids
/// re-introducing the "Session row clipped" class of bug.
fn count_detail_lines(
    kind_label: &'static str,
    fields: &[HeaderField],
    extra_mux_rows: &[HeaderField],
    expand_linked: bool,
    panel_width: u16,
    indent: usize,
) -> usize {
    let sections = crate::tui::detail::group_fields_into_sections(kind_label, fields);
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
                && let Some(sub_kind) = field.expanded_kind_label
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
fn header_field_line_count(field: &HeaderField, panel_width: u16, indent: usize) -> usize {
    const LABEL_WIDTH: usize = 10;
    let effective_width = (panel_width as usize).saturating_sub(indent);
    if effective_width == 0 {
        return 1;
    }
    let value_width = UnicodeWidthStr::width(field.value.as_str());
    let annotation_width = field
        .annotation
        .map(|annotation| 1 + UnicodeWidthStr::width(annotation))
        .unwrap_or(0);
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

fn draw_detail_header(
    detail: &NodeDetail,
    expand_linked: bool,
    extra_mux_rows: &[HeaderField],
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
) {
    let lines = emit_detail_section_lines(
        detail.kind_label,
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
fn emit_detail_section_lines(
    kind_label: &'static str,
    fields: &[HeaderField],
    extra_mux_rows: &[HeaderField],
    expand_linked: bool,
    panel_width: usize,
    indent: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let sections = crate::tui::detail::group_fields_into_sections(kind_label, fields);
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
                && let Some(sub_kind) = field.expanded_kind_label
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
fn mux_runtime_rows(app: &App) -> Vec<HeaderField> {
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
        expanded_kind_label: None,
        expanded_fields: Vec::new(),
    }]
}

/// Short-form elapsed-duration formatter used by the runtime Mux
/// rows. Mirrors the recency formatter in `rows/mod.rs` but takes
/// a `Duration::as_secs` payload rather than an epoch delta so the
/// preview cache's `Instant::elapsed` value plugs in directly.
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

/// Right-anchored labeled rule used between detail sections. Wraps
/// [`chip_divider_line`] with the section's static label.
fn section_divider_line(kind: SectionKind, width: usize, theme: &Theme) -> Line<'static> {
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
/// flush right (T8-041) — keeps the bold zone label easy to scan
/// vertically when Upstream / Downstream summaries grow.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ChipAnchor {
    Left,
    Right,
}

fn chip_divider_line(
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
    let chip_span = Span::styled(
        chip_text,
        Style::default()
            .fg(theme.panel_focus_accent)
            .add_modifier(theme.badge),
    );
    let suffix_span = (suffix_width > 0)
        .then(|| Span::styled(suffix_text, Style::default().fg(theme.secondary_text)));
    let mut spans = Vec::with_capacity(4);
    if leading_rule > 0 {
        spans.push(Span::styled(
            "─".repeat(leading_rule),
            Style::default().add_modifier(theme.divider),
        ));
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
    spans.push(Span::styled(
        "─".repeat(trailing_rule),
        Style::default().add_modifier(theme.divider),
    ));
    Line::from(spans)
}

fn render_header_field(field: &HeaderField, section: SectionKind, theme: &Theme) -> Line<'static> {
    let label = Span::styled(
        format!("{:<10}", field.label),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let value_style = field_value_style(section, field, theme);
    let mut spans = vec![label, Span::styled(field.value.clone(), value_style)];
    if let Some(annotation) = field.annotation {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            annotation.to_string(),
            Style::default().fg(theme.warning),
        ));
    }
    Line::from(spans)
}

/// Per-(section, field-label) colorization per ADR 0033's mapping.
/// Placeholders always inherit `theme.placeholder` regardless of
/// section so the dim treatment stays consistent across the pane.
fn field_value_style(section: SectionKind, field: &HeaderField, theme: &Theme) -> Style {
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
fn pr_value_style(value: &str, theme: &Theme) -> Style {
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
        RowKind::Pin(_) => {
            let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
            if diagnostics.is_empty() {
                Text::raw("pin diagnostic unavailable — try `r` to refresh")
            } else {
                Text::raw(render_pin_diagnostics(&diagnostics))
            }
        }
        _ => match selection {
            RowId::Group(NodeId::MuxSession(_)) => mux_preview_text(app, live_preview, height),
            RowId::MuxSession(NodeId::MuxSession(_)) => mux_preview_text(app, live_preview, height),
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

fn render_pin_diagnostics(diagnostics: &[crate::tui::actions::PinDiagnosticView]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| match diagnostic {
            crate::tui::actions::PinDiagnosticView::Unbound {
                pin_id,
                expected_mux_native_id,
                last_session,
            } => match last_session {
                Some(last) => format!(
                    "Pin `{pin_id}` is unbound.\nExpected mux: {expected_mux_native_id}\n\
                     Last session: {} (observed {})\n\
                     Enter resumes into the recorded session.",
                    last.session_id, last.observed_epoch
                ),
                None => format!(
                    "Pin `{pin_id}` is unbound.\nExpected mux: {expected_mux_native_id}\n\
                     Enter launches the pin."
                ),
            },
            crate::tui::actions::PinDiagnosticView::StaleMux { pin_id, mux } => format!(
                "Pin `{pin_id}` has a stale mux.\nMux: {}\nEnter relaunches the harness in the existing mux.",
                mux.native_id
            ),
            crate::tui::actions::PinDiagnosticView::Ambiguous {
                pin_id,
                chosen,
                competing,
            } => {
                let competing = competing
                    .iter()
                    .map(|id| format!("{}:{}", id.harness_key, id.session_key))
                    .collect::<Vec<_>>()
                    .join("\n  ");
                format!(
                    "Pin `{pin_id}` is ambiguous.\nChosen: {}:{}\nCompeting sessions:\n  {competing}\nPress b for the bind command.",
                    chosen.harness_key, chosen.session_key
                )
            }
            crate::tui::actions::PinDiagnosticView::Drift {
                pin_id,
                declared_cwd,
                observed_cwd,
            } => format!(
                "Pin `{pin_id}` has cwd drift.\nDeclared cwd: {declared_cwd}\nObserved cwd: {observed_cwd}\nBinding still holds."
            ),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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

fn contextual_status_text(app: &App) -> String {
    let focus_hint = match app.focus() {
        Focus::Left => "j/k move · h/l fold · Enter default",
        // T8-029: with the right pane focused, j/k drive the graph
        // explorer cursor (T8-028), Enter drills or expands a group
        // depending on the cursor position, `e` toggles a group,
        // and Backspace pops the breadcrumb stack.
        Focus::Right => "j/k cursor · Enter drill/expand · e group · ⌫ back",
    };
    let action_hint = default_action_status_hint(app);
    format!("{action_hint} · {focus_hint} · Tab focus · r refresh · q quit")
}

/// Status-bar action hint that advertises `Enter` as the primary
/// default action on the selected row (T8-043), with the legacy
/// single-key accelerator (`a` / `v`) listed alongside. Falls back
/// to the attach-disabled reason for rows that have neither a mux
/// target nor a viewable transcript so the operator still sees a
/// one-line "disabled because …" cue.
fn default_action_status_hint(app: &App) -> String {
    let selection = match app.selection() {
        Some(id) => id,
        None => return attach_disabled_reason(&crate::tui::actions::AttachDisabled::NoSelection),
    };
    let row = match app.tree().rows.iter().find(|r| &r.id == selection) {
        Some(row) => row,
        None => return attach_disabled_reason(&crate::tui::actions::AttachDisabled::NoSelection),
    };
    // Group rows: Enter expands/collapses; h/l explicitly fold.
    if matches!(row.kind, RowKind::Group(_)) {
        return "Enter/l expand · h collapse".to_string();
    }
    // Pin rows surface a per-binding-state hint (ADR 0057 / H-PIN-018).
    if let RowKind::Pin(pin) = &row.kind {
        let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
        let has_b = if let Some(hint) = crate::tui::actions::pin_status_hint(&diagnostics) {
            let has_binding = hint.contains(" b bind");
            if has_binding {
                return format!("{hint} · Del remove");
            }
            return format!("{hint} · b bind · Del remove");
        } else {
            false
        };
        let launch_hint = match pin.state_label {
            "stale-mux" => format!(
                "Enter to relaunch `{}` in existing mux `{}`",
                pin.display_name, pin.mux_label
            ),
            "bound" => format!(
                "Enter to attach `{}` via mux `{}`",
                pin.display_name, pin.mux_label
            ),
            _ => format!("Enter to launch `{}`", pin.display_name),
        };
        if has_b {
            return format!("{launch_hint} · b bind · Del remove");
        }
        return format!("{launch_hint} · b bind · Del remove");
    }
    if let RowKind::AgentSession(session) = &row.kind
        && session.pin_id.is_some()
    {
        let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
        if let Some(hint) = crate::tui::actions::pin_status_hint(&diagnostics) {
            if !hint.contains(" b bind") {
                return format!("{hint} · b bind");
            }
            return hint;
        }
    }
    match resolve_attach_target(app) {
        Ok(target) => {
            let label = target_label(&target);
            match selected_mux_state(app) {
                Some(MuxIndicator::Ambiguous { .. }) => {
                    format!("Enter/a attach preferred {label} · m choose")
                }
                _ => format!("Enter/a attach {}", compact_mux_label(&label)),
            }
        }
        Err(reason) => {
            // Un-muxed agent session rows still have a viewer-based
            // default action — `Enter` opens the transcript, `v`
            // does the same. Advertise that primary action instead
            // of the attach-disabled reason.
            if let RowKind::AgentSession(session) = &row.kind
                && matches!(session.mux_state, MuxIndicator::Unmuxed)
            {
                let resume = crate::tui::resume::resolve_resume_target(&session.session);
                if matches!(resume, crate::tui::resume::ResumeTarget::Launch { .. }) {
                    return format!("Enter/v view {} · S resume", compact_session_label(session));
                }
                return format!("Enter/v view {}", compact_session_label(session));
            }
            attach_disabled_reason(&reason)
        }
    }
}

/// Compact label for an agent session row used in the status hint.
/// Shows the alias / title when one is set; otherwise falls back to
/// `harness:short_id` so the operator can still tell which row Enter
/// will act on.
fn compact_session_label(session: &AgentSessionRow) -> String {
    if let Some(label) = session.display_label() {
        return label.to_string();
    }
    format!("{}:{}", session.harness_label, session.short_id)
}

fn selected_mux_state(app: &App) -> Option<MuxIndicator> {
    let selection = app.selection()?;
    let row = app.tree().rows.iter().find(|row| &row.id == selection)?;
    match &row.kind {
        RowKind::AgentSession(session) => Some(session.mux_state),
        _ => None,
    }
}

/// Build the chip-style divider above the preview body. Sits in
/// the same right-anchored position as the section dividers
/// (Session / Mux / PR / Lineage) so the `[ Preview ]` chip lines
/// up below them. No inline suffix: the mux pane label and
/// captured-time freshness already render in the Mux section
/// above, and a second copy here pushed the chip far to the left.
fn preview_divider_line(_app: &App, width: usize, theme: &Theme) -> Line<'static> {
    chip_divider_line("Preview", None, width, theme, ChipAnchor::Left)
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
    use crate::filter::RowFilter;
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
        RepoId, RepoNode,
    };
    use crate::resolve::resolve_snapshot;
    use crate::tui::SessionsGrouping;
    use crate::tui::app::Msg;
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
    use crate::tui::{RunConfig, View};

    fn seeded_app() -> App {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
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
                last_active_epoch: None,
                session_kind: None,
            }));
        let snapshot = resolve_snapshot(snapshot);

        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
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
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
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
                last_active_epoch: None,
                session_kind: None,
            }));
        let mux_graph_id = MuxSessionId::new(native_id);
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_graph_id.clone(),
            backend: "tmux".to_string(),
            native_id: native_id.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
            filter: RowFilter::default(),
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
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

    fn pinned_app(binding: crate::model::PinBinding) -> App {
        use crate::model::{PinCandidate, PinMuxRef, Provenance};
        let mut snapshot = GraphSnapshot::empty();
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/home/op/src/proj".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/tmp/.conspectus.toml".to_string(),
            binding: Some(binding),
        });
        // Skip `resolve_snapshot` here — it would overwrite the
        // explicit binding state with whatever the resolver derives
        // from the empty live evidence. Builder consumes the
        // pre-bound snapshot directly.
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: crate::tui::app::GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        // Step to the synthetic Pins group header, then to the pin row.
        let pin_row = app
            .tree()
            .rows
            .iter()
            .find(|r| matches!(&r.id, crate::tui::rows::RowId::Pin { .. }))
            .map(|r| r.id.clone())
            .expect("pin row emitted");
        app.set_selection(pin_row);
        app
    }

    #[test]
    fn status_hint_for_unbound_pin_advertises_enter_to_launch() {
        let app = pinned_app(crate::model::PinBinding::Unbound);
        let hint = default_action_status_hint(&app);
        assert!(
            hint.contains("Enter to launch") && hint.contains("ingest"),
            "unexpected hint for unbound pin: {hint}"
        );
    }

    #[test]
    fn status_hint_for_stale_mux_pin_advertises_relaunch_in_existing_mux() {
        let app = pinned_app(crate::model::PinBinding::StaleMux {
            mux: crate::model::MuxSessionId::new("tmux:ingest"),
        });
        let hint = default_action_status_hint(&app);
        assert!(
            hint.contains("relaunch") && hint.contains("existing mux"),
            "unexpected hint for stale-mux pin: {hint}"
        );
    }

    #[test]
    fn pin_diagnostic_preview_lists_ambiguous_competitors() {
        let text = render_pin_diagnostics(&[crate::tui::actions::PinDiagnosticView::Ambiguous {
            pin_id: "ingest".to_string(),
            chosen: AgentSessionId::new("codex", "/state", "alpha"),
            competing: vec![
                AgentSessionId::new("codex", "/state", "beta"),
                AgentSessionId::new("codex", "/state", "gamma"),
            ],
        }]);

        assert!(text.contains("Pin `ingest` is ambiguous."));
        assert!(text.contains("Chosen: codex:alpha"));
        assert!(text.contains("codex:beta"));
        assert!(text.contains("codex:gamma"));
        assert!(text.contains("Press b for the bind command."));
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
            "same-line preview missing: {text}"
        );
        assert!(
            text.contains("Phase 8 walkthrough"),
            "right-panel title row missing: {text}"
        );
        assert!(text.contains(" Preview "), "preview chip missing: {text}");
    }

    #[test]
    fn header_renders_per_harness_and_per_mux_state_chips_at_wide_width() {
        // Phase 5: dense header. With a single codex session that's
        // un-muxed, the chip row should carry one `[codex] 1` chip
        // plus the three mux-state glyphs (◉/◐/◯) with counts.
        let app = seeded_app();
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let header = text.lines().next().expect("header line");

        assert!(header.contains("codex"), "harness chip missing: {header}");
        assert!(header.contains("◉"), "mux attached glyph missing: {header}",);
        assert!(
            header.contains("◐"),
            "mux ambiguous glyph missing: {header}",
        );
        assert!(header.contains("◯"), "mux unmuxed glyph missing: {header}",);
        // One codex session, all three mux counts are visible.
        assert!(header.contains(" 1"), "codex count missing: {header}");
    }

    #[test]
    fn header_falls_back_to_prefix_only_at_very_narrow_width() {
        // When the terminal is narrower than the chip section can
        // afford, the header collapses to just the prefix (no chips).
        // The existing "N agents · M mux" tail still shows so the
        // operator sees their counts even without the chip detail.
        let app = seeded_app();
        let area = Rect::new(0, 0, 40, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let header = text.lines().next().expect("header line");

        assert!(
            !header.contains('◉') && !header.contains('◐') && !header.contains('◯'),
            "mux chips should be dropped at narrow width: {header}",
        );
    }

    #[test]
    fn left_panel_title_renders_a_view_tab_strip() {
        // Phase 11: the left pane title lists every view as a tab
        // strip (sessions · mux · union · prs · forks) with the
        // active one accented. Operators see the available views at
        // a glance instead of having to remember the 1–5
        // accelerators.
        let app = seeded_app();
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let title_line = text
            .lines()
            .find(|l| l.contains("union") && l.contains("forks"))
            .expect("left pane tab strip line present");
        for label in ["sessions", "mux", "union", "prs", "forks"] {
            assert!(
                title_line.contains(label),
                "tab strip missing `{label}`: {title_line}",
            );
        }
    }

    #[test]
    fn right_panel_title_names_the_selected_node_kind() {
        // Phase 11: instead of a constant `detail`, the right pane
        // title carries the kind of node currently being inspected
        // — `session` for an agent session, `repo` / `mux` / `pr` /
        // etc. for other kinds — so the operator can tell what
        // they're looking at without re-reading the body.
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let title_line = text
            .lines()
            .find(|l| l.contains("session ") && l.contains("───"))
            .expect("right-pane title with session kind");
        assert!(
            !title_line.contains("detail"),
            "right title should drop the generic `detail` label: {title_line}",
        );
    }

    #[test]
    fn focus_marker_prefixes_only_the_active_pane_title() {
        // Phase 9 refinement: focus is signaled by a `▸ ` glyph on
        // the active pane's title rather than by holistically
        // styling the panel. The inactive pane gets two-space
        // padding so titles align column-wise and content colors
        // stay untouched between focus states. The right-pane label
        // varies by selection kind (Phase 11) so the assertion is
        // on the *count* of `▸` markers, not on a specific suffix.
        let area = Rect::new(0, 0, 160, 24);
        let app = seeded_app();
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert_eq!(
            text.matches('▸').count(),
            1,
            "exactly one focus marker should be visible: {text}",
        );
        let header_band = text.lines().take(3).collect::<Vec<_>>().join("\n");
        let left_border = header_band
            .lines()
            .nth(1)
            .map(|l| l.split('│').next().unwrap_or(""))
            .unwrap_or("");
        assert!(
            left_border.contains('▸'),
            "marker should sit in the left pane's title when left is focused: \
             left border was `{left_border}` and full header was:\n{header_band}",
        );

        let mut app = seeded_app();
        app.update(Msg::CycleFocus);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert_eq!(
            text.matches('▸').count(),
            1,
            "exactly one focus marker should still be visible after CycleFocus: {text}",
        );
        // After CycleFocus the marker moves from the left tab strip
        // to the right pane title — its column position shifts past
        // the panel split (well to the right of column 0).
        let header_line = text.lines().nth(1).expect("header line");
        let marker_col = header_line.find('▸').expect("marker present");
        assert!(
            marker_col > 60,
            "marker should sit in the right pane after CycleFocus (col={marker_col}): {header_line}",
        );
    }

    fn two_repo_app() -> App {
        // Build two repos with very different name lengths so the
        // group-row body widths diverge. Each carries one session
        // so both rows participate in summary-chip rendering.
        let mut snapshot = GraphSnapshot::empty();
        for name in ["x", "very-long-project-name"] {
            let common = format!("/home/op/src/{name}");
            let repo_id = RepoId::new(common.clone());
            snapshot
                .nodes
                .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
            snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(repo_id, common.clone()),
                root: common.clone(),
                git_dir: None,
                current_branch: None,
            }));
            snapshot
                .nodes
                .push(GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "/state", name),
                    harness_key: "codex".to_string(),
                    cwd: Some(common),
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                }));
        }
        let snapshot = resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });
        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app
    }

    #[test]
    fn group_row_summary_chips_align_across_visible_groups() {
        // Two group rows with very different body widths must have
        // their `(N)` chips start at the same column so the eye can
        // scan summary state without zig-zagging across rows.
        let app = two_repo_app();
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        let short_line = text
            .lines()
            .find(|l| l.contains("~/src/x") && !l.contains("very-long"))
            .expect("short-named group row present");
        let long_line = text
            .lines()
            .find(|l| l.contains("~/src/very-long-project-name"))
            .expect("long-named group row present");
        let short_col = short_line.find("(1)").expect("short row has count chip");
        let long_col = long_line.find("(1)").expect("long row has count chip");
        assert_eq!(
            short_col, long_col,
            "(N) chips should start at the same column across group rows:\n  short: {short_line}\n  long:  {long_line}",
        );
    }

    #[test]
    fn group_row_secondary_column_starts_at_same_column_across_groups() {
        // Label column anchor: the secondary segment (path or
        // `+`-delimited member list) must start at the same column
        // on every visible group row, even when the labels have
        // very different widths.
        let app = two_repo_app();
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        let short_line = text
            .lines()
            .find(|l| l.contains("~/src/x") && !l.contains("very-long"))
            .expect("short-named group row present");
        let long_line = text
            .lines()
            .find(|l| l.contains("~/src/very-long-project-name"))
            .expect("long-named group row present");
        let short_col = short_line
            .find("~/src/x")
            .expect("short row secondary segment present");
        let long_col = long_line
            .find("~/src/very-long-project-name")
            .expect("long row secondary segment present");
        assert_eq!(
            short_col, long_col,
            "secondary segment should start at the same column across group rows:\n  short: {short_line}\n  long:  {long_line}",
        );
    }

    #[test]
    fn group_rows_carry_session_count_chip_only_when_no_ambiguity() {
        // ADR 0072: group rows show `(N)` total agents and nothing
        // else when none of their descendants are in the ambiguous
        // candidate-set state. The per-bucket muxed/unmuxed counts
        // from the original Phase 7 chip strip are gone; ambiguity
        // is surfaced only by the trailing `⚠` glyph (asserted in
        // `group_rows_show_warning_glyph_when_descendant_is_ambiguous`).
        let app = seeded_app();
        let area = Rect::new(0, 0, 160, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let group_line = text
            .lines()
            .find(|l| l.contains("~/src/proj"))
            .expect("group line present");
        assert!(
            group_line.contains("(1)"),
            "group should advertise its agent count: {group_line}",
        );
        assert!(
            !group_line.contains('◉') && !group_line.contains('◐') && !group_line.contains('⚠'),
            "non-ambiguous group should carry no per-bucket glyphs and no warning: {group_line}",
        );
    }

    #[test]
    fn group_rows_show_warning_glyph_when_descendant_is_ambiguous() {
        // ADR 0072: when any descendant session is in the
        // `Ambiguous` candidate-set state, the group row gains a
        // single `⚠` after the `(N)` count chip. The catalog of
        // ambiguous muxes lives on the group's detail pane
        // (ADR 0071); the row glyph is just the flag.
        let theme = Theme::default();
        let summary = GroupSummary {
            agents: 2,
            attached: 0,
            ambiguous: 2,
            unmuxed: 0,
        };
        let mut spans: Vec<Span<'static>> = Vec::new();
        append_group_summary_spans(&mut spans, summary, &theme, 3);
        let rendered: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(rendered.contains("(2)"), "count chip missing: {rendered}");
        assert!(rendered.contains('⚠'), "warning glyph missing: {rendered}");
        assert!(
            !rendered.contains('◉') && !rendered.contains('◐') && !rendered.contains('◯'),
            "per-bucket mux glyphs should not appear on the group summary: {rendered}",
        );

        // The unambiguous case omits the warning glyph entirely.
        let clean_summary = GroupSummary {
            agents: 2,
            attached: 2,
            ambiguous: 0,
            unmuxed: 0,
        };
        let mut clean_spans: Vec<Span<'static>> = Vec::new();
        append_group_summary_spans(&mut clean_spans, clean_summary, &theme, 3);
        let clean_rendered: String = clean_spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !clean_rendered.contains('⚠'),
            "warning should be hidden when no descendant is ambiguous: {clean_rendered}",
        );
    }

    #[test]
    fn detail_pane_omits_initial_node_divider() {
        // The right pane starts directly with the selected node's
        // fields. Later zones still render labeled dividers; empty
        // sections are suppressed entirely. ADR 0074 collapsed the
        // prior `Upstream` / `Downstream` chip dividers into one
        // `Related` chip; this test pins the new label.
        let app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            !text.contains(" Node "),
            "unexpected Node section divider label: {text}",
        );
        assert!(
            text.contains(" Related "),
            "expected Related section divider label: {text}",
        );
        assert!(
            text.contains(" Preview "),
            "expected Preview section divider label: {text}",
        );
    }

    #[test]
    fn related_row_keeps_verb_and_neighbor_label_on_one_line() {
        // ADR 0074 §3: the prior two-line composite (`relation
        // [kind]` row + indented neighbor-label row) collapses to a
        // single `<verb> <glyph> <neighbor_label>` line. The verb
        // carries the relation, the glyph carries the neighbor
        // kind, and the row is selectable as one cursor stop.
        let app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let line = text
            .lines()
            .find(|line| line.contains("attached to") && line.contains("tmux:editor"))
            .expect("validated `attached to … tmux:editor` row");
        let verb_idx = line.find("attached to").expect("verb on row");
        let label_idx = line.find("tmux:editor").expect("neighbor label on row");
        assert!(
            verb_idx < label_idx,
            "verb should sit left of the neighbor label: {line}",
        );
    }

    #[test]
    fn link_rows_hide_edge_meta_by_default_and_surface_it_after_toggle() {
        // T8-042: by default the explorer's single-link composite
        // collapses to just its header row (no `provenance ·
        // confidence · state` trailing line). After Msg::ToggleEdgeMeta
        // the trailing meta surfaces.
        let mut app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let default_text = buffer_to_string(&render_to_buffer(&app, area));
        assert!(
            !default_text.contains("discovered · "),
            "edge meta should be hidden by default: {default_text}",
        );
        // Toggle to opt-in.
        app.update(Msg::ToggleEdgeMeta);
        let toggled_text = buffer_to_string(&render_to_buffer(&app, area));
        assert!(
            toggled_text.contains("discovered · ") || toggled_text.contains("strong_discovered · "),
            "edge meta should surface after the toggle: {toggled_text}",
        );
    }

    #[test]
    fn kind_chip_span_renders_per_kind_glyph_in_node_kind_color() {
        // ADR 0073 §3: the dim `[kind]` text chip is replaced by the
        // per-kind slate glyph in the node-kind color. Every known
        // tag round-trips through `NodeKind::from_snake_case` and
        // resolves to the corresponding glyph; unknown tags fall
        // back to a dim `?` so the chip slot stays visible without
        // misleading the operator.
        let theme = Theme::default();
        let cases: [(&str, &str, ratatui::style::Color); 4] = [
            ("workspace", "▦", theme.node_workspace),
            ("mux_session", "▣", theme.node_mux_session),
            ("fork", "⑂", theme.node_fork),
            ("checkout", "◇", theme.node_checkout),
        ];
        for (tag, glyph, expected_color) in cases {
            let span = kind_chip_span(tag, &theme);
            assert_eq!(span.content.as_ref(), glyph, "wrong glyph for `{tag}`");
            assert_eq!(
                span.style.fg,
                Some(expected_color),
                "wrong color for `{tag}`"
            );
        }
        // ForgePr's kind glyph reuses `pr_open` at chip surfaces
        // because the chip layer doesn't carry PR state.
        let pr = kind_chip_span("forge_pr", &theme);
        assert_eq!(pr.content.as_ref(), "⇄");
        assert_eq!(pr.style.fg, Some(theme.pr_open));
        // Unknown tag → dim `?` fallback.
        let unknown = kind_chip_span("not_a_kind", &theme);
        assert_eq!(unknown.content.as_ref(), "?");
        assert!(unknown.style.add_modifier.contains(theme.placeholder));
    }

    #[test]
    fn right_panel_title_prefixes_kind_glyph_when_detail_resolves() {
        // ADR 0073 §3: `<glyph> <label>` in the right-panel title.
        // The glyph appears in the node-kind color; the label stays
        // bold. The muxed fixture selects an agent-session row on
        // the left, so the right-panel title reads
        // `▸ ● session ◀ …` (`AgentSession` glyph is preserved at
        // pill-less surfaces per the §3 amendment).
        let app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let agent_glyph = crate::tui::icons::NodeKind::AgentSession.default_glyph();
        let pattern = format!("{agent_glyph} session");
        assert!(
            text.lines().any(|line| line.contains(&pattern)),
            "expected `{pattern}` in right-panel title; rendered:\n{text}",
        );
    }

    #[test]
    fn related_row_orders_verb_glyph_then_neighbor_label() {
        // ADR 0073 §3 + ADR 0074 §3: each related-entities row reads
        // as `<verb 22w> <glyph> <neighbor_label>`. The glyph sits
        // between the verb column and the neighbor label so the
        // operator can scan kinds without reading the label first.
        // Pin glyph + ordering on the validated `attached to … ▣
        // tmux:editor` row that the muxed fixture produces.
        let app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let mux_glyph = crate::tui::icons::NodeKind::MuxSession.default_glyph();
        let line = text
            .lines()
            .find(|line| line.contains("attached to") && line.contains(mux_glyph))
            .expect("validated row carrying the mux-kind glyph");
        let verb_idx = line.find("attached to").expect("verb on row");
        let glyph_idx = line.find(mux_glyph).expect("kind glyph on row");
        let label_idx = line.find("tmux:editor").expect("neighbor label on row");
        assert!(
            verb_idx < glyph_idx && glyph_idx < label_idx,
            "row order should be verb → glyph → label: {line}",
        );
    }

    #[test]
    fn other_link_line_dispatches_per_edge_state() {
        // ADR 0075: Other-zone rows visually distinguish AltOf
        // (quiet) from Conflict (loud + `⚠`) so an operator
        // skimming the zone reads the resolver state at a glance.
        // Test the three shapes by constructing fake links and
        // rendering directly through `render_other_link_line` — no
        // App needed.
        use crate::model::{Confidence, NodeId, Provenance, RelationKind};
        use crate::tui::explorer::{
            CoreField, Direction, EdgeStateLabel, LinkStateLabel, RelationshipGroup,
            RelationshipLink,
        };
        let theme = Theme::default();
        let group = RelationshipGroup {
            direction: Direction::Downstream,
            relation: RelationKind::LinkedToMux,
            neighbor_kind: "mux_session".into(),
            links: Vec::new(),
            unresolved: Vec::new(),
            ambiguous: false,
            unresolved_count: 0,
        };
        let link = |edge_state: EdgeStateLabel| RelationshipLink {
            link_id: "l".into(),
            neighbor_id: NodeId::MuxSession(crate::model::MuxSessionId::new("x")),
            neighbor_kind: "mux_session",
            neighbor_label: "tmux:x".into(),
            neighbor_short_id: "x".into(),
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            state: LinkStateLabel::Active,
            resolved_winner: false,
            edge_state,
            preview: Vec::<CoreField>::new(),
        };

        // Conflict: prefix `⚠`, label color = edge_conflict, BOLD.
        let conflict_line = render_other_link_line(
            &group,
            &link(EdgeStateLabel::Conflict),
            false,
            &theme,
            false,
        );
        let text: String = conflict_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            text.starts_with("⚠ "),
            "Conflict rows must lead with the `⚠ ` prefix (NO_COLOR-safe signal): `{text}`",
        );
        let label_span = conflict_line
            .spans
            .iter()
            .find(|s| s.content.contains("tmux:x"))
            .expect("neighbor-label span present on conflict row");
        assert_eq!(label_span.style.fg, Some(theme.edge_conflict));
        assert!(label_span.style.add_modifier.contains(Modifier::BOLD));

        // AltOf: no prefix, label color = edge_alt_of, no BOLD.
        let alt_line = render_other_link_line(
            &group,
            &link(EdgeStateLabel::AltOf(RelationKind::LinkedToMux)),
            false,
            &theme,
            false,
        );
        let alt_text: String = alt_line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            !alt_text.starts_with("⚠ "),
            "AltOf rows should not carry the conflict prefix: `{alt_text}`",
        );
        let alt_label = alt_line
            .spans
            .iter()
            .find(|s| s.content.contains("tmux:x"))
            .expect("neighbor-label span present on alt row");
        assert_eq!(alt_label.style.fg, Some(theme.edge_alt_of));
        assert!(!alt_label.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn validated_link_line_drops_the_legacy_winner_star() {
        // ADR 0075: the validated zone never carries `★` — every
        // row there is a resolver winner, so the marker no longer
        // earns its column. The same fact also gets enforced
        // structurally by `render_related_row`'s removal of the
        // `resolved_winner` branch, but the buffer-level assertion
        // pins it from the operator-visible angle.
        use crate::model::{Confidence, NodeId, Provenance, RelationKind};
        use crate::tui::explorer::{
            CoreField, Direction, EdgeStateLabel, LinkStateLabel, RelationshipGroup,
            RelationshipLink,
        };
        let theme = Theme::default();
        let group = RelationshipGroup {
            direction: Direction::Downstream,
            relation: RelationKind::LinkedToMux,
            neighbor_kind: "mux_session".into(),
            links: Vec::new(),
            unresolved: Vec::new(),
            ambiguous: false,
            unresolved_count: 0,
        };
        let link = RelationshipLink {
            link_id: "l".into(),
            neighbor_id: NodeId::MuxSession(crate::model::MuxSessionId::new("x")),
            neighbor_kind: "mux_session",
            neighbor_label: "tmux:x".into(),
            neighbor_short_id: "x".into(),
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            state: LinkStateLabel::Active,
            resolved_winner: true,
            edge_state: EdgeStateLabel::Resolves,
            preview: Vec::<CoreField>::new(),
        };
        let line = render_validated_link_line(&group, &link, false, &theme, true);
        let text: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            !text.contains("★"),
            "validated row should no longer carry the `★` marker: `{text}`",
        );
    }

    #[test]
    fn related_zone_header_renders_aggregate_left_of_label() {
        // T8-041 (carried through ADR 0074 §5): the bold zone label
        // anchors flush right and the summary segment (`N validated
        // · M other …`) sits to the left of the chip on the same
        // divider line. Pin the relative ordering plus the new
        // summary vocabulary.
        let app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        let line = text
            .lines()
            .find(|line| line.contains(" Related "))
            .expect("Related divider line");
        let validated_idx = line.find("validated").expect("`validated` segment on line");
        let label_idx = line.find(" Related ").expect("Related chip on line");
        assert!(
            validated_idx < label_idx,
            "summary `validated …` should render left of the Related chip; got: {line}",
        );
    }

    #[test]
    fn detail_pane_shows_linked_to_mux_row_and_drills_into_mux() {
        // T8-029 (locked decision 8): instead of expanding linked
        // entity details in place, the explorer drills. Pressing
        // Enter on the cursor while it sits on the `linked_to_mux`
        // row should refocus the right pane on the mux node and
        // push a breadcrumb hop. The Node-zone fields then mirror
        // the mux summary (`backend · native_id`, etc.).
        let mut app = muxed_app("editor", None);
        let area = Rect::new(0, 0, 120, 24);
        let initial = buffer_to_string(&render_to_buffer(&app, area));
        assert!(
            initial.contains("attached to"),
            "session detail should expose the `attached to` row (ADR 0074 verb catalog): {initial}"
        );
        app.update(Msg::CycleFocus);
        // Walk the cursor onto the link row, then activate.
        use crate::tui::explorer::ExplorerRow;
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);
        let drilled = buffer_to_string(&render_to_buffer(&app, area));
        assert!(
            drilled.contains("backend") && drilled.contains("tmux"),
            "after drilldown the Node zone should expose the mux fields: {drilled}"
        );
        assert!(
            drilled.contains("◀"),
            "breadcrumb back-hint should surface in the right-pane title: {drilled}"
        );
        // T8-038: the breadcrumb chain should surface in compact
        // `kind:short_tag` form so the operator can see depth at a
        // glance, with a `depth N` marker so even an elided chain
        // tells the operator where they are.
        assert!(
            drilled.contains("session:"),
            "breadcrumb chain should carry the previous session in short form: {drilled}"
        );
        assert!(
            drilled.contains("depth 1"),
            "breadcrumb title should carry the depth marker: {drilled}"
        );
    }

    #[test]
    fn header_field_line_count_grows_with_value_wrap() {
        // Reproduces the "Session section vanishes" bug: the right
        // pane's `name` field for a mux with a very long native_id
        // wraps onto multiple terminal rows. Pre-fix the budget
        // counted it as a single row and clipped the Session section
        // below.
        let short = HeaderField {
            label: "name",
            value: "editor".to_string(),
            placeholder: false,
            annotation: None,
            target: None,
            expanded_kind_label: None,
            expanded_fields: Vec::new(),
        };
        let long = HeaderField {
            label: "name",
            value: "a".repeat(120),
            placeholder: false,
            annotation: None,
            target: None,
            expanded_kind_label: None,
            expanded_fields: Vec::new(),
        };
        // 40-cell-wide panel: short value fits on one row, long
        // value wraps onto multiple rows once the 10-cell label
        // column is added.
        assert_eq!(header_field_line_count(&short, 40, 0), 1);
        assert!(header_field_line_count(&long, 40, 0) >= 3);
        // The panel-width=0 edge case shouldn't divide by zero or
        // claim zero lines — fall back to a single row.
        assert_eq!(header_field_line_count(&long, 0, 0), 1);
        // An indent shrinks the effective width: a value that fits at
        // indent=0 should report more lines once it's nested.
        let just_fits = HeaderField {
            label: "name",
            value: "a".repeat(28),
            placeholder: false,
            annotation: None,
            target: None,
            expanded_kind_label: None,
            expanded_fields: Vec::new(),
        };
        assert_eq!(header_field_line_count(&just_fits, 40, 0), 1);
        assert!(header_field_line_count(&just_fits, 40, 4) >= 2);
    }

    #[test]
    fn header_zone_height_accounts_for_wrapped_field_values() {
        use crate::tui::detail::SectionKind;

        let mux_field = |value: &str| HeaderField {
            label: "name",
            value: value.to_string(),
            placeholder: false,
            annotation: None,
            target: None,
            expanded_kind_label: None,
            expanded_fields: Vec::new(),
        };
        let session_field = HeaderField {
            label: "session",
            value: "codex:abc".to_string(),
            placeholder: false,
            annotation: None,
            target: Some(NodeId::AgentSession(AgentSessionId::new(
                "codex", "/state", "abc",
            ))),
            expanded_kind_label: None,
            expanded_fields: Vec::new(),
        };

        let with_value = |value: &str| NodeDetail {
            kind_label: "mux_session",
            title_line: "tmux:editor".to_string(),
            short_id: "deadbeef".to_string(),
            full_id: NodeId::MuxSession(MuxSessionId::new("editor")),
            header_fields: vec![mux_field(value), session_field.clone()],
            outgoing_links: Vec::new(),
            incoming_links: Vec::new(),
            resolved: Vec::new(),
            diagnostics: Vec::new(),
        };

        // Sanity: each detail has both a Mux and a Session section.
        let detail = with_value("short");
        let sections = detail.sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].kind, SectionKind::Mux);
        assert_eq!(sections[1].kind, SectionKind::Session);

        let short_height = header_zone_height(&with_value("short"), false, &[], 40, 80);
        let long_height = header_zone_height(&with_value(&"x".repeat(200)), false, &[], 40, 80);
        assert!(
            long_height > short_height,
            "long field value should grow the budget so the Session \
             section below stays visible (short={short_height}, long={long_height})"
        );
    }

    #[test]
    fn mux_detail_session_section_shows_session_id_when_collapsed() {
        // Regression: with a long mux native_id, the right pane's
        // Session section originally vanished entirely. After the
        // wrap-aware budget fix the section divider returned, but
        // the operator reported the linked-session row still didn't
        // render its id until they pressed `e`. This test pins the
        // expectation that the collapsed Session row always carries
        // the `session  <harness>:<key>` line, even when the panel
        // is tall enough to need no clamp.
        use crate::model::{
            Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
            Provenance, RelationKind,
        };
        let long_native = "agentdeck_-local-command-caveat-Caveat-The-messages-\
                           below-were-generated-by-the-user-while-running-local-\
                           comm-Branch-_573ac208";
        let mux_graph_id = MuxSessionId::new(long_native);

        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
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
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_graph_id.clone(),
            backend: "tmux".to_string(),
            native_id: long_native.to_string(),
            cwd: Some("/home/op/src/proj".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: Some(true),
            activity_epoch: Some(1_700_000_000),
            created_epoch: None,
        }));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_graph_id.clone()),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);

        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snapshot,
            home: Some(std::path::Path::new("/home/op")),
            filter: RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Mux;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.update(Msg::NavDown);

        // Render a generously tall area so the height clamp never
        // kicks in — the bug should reproduce purely from the
        // section-content path, not from vertical clamping.
        let area = Rect::new(0, 0, 120, 40);
        let collapsed = buffer_to_string(&render_to_buffer(&app, area));
        // T8-029 + ADR 0074: the linked session now surfaces in the
        // mux's `Related` zone via the inbound `attached session`
        // verb (the session is the link's source, the mux its
        // target). The row carries the session id by `harness:key`.
        assert!(
            collapsed.contains(" Related "),
            "Related section divider should render for the mux: {collapsed}"
        );
        assert!(
            collapsed.contains("attached session"),
            "inbound `attached session` verb should label the row: {collapsed}"
        );
        assert!(
            collapsed.contains("codex:abc"),
            "the related row should expose the session id: {collapsed}"
        );
    }

    #[test]
    fn expanded_session_under_mux_matches_standalone_session_detail() {
        // The expanded representation should reuse the same
        // section-divided, 10-char-bold-label rendering as a
        // standalone session detail — only indented. This pins the
        // per-row labels and the Mux divider so the two surfaces
        // can't drift visually.
        use crate::model::{
            Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
            Provenance, RelationKind,
        };
        let mux_graph_id = MuxSessionId::new("editor");

        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(
                "/home/op/src/proj",
            ))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
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
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_graph_id.clone(),
            backend: "tmux".to_string(),
            native_id: "editor".to_string(),
            cwd: Some("/home/op/src/proj".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: Some(true),
            activity_epoch: Some(1_700_000_000),
            created_epoch: None,
        }));
        snapshot.candidate_links.push(GraphLink {
            id: "session-mux".to_string(),
            source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_graph_id.clone()),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);

        let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot: &snapshot,
            home: Some(std::path::Path::new("/home/op")),
            filter: RowFilter::default(),
            grouping: crate::tui::MuxGrouping::Session,
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Mux;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        app.update(Msg::NavDown);
        app.update(Msg::CycleFocus);
        // Walk the cursor onto the upstream `linked_to_mux` row,
        // then activate to drill into the linked session.
        use crate::tui::explorer::ExplorerRow;
        let link_idx = app
            .explorer()
            .expect("state")
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row,
                    ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                )
            })
            .expect("link row");
        for _ in 0..link_idx {
            app.update(Msg::ExplorerNavDown);
        }
        app.update(Msg::ExplorerActivate);

        let area = Rect::new(0, 0, 120, 40);
        let drilled = buffer_to_string(&render_to_buffer(&app, area));
        // T8-029: post-drill the right pane is now focused on the
        // session itself. Its Node zone exposes the standalone
        // session core fields (id, harness, alias, cwd, status).
        assert!(
            drilled.contains("id"),
            "drilled session Node zone should carry the id row: {drilled}"
        );
        assert!(
            drilled.contains("codex"),
            "drilled session Node zone should expose the harness: {drilled}"
        );
        // Breadcrumb back-hint surfaces in the right-pane title
        // after a drilldown.
        assert!(
            drilled.contains("◀"),
            "drilldown should add a breadcrumb back-hint to the title: {drilled}"
        );
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
    fn session_harness_span_renders_as_filled_badge() {
        // Phase 4 + Phase 10 refinement: the harness label renders
        // as a single fixed-width pill — padding cells included —
        // styled REVERSED+BOLD over the harness color. Every badge
        // is the same visible width regardless of label length so
        // the recency column lands at the same column on every row.
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        use crate::tui::widgets::badge::HARNESS_BADGE_WIDTH;
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let row = AgentSessionRow {
            session: AgentSessionId::new("codex", "/state", "abc"),
            short_id: "abcdef".into(),
            harness_label: "codex".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            pin_id: None,
        };
        let spans = render_session_spans(&row, &theme, now);
        let badge = spans
            .iter()
            .find(|s| s.content.trim() == "codex")
            .expect("harness badge span present");
        assert_eq!(badge.content.chars().count(), HARNESS_BADGE_WIDTH);
        assert_eq!(badge.style.fg, Some(theme.harness_color("codex")));
        assert!(badge.style.add_modifier.contains(Modifier::REVERSED));
        assert!(badge.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn node_kind_glyph_span_uses_slate_glyph_and_theme_color() {
        // ADR 0073 §3: each row carries a prefix glyph in the kind
        // color. The helper returns `<glyph> ` (glyph + trailing
        // space) so callers can splice it before the row body
        // without per-call padding.
        let theme = Theme::default();
        let span = node_kind_glyph_span(NodeKind::Repo, &theme);
        assert_eq!(span.content, "◆ ");
        assert_eq!(span.style.fg, Some(theme.node_repo));
        let workspace = node_kind_glyph_span(NodeKind::Workspace, &theme);
        assert_eq!(workspace.content, "▦ ");
        assert_eq!(workspace.style.fg, Some(theme.node_workspace));
    }

    #[test]
    fn forge_pr_glyph_span_picks_color_from_pr_state() {
        // The PR glyph color follows `theme.pr_*` based on PR state
        // (ADR 0073 §2). Draft overrides state.
        use crate::tui::rows::PrRow;
        let theme = Theme::default();
        let base = PrRow {
            pr_number: 1,
            repo_display: "owner/repo".into(),
            state: Some("open".into()),
            is_draft: false,
            branch_name: None,
            updated_recency: None,
            attached_count: 0,
            url: None,
            primary_node: NodeId::ForgePr(crate::model::ForgePrId::new(
                "github",
                "github.com",
                "owner",
                "repo",
                1,
            )),
        };
        assert_eq!(
            forge_pr_glyph_span(&base, &theme).style.fg,
            Some(theme.pr_open)
        );
        let closed = PrRow {
            state: Some("closed".into()),
            ..base.clone()
        };
        assert_eq!(
            forge_pr_glyph_span(&closed, &theme).style.fg,
            Some(theme.pr_closed)
        );
        let merged = PrRow {
            state: Some("merged".into()),
            ..base.clone()
        };
        assert_eq!(
            forge_pr_glyph_span(&merged, &theme).style.fg,
            Some(theme.pr_merged)
        );
        let draft = PrRow {
            is_draft: true,
            state: Some("open".into()),
            ..base.clone()
        };
        assert_eq!(
            forge_pr_glyph_span(&draft, &theme).style.fg,
            Some(theme.pr_draft),
            "draft overrides state-based color",
        );
        // Glyph itself stays the `NodeKind::ForgePr` slate glyph
        // regardless of color.
        assert_eq!(forge_pr_glyph_span(&base, &theme).content, "⇄ ");
    }

    #[test]
    fn row_kind_glyph_span_dispatches_per_row_kind() {
        use crate::tui::rows::{ForkRow, GroupRow, MuxCandidateRow, PinRow, RepoRow};
        let theme = Theme::default();

        // Group rows derive their kind from `primary_node`; synthetic
        // group buckets without a backing node skip the glyph.
        let workspace_group = RowKind::Group(GroupRow {
            display_path: "/ws".into(),
            primary_node: Some(NodeId::Workspace(crate::model::WorkspaceId::new("/ws"))),
            is_launch_context: false,
        });
        assert_eq!(
            row_kind_glyph_span(&workspace_group, &theme)
                .map(|s| s.content.to_string())
                .as_deref(),
            Some("▦ "),
        );

        let synthetic_group = RowKind::Group(GroupRow {
            display_path: "(ungrouped)".into(),
            primary_node: None,
            is_launch_context: false,
        });
        assert!(
            row_kind_glyph_span(&synthetic_group, &theme).is_none(),
            "synthetic group buckets have no NodeKind",
        );

        // Pin rows are sentinels (📌 in the row body); no kind glyph.
        let pin = RowKind::Pin(PinRow {
            pin_id: "p".into(),
            display_name: "Pinned".into(),
            harness: "claude".into(),
            cwd: "/x".into(),
            mux_name: "m".into(),
            mux_socket: None,
            launch_argv: Vec::new(),
            store_path: "/store".into(),
            harness_label: "claude".into(),
            cwd_display: "/x".into(),
            mux_label: "m".into(),
            state_label: "unbound",
        });
        assert!(row_kind_glyph_span(&pin, &theme).is_none());

        // AgentSession rows skip the kind glyph in row contexts —
        // the colored harness pill already carries the identity
        // signal, so a stacked `●` would only repeat what the pill
        // already says (ADR 0073 amendment).
        let agent = RowKind::AgentSession(crate::tui::rows::AgentSessionRow {
            session: AgentSessionId::new("claude", "/state", "abc"),
            short_id: "abc".into(),
            harness_label: "claude".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: crate::tui::rows::MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc")),
            pin_id: None,
        });
        assert!(
            row_kind_glyph_span(&agent, &theme).is_none(),
            "row-context AgentSession should defer to the harness pill",
        );

        // MuxSession and AgentSessionMuxCandidate both get `▣` since
        // a candidate row points at a mux.
        let mux = RowKind::MuxSession(crate::tui::rows::MuxSessionRow {
            mux: MuxSessionId::new("project"),
            backend: "tmux".into(),
            native_id: "project".into(),
            client_attached: Some(true),
            cwd_display: None,
            attached_count: 1,
            ambiguous_count: 0,
            recency: None,
            activity_epoch: None,
            agent_labels: Vec::new(),
            single_session_preview: None,
            pin_id: None,
            primary_node: NodeId::MuxSession(MuxSessionId::new("project")),
        });
        assert_eq!(
            row_kind_glyph_span(&mux, &theme)
                .map(|s| s.content.to_string())
                .as_deref(),
            Some("▣ "),
        );
        let candidate = RowKind::AgentSessionMuxCandidate(MuxCandidateRow {
            mux: MuxSessionId::new("project"),
            mux_label: "tmux:project".into(),
            is_preferred: true,
            primary_node: NodeId::MuxSession(MuxSessionId::new("project")),
        });
        assert_eq!(
            row_kind_glyph_span(&candidate, &theme)
                .map(|s| s.content.to_string())
                .as_deref(),
            Some("▣ "),
        );

        // Fork rows get `⑂`.
        let fork = RowKind::Fork(ForkRow {
            fork_label: "alpha".into(),
            provider: "github".into(),
            scope: None,
            parent_label: None,
            child_count: 0,
            primary_node: NodeId::Fork(crate::model::ForkId::new("alpha")),
        });
        let fork_glyph = row_kind_glyph_span(&fork, &theme).unwrap();
        assert_eq!(fork_glyph.content, "⑂ ");
        assert_eq!(fork_glyph.style.fg, Some(theme.node_fork));

        // Repo rows get `◆`.
        let repo = RowKind::Repo(RepoRow {
            short_id: "abc".into(),
            display_name: "repo-a".into(),
            canonical_path: None,
            common_dir: "/x/.git".into(),
            primary_node: NodeId::Repo(RepoId::new("/x/.git")),
        });
        assert_eq!(
            row_kind_glyph_span(&repo, &theme)
                .map(|s| s.content.to_string())
                .as_deref(),
            Some("◆ "),
        );
    }

    #[test]
    fn session_alias_renders_after_mux_glyph_in_left_row() {
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let row = AgentSessionRow {
            session: AgentSessionId::new("codex", "/state", "abc"),
            short_id: "abcdef".into(),
            harness_label: "codex".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: Some("harness title".into()),
            alias: Some("ingest-refactor".into()),
            primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            pin_id: None,
        };

        let spans = render_session_spans(&row, &theme, now);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
        assert!(
            rendered.contains("◯  ingest-refactor"),
            "alias should render after the mux glyph: {rendered}"
        );
        assert!(
            rendered.starts_with("abc  "),
            "external session id should lead the row: {rendered}"
        );
        let alias = spans
            .iter()
            .find(|span| span.content.trim() == "ingest-refactor")
            .expect("alias span present");
        assert!(alias.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn session_id_is_strictly_truncated_in_left_row() {
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let long_id = "ffffffff-1111-2222-3333-444444444444";
        let row = AgentSessionRow {
            session: AgentSessionId::new("opencode", "/state", long_id),
            short_id: "abcdef".into(),
            harness_label: "opencode".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("opencode", "/state", long_id)),
            pin_id: None,
        };

        let spans = render_session_spans(&row, &theme, now);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
        assert!(
            rendered.starts_with("ffffffff  "),
            "left row should strictly truncate long session id without ellipsis: {rendered}"
        );
    }

    #[test]
    fn session_display_label_is_truncated_in_left_row() {
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let long_title =
            "The conspectus TUI, fashioned after a long prompt, should not consume the row";
        let row = AgentSessionRow {
            session: AgentSessionId::new("codex", "/state", "abc"),
            short_id: "abcdef".into(),
            harness_label: "codex".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: Some(long_title.into()),
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            pin_id: None,
        };

        let spans = render_session_spans(&row, &theme, now);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
        let label = spans
            .iter()
            .find(|span| span.content.contains("The conspectus"))
            .expect("display label span present");
        assert!(
            rendered.contains("The conspectus TUI, fashioned a…"),
            "long display label should be capped: {rendered}"
        );
        assert_eq!(
            unicode_width::UnicodeWidthStr::width(label.content.trim()),
            32
        );
        assert!(!label.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn mux_session_row_mirrors_session_column_order() {
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let row = MuxSessionRow {
            mux: MuxSessionId::new("tmux:editor"),
            backend: "tmux".into(),
            native_id: "editor".into(),
            client_attached: Some(true),
            cwd_display: Some("~/src/conspectus".into()),
            attached_count: 1,
            ambiguous_count: 0,
            recency: Some("3s".into()),
            activity_epoch: Some(now - 3),
            agent_labels: vec!["codex".into()],
            single_session_preview: Some("running cargo test".into()),
            pin_id: None,
            primary_node: NodeId::MuxSession(MuxSessionId::new("tmux:editor")),
        };

        let spans = render_mux_session_spans(&row, &theme, now, 100);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();

        assert!(
            UnicodeWidthStr::width(rendered.as_str()) <= 100,
            "mux row should fit the given width: {rendered:?}"
        );
        assert!(
            rendered.contains('◉'),
            "attached glyph should still render: {rendered}"
        );
        assert!(
            rendered.contains("  3s"),
            "recency should render right-aligned: {rendered}"
        );
        assert!(
            rendered.contains(" codex "),
            "agent harness badge should label the mux row: {rendered}"
        );
        assert!(
            rendered.contains("running cargo test"),
            "preview should flow into the trailing column: {rendered}"
        );
        assert!(
            !rendered.contains("~/src/conspectus"),
            "cwd column was dropped from the mux row: {rendered}"
        );
        assert!(
            !rendered.contains("tmux:"),
            "backend prefix should be stripped from the mux label: {rendered}"
        );

        // Column order: native id label · harness chip · recency ·
        // attached glyph · preview. Probe by substring index since the
        // chip widget adds internal padding.
        let label_idx = rendered.find("editor").expect("label present");
        let chip_idx = rendered.find("codex").expect("harness chip present");
        let recency_idx = rendered.find("3s").expect("recency present");
        let glyph_idx = rendered.find('◉').expect("glyph present");
        let preview_idx = rendered
            .find("running cargo test")
            .expect("preview present");
        assert!(label_idx < chip_idx, "label before chip: {rendered}");
        assert!(chip_idx < recency_idx, "chip before recency: {rendered}");
        assert!(recency_idx < glyph_idx, "recency before glyph: {rendered}");
        assert!(glyph_idx < preview_idx, "glyph before preview: {rendered}");
    }

    #[test]
    fn session_project_column_renders_before_inline_preview() {
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;
        let row = AgentSessionRow {
            session: AgentSessionId::new("codex", "/state", "abc"),
            short_id: "abcdef".into(),
            harness_label: "codex".into(),
            cwd_display: None,
            project_display: Some("conspectus".into()),
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: Some("latest message".into()),
            title: None,
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            pin_id: None,
        };

        let mut spans = render_session_spans(&row, &theme, now);
        append_session_preview(&mut spans, &row, 120, &theme);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
        let project_idx = rendered.find("conspectus").expect("project rendered");
        let preview_idx = rendered
            .find("latest message")
            .expect("inline preview rendered");
        assert!(
            project_idx < preview_idx,
            "project column should precede preview: {rendered}"
        );
    }

    #[test]
    fn session_recency_span_picks_bucket_style_from_theme() {
        // Build a minimal AgentSessionRow directly so we can pin the
        // activity_epoch and assert the recency span's style without
        // staging a full snapshot. The render_session_spans helper is
        // intentionally cheap to call from the test module.
        use crate::tui::rows::{AgentSessionRow, MuxIndicator};
        let theme = Theme::default();
        let now: i64 = 1_700_000_000;

        let make_row = |recency: Option<&str>, epoch: Option<i64>| AgentSessionRow {
            session: AgentSessionId::new("codex", "/state", "abc"),
            short_id: "abcdef".into(),
            harness_label: "codex".into(),
            cwd_display: None,
            project_display: None,
            recency: recency.map(|s| s.to_string()),
            activity_epoch: epoch,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: None,
            primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
            pin_id: None,
        };
        // Locate the recency span by its formatted content (4-cell
        // right-aligned tag). Index varies with harness label length
        // once the badge widget pads short labels — looking up by
        // content keeps the test resilient to badge layout changes.
        fn recency_span<'a>(spans: &'a [Span<'static>], rendered: &str) -> &'a Span<'static> {
            spans
                .iter()
                .find(|s| s.content.trim() == rendered.trim())
                .expect("recency span present")
        }

        let fresh = make_row(Some("1m"), Some(now - 60));
        let spans = render_session_spans(&fresh, &theme, now);
        assert_eq!(
            recency_span(&spans, "1m").style,
            theme.recency_fresh.into_style(),
            "fresh row should inherit recency_fresh from the theme",
        );

        let cold = make_row(Some("3d"), Some(now - 3 * 24 * 60 * 60));
        let spans = render_session_spans(&cold, &theme, now);
        assert_eq!(
            recency_span(&spans, "3d").style,
            theme.recency_cold.into_style(),
        );

        let unknown = make_row(None, None);
        let spans = render_session_spans(&unknown, &theme, now);
        assert_eq!(
            recency_span(&spans, "—").style,
            Style::default().add_modifier(theme.placeholder),
            "missing activity_epoch falls back to placeholder dimming",
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
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
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
                last_active_epoch: None,
                session_kind: None,
            }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".to_string(),
            native_id: "editor".to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
            filter: RowFilter::default(),
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        config.live_preview_enabled = false;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
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
        // NOT suppress same-line previews in the row tree — only
        // live extras (pane capture + transcript-tail) in the
        // right panel. The graph-resident `stale msg` is allowed
        // to remain in the session row.
    }

    #[test]
    fn right_focus_keeps_selected_row_highlighted_and_changes_status_scope() {
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        app.update(Msg::NavDown);
        app.update(Msg::CycleFocus);

        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            // T8-029: right-focus hint now describes the explorer
            // cursor instead of preview scroll.
            text.contains("j/k cursor"),
            "right focus status hint missing: {text}"
        );
        assert!(
            text.contains("[right]"),
            "right focus marker missing: {text}"
        );

        // Selected row should still carry the inactive-selection
        // indicator (BOLD without REVERSED) in the left pane while
        // focus is on the right pane.
        let selected_carries_inactive_indicator = (0..buffer.area.height).any(|y| {
            let left_width = buffer.area.width / 2;
            let any_bold = (0..left_width)
                .any(|x| buffer[(x, y)].style().add_modifier.contains(Modifier::BOLD));
            let any_reversed = (0..left_width).any(|x| {
                buffer[(x, y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            });
            any_bold && !any_reversed
        });
        assert!(
            selected_carries_inactive_indicator,
            "selected row should remain highlighted via BOLD when right pane has focus"
        );
    }

    #[test]
    fn contextual_status_offers_enter_view_for_unmuxed_session() {
        let mut app = seeded_app();
        app.update(Msg::NavDown);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        // T8-043: un-muxed agent sessions now advertise Enter
        // (and `v`) as the primary default action rather than the
        // attach-disabled reason. Sessions backed by a harness that
        // exposes a resume command additionally surface `S` resume.
        assert!(
            text.contains("Enter/v view"),
            "expected Enter/v view hint for un-muxed session: {text}"
        );
        assert!(
            !text.contains("attach: session is not attached to any mux"),
            "Enter hint should replace the attach-disabled reason on viewable rows: {text}"
        );
        assert!(
            text.contains("S resume"),
            "codex sessions should advertise S resume: {text}"
        );
    }

    #[test]
    fn contextual_status_advertises_enter_attach_for_muxed_session() {
        // Muxed session: Enter (and `a`) attach to the resolved mux.
        let app = muxed_app("editor", None);
        // muxed_app already navigates onto the session row.
        let mut app = app;
        app.update(Msg::NavDown);
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("Enter/a attach"),
            "expected Enter/a attach hint for muxed session: {text}"
        );
    }

    #[test]
    fn status_bar_shows_group_filter_and_sort_settings() {
        let mut app = seeded_app();
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetGrouping(
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::None),
        ));
        app.apply_controls_action(crate::tui::widgets::controls::ControlsAction::SetFilter(
            crate::filter::RowFilter {
                harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
                ..crate::filter::RowFilter::default()
            },
        ));

        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);
        assert!(
            text.contains("group:none"),
            "grouping chip missing from status bar: {text}"
        );
        assert!(
            text.contains("filter:harness:codex"),
            "filter chip missing from status bar: {text}"
        );
        assert!(
            text.contains("sort:recency"),
            "sort chip missing from status bar: {text}"
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
        let preview_divider = text
            .lines()
            .find(|l| l.contains(" Preview "))
            .expect("preview divider line present");
        // The pane label + captured-time freshness now live in the
        // Mux detail section above, so they should NOT appear on
        // the preview divider itself — that was the duplication
        // the styling refresh removed.
        assert!(
            !preview_divider.contains("tmux:"),
            "preview divider should no longer duplicate the mux pane label: \
             {preview_divider}",
        );
        assert!(
            !preview_divider.contains("captured"),
            "preview divider should drop the captured-time tag (now in the Mux \
             section): {preview_divider}",
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
    fn compact_path_helpers_split_workspace_display_at_double_space() {
        // `format_workspace_display` joins label, members, and
        // provider with `  ` separators. The renderer bolds the
        // label and renders the rest with `theme.placeholder`, so
        // the helpers must split there to keep the member list and
        // provider chip out of the bold span — parallel to how a
        // repo header bolds the basename and leaves the CWD path
        // non-bold.
        let display = "nix-config  config+personal+work-config  (agent-deck)";
        assert_eq!(compact_path_label(display), "nix-config");
        assert_eq!(
            compact_path_secondary(display),
            "config+personal+work-config  (agent-deck)"
        );

        // Workspace with no members still splits cleanly because
        // `format_workspace_display` keeps the `  (<provider>)`
        // separator.
        let display = "nix-config  (agent-deck)";
        assert_eq!(compact_path_label(display), "nix-config");
        assert_eq!(compact_path_secondary(display), "(agent-deck)");
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
        // the rendered tree spills well past a small viewport. The
        // repo path is intentionally long: before the T8-019
        // regression fix, the left tree wrapped that group row but
        // computed scroll offsets as if every row occupied one
        // physical line. That put the selected row one line below
        // the viewport instead of on the bottom line.
        let repo_root =
            "/home/op/src/proj-with-a-very-long-display-path-that-would-wrap-before-clipping";
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_root))));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_root), repo_root),
            root: repo_root.to_string(),
            git_dir: None,
            current_branch: None,
        }));
        for i in 0..20 {
            snapshot
                .nodes
                .push(GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                    harness_key: "codex".to_string(),
                    cwd: Some(repo_root.to_string()),
                    title: None,
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
                }));
        }
        let snapshot = crate::resolve::resolve_snapshot(snapshot);
        let tree = build_sessions_tree(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            cwd: None,
            filter: RowFilter::default(),
        });

        let mut config = RunConfig::defaults();
        config.default_view = View::Sessions;
        let mut app = App::new(config);
        app.update(Msg::SetData {
            snapshot: GraphDb::from_snapshot(&snapshot),
            tree,
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });

        // Jump to the last visible row — it lives well below the
        // viewport for a 10-tall window.
        app.update(Msg::End);

        // Render into a side-by-side 120x24 window. The left panel
        // inner viewport is 20 rows tall, so the final selected row
        // should land exactly on y=21, the bottom content row.
        let area = Rect::new(0, 0, 120, 24);
        let buffer = render_to_buffer(&app, area);
        let text = buffer_to_string(&buffer);

        // The last row should be visible. Confirm via the external
        // session id of the last session pushed (s19).
        let visible = app.visible_rows();
        let last_session_id = match &visible.last().unwrap().kind {
            RowKind::AgentSession(s) => s.session.session_key.clone(),
            other => panic!("expected last row to be a session, got {other:?}"),
        };
        assert!(
            text.contains(&last_session_id),
            "selected row's external id ({last_session_id}) should be visible after End; got:\n{text}"
        );
        let bottom_left_line: String = (1..59).map(|x| buffer[(x, 21)].symbol()).collect();
        assert!(
            bottom_left_line.contains(&last_session_id),
            "selected row's external id ({last_session_id}) should land on the bottom visible left-panel line; got {bottom_left_line:?}\n{text}"
        );

        // The first row (the repo group) should now be scrolled
        // off the top.
        let top_left_line: String = (1..59).map(|x| buffer[(x, 2)].symbol()).collect();
        assert!(
            !top_left_line.contains("proj-with-a-very-long"),
            "top-of-tree group should be scrolled away when selection is at End; got top line {top_left_line:?}\n{text}"
        );
    }
}
