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
use ratatui::layout::{Margin, Rect};
use ratatui::macros::{horizontal, span, vertical};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use unicode_width::UnicodeWidthStr;

use crate::model::{MuxSessionId, NodeId};
use crate::tui::Msg;
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

/// Render one frame.
///
/// Takes `&mut App` because the draw path dispatches
/// [`Msg::LeftViewportChanged`] / [`Msg::ExplorerViewportChanged`]
/// mid-frame so the reducer can reconcile scroll offsets (H-TUI-005
/// waves 1 + 2). The reconciliation math itself lives in the
/// reducer; draw only measures viewport height + the post-wrap
/// explorer cursor row span and dispatches those measurements as
/// Msgs. A strict `&App → buffer` shape would require restructuring
/// draw into separate measure + render passes so the runtime can
/// dispatch the Msgs upstream — that's an ADR 0085 contract-5
/// optional cleanup, not a correctness need. The mutation surface
/// today is bounded to those two Msg dispatches; every subsequent
/// draw helper (`draw_header` / `draw_status_bar` / overlays /
/// toast) takes `&App` and is byte-identical over the same App
/// state.
pub fn draw(app: &mut App, frame: &mut Frame<'_>) {
    let area = frame.area();
    let layout = vertical![==1, >=3, ==1].split(area);

    draw_header(app, frame, layout[0]);
    draw_body(app, frame, layout[1]);
    draw_status_bar(app, frame, layout[2]);
    draw_controls_overlay(app, frame, area);
    draw_pins_overlay(app, frame, area);
    draw_search_overlay(app, frame, area);
    draw_help_overlay(app, frame, area);
    draw_rename_overlay(app, frame, area);
    draw_worktree_menu(app, frame, area);
    draw_new_mux_form(app, frame, area);
    draw_mux_menu(app, frame, area);
    draw_mux_launch_form(app, frame, area);
    draw_value_modal(app, frame, area);
    draw_toast(app, frame, area);
}

fn draw_mux_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.mux_menu() else {
        return;
    };
    use crate::tui::widgets::mux_menu::MuxMenuWidget;
    frame.render_widget(MuxMenuWidget::new(state, app.theme()), area);
}

fn draw_mux_launch_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.mux_launch_form() else {
        return;
    };
    use crate::tui::widgets::mux_launch::MuxLaunchFormWidget;
    frame.render_widget(MuxLaunchFormWidget::new(state, app.theme()), area);
}

fn draw_worktree_menu(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.worktree_menu() else {
        return;
    };
    use crate::tui::widgets::worktree_menu::WorktreeMenuWidget;
    frame.render_widget(WorktreeMenuWidget::new(state, app.theme()), area);
}

fn draw_new_mux_form(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.new_mux_form() else {
        return;
    };
    use crate::tui::widgets::new_mux::NewMuxFormWidget;
    frame.render_widget(NewMuxFormWidget::new(state, app.theme()), area);
}

fn draw_toast(app: &App, frame: &mut Frame<'_>, area: Rect) {
    if !app.toast().has_toast() {
        return;
    }
    // The upstream engine impls `Widget for &ToastEngine`, so we
    // render through the shared borrow without any wrapper widget.
    // `set_area` + `tick` already ran via `prepare_toast_for_render`
    // before `terminal.draw` was called.
    frame.render_widget(app.toast(), area);
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
    let widget = TextInputWidget::new(state).theme(app.theme());
    frame.render_widget(widget, area);
}

fn draw_controls_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.controls_overlay() else {
        return;
    };
    use crate::tui::widgets::controls::ControlsOverlayWidget;
    let widget = ControlsOverlayWidget::new(state, app.controls_context(), app.theme());
    frame.render_widget(widget, area);
}

fn draw_pins_overlay(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(state) = app.pins_overlay() else {
        return;
    };
    use crate::tui::widgets::pins::PinsOverlayWidget;
    let widget = PinsOverlayWidget::new(state, app.theme());
    frame.render_widget(widget, area);
}

// -----------------------------------------------------------------------------
// Header / status bar
// -----------------------------------------------------------------------------

fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    // H-UI-004 audit shape (ADR 0078):
    //   [updated Ns ago · ] N/M sessions · M mux
    //     [ · <opt-in harness chips>]
    //     [ · ⚠ N when ambiguous > 0]
    //
    // The `Conspectus` brand and `sessions` view-label words moved
    // out — the brand was self-evident inside the TUI and the active
    // view was already in the left-panel title strip with stronger
    // visual weight. The mux-state chip section collapsed to a
    // single `⚠ N` because ADR 0072 made the per-row chip binary and
    // every group row already owns the ambiguity glyph. Per-harness
    // chips became opt-in via `[tui] show_harness_chips`. Freshness
    // promoted to the lead position.
    let theme = app.theme();
    let (agents_total, mux_total) = snapshot_counts(app.graph_db());
    let visible_sessions = visible_agent_session_count(app);
    let freshness = header_freshness(app);
    let session_cell = format_count_with_filtered(visible_sessions, agents_total);

    // Identity prefix renders bold; chips after it carry their own
    // colors and stay independent of the prefix style.
    let prefix = format!("{freshness}{session_cell} sessions · {mux_total} mux");
    let mut spans: Vec<Span<'static>> = vec![span!(Modifier::BOLD; "{prefix}")];

    // Counts walk the visible row tree, matching the count rule.
    let counts = HeaderCounts::from_app(app);

    if app.config().show_harness_chips {
        append_harness_chips(&mut spans, &counts, theme);
    }
    if counts.mux_ambiguous > 0 {
        append_ambiguity_chip(&mut spans, counts.mux_ambiguous, theme);
    }

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

/// Separators between the chip sections (H-UI-004). `SECTION_SEPARATOR`
/// joins the prefix to the optional opt-in / triage chips; the per-chip
/// `CHIP_SEPARATOR` joins individual harness chips within the opt-in
/// section.
const CHIP_SEPARATOR: &str = "  ";
const SECTION_SEPARATOR: &str = "  ·  ";

/// Render the opt-in per-harness chip block (H-UI-004 §"Harness chips").
/// Drops out cleanly when the row tree is empty so a freshly-launched
/// dashboard with no rows yet doesn't get a hanging trailing
/// separator.
fn append_harness_chips(spans: &mut Vec<Span<'static>>, counts: &HeaderCounts, theme: &Theme) {
    use crate::tui::widgets::badge::harness_badge;
    if counts.by_harness.is_empty() {
        return;
    }
    spans.push(Span::raw(SECTION_SEPARATOR));
    let mut first = true;
    for (label, count) in &counts.by_harness {
        if !first {
            spans.push(Span::raw(CHIP_SEPARATOR));
        }
        first = false;
        spans.push(harness_badge(label, theme));
        spans.push(span!(" {count}"));
    }
}

/// Render the ambiguity triage chip (`⚠ N`) — only called when N > 0
/// (H-UI-004 §"Mux-state chips"). Uses ADR 0072's `⚠` vocabulary so
/// the header signal aligns with the per-group glyph the row tree
/// already shows.
fn append_ambiguity_chip(spans: &mut Vec<Span<'static>>, ambiguous: usize, theme: &Theme) {
    spans.push(Span::raw(SECTION_SEPARATOR));
    spans.push(span!(
        Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
        "⚠"
    ));
    spans.push(span!(" {ambiguous}"));
}

/// Render the header's session count. When a filter is active and
/// the visible row count differs from the snapshot's total, format
/// as `<filtered>/<total>` (H-UI-004 §"Count wording"); otherwise
/// keep the bare count so unfiltered runs render minimally.
fn format_count_with_filtered(visible: usize, total: usize) -> String {
    if visible == total {
        total.to_string()
    } else {
        format!("{visible}/{total}")
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
        span!(theme.placeholder; "{scope} "),
        span!(Style::default().fg(theme.cwd_mark); "{settings}"),
        span!(theme.placeholder; " · {hints}"),
    ];

    push_in_flight_chips(app, theme, &mut spans);

    if stale {
        spans.push(span!(
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
            "  stale"
        ));
    }

    push_provider_chips(app, theme, &mut spans);

    let widget = Paragraph::new(Line::from(spans));
    frame.render_widget(widget, area);
}

/// Braille-dot spinner frames (~120ms per frame at cadence
/// `SPINNER_FRAME_MS`). Cycles through the eight-position pattern
/// commonly used by cli.rs / npm / systemd loaders.
const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
const SPINNER_FRAME_MS: u128 = 120;

/// Pick a spinner glyph based on how long an op has been in flight.
/// Advances one frame per [`SPINNER_FRAME_MS`]; wraps modulo the
/// eight-frame cycle.
fn spinner_glyph(started_at: std::time::Instant) -> &'static str {
    let elapsed_ms = started_at.elapsed().as_millis();
    let idx = ((elapsed_ms / SPINNER_FRAME_MS) as usize) % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

/// Append one spinner chip per in-flight async op (H-WIDG-007).
/// Each chip shows a Braille spinner glyph advanced by wall-clock
/// elapsed time plus the op's label. The runtime redraws at least
/// every ~100ms via the poll timeout, so the spinner animates at a
/// natural cadence without a separate tick source.
fn push_in_flight_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let ops = app.in_flight_ops();
    if ops.is_empty() {
        return;
    }
    let style = Style::default()
        .fg(theme.panel_focus_accent)
        .add_modifier(Modifier::BOLD);
    for op in ops {
        spans.push(Span::raw("  "));
        spans.push(span!(style; "{} {}", spinner_glyph(op.started_at), op.label));
    }
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
        spans.push(span!(style; "{label}"));
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
    use crate::model::GraphNode;
    let Some(database) = database else {
        return (0, 0);
    };
    let snapshot = database.snapshot();
    let agents = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::AgentSession(_)))
        .count();
    let mux = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::MuxSession(_)))
        .count();
    (agents, mux)
}

// -----------------------------------------------------------------------------
// Body: left tree + right detail
// -----------------------------------------------------------------------------

fn draw_body(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let threshold = app.config().narrow_layout_threshold;
    let split = if area.width < threshold {
        vertical![==50%, ==50%].split(area)
    } else {
        horizontal![==50%, ==50%].split(area)
    };
    draw_left_panel(app, frame, split[0]);
    draw_right_panel(app, frame, split[1]);
}

fn draw_left_panel(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
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

    // H-TUI-005 wave 2: the reducer owns scroll reconciliation.
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
fn scrollbar_layout(area: Rect, content_length: usize) -> (Rect, Option<Rect>) {
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
fn render_vertical_scrollbar(
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
fn left_panel_title(app: &App) -> Line<'static> {
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
        spans.push(span!(Style::default().fg(color); "{} ", style.glyph));
    }
    spans.push(span!(Modifier::BOLD; "{label}"));
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
            .map_or(0, |_| 2);
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
        "pin" => "pin",
        _ => "detail",
    }
}

/// `▸ ` when the given panel has focus, two-space pad otherwise.
/// Keeps title widths consistent across focus states so the tab
/// strip and right-pane label sit at the same offset before and
/// after `Tab`.
fn focus_marker_span(app: &App, panel: Focus) -> Span<'static> {
    let focused = app.focus() == panel;
    let glyph = if focused { "▸ " } else { "  " };
    span!(Style::default().fg(app.theme().panel_focus_accent); "{glyph}")
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
        return format!("No rows match `{chips}`.\nPress `F` to clear filters, `f` to edit.");
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
fn group_row_body_width(
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
            // Final glyph + theme entry land alongside the rest of
            // H-PIN-016's TUI polish; for the v1 slice we reuse the
            // existing `placeholder` modifier to keep the row visibly
            // distinct without inventing a new Theme key.
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
    if placeholder {
        // Mirror the mux view: an unbound-pin session has no live mux
        // to attach to, so the attached-glyph column shows the same
        // dotted-circle marker as the mux-view placeholder row.
        spans.push(span!(Style::default().fg(theme.pin_placeholder); "◌"));
    } else {
        spans.push(mux_indicator_span(session.mux_state, theme));
    }
    if session.pin_id.is_some() {
        // ADR 0057 bound-pin marker. Glyph + theme entry are
        // finalized alongside the rest of the H-PIN-016 styling
        // polish; for the v1 slice we reuse `placeholder` so the
        // marker reads without depending on a new theme key.
        spans.push(span!(theme.placeholder; "  📌"));
    }
    // P8-015: the row's tree label surfaces the operator-chosen
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
fn render_repo_spans(repo: &crate::tui::rows::RepoRow, theme: &Theme) -> Vec<Span<'static>> {
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
    if placeholder {
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

fn row_primary_node_is_pin(node: &NodeId) -> bool {
    matches!(node, NodeId::Pin(_))
}

fn append_mux_agent_labels(
    spans: &mut Vec<Span<'static>>,
    mux: &MuxSessionRow,
    theme: &Theme,
    width: usize,
) {
    use crate::tui::widgets::badge::harness_badge;

    if mux.agent_labels.is_empty() {
        spans.push(span!(theme.placeholder; " no agent "));
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
    spans.push(span!(
        Style::default().fg(theme.secondary_text).add_modifier(Modifier::ITALIC);
        "{}",
        truncate_to_width(preview, width - used - 2)
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
fn node_kind_glyph_span(kind: NodeKind, theme: &Theme) -> Span<'static> {
    let style = node_kind_style(kind, theme);
    span!(Style::default().fg(style.color); "{} ", style.glyph)
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
    span!(Style::default().fg(color); "{} ", style.glyph)
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
/// kind backs it (synthetic group buckets).
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
        RowKind::Pin(_) => NodeKind::Pin,
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
        MuxIndicator::Attached => span!(Style::default().fg(theme.mux_attached); "◉"),
        MuxIndicator::Ambiguous { .. } | MuxIndicator::Unmuxed => {
            span!(theme.mux_unmuxed; "◯")
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
    span!(Style::default().fg(app.theme().disclosure); "{glyph}")
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

/// Truncate `text` to `width` cells by replacing a middle slice with
/// `…` when the natural width overflows. Keeps the leading and
/// trailing context visible so a path like
/// `/fixture/atelier-demo/repo-a` collapses to
/// `/fixture/…/repo-a` rather than dropping the basename. The left
/// half is preferred when the budget is odd.
fn truncate_to_width_middle(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let text_width = UnicodeWidthStr::width(text);
    if text_width <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let usable = width - 1;
    let left_budget = usable.div_ceil(2);
    let right_budget = usable - left_budget;
    let chars: Vec<char> = text.chars().collect();

    let mut left = String::new();
    let mut left_used = 0usize;
    for ch in &chars {
        let cw = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
        if left_used + cw > left_budget {
            break;
        }
        left.push(*ch);
        left_used += cw;
    }

    let mut right_chars: Vec<char> = Vec::new();
    let mut right_used = 0usize;
    for ch in chars.iter().rev() {
        let cw = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
        if right_used + cw > right_budget {
            break;
        }
        right_chars.push(*ch);
        right_used += cw;
    }
    let right: String = right_chars.into_iter().rev().collect();

    let mut out = String::with_capacity(width);
    out.push_str(&left);
    out.push('…');
    out.push_str(&right);
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
            out.push(span!(
                style;
                "{}",
                truncate_to_width_strict(span.content.as_ref(), remaining)
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

fn draw_right_panel(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
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
    if app.explorer().is_some() {
        // H-TUI-005 wave 1: `app` is `&mut` here so scroll
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
        // Reserve a usable minimum for the preview zone so a full
        // Related list cannot collapse the preview to 1–2 lines.
        // Below this floor on very small terminals the layout
        // falls back to the prior behavior (preview keeps at least
        // two rows after the divider).
        const MIN_PREVIEW_HEIGHT: u16 = 6;
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
        let header_height = (wrapped_rows as u16)
            .saturating_add(1)
            .min(max_header_height)
            .max(3);
        let split = vertical![==header_height, ==1, >=0].split(inner);
        // H-TUI-005 wave 2: the reducer owns scroll reconciliation.
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

/// Output of [`render_explorer_lines`]: the rendered lines plus the
/// index in `lines` of the cursor's row (when one of the rendered
/// rows is selected). Used by [`draw_right_panel`] to compute the
/// post-wrap scroll offset that keeps the cursor in view.
struct ExplorerRender {
    lines: Vec<Line<'static>>,
    cursor_line: Option<usize>,
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
    let label = span!(Modifier::BOLD; "  {:<14}", field.label);
    let mut spans = vec![label];
    // ADR 0073 §3: render the kind-chip glyph before the value so the
    // chip stays anchored beside the label even when the value wraps
    // onto a new line in a narrow pane (otherwise the chip strands at
    // the wrap tail and reads as a stray symbol).
    if let Some(kind) = field.kind_chip {
        spans.push(kind_chip_span(kind, theme));
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
    spans.push(span!(row_style; "{verb_text}"));
    spans.push(kind_chip);
    spans.push(Span::raw(" "));
    spans.push(span!(label_style; "{}", link.neighbor_label.clone()));
    if show_edge_meta {
        let trailing = format!(
            "  · {} · {} · {}",
            link.provenance.snake_case(),
            link.confidence.snake_case(),
            link.state.snake_case(),
        );
        spans.push(span!(label_style; "{trailing}"));
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
    Line::from(span!(style; "{text}"))
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
    Line::from(span!(style; "{text}"))
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

/// Sum of post-wrap terminal rows the given lines occupy when
/// rendered into a paragraph `width` wide. Mirrors the per-line
/// count used by the explorer header budget — used here to drive
/// scrollbar `content_length` (ADR 0076).
fn wrapped_line_count(lines: &[Line<'_>], width: u16) -> usize {
    let pane_width = width.max(1) as usize;
    lines
        .iter()
        .map(|line| {
            let line_width = line
                .spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum::<usize>()
                .max(1);
            line_width.div_ceil(pane_width).max(1)
        })
        .sum()
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

fn render_header_field(field: &HeaderField, section: SectionKind, theme: &Theme) -> Line<'static> {
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
    let total_rows = wrapped_line_count(&preview.lines, area.width);
    let (content_area, scrollbar_area) = scrollbar_layout(area, total_rows);
    let widget = Paragraph::new(preview)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll(), 0));
    frame.render_widget(widget, content_area);
    if let Some(sb_area) = scrollbar_area {
        render_vertical_scrollbar(frame, sb_area, total_rows, app.preview_scroll() as usize);
    }
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
    let selected_pin_id = match &row.kind {
        RowKind::AgentSession(session) => session.pin_id.as_deref(),
        RowKind::MuxSession(mux) => mux.pin_id.as_deref(),
        _ => None,
    };
    if selected_pin_id.is_some() {
        let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
        if let Some(hint) = crate::tui::actions::pin_status_hint(&diagnostics) {
            if !hint.contains(" b bind") {
                return format!("{hint} · b bind");
            }
            return hint;
        }
        if pin_placeholder_row_kind(&row.kind) {
            let pin_id = selected_pin_id.unwrap_or("pin");
            let pin = app
                .graph_db()
                .and_then(|db| db.snapshot().pins.iter().find(|pin| pin.id == pin_id));
            let display = pin.map_or(pin_id, |pin| pin.display_name.as_str());
            let launch_hint = match pin.and_then(|p| p.binding.as_ref()) {
                Some(crate::model::PinBinding::StaleMux { mux }) => format!(
                    "Enter to relaunch `{display}` in existing mux `{}`",
                    mux.native_id
                ),
                _ => format!("Enter to launch `{display}`"),
            };
            return format!("{launch_hint} · b bind · Del remove");
        }
    }
    match resolve_attach_target(app) {
        Ok(target) => {
            let label = target_label(&target);
            match selected_mux_state(app) {
                Some(MuxIndicator::Ambiguous { .. }) => {
                    format!("Enter/a attach preferred {label} · Tab inspect candidates")
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

fn pin_placeholder_row_kind(kind: &RowKind) -> bool {
    match kind {
        RowKind::AgentSession(session) => row_primary_node_is_pin(&session.primary_node),
        RowKind::MuxSession(mux) => row_primary_node_is_pin(&mux.primary_node),
        _ => false,
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
pub fn render_to_buffer(app: &mut App, area: Rect) -> ratatui::buffer::Buffer {
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
#[path = "ui_tests.rs"]
mod tests;
