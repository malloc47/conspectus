//! Header line and status bar: counts, chips, and contextual hints (ADR 0078).

use tuikit::priority_line::{PriorityLine, Segment};

use super::*;

/// How long each part of the header holds on as the screen narrows, the
/// highest longest; the session count has no rank and never gives way.
/// Freshness and the load-bearing counts outlast the triage chip, which
/// outlasts the stopped count, and the opt-in harness chips go first,
/// from the end (ADR 0078's order of what the header carries).
mod header_rank {
    pub(super) const FRESHNESS: u32 = 5;
    pub(super) const MUX: u32 = 4;
    pub(super) const AMBIGUITY: u32 = 3;
    pub(super) const STOPPED: u32 = 2;
    pub(super) const HARNESS_CHIPS: u32 = 1;
}

pub(super) fn draw_header(app: &App, buf: &mut Buffer, area: Rect) {
    let room = usize::from(area.width);
    Paragraph::new(header_line(app).line(room)).render(area, buf);
}

/// The header as the framework's [`PriorityLine`], so a narrow screen
/// drops its least important parts whole and says so with `…` instead of
/// cutting one mid-word.
///
/// Header shape (ADR 0078):
///
/// ```text
/// [updated Ns ago · ] N/M sessions · M mux[ · K stopped]
///   [  ·  <opt-in harness chips>]
///   [  ·  ▲ N when ambiguous > 0]
/// ```
///
/// The `Conspectus` brand and `sessions` view-label words moved out: the
/// brand was self-evident inside the TUI and the active view was already
/// in the left-panel title strip with stronger visual weight. The
/// mux-state chip section collapsed to a single `▲ N` because ADR 0072
/// made the per-row chip binary and every group row already owns the
/// ambiguity glyph. Per-harness chips became opt-in via
/// `[tui] show_harness_chips`. Freshness promoted to the lead position.
pub(super) fn header_line(app: &App) -> PriorityLine {
    let theme = app.theme();
    let (agents_total, mux_total) = snapshot_counts(app.snapshot_handle());
    let visible_sessions = visible_agent_session_count(app, agents_total);
    let session_cell = format_count_with_filtered(visible_sessions, agents_total);

    // The counts render bold, separators included; chips after them
    // carry their own colors.
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut line = PriorityLine::new(theme).separator(Span::styled(marks::SPACED_SEPARATOR, bold));
    if let Some(freshness) = header_freshness(app) {
        line.push(Segment::new(Span::styled(freshness, bold)).rank(header_rank::FRESHNESS));
    }
    line.push(Segment::new(span!(bold; "{session_cell} sessions")));
    line.push(Segment::new(span!(bold; "{mux_total} mux")).rank(header_rank::MUX));
    let gone = gone_mux_count(app.snapshot_handle());
    if gone > 0 {
        line.push(Segment::new(span!(bold; "{gone} stopped")).rank(header_rank::STOPPED));
    }

    // Counts walk the visible row tree, matching the count rule.
    let counts = HeaderCounts::from_app(app);
    if app.config().show_harness_chips {
        push_harness_chips(&mut line, &counts, theme);
    }
    if counts.mux_ambiguous > 0 {
        push_ambiguity_chip(&mut line, counts.mux_ambiguous, theme);
    }
    line
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

/// The gaps before the header's chip sections. `SECTION_SEPARATOR` sets
/// the optional opt-in and triage chips apart from the counts; the
/// per-chip `CHIP_SEPARATOR` joins individual harness chips within the
/// opt-in section.
pub(super) const CHIP_SEPARATOR: &str = "  ";

pub(super) const SECTION_SEPARATOR: &str = "  ·  ";

/// Add the opt-in per-harness chips (ADR 0078), one segment each: the
/// first set apart from the counts, the rest a chip's gap from each
/// other. They share a rank, so on a narrow screen they go from the end
/// and the first stays longest. Nothing when the row tree is empty, so a
/// freshly launched dashboard has no hanging separator.
pub(super) fn push_harness_chips(line: &mut PriorityLine, counts: &HeaderCounts, theme: &Theme) {
    use crate::tui::widgets::badge::harness_badge;
    for (index, (label, count)) in counts.by_harness.iter().enumerate() {
        let gap = if index == 0 {
            SECTION_SEPARATOR
        } else {
            CHIP_SEPARATOR
        };
        line.push(
            Segment::new(vec![harness_badge(label, theme), span!(" {count}")])
                .rank(header_rank::HARNESS_CHIPS)
                .gap(gap),
        );
    }
}

/// Add the ambiguity triage chip (`▲ N`), only called when N > 0, set
/// apart from what precedes it. Uses ADR 0072's `▲` vocabulary so the
/// header signal aligns with the per-group glyph the row tree already
/// shows.
pub(super) fn push_ambiguity_chip(line: &mut PriorityLine, ambiguous: usize, theme: &Theme) {
    let chip = StatusChip::new(marks::WARNING, Tone::Warning)
        .bold()
        .span(theme);
    line.push(
        Segment::new(vec![chip, span!(" {ambiguous}")])
            .rank(header_rank::AMBIGUITY)
            .gap(SECTION_SEPARATOR),
    );
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

/// The header's `updated Ns ago`, or `None` before a snapshot has
/// loaded.
pub(super) fn header_freshness(app: &App) -> Option<String> {
    let loaded_at = app.loaded_at_epoch()?;
    let now = current_unix_epoch_for_render();
    Some(format!("updated {}", Age::between(loaded_at, now).ago()))
}

pub(super) fn draw_status_bar(app: &App, buf: &mut Buffer, area: Rect) {
    let theme = app.theme();
    if let Some(status) = app.status() {
        let mut spans = Vec::new();
        push_unseen_messages_chip(app, theme, &mut spans);
        push_in_flight_chips(app, theme, &mut spans);
        // In its severity's color: a hint, a confirmation, a refusal,
        // and a failure each look like what they are.
        spans.push(status.span(theme));
        Paragraph::new(Line::from(spans)).render(area, buf);
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
        spans.push(
            StatusChip::new("stale", Tone::Warning)
                .bold()
                .pad(0, 2)
                .span(theme),
        );
    }
    let mut provider_spans = Vec::new();
    push_provider_chips(app, theme, &mut provider_spans);
    let reserved = text::spans_width(&spans) + text::spans_width(&provider_spans);
    let room = usize::from(area.width).saturating_sub(reserved);
    spans.extend(status_segments(app, theme).spans(room));
    spans.extend(provider_spans);

    Paragraph::new(Line::from(spans)).render(area, buf);
}

/// ADR 0105: warnings and errors the operator hasn't opened the
/// Messages overlay for stay flagged after their status message is
/// cleared. Leads the bar so long hints can't push it off screen.
pub(super) fn push_unseen_messages_chip(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let unseen = app.messages().unseen();
    if unseen == 0 {
        return;
    }
    spans.push(
        StatusChip::new(format!("{unseen} · ! messages"), Tone::Error)
            .mark(marks::WARNING)
            .bold()
            .pad(0, 2)
            .span(theme),
    );
}

/// The busy spinner's frame for an op started at `started_at`: the
/// framework's spinner, advanced by wall-clock time, so it needs no
/// tick of its own.
pub(super) fn spinner_glyph(started_at: std::time::Instant) -> &'static str {
    tuikit::chip::spinner_frame(started_at.elapsed())
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
    for op in ops {
        spans.push(StatusChip::busy(op.label.as_str(), op.started_at.elapsed()).span(theme));
        spans.push(Span::raw("  "));
    }
}

/// Append right-side provider status chips to `spans` from
/// `App::provider_status`.
pub(super) fn push_provider_chips(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let status = app.provider_status();

    let mut push_chip = |label: String, tone: Tone| {
        spans.push(Span::raw("  "));
        spans.push(StatusChip::new(label, tone).span(theme));
    };

    if status.tmux_disabled {
        push_chip("tmux:off".to_string(), Tone::Muted);
    } else if let Some(false) = status.tmux_available {
        let reason = status.tmux_reason.as_deref().unwrap_or("unavailable");
        push_chip(format!("tmux:{reason}"), Tone::Warning);
    }

    if status.forge_disabled {
        push_chip("forge:off".to_string(), Tone::Muted);
    } else if let Some(false) = status.forge_available {
        let reason = status.forge_reason.as_deref().unwrap_or("error");
        // Reasons name `gh` themselves ("gh is not authenticated");
        // the chip's prefix already does.
        let reason = reason.strip_prefix("gh ").unwrap_or(reason);
        push_chip(format!("gh:{reason}"), Tone::Warning);
    }
}

/// How long each piece of the status bar holds on as the bar narrows, the
/// highest longest. No two share a rank, so the bar packs what fits in
/// this order: what a narrowed view is narrowed by, the way back, and the
/// way out first; the refresh hint last.
mod status_rank {
    pub(super) const FIND: u32 = 11;
    pub(super) const BACK: u32 = 10;
    pub(super) const QUIT: u32 = 9;
    pub(super) const FILTER: u32 = 8;
    /// The right pane's keys, with the right pane focused: they matter
    /// more than the action hint, which describes the left pane's row.
    pub(super) const RIGHT_PANE_KEYS: u32 = 7;
    pub(super) const DEFAULT_ACTION: u32 = 6;
    pub(super) const GROUP: u32 = 5;
    pub(super) const SORT: u32 = 4;
    pub(super) const LEFT_PANE_KEYS: u32 = 3;
    pub(super) const FOCUS: u32 = 2;
    pub(super) const REFRESH: u32 = 1;
}

/// The active view state in a compact, always-visible form so grouping,
/// filtering, and sorting are discoverable without opening the controls
/// overlay, as separately droppable pieces, each with the rank it keeps
/// when the bar is too narrow for all of them.
fn view_state_segments(app: &App) -> Vec<(u32, String)> {
    let filter = render_filter_chips(app.filter());
    let filter = if filter.is_empty() {
        "all".to_string()
    } else {
        filter
    };
    let mut segments = vec![
        (
            status_rank::GROUP,
            format!("group:{}", app.grouping().as_str()),
        ),
        (status_rank::FILTER, format!("filter:{filter}")),
        (
            status_rank::SORT,
            format!("sort:{}", sort_chip_label(app.sort())),
        ),
    ];
    if let Some(find) = render_find_chip(app) {
        segments.push((status_rank::FIND, find));
    }
    segments
}

/// The content search as a chip: its text, then how far the scan has
/// got, or how many sessions it found once it is done.
pub(super) fn render_find_chip(app: &App) -> Option<String> {
    let content = app.content()?;
    let text = text::truncate_end(&content.text, 24);
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

/// The key hints as separately droppable pieces with the rank each keeps
/// on a narrow bar.
fn contextual_status_segments(app: &App) -> Vec<(u32, String)> {
    let (focus_rank, focus_hint) = match app.focus() {
        Focus::Left => (
            status_rank::LEFT_PANE_KEYS,
            "j/k move · h/l fold · Enter default".to_string(),
        ),
        // With the right pane focused, its own table's hint: moving,
        // Enter with what it does on the cursor's row (copy a field,
        // follow a link, fold the Other zone), folding, and going back.
        Focus::Right => (status_rank::RIGHT_PANE_KEYS, right_pane_hint(app)),
    };
    let refresh_hint = if app.active_view() == crate::tui::View::Prs {
        "r refresh · ^R forges"
    } else {
        "r refresh"
    };
    let mut segments = vec![
        (status_rank::DEFAULT_ACTION, default_action_status_hint(app)),
        (focus_rank, focus_hint),
        (status_rank::FOCUS, "Tab focus".to_string()),
        (status_rank::REFRESH, refresh_hint.to_string()),
        (status_rank::QUIT, "q quit".to_string()),
    ];
    if app.focus() == Focus::Left && app.can_nav_back() {
        segments.push((status_rank::BACK, "Backspace back".to_string()));
    }
    segments
}

/// The right pane's keys as the status bar names them, generated from
/// the table it resolves: the explorer's footer, whose `Enter` verb is
/// the cursor row's, or the table's hint with no explorer to read.
fn right_pane_hint(app: &App) -> String {
    use crate::tui::keybindings::RIGHT_PANE_KEYS;
    match (app.explorer(), app.graph_source()) {
        (Some(explorer), Some(source)) => explorer.footer(RIGHT_PANE_KEYS, &source).text(),
        _ => tuikit::explorer::hint(RIGHT_PANE_KEYS).text(),
    }
}

/// The status bar's settings chips and key hints as the framework's
/// [`PriorityLine`]: in the room the bar's fixed parts leave, pieces drop
/// by rank (the least important first), the room is filled back with
/// whatever still fits, a `…` marks that some were dropped, and nothing
/// is cut mid-word. The settings take the `cwd_mark` color, the hints
/// recede.
fn status_segments(app: &App, theme: &Theme) -> PriorityLine {
    let settings = Style::default().fg(theme.cwd_mark());
    let hints = Style::default().add_modifier(theme.muted);
    let mut line = PriorityLine::new(theme);
    for (rank, text) in view_state_segments(app) {
        line.push(Segment::new(Span::styled(text, settings)).rank(rank));
    }
    for (rank, text) in contextual_status_segments(app) {
        line.push(Segment::new(Span::styled(text, hints)).rank(rank));
    }
    line
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
