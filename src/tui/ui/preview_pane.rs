//! Preview pane: transcripts, pane captures, and pin diagnostics.

use tuikit::empty_state::EmptyState;
use tuikit::frame::SectionRule;

use super::*;
use crate::tui::keybindings::key_name;
use crate::tui::preview::{
    PreviewSource, PreviewSubject, TranscriptPreview, node_source, preview_subject,
};

/// A Preview body, and where its window sits when it scrolls itself.
pub(super) struct PreviewView {
    pub text: Text<'static>,
    /// Set when the body is already cut to the zone, as a transcript
    /// is: the Preview's scroll region placed over all of its rows, for
    /// the scrollbar. `None` scrolls the text by the Preview's offset
    /// from the top.
    pub region: Option<ScrollRegion>,
}

impl PreviewView {
    pub(super) fn text(text: impl Into<Text<'static>>) -> Self {
        Self {
            text: text.into(),
            region: None,
        }
    }

    /// Put `header` above this view, as a failure banner or a Related
    /// row's evidence leads the zone.
    pub(super) fn below(self, header: Text<'static>) -> Self {
        let mut text = header;
        text.extend(self.text);
        Self {
            text,
            region: self.region,
        }
    }
}

/// Draw the selected row's Preview and return its scroll region as the
/// body drew it.
pub(super) fn draw_detail_preview(app: &App, buf: &mut Buffer, area: Rect) -> ScrollRegion {
    let view = preview_view_for_selection(app, area.width, area.height as usize);
    draw_preview_view(app, view, buf, area)
}

/// Draw a Preview body through the Preview's scroll region (ADR 0181):
/// wrapped and scrolled from the top by its offset, a leading banner
/// with it, or, for a view that cut its own window, as is under a banner
/// that stays put. Returns the region as drawn, over the rows its offset
/// counts.
pub(super) fn draw_preview_view(
    app: &App,
    view: PreviewView,
    buf: &mut Buffer,
    area: Rect,
) -> ScrollRegion {
    match view.region {
        None => app
            .preview_region()
            .draw_lines(Anchor::Top, area, buf, view.text),
        // The thumb tracks the transcript; a failure banner above it
        // stays put.
        Some(placed) => placed.draw_cut(area, buf, view.text),
    }
}

/// The Preview body as text. Tests read the zone through this.
#[cfg(test)]
pub(super) fn preview_text_for_selection(app: &App, width: u16, height: usize) -> Text<'static> {
    preview_view_for_selection(app, width, height).text
}

/// Source the preview body from whatever the selection points at
/// (ADR 0112), led by the failure banner when the row's last operation
/// failed (ADR 0105). See [`preview_source`] for which source each row
/// shows; rows without one show their fields or a placeholder.
pub(super) fn preview_view_for_selection(app: &App, width: u16, height: usize) -> PreviewView {
    match latest_failure_for_selection(app) {
        Some(entry) => {
            // The body gets the rows the banner leaves, so its newest
            // output stays on screen below the banner.
            let banner = failure_banner(entry, app.theme());
            let body_height = rows_under(&banner.lines, width, height);
            selection_preview_body(app, width, body_height).below(banner)
        }
        None => selection_preview_body(app, width, height),
    }
}

/// Lines of output a failure banner shows before pointing at `!`.
const FAILURE_BANNER_LINES: usize = 6;

/// ADR 0105: the selected row's pin or mux, when its most recent
/// logged outcome is a warning or error.
fn latest_failure_for_selection(app: &App) -> Option<&crate::tui::messages::LogEntry> {
    use crate::tui::messages::LogTarget;
    let selection = app.selection()?;
    let row = app.tree().rows.iter().find(|r| &r.id == selection)?;
    let mut targets = Vec::new();
    let pin_id = match (&row.kind, selection) {
        (_, RowId::Pin { pin_id }) => Some(pin_id.clone()),
        (RowKind::AgentSession(session), _) => session.pin_id.clone(),
        (RowKind::MuxSession(mux), _) => mux.pin_id.clone(),
        _ => None,
    };
    if let Some(pin_id) = pin_id {
        targets.push(LogTarget::Pin(pin_id));
    }
    if let Ok(target) = resolve_attach_target(app) {
        targets.push(LogTarget::Mux(target.mux));
    }
    crate::tui::messages::latest_failure_for(app.messages(), &targets)
}

fn failure_banner(entry: &crate::tui::messages::LogEntry, theme: &Theme) -> Text<'static> {
    use crate::tui::messages::LogEntryExt;
    let style = entry.severity.style(theme);
    let mut lines = vec![Line::from(Span::styled(
        format!("{} {}", entry.severity.mark(), entry.summary),
        style.add_modifier(Modifier::BOLD),
    ))];
    if let Some(code) = entry.item.command.as_ref().and_then(|c| c.exit_code) {
        lines.push(Line::styled(format!("exit status {code}"), style));
    }
    for line in entry.output_tail(FAILURE_BANNER_LINES) {
        lines.push(Line::raw(format!("  {line}")));
    }
    lines.push(Line::styled(
        "! for the full output".to_string(),
        Style::default().add_modifier(theme.muted),
    ));
    lines.push(Line::styled(
        marks::RULE.repeat(40),
        Style::default().add_modifier(theme.muted),
    ));
    Text::from(lines)
}

/// A Preview with nothing to show, for now or at all (not loaded yet,
/// no source, switched off), as the framework's empty state: its message
/// wrapped to the zone's `width`, receding. A Preview that failed to load
/// says why in the body's own look instead; that is content.
pub(super) fn placeholder(app: &App, message: &str, width: u16) -> Text<'static> {
    Text::from(EmptyState::new(message).lines(usize::from(width), app.theme()))
}

/// What a Preview says when `--no-live-preview` hides the conversation
/// and terminal content (ADR 0112 §6).
const LIVE_PREVIEW_OFF: &str = "preview disabled (--no-live-preview)";

fn selection_preview_body(app: &App, width: u16, height: usize) -> PreviewView {
    let Some(selection) = app.selection() else {
        return PreviewView::text("");
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return PreviewView::text("");
    };
    let live_preview = app.config().live_preview_enabled;
    match preview_subject(app) {
        PreviewSubject::Selection => match crate::tui::preview::preview_source(app) {
            Some(source) => source_view(app, &source, width, height),
            None => PreviewView::text(sourceless_preview(app, row, selection, live_preview, width)),
        },
        PreviewSubject::Node(node) => match node_source(app, &node) {
            Some(source) => source_view(app, &source, width, height),
            None => PreviewView::text(
                sourceless_node_text(app, &node)
                    .unwrap_or_else(|| placeholder(app, "no preview for this node", width)),
            ),
        },
        PreviewSubject::Field(value) => field_view(app, &value, width, height),
        PreviewSubject::Unresolved => PreviewView::text(""),
    }
}

/// A field row's preview by the type of its value (ADR 0112 §2).
fn field_view(
    app: &App,
    value: &crate::tui::explorer::FieldValue,
    width: u16,
    height: usize,
) -> PreviewView {
    use crate::tui::explorer::FieldValue;
    if let Some(lines) = crate::tui::preview::value_lines(
        value,
        Some(crate::tui::rows::tree_current_unix_epoch()),
        app.theme().muted,
    ) {
        return PreviewView::text(lines);
    }
    match crate::tui::preview::value_source(app, value) {
        Some(source) => source_view(app, &source, width, height),
        None => match value {
            FieldValue::Node(node) => PreviewView::text(
                sourceless_node_text(app, node)
                    .unwrap_or_else(|| placeholder(app, "no preview for this node", width)),
            ),
            _ => PreviewView::text(""),
        },
    }
}

/// What a node with no source previews instead: a slot's status,
/// which says why it has no session or pane to show (ADR 0112 §1).
pub(super) fn sourceless_node_text(app: &App, node: &NodeId) -> Option<Text<'static>> {
    let NodeId::Pin(pin) = node else {
        return None;
    };
    let snapshot = app.snapshot_handle()?.snapshot();
    render_pin_summary(snapshot, &pin.id).map(Text::raw)
}

/// The Preview of a loaded source, or why it can't show one.
pub(super) fn source_view(
    app: &App,
    source: &PreviewSource,
    width: u16,
    height: usize,
) -> PreviewView {
    if source.is_live() && !app.config().live_preview_enabled {
        return PreviewView::text(placeholder(app, LIVE_PREVIEW_OFF, width));
    }
    match source {
        PreviewSource::Path(_)
        | PreviewSource::Git(_)
        | PreviewSource::Process(_)
        | PreviewSource::PullRequest(_) => loaded_view(app, source, width),
        PreviewSource::Transcript(_) => transcript_view(app, source, width, height),
        PreviewSource::Pane(mux) => PreviewView::text(format_preview_for_mux(
            app,
            mux,
            width,
            height,
            app.config().color,
        )),
    }
}

/// Preview for a row with no source: a slot's status, or a
/// placeholder.
fn sourceless_preview(
    app: &App,
    row: &crate::tui::rows::Row,
    selection: &RowId,
    live_preview: bool,
    width: u16,
) -> Text<'static> {
    let pin_summary = |pin_id: &str| {
        app.snapshot_handle()
            .and_then(|handle| render_pin_summary(handle.snapshot(), pin_id))
            .map_or_else(
                || {
                    Text::raw(format!(
                        "pin unavailable — try `{}` to refresh",
                        key_name("refresh")
                    ))
                },
                Text::raw,
            )
    };
    let disabled_or = |text: &'static str| {
        placeholder(
            app,
            if live_preview { text } else { LIVE_PREVIEW_OFF },
            width,
        )
    };
    match (&row.kind, selection) {
        (_, RowId::Pin { pin_id }) => {
            // A pin whose mux is live but can't be attached (it hosts
            // this TUI) has a pane but no target to capture.
            let live_mux = app
                .snapshot_handle()
                .and_then(|handle| crate::tui::actions::pin_live_mux(handle.snapshot(), pin_id));
            if live_mux.is_some() {
                disabled_or("no mux target for this row")
            } else {
                pin_summary(pin_id)
            }
        }
        (RowKind::AgentSession(_), _)
        | (_, RowId::Group(NodeId::MuxSession(_)) | RowId::MuxSession(NodeId::MuxSession(_))) => {
            disabled_or("no mux target for this row")
        }
        _ => disabled_or("no preview for this row"),
    }
}

/// A loaded source composed as lines, scrolled from the top, or what
/// stands in for it.
fn loaded_view(app: &App, source: &PreviewSource, width: u16) -> PreviewView {
    let Some(entry) = app.previews().get(source) else {
        return PreviewView::text(placeholder(app, "loading…", width));
    };
    match &entry.content {
        PreviewContent::Lines(lines) => PreviewView::text(lines.clone()),
        PreviewContent::Unavailable(reason) => PreviewView::text(reason.clone()),
        PreviewContent::Failed(message) => PreviewView::text(message.clone()),
        _ => PreviewView::text(""),
    }
}

/// A transcript, rendered by the native viewer (ADR 0052) and
/// bottom-anchored with its newest turns in view, or anchored at the
/// search match it opens on. The Preview's scroll region counts rows
/// from that anchor (ADR 0181). One column stays free for the scrollbar.
fn transcript_view(app: &App, source: &PreviewSource, width: u16, height: usize) -> PreviewView {
    let Some(entry) = app.previews().get(source) else {
        return PreviewView::text(placeholder(app, "loading transcript…", width));
    };
    let preview = match &entry.content {
        PreviewContent::Transcript(preview) => preview,
        PreviewContent::Unavailable(reason) => {
            return PreviewView::text(format!("transcript unavailable: {reason}"));
        }
        PreviewContent::Failed(message) => {
            return PreviewView::text(format!("transcript unreadable: {message}"));
        }
        _ => return PreviewView::text("transcript unavailable"),
    };
    let content_width = width.saturating_sub(1).max(1);
    let search = crate::tui::preview::search::preview_search(app);
    // Draw warms the cache first (`App::prepare_preview`); callers
    // that skip it, like tests, compose a copy.
    let composed;
    let preview: &TranscriptPreview = if preview.transcript.cached_lines(content_width).is_some() {
        preview
    } else {
        let mut copy = preview.as_ref().clone();
        crate::tui::preview::search::apply(&mut copy, search.as_ref());
        copy.transcript.lines(app.theme(), content_width);
        composed = copy;
        &composed
    };
    let Some(cache) = preview.transcript.rendered.as_ref() else {
        return PreviewView::text(placeholder(app, "no turns yet", width));
    };
    let lines = &cache.lines;
    if lines.is_empty() {
        return PreviewView::text(placeholder(app, "no turns yet", width));
    }
    let total = lines.len();
    let height = height.max(1);
    let region = app.preview_region();
    let placed = match crate::tui::preview::search::match_top(preview) {
        // Opened on a search match: the offset counts rows from it.
        Some(row) if search.as_ref().is_some_and(|s| s.turn.is_some()) => {
            region.marked(row).place(Anchor::Mark, height, total)
        }
        _ => region.place(Anchor::Bottom, height, total),
    };
    let shown = placed.window().visible();
    let top = shown.start;
    let mut window = lines[shown].to_vec();
    if search.is_some() {
        preview.marks.paint(&mut window, top, cache, app.theme());
    }
    PreviewView {
        text: Text::from(window),
        region: Some(placed),
    }
}

/// Lay the pane capture out per the active wrap mode (ADR 0106) and
/// keep the bottom `height` rows: the pane's newest output.
pub(super) fn format_preview_for_mux(
    app: &App,
    mux: &MuxSessionId,
    width: u16,
    height: usize,
    color: bool,
) -> Text<'static> {
    match app.mux_preview(mux) {
        Some(entry) => match &entry.content {
            PreviewContent::Text(capture) => {
                // ADR 0025 and ADR 0106, through the framework's rich
                // text (ADR 0181): the escapes parsed (or dropped without
                // color), then the capture fitted in the active wrap mode.
                let parsed = tuikit::richtext::ansi_text(&capture.text, color);
                let rows = tuikit::richtext::layout_capture(
                    &parsed.lines,
                    app.preview_wrap(),
                    capture.width,
                    width,
                );
                if rows.is_empty() {
                    return placeholder(app, "(empty pane)", width);
                }
                if let Some(search) = crate::tui::preview::search::preview_search(app) {
                    crate::tui::preview::search::mark_rows(
                        rows,
                        &search.query,
                        app.theme().match_style(),
                        height,
                    )
                } else {
                    let start = rows.len().saturating_sub(height.max(1));
                    Text::from(rows[start..].to_vec())
                }
            }
            PreviewContent::NoTarget => Text::raw(format!(
                "tmux target not found — try `{}` to refresh",
                key_name("refresh")
            )),
            PreviewContent::Unavailable(reason) => Text::raw(format!("tmux unavailable: {reason}")),
            PreviewContent::Failed(message) => Text::raw(message.clone()),
            PreviewContent::Unsupported => {
                Text::raw("preview unavailable (runner does not implement capture)")
            }
            // A pane source only ever holds a capture.
            PreviewContent::Transcript(_) | PreviewContent::Lines(_) => Text::raw(""),
        },
        None => placeholder(app, "loading mux preview…", width),
    }
}

/// What a slot previews when it has no pane or session to show: its
/// status and, when it needs attention, what to do about it.
pub(super) fn render_pin_summary(
    snapshot: &crate::model::GraphSnapshot,
    pin_id: &str,
) -> Option<String> {
    let pin = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == pin_id)?;
    let mut text = format!(
        "Pin `{pin_id}` ({} · {})",
        pin.kind.as_str(),
        pin.status.label_for(pin.kind)
    );
    if let Some(hint) = crate::tui::actions::pin_status_hint(snapshot, pin_id) {
        text.push('\n');
        text.push_str(&hint);
    }
    Some(text)
}

/// The `Preview` rule above the preview body. Its chip ends in the same
/// column as the `Related` rule's, so the two labels line up. It carries
/// no summary.
pub(super) fn preview_divider_line(width: usize, theme: &Theme) -> Line<'static> {
    SectionRule::new("Preview").line(width, theme)
}
