//! Header line and status bar: counts, chips, and contextual hints (ADR 0078).

use tuikit::keys::Footer;
use tuikit::priority_line::{PriorityLine, Segment};
use tuikit::status_bar::{Annotation, KeyHintBar, StatusBar};

use super::*;
use crate::tui::keybindings::{MAIN_VIEW_KEYS, MainKey, RIGHT_PANE_KEYS, status_rank};
use crate::tui::keymap::{SelectedDefault, selected_default_action};

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
    status_bar(app).render(area, buf);
}

/// The status bar as the framework's [`StatusBar`] (ADR 0078): a message
/// in its severity's color, or the view's settings and the keys of the
/// scope that has them, generated from their key tables. The unseen and
/// busy chips lead it either way; the scope tag (the focused pane, while
/// no overlay has the keys) and the stale mark lead the keys, and the
/// provider chips follow them. The settings and the keys give way by rank
/// to fit the cells those leave.
pub(super) fn status_bar(app: &App) -> StatusBar<'_> {
    let theme = app.theme();
    let mut lead = Vec::new();
    push_unseen_messages_chip(app, theme, &mut lead);
    push_in_flight_chips(app, theme, &mut lead);
    let bar = StatusBar::new(theme).lead(lead).message(app.status());
    if bar.shows_message() {
        return bar;
    }

    let mut scope = Vec::new();
    if !app.has_modal() {
        let tag = match app.focus() {
            Focus::Left => "[left]",
            Focus::Right => "[right]",
        };
        scope.push(span!(theme.muted; "{tag} "));
    }
    if app.refresh_failure().is_some() {
        // Kept outside the line, so the stale mark is never the thing
        // that gets pushed off the end.
        scope.push(
            StatusChip::new("stale", Tone::Warning)
                .bold()
                .pad(0, 2)
                .span(theme),
        );
    }
    let mut trail = Vec::new();
    push_provider_chips(app, theme, &mut trail);

    let settings = Style::default().fg(theme.cwd_mark());
    let mut bar = bar.scope(scope);
    for (rank, text) in view_state_segments(app) {
        bar = bar.segment(Segment::new(Span::styled(text, settings)).rank(rank));
    }
    bar.hints(&status_hints(app)).trail(trail)
}

/// ADR 0105: warnings and errors the operator hasn't opened the
/// Messages overlay for stay flagged after their status message is
/// cleared. Leads the bar so long hints can't push it off screen. It
/// names the key that opens the log, except while an overlay has the
/// keys and the key would not reach it.
pub(super) fn push_unseen_messages_chip(app: &App, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let unseen = app.messages().unseen();
    if unseen == 0 {
        return;
    }
    let text = if app.has_modal() {
        unseen.to_string()
    } else {
        format!(
            "{unseen} · {}",
            crate::tui::keybindings::messages_hint("messages")
        )
    };
    spans.push(
        StatusChip::new(text, Tone::Error)
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

/// The keys the status bar names: those of the scope that has them,
/// generated from its key tables. With an overlay on top, its own keys
/// (`Overlay::hints`). On the left pane, the main view's table, each row
/// worded for the selection. On the right pane, `a` when the right pane's
/// entity attaches, the right pane's table (with `Enter`'s verb from the
/// cursor's row), and the program's keys.
pub(crate) fn status_hints(app: &App) -> KeyHintBar {
    if let Some(hints) = app.top_modal_hints() {
        return KeyHintBar::new().footer(&hints, status_rank::OVERLAY_KEYS);
    }
    let selection = selection_keys(app);
    let program = |key: &MainKey| match key {
        MainKey::Focus => Some(match app.focus() {
            Focus::Left => selection.tab.clone().unwrap_or_default(),
            Focus::Right => Annotation::new(),
        }),
        MainKey::Refresh | MainKey::Quit => Some(Annotation::new()),
        // A hard refresh asks the forges again, which only the PRs view
        // shows.
        MainKey::HardRefresh => (app.active_view() == View::Prs).then(Annotation::new),
        _ => None,
    };
    match app.focus() {
        Focus::Left => {
            KeyHintBar::new().annotated(MAIN_VIEW_KEYS, status_rank::LEFT_PANE_KEYS, |key| {
                match key {
                    MainKey::Default => selection.enter.clone(),
                    MainKey::Attach => selection.attach.clone(),
                    MainKey::View => selection.view.clone(),
                    MainKey::Resume => selection.resume.then(Annotation::new),
                    MainKey::Menu => selection.menu.clone(),
                    MainKey::Up | MainKey::Down | MainKey::Collapse | MainKey::Expand => {
                        Some(Annotation::new())
                    }
                    MainKey::Back => app.can_nav_back().then(Annotation::new),
                    // The unseen-messages chip names it.
                    MainKey::Messages => None,
                    other => program(other),
                }
            })
        }
        Focus::Right => KeyHintBar::new()
            .annotated(
                MAIN_VIEW_KEYS,
                status_rank::DEFAULT_ACTION,
                |key| match key {
                    MainKey::Attach => selection.attach.clone().filter(Annotation::is_available),
                    _ => None,
                },
            )
            .footer(&right_pane_footer(app), status_rank::RIGHT_PANE_KEYS)
            .annotated(MAIN_VIEW_KEYS, status_rank::LEFT_PANE_KEYS, program),
    }
}

/// The right pane's keys as the status bar names them, generated from the
/// table it resolves: the explorer's footer, whose `Enter` verb is the
/// cursor row's, or the table's hint with no explorer to read.
fn right_pane_footer(app: &App) -> Footer {
    match (app.explorer(), app.graph_source()) {
        (Some(explorer), Some(source)) => explorer.footer(RIGHT_PANE_KEYS, &source),
        _ => tuikit::explorer::hint(RIGHT_PANE_KEYS),
    }
}

/// What the keys that act on the selection do, as the status bar words
/// them: each an annotation on its row of [`MAIN_VIEW_KEYS`], or `None`
/// when the key does nothing worth naming here.
#[derive(Debug, Default)]
pub(super) struct SelectionKeys {
    /// `Enter` on the left pane: the row's default action.
    pub(super) enter: Option<Annotation>,
    /// `a`: attach, or why it cannot.
    pub(super) attach: Option<Annotation>,
    /// `v`: open the transcript, where `Enter` does too.
    pub(super) view: Option<Annotation>,
    /// `S`: resume the un-muxed session.
    pub(super) resume: bool,
    /// `.`: the row's menu.
    pub(super) menu: Option<Annotation>,
    /// `Tab`, when going to the right pane is what the selection needs:
    /// its competing mux candidates.
    pub(super) tab: Option<Annotation>,
}

/// What the selection's keys do: `Enter` by the dispatcher's own
/// classification of the row ([`selected_default_action`]), the others by
/// the resolvers their keys run. A group row folds; a search hit opens
/// the viewer at the match; a history record rebuilds; a pin whose state
/// decides says so; a row that attaches names its mux (the preferred one
/// of several, pointing `Tab` at the candidates); an un-muxed session
/// names itself for the viewer and offers `S` when it can resume; any
/// other row says why `a` cannot attach.
pub(super) fn selection_keys(app: &App) -> SelectionKeys {
    let mut keys = SelectionKeys::default();
    let attach = resolve_attach_target(app);
    let row = app
        .selection()
        .and_then(|selection| app.tree().rows.iter().find(|row| &row.id == selection));
    let Some(row) = row else {
        keys.attach = attach.err().map(|reason| refused_attach(&reason));
        return keys;
    };
    if matches!(row.kind, RowKind::Group(_)) {
        keys.enter = Some(Annotation::new().verb("fold"));
        return keys;
    }
    if crate::tui::actions::viewer_open_target(app).is_some() {
        let open = Annotation::new().verb("open").detail("at the match");
        keys.enter = Some(open.clone());
        keys.view = Some(open);
        keys.menu = Some(Annotation::new());
        return keys;
    }
    if crate::tui::actions::selected_record(app).is_some() {
        keys.enter = Some(Annotation::new().verb("rebuild"));
        keys.menu = Some(Annotation::new().detail("(pin, forget, plan, clean up)"));
        return keys;
    }
    let pin = match (&row.id, &row.kind) {
        (RowId::Pin { pin_id }, _) => Some(pin_id.as_str()),
        (_, RowKind::AgentSession(session)) => session.pin_id.as_deref(),
        (_, RowKind::MuxSession(mux)) => mux.pin_id.as_deref(),
        _ => None,
    };
    if let Some(enter) = pin
        .zip(app.snapshot_handle())
        .and_then(|(pin, handle)| pin_enter(handle.snapshot(), pin))
    {
        keys.enter = Some(enter);
        return keys;
    }
    let default = selected_default_action(app);
    let other_enter = |keys: &mut SelectionKeys| match default {
        SelectedDefault::ToggleExpand if row.expandable => {
            keys.enter = Some(Annotation::new().verb("fold"));
        }
        SelectedDefault::Scrollback => {
            keys.enter = Some(Annotation::new().verb("open").detail("scrollback"));
        }
        _ => {}
    };
    match attach {
        Ok(target) => {
            let label = target.label();
            let ambiguous = matches!(
                selected_mux_state(app),
                Some(MuxIndicator::Ambiguous { .. })
            );
            let detail = if ambiguous {
                keys.tab = Some(
                    Annotation::new()
                        .verb("inspect candidates")
                        .rank(status_rank::DEFAULT_ACTION),
                );
                format!("preferred {label}")
            } else {
                compact_mux_label(&label)
            };
            let attaches = Annotation::new().verb("attach").detail(detail);
            if default == SelectedDefault::Attach {
                keys.enter = Some(attaches.clone());
            } else {
                other_enter(&mut keys);
            }
            keys.attach = Some(attaches);
        }
        Err(reason) => {
            // An un-muxed agent session opens in the viewer instead,
            // with `Enter` as with `v`.
            if let RowKind::AgentSession(session) = &row.kind
                && matches!(session.mux_state, MuxIndicator::Unmuxed)
            {
                let view = Annotation::new()
                    .verb("view")
                    .detail(compact_session_label(session));
                keys.enter = Some(view.clone());
                keys.view = Some(view);
                keys.resume = app.snapshot_handle().is_some_and(|handle| {
                    matches!(
                        crate::tui::resume::resolve_resume_target_in(
                            handle.snapshot(),
                            &session.session
                        ),
                        crate::tui::resume::ResumeTarget::Launch { .. }
                    )
                });
                return keys;
            }
            let refused = refused_attach(&reason);
            if default == SelectedDefault::Attach {
                keys.enter = Some(refused.clone());
            } else {
                other_enter(&mut keys);
            }
            keys.attach = Some(refused);
        }
    }
    keys
}

/// `a attach`, dimmed, with why attaching is refused.
fn refused_attach(reason: &crate::tui::actions::AttachDisabled) -> Annotation {
    Annotation::new()
        .verb("attach")
        .because(crate::tui::actions::attach_disabled_why(reason))
}

/// What `Enter` does on a row that is, or carries, the pin `pin_id`, when
/// the pin's state decides it, as `default_effect` does: a live pull
/// request shows its URL; a pin whose target is gone, missing, or not
/// built yet rebuilds; one whose target could not be checked, or a live
/// one with nothing to rebuild, cannot. `None` for a live pin to a mux or
/// a session, whose row attaches or opens as any other does.
fn pin_enter(snapshot: &crate::model::GraphSnapshot, pin_id: &str) -> Option<Annotation> {
    use crate::model::{PinKind, SlotStatus};
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == pin_id)?;
    let rebuild = Annotation::new().verb("rebuild");
    Some(match (pin.status, pin.kind) {
        (SlotStatus::Live, PinKind::Mux | PinKind::Session) => return None,
        (SlotStatus::Live, PinKind::Pr) => Annotation::new().verb("show URL"),
        (SlotStatus::Unknown, _) => rebuild.because("couldn't check whether its target exists"),
        (
            SlotStatus::Gone | SlotStatus::Missing | SlotStatus::Planned,
            PinKind::Mux | PinKind::Session | PinKind::Checkout | PinKind::Repo,
        ) => rebuild,
        _ => rebuild.unavailable(),
    })
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
