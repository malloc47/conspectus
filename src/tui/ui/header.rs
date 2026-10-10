//! Header line and status bar: counts, chips, and contextual hints (ADR 0078).

use super::*;

pub(super) fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    // Header shape (ADR 0078):
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
    let (agents_total, mux_total) = snapshot_counts(app.snapshot_handle());
    let visible_sessions = visible_agent_session_count(app, agents_total);
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
pub(super) struct HeaderCounts {
    pub(super) by_harness: Vec<(String, usize)>,
    pub(super) mux_attached: usize,
    pub(super) mux_ambiguous: usize,
    pub(super) mux_unmuxed: usize,
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

/// Separators between the chip sections. `SECTION_SEPARATOR`
/// joins the prefix to the optional opt-in / triage chips; the per-chip
/// `CHIP_SEPARATOR` joins individual harness chips within the opt-in
/// section.
pub(super) const CHIP_SEPARATOR: &str = "  ";

pub(super) const SECTION_SEPARATOR: &str = "  ·  ";

/// Render the opt-in per-harness chip block (ADR 0078).
/// Drops out cleanly when the row tree is empty so a freshly-launched
/// dashboard with no rows yet doesn't get a hanging trailing
/// separator.
pub(super) fn append_harness_chips(
    spans: &mut Vec<Span<'static>>,
    counts: &HeaderCounts,
    theme: &Theme,
) {
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

/// Render the ambiguity triage chip (`⚠ N`) — only called when N > 0.
/// Uses ADR 0072's `⚠` vocabulary so
/// the header signal aligns with the per-group glyph the row tree
/// already shows.
pub(super) fn append_ambiguity_chip(
    spans: &mut Vec<Span<'static>>,
    ambiguous: usize,
    theme: &Theme,
) {
    spans.push(Span::raw(SECTION_SEPARATOR));
    spans.push(span!(
        Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
        "⚠"
    ));
    spans.push(span!(" {ambiguous}"));
}

/// Render the header's session count. When a filter is active and
/// the visible row count differs from the snapshot's total, format
/// as `<filtered>/<total>`; otherwise
/// keep the bare count so unfiltered runs render minimally.
pub(super) fn format_count_with_filtered(visible: usize, total: usize) -> String {
    if visible == total {
        total.to_string()
    } else {
        format!("{visible}/{total}")
    }
}

/// Count the agent sessions the visible row tree accounts for, so the
/// header's `N/M sessions` mirrors what the left panel shows after
/// filters apply.
///
/// The Mux view renders a single attributed agent inline in its mux
/// row and only emits agent child rows for muxes with several agents,
/// so counting `AgentSession` rows there would read `0/M`. Instead it
/// reports the plain total when no filter narrows the view, and
/// otherwise sums the sessions attributed to the visible mux rows.
pub(super) fn visible_agent_session_count(app: &App, total: usize) -> usize {
    let rows = &app.tree().rows;
    if app.active_view() == View::Mux {
        if !app.filter().has_narrowing_predicates() {
            return total;
        }
        return rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux.attached_count),
                _ => None,
            })
            .sum();
    }
    rows.iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .count()
}

/// Build the `updated Ns ago · ` slice of the header, or an empty
/// string when no snapshot has loaded yet. The trailing separator
/// is part of the returned slice so callers don't have to special-
/// case the empty form.
pub(super) fn header_freshness(app: &App) -> String {
    let Some(loaded_at) = app.loaded_at_epoch() else {
        return String::new();
    };
    let now = current_unix_epoch_for_render();
    match format_recency(Some(now), Some(loaded_at)) {
        Some(label) => format!("updated {label} ago · "),
        None => String::new(),
    }
}

pub(super) fn draw_status_bar(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let theme = app.theme();
    if let Some(message) = app.status_message() {
        let mut spans = Vec::new();
        push_unseen_messages_chip(app, theme, &mut spans);
        spans.push(span!(Style::default().fg(theme.warning); "{message}"));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let stale = app.refresh_failure().is_some();

    let hints = contextual_status_text(app);
    let scope = match app.focus() {
        Focus::Left => "[left]",
        Focus::Right => "[right]",
    };
    let settings = render_view_state_chips(app);

    let mut spans = Vec::new();
    push_unseen_messages_chip(app, theme, &mut spans);
    spans.extend([
        span!(theme.placeholder; "{scope} "),
        span!(Style::default().fg(theme.cwd_mark); "{settings}"),
        span!(theme.placeholder; " · {hints}"),
    ]);

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

/// ADR 0105: warnings and errors the operator hasn't opened the
/// Messages overlay for stay flagged after their status message is
/// cleared. Leads the bar so long hints can't push it off screen.
pub(super) fn push_unseen_messages_chip(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let unseen = app.messages().unseen();
    if unseen == 0 {
        return;
    }
    spans.push(span!(
        Style::default().fg(theme.error).add_modifier(Modifier::BOLD);
        "⚠ {unseen} · ! messages  "
    ));
}

/// Braille-dot spinner frames (~120ms per frame at cadence
/// `SPINNER_FRAME_MS`). Cycles through the eight-position pattern
/// commonly used by cli.rs / npm / systemd loaders.
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub(super) const SPINNER_FRAME_MS: u128 = 120;

/// Pick a spinner glyph based on how long an op has been in flight.
/// Advances one frame per [`SPINNER_FRAME_MS`]; wraps modulo the
/// eight-frame cycle.
pub(super) fn spinner_glyph(started_at: std::time::Instant) -> &'static str {
    let elapsed_ms = started_at.elapsed().as_millis();
    let idx = ((elapsed_ms / SPINNER_FRAME_MS) as usize) % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

/// Append one spinner chip per in-flight async op.
/// Each chip shows a Braille spinner glyph advanced by wall-clock
/// elapsed time plus the op's label. The runtime redraws at least
/// every ~100ms via the poll timeout, so the spinner animates at a
/// natural cadence without a separate tick source.
pub(super) fn push_in_flight_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
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
pub(super) fn push_provider_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
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
pub(super) fn render_view_state_chips(app: &App) -> String {
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

pub(super) fn sort_chip_label(sort: crate::tui::Sort) -> &'static str {
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
pub(super) fn render_filter_chips(filter: &crate::filter::RowFilter) -> String {
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
pub(super) fn truncate_chip_list(values: &[String], cap: usize) -> String {
    if values.len() <= cap {
        return values.join(",");
    }
    let head: Vec<&str> = values
        .iter()
        .take(cap)
        .map(std::string::String::as_str)
        .collect();
    let extra = values.len() - cap;
    format!("{}+{extra} more", head.join(","))
}

pub(super) fn view_label(view: View) -> &'static str {
    match view {
        View::Sessions => "sessions",
        View::Mux => "mux",
        View::Union => "union",
        View::Prs => "prs",
        View::Forks => "forks",
    }
}

pub(super) fn snapshot_counts(handle: Option<&SnapshotHandle>) -> (usize, usize) {
    use crate::model::GraphNode;
    let Some(handle) = handle else {
        return (0, 0);
    };
    let snapshot = handle.snapshot();
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

pub(super) fn contextual_status_text(app: &App) -> String {
    let focus_hint = match app.focus() {
        Focus::Left => "j/k move · h/l fold · Enter default",
        // With the right pane focused, j/k drive the graph
        // explorer cursor, Enter drills or expands a group
        // depending on the cursor position, `e` toggles a group,
        // and Backspace pops the breadcrumb stack.
        Focus::Right => "j/k cursor · Enter drill/expand · e group · ⌫ back",
    };
    let action_hint = default_action_status_hint(app);
    format!("{action_hint} · {focus_hint} · Tab focus · r refresh · q quit")
}

/// Status-bar action hint that advertises `Enter` as the primary
/// default action on the selected row, with the legacy
/// single-key accelerator (`a` / `v`) listed alongside. Falls back
/// to the attach-disabled reason for rows that have neither a mux
/// target nor a viewable transcript so the operator still sees a
/// one-line "disabled because …" cue.
pub(super) fn default_action_status_hint(app: &App) -> String {
    let Some(selection) = app.selection() else {
        return attach_disabled_reason(&crate::tui::actions::AttachDisabled::NoSelection);
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return attach_disabled_reason(&crate::tui::actions::AttachDisabled::NoSelection);
    };
    // Group rows: Enter expands/collapses; h/l explicitly fold.
    if matches!(row.kind, RowKind::Group(_)) {
        return "Enter/l expand · h collapse".to_string();
    }
    // Pin rows surface a per-binding-state hint (ADR 0057).
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
                .snapshot_handle()
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
                let resume = app.snapshot_handle().map(|handle| {
                    crate::tui::resume::resolve_resume_target_in(
                        handle.snapshot(),
                        &session.session,
                    )
                });
                if matches!(
                    resume,
                    Some(crate::tui::resume::ResumeTarget::Launch { .. })
                ) {
                    return format!("Enter/v view {} · S resume", compact_session_label(session));
                }
                return format!("Enter/v view {}", compact_session_label(session));
            }
            attach_disabled_reason(&reason)
        }
    }
}

pub(super) fn pin_placeholder_row_kind(kind: &RowKind) -> bool {
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
pub(super) fn compact_session_label(session: &AgentSessionRow) -> String {
    if let Some(label) = session.display_label() {
        return label.to_string();
    }
    format!("{}:{}", session.harness_label, session.short_id)
}

pub(super) fn selected_mux_state(app: &App) -> Option<MuxIndicator> {
    let selection = app.selection()?;
    let row = app.tree().rows.iter().find(|row| &row.id == selection)?;
    match &row.kind {
        RowKind::AgentSession(session) => Some(session.mux_state),
        _ => None,
    }
}
