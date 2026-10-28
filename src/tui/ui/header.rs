//! Header line and status bar: counts, chips, and contextual hints (ADR 0078).

use super::*;

pub(super) fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    // Header shape (ADR 0078):
    //   [updated Ns ago · ] N/M sessions · M mux
    //     [ · <opt-in harness chips>]
    //     [ · ▲ N when ambiguous > 0]
    //
    // The `Conspectus` brand and `sessions` view-label words moved
    // out — the brand was self-evident inside the TUI and the active
    // view was already in the left-panel title strip with stronger
    // visual weight. The mux-state chip section collapsed to a
    // single `▲ N` because ADR 0072 made the per-row chip binary and
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
    let gone = gone_mux_count(app.snapshot_handle());
    let gone_cell = if gone > 0 {
        format!(" · {gone} stopped")
    } else {
        String::new()
    };
    let prefix = format!("{freshness}{session_cell} sessions · {mux_total} mux{gone_cell}");
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

/// Render the ambiguity triage chip (`▲ N`) — only called when N > 0.
/// Uses ADR 0072's `▲` vocabulary so
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
        "▲"
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
/// filters apply. `N/` appears only when the view lists fewer sessions
/// than there are, so it never reads as if something were filtered out
/// when nothing was.
///
/// - Sessions lists the sessions themselves: count its session rows.
/// - Search lists the sessions the search it shows found, as session
///   rows or, in the flat listing, as the hits that name them. Before
///   any search it lists nothing and hides nothing: the plain total.
/// - Mux and PRs list other things and only some of the sessions with
///   them (a single agent sits inline in its mux row; a PR lists only
///   the sessions on its branch), so counting rows there would read
///   `0/M` or `4/M` unfiltered. They report the plain total until a
///   filter narrows them, then the sessions their visible rows hold.
/// - Pinned and History list entities of every kind and ignore the
///   filter: the plain total.
pub(super) fn visible_agent_session_count(app: &App, total: usize) -> usize {
    let rows = &app.tree().rows;
    let session_rows = || {
        rows.iter()
            .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
            .count()
    };
    match app.active_view() {
        View::Sessions => session_rows(),
        View::Search if app.search_history().viewed().is_none() => total,
        View::Search => {
            let mut found = std::collections::BTreeSet::new();
            for row in rows {
                match &row.kind {
                    RowKind::AgentSession(session) => found.insert(&session.session),
                    RowKind::Hit(hit) => found.insert(&hit.session),
                    _ => false,
                };
            }
            found.len()
        }
        View::Mux | View::Prs if !filter_narrows_view(app) => total,
        View::Mux => rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux.attached_count),
                _ => None,
            })
            .sum(),
        View::Prs => session_rows(),
        View::Pinned | View::History => total,
    }
}

/// Whether the filter leaves anything out of the Mux or PRs view: a
/// session predicate, the content search, pinned-only, or, in PRs, a
/// PR state or branch.
fn filter_narrows_view(app: &App) -> bool {
    let filter = app.filter();
    filter.has_narrowing_predicates()
        || app.content().is_some()
        || filter.pinned_only
        || app.active_view() == View::Prs
            && (filter.pr_state.is_some() || filter.pr_branch.is_some())
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
        push_in_flight_chips(app, theme, &mut spans);
        spans.push(span!(Style::default().fg(theme.warning); "{message}"));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let stale = app.refresh_failure().is_some();

    let scope = match app.focus() {
        Focus::Left => "[left]",
        Focus::Right => "[right]",
    };

    let mut spans = Vec::new();
    push_unseen_messages_chip(app, theme, &mut spans);
    push_in_flight_chips(app, theme, &mut spans);
    spans.push(span!(theme.muted; "{scope} "));
    if stale {
        // Reserved up front so the stale mark is never the thing that
        // gets pushed off the end.
        spans.push(span!(
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
            "stale  "
        ));
    }
    let mut provider_spans = Vec::new();
    push_provider_chips(app, theme, &mut provider_spans);
    let reserved = spans_cells(&spans) + spans_cells(&provider_spans);
    let room = usize::from(area.width).saturating_sub(reserved);
    spans.extend(fit_status_segments(app, theme, room));
    spans.extend(provider_spans);

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
        "▲ {unseen} · ! messages  "
    ));
}

/// Braille-dot spinner frames (~120ms per frame at cadence
/// `SPINNER_FRAME_MS`). Cycles through the eight-position pattern
/// commonly used by cli.rs / npm / systemd loaders.
pub(super) const SPINNER_FRAMES: [&str; 8] = crate::tui::icons::marks::SPINNER;

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
/// natural cadence without a separate tick source. The chips lead
/// the bar, like the unseen-messages chip, so long hints can't push
/// them off screen (ADR 0127).
pub(super) fn push_in_flight_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let ops = app.in_flight_ops();
    if ops.is_empty() {
        return;
    }
    let style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    for op in ops {
        spans.push(span!(style; "{} {}", spinner_glyph(op.started_at), op.label));
        spans.push(Span::raw("  "));
    }
}

/// Append right-side provider status chips to `spans` from
/// `App::provider_status`.
pub(super) fn push_provider_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let status = app.provider_status();

    let mut push_chip = |label: &str, style: Style| {
        spans.push(Span::raw("  "));
        spans.push(span!(style; "{label}"));
    };

    if status.tmux_disabled {
        push_chip("tmux:off", Style::default().add_modifier(theme.muted));
    } else if let Some(false) = status.tmux_available {
        let reason = status.tmux_reason.as_deref().unwrap_or("unavailable");
        push_chip(
            &format!("tmux:{reason}"),
            Style::default().fg(theme.warning),
        );
    }

    if status.forge_disabled {
        push_chip("forge:off", Style::default().add_modifier(theme.muted));
    } else if let Some(false) = status.forge_available {
        let reason = status.forge_reason.as_deref().unwrap_or("error");
        // Reasons name `gh` themselves ("gh is not authenticated");
        // the chip's prefix already does.
        let reason = reason.strip_prefix("gh ").unwrap_or(reason);
        push_chip(&format!("gh:{reason}"), Style::default().fg(theme.warning));
    }
}

/// The active view state in a compact, always-visible form so grouping,
/// filtering, and sorting are discoverable without opening the controls
/// overlay, as separately droppable pieces, each with the priority it
/// keeps when the bar is too narrow for all of them (1 stays longest).
fn view_state_segments(app: &App) -> Vec<(u8, String)> {
    let filter = render_filter_chips(app.filter());
    let filter = if filter.is_empty() {
        "all".to_string()
    } else {
        filter
    };
    let mut segments = vec![
        (5, format!("group:{}", app.grouping().as_str())),
        (3, format!("filter:{filter}")),
        (6, format!("sort:{}", sort_chip_label(app.sort()))),
    ];
    if let Some(find) = render_find_chip(app) {
        segments.push((1, find));
    }
    segments
}

/// The content search as a chip: its text, then how far the scan has
/// got, or how many sessions it found once it is done.
pub(super) fn render_find_chip(app: &App) -> Option<String> {
    let content = app.content()?;
    let text = if content.text.chars().count() > 24 {
        let head: String = content.text.chars().take(23).collect();
        format!("{head}…")
    } else {
        content.text.clone()
    };
    if matches!(
        app.active_view(),
        crate::tui::View::Pinned | crate::tui::View::History
    ) {
        return Some(format!("find:\"{text}\" not applied here"));
    }
    let progress = content.status_note(std::time::Instant::now());
    Some(format!("find:\"{text}\" {progress}"))
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
/// stable across runs (harness → max-age → mux-state → state → branch) so the
/// operator builds muscle memory for where each predicate lives.
pub(super) fn render_filter_chips(filter: &crate::filter::RowFilter) -> String {
    if !filter.has_narrowing_predicates() && filter.pr_branch.is_none() && filter.pr_state.is_none()
    {
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
    if let Some(pr_state) = &filter.pr_state {
        let labels: Vec<String> = pr_state
            .values()
            .iter()
            .map(|key| key.as_str().to_string())
            .collect();
        if !labels.is_empty() {
            chips.push(format!("state:{}", truncate_chip_list(&labels, 4)));
        }
    }
    if let Some(pr_branch) = filter.pr_branch {
        chips.push(format!("branch:{}", pr_branch.as_str()));
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
        View::Prs => "prs",
        View::Pinned => "pinned",
        View::History => "history",
        View::Search => "search",
    }
}

/// Muxes whose session a successful probe found gone (ADR 0140).
pub(super) fn gone_mux_count(handle: Option<&SnapshotHandle>) -> usize {
    use crate::model::GraphNode;
    let Some(handle) = handle else {
        return 0;
    };
    let snapshot = handle.snapshot();
    snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::MuxSession(_)))
        .filter(|node| {
            snapshot
                .presence(&node.id())
                .is_some_and(crate::model::NodePresence::is_gone)
        })
        .count()
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
    // Gone muxes are history, not muxes that exist (ADR 0140).
    let mux = snapshot
        .live_nodes()
        .filter(|node| matches!(node, GraphNode::MuxSession(_)))
        .count();
    (agents, mux)
}

/// The key hints as separately droppable pieces with the priority each
/// keeps on a narrow bar.
fn contextual_status_segments(app: &App) -> Vec<(u8, String)> {
    let focus_hint = match app.focus() {
        Focus::Left => "j/k move · h/l fold · Enter default",
        // With the right pane focused, j/k drive the graph
        // explorer cursor, Enter drills or expands a group
        // depending on the cursor position, `e` toggles a group,
        // and Backspace pops the breadcrumb stack.
        Focus::Right => "j/k cursor · Enter drill/expand · e group · Backspace back",
    };
    let refresh_hint = if app.active_view() == crate::tui::View::Prs {
        "r refresh · ^R forges"
    } else {
        "r refresh"
    };
    let mut segments = vec![
        (4, default_action_status_hint(app)),
        // With the right pane focused its own keys matter more than the
        // action hint, which describes the left pane's row.
        (
            if app.focus() == Focus::Right { 3 } else { 7 },
            focus_hint.to_string(),
        ),
        (8, "Tab focus".to_string()),
        (9, refresh_hint.to_string()),
        (2, "q quit".to_string()),
    ];
    if app.focus() == Focus::Left && app.can_nav_back() {
        segments.push((1, "Backspace back".to_string()));
    }
    segments
}

/// The status bar's settings chips and key hints, fitted to `room`
/// cells: pieces drop in order of priority (the least important first),
/// a `…` marks that some were dropped, and nothing is cut mid-word.
fn fit_status_segments(app: &App, theme: &Theme, room: usize) -> Vec<Span<'static>> {
    #[derive(Clone)]
    struct Piece {
        order: usize,
        priority: u8,
        text: String,
        settings: bool,
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for (priority, text) in view_state_segments(app) {
        pieces.push(Piece {
            order: pieces.len(),
            priority,
            text,
            settings: true,
        });
    }
    for (priority, text) in contextual_status_segments(app) {
        pieces.push(Piece {
            order: pieces.len(),
            priority,
            text,
            settings: false,
        });
    }
    let width = |kept: &[&Piece]| -> usize {
        kept.iter()
            .map(|piece| UnicodeWidthStr::width(piece.text.as_str()))
            .sum::<usize>()
            + 3 * kept.len().saturating_sub(1)
    };
    let choose = |room: usize| -> Vec<Piece> {
        let mut by_priority: Vec<&Piece> = pieces.iter().collect();
        by_priority.sort_by_key(|piece| (piece.priority, piece.order));
        let mut kept: Vec<&Piece> = Vec::new();
        for piece in by_priority {
            let mut trial = kept.clone();
            trial.push(piece);
            if width(&trial) <= room {
                kept = trial;
            }
        }
        kept.sort_by_key(|piece| piece.order);
        kept.into_iter().cloned().collect()
    };
    let mut kept = choose(room);
    let dropped = kept.len() < pieces.len();
    if dropped {
        kept = choose(room.saturating_sub(2));
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (index, piece) in kept.iter().enumerate() {
        if index > 0 {
            spans.push(span!(theme.muted; " · "));
        }
        if piece.settings {
            spans.push(span!(Style::default().fg(theme.cwd_mark()); "{}", piece.text));
        } else {
            spans.push(span!(theme.muted; "{}", piece.text));
        }
    }
    if dropped {
        spans.push(span!(theme.muted; " …"));
    }
    spans
}

fn spans_cells(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
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
    if crate::tui::actions::viewer_open_target(app).is_some() {
        return "Enter/v open at the match · . menu".to_string();
    }
    if crate::tui::actions::selected_record(app).is_some() {
        return "Enter rebuild · . menu (pin, forget, plan, clean up)".to_string();
    }
    let selected_pin_id = match (&row.id, &row.kind) {
        (RowId::Pin { pin_id }, _) => Some(pin_id.as_str()),
        (_, RowKind::AgentSession(session)) => session.pin_id.as_deref(),
        (_, RowKind::MuxSession(mux)) => mux.pin_id.as_deref(),
        _ => None,
    };
    if let Some(pin_id) = selected_pin_id
        && let Some(handle) = app.snapshot_handle()
        && let Some(hint) = crate::tui::actions::pin_status_hint(handle.snapshot(), pin_id)
    {
        return hint;
    }
    match resolve_attach_target(app) {
        Ok(target) => {
            let label = target.label();
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
