//! Preview pane: transcripts, pane captures, and pin diagnostics.

use super::*;
use crate::tui::preview::{PreviewSource, PreviewSubject, node_source, preview_subject};

/// A Preview body, and where its window sits when it scrolls itself.
pub(super) struct PreviewView {
    pub text: Text<'static>,
    /// Set when the body is already cut to the zone, as a transcript
    /// is: the rows it has in all and the first one shown, for the
    /// scrollbar. `None` scrolls the text by the Preview's offset from
    /// the top.
    pub window: Option<PreviewWindow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreviewWindow {
    pub total: usize,
    pub top: usize,
}

impl PreviewView {
    pub(super) fn text(text: impl Into<Text<'static>>) -> Self {
        Self {
            text: text.into(),
            window: None,
        }
    }

    /// Put `header` above this view, as a failure banner or a Related
    /// row's evidence leads the zone.
    pub(super) fn below(self, header: Text<'static>) -> Self {
        let mut text = header;
        text.extend(self.text);
        Self {
            text,
            window: self.window,
        }
    }
}

pub(super) fn draw_detail_preview(
    app: &App,
    _detail: &NodeDetail,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    let view = preview_view_for_selection(app, area.width, area.height as usize);
    draw_preview_view(app, view, frame, area);
}

/// Draw a Preview body: scrolled from the top by the Preview's offset,
/// or, for a view that cut its own window, as is with a scrollbar.
pub(super) fn draw_preview_view(app: &App, view: PreviewView, frame: &mut Frame<'_>, area: Rect) {
    match view.window {
        None => {
            let total_rows = wrapped_line_count(&view.text.lines, area.width);
            let (content_area, scrollbar_area) = scrollbar_layout(area, total_rows);
            let widget = Paragraph::new(view.text)
                .wrap(Wrap { trim: false })
                .scroll((app.preview_scroll(), 0));
            frame.render_widget(widget, content_area);
            if let Some(sb_area) = scrollbar_area {
                render_vertical_scrollbar(
                    frame,
                    sb_area,
                    total_rows,
                    app.preview_scroll() as usize,
                );
            }
        }
        Some(window) => {
            // The thumb tracks the transcript; a failure banner above
            // it stays put.
            let (content_area, scrollbar_area) =
                scrollbar_layout(area, window.total.max(view.text.lines.len()));
            frame.render_widget(
                Paragraph::new(view.text).wrap(Wrap { trim: false }),
                content_area,
            );
            if let Some(sb_area) = scrollbar_area {
                render_vertical_scrollbar(frame, sb_area, window.total, window.top);
            }
        }
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
/// shows; rows without one show pin diagnostics or a placeholder.
pub(super) fn preview_view_for_selection(app: &App, width: u16, height: usize) -> PreviewView {
    match latest_failure_for_selection(app) {
        Some(entry) => {
            // The body gets the rows the banner leaves, so its newest
            // output stays on screen below the banner.
            let banner = failure_banner(entry, app.theme());
            let banner_rows = wrapped_line_count(&banner.lines, width);
            let body_height = height.saturating_sub(banner_rows).max(1);
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
    app.messages().latest_failure_for(&targets)
}

fn failure_banner(entry: &crate::tui::messages::LogEntry, theme: &Theme) -> Text<'static> {
    let style = crate::tui::widgets::messages::level_style(entry.level, theme);
    let mut lines = vec![Line::from(Span::styled(
        format!("{} {}", entry.level.glyph(), entry.summary),
        style.add_modifier(Modifier::BOLD),
    ))];
    if let Some(code) = entry.command.as_ref().and_then(|c| c.exit_code) {
        lines.push(Line::styled(format!("exit status {code}"), style));
    }
    for line in entry.output_tail(FAILURE_BANNER_LINES) {
        lines.push(Line::raw(format!("  {line}")));
    }
    lines.push(Line::styled(
        "! for the full output".to_string(),
        Style::default().add_modifier(theme.placeholder),
    ));
    lines.push(Line::styled(
        "─".repeat(40),
        Style::default().add_modifier(theme.divider),
    ));
    Text::from(lines)
}

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
            None => PreviewView::text(sourceless_preview(app, row, selection, live_preview)),
        },
        PreviewSubject::Node(node) => match node_source(app, &node) {
            Some(source) => source_view(app, &source, width, height),
            None => PreviewView::text("no preview for this node"),
        },
        PreviewSubject::Field(value) => source_view(
            app,
            &crate::tui::preview::value_source(&value),
            width,
            height,
        ),
        PreviewSubject::Unresolved => PreviewView::text(""),
    }
}

/// The Preview of a loaded source, or why it can't show one.
pub(super) fn source_view(
    app: &App,
    source: &PreviewSource,
    width: u16,
    height: usize,
) -> PreviewView {
    if source.is_live() && !app.config().live_preview_enabled {
        // `--no-live-preview` hides conversation and terminal content
        // (ADR 0112 §6).
        return PreviewView::text("preview disabled (--no-live-preview)");
    }
    match source {
        PreviewSource::Path(_) | PreviewSource::Git(_) => loaded_view(app, source),
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

/// Preview for a row with no source: a pin's diagnostics, which say
/// what `Enter` will do, or a placeholder.
fn sourceless_preview(
    app: &App,
    row: &crate::tui::rows::Row,
    selection: &RowId,
    live_preview: bool,
) -> Text<'static> {
    let pin_diagnostics = || {
        let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
        if diagnostics.is_empty() {
            Text::raw("pin diagnostic unavailable — try `r` to refresh")
        } else {
            Text::raw(render_pin_diagnostics(&diagnostics))
        }
    };
    let disabled_or = |text: &'static str| {
        Text::raw(if live_preview {
            text
        } else {
            "preview disabled (--no-live-preview)"
        })
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
                pin_diagnostics()
            }
        }
        (RowKind::Pin(_), _) => pin_diagnostics(),
        (RowKind::AgentSession(_) | RowKind::AgentSessionMuxCandidate(_), _)
        | (_, RowId::Group(NodeId::MuxSession(_)) | RowId::MuxSession(NodeId::MuxSession(_))) => {
            disabled_or("no mux target for this row")
        }
        _ => disabled_or("no preview for this row"),
    }
}

/// A loaded source composed as lines, scrolled from the top, or what
/// stands in for it.
fn loaded_view(app: &App, source: &PreviewSource) -> PreviewView {
    let Some(entry) = app.previews().get(source) else {
        return PreviewView::text("loading…");
    };
    match &entry.content {
        PreviewContent::Lines(lines) => PreviewView::text(lines.clone()),
        PreviewContent::Unavailable(reason) => PreviewView::text(reason.clone()),
        PreviewContent::Failed(message) => PreviewView::text(message.clone()),
        _ => PreviewView::text(""),
    }
}

/// A transcript, rendered by the native viewer (ADR 0052) and
/// bottom-anchored with its newest turns in view. The Preview's scroll
/// offset counts rows up from there. One column stays free for the
/// scrollbar.
fn transcript_view(app: &App, source: &PreviewSource, width: u16, height: usize) -> PreviewView {
    let Some(entry) = app.previews().get(source) else {
        return PreviewView::text("loading transcript…");
    };
    let state = match &entry.content {
        PreviewContent::Transcript(state) => state,
        PreviewContent::Unavailable(reason) => {
            return PreviewView::text(format!("transcript unavailable: {reason}"));
        }
        PreviewContent::Failed(message) => {
            return PreviewView::text(format!("transcript unreadable: {message}"));
        }
        _ => return PreviewView::text("transcript unavailable"),
    };
    let content_width = width.saturating_sub(1).max(1);
    // Draw warms the cache first (`App::prepare_preview`); callers
    // that skip it, like tests, compose a copy.
    let lines: std::borrow::Cow<'_, [Line<'static>]> =
        if let Some(lines) = crate::viewer::widget::cached_body_lines(state, content_width) {
            std::borrow::Cow::Borrowed(lines)
        } else {
            let mut state = state.as_ref().clone();
            std::borrow::Cow::Owned(
                crate::viewer::widget::body_lines(&mut state, app.theme(), content_width).to_vec(),
            )
        };
    if lines.is_empty() {
        return PreviewView::text("no turns yet");
    }
    let total = lines.len();
    let height = height.max(1);
    let bottom_top = total.saturating_sub(height);
    let top = bottom_top.saturating_sub(app.preview_scroll() as usize);
    let end = (top + height).min(total);
    PreviewView {
        text: Text::from(lines[top..end].to_vec()),
        window: Some(PreviewWindow { total, top }),
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
                let parsed = render_captured_pane(&capture.text, color);
                let rows = crate::tui::preview_wrap::layout_capture(
                    &parsed.lines,
                    app.preview_wrap(),
                    capture.width,
                    width,
                );
                if rows.is_empty() {
                    return Text::raw("(empty pane)");
                }
                let start = rows.len().saturating_sub(height.max(1));
                Text::from(rows[start..].to_vec())
            }
            PreviewContent::NoTarget => Text::raw("tmux target not found — try `r` to refresh"),
            PreviewContent::Unavailable(reason) => Text::raw(format!("tmux unavailable: {reason}")),
            PreviewContent::Failed(message) => Text::raw(message.clone()),
            PreviewContent::Unsupported => {
                Text::raw("preview unavailable (runner does not implement capture)")
            }
            // A pane source only ever holds a capture.
            PreviewContent::Transcript(_) | PreviewContent::Lines(_) => Text::raw(""),
        },
        None => Text::raw("loading mux preview…"),
    }
}

pub(super) fn render_pin_diagnostics(
    diagnostics: &[crate::tui::actions::PinDiagnosticView],
) -> String {
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
            crate::tui::actions::PinDiagnosticView::StaleMux {
                pin_id,
                mux,
                claimed_elsewhere,
                harness_running,
            } => {
                let action = if *harness_running {
                    "The harness is running there but its session isn't identified; \
                     Enter attaches."
                } else {
                    "Enter relaunches the harness in the existing mux."
                };
                let mut text = format!(
                    "Pin `{pin_id}` has a stale mux.\nMux: {}\n{action}",
                    mux.native_id
                );
                for claim in claimed_elsewhere {
                    text.push_str(&format!(
                        "\nSession {}:{} matched here but is bound to pin `{}`.",
                        claim.session.harness_key, claim.session.session_key, claim.claimed_by_pin
                    ));
                }
                text
            }
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
pub(super) fn render_captured_pane(text: &str, color: bool) -> Text<'static> {
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

/// Build the chip-style divider above the preview body. Sits in
/// the same right-anchored position as the section dividers
/// (Session / Mux / PR / Lineage) so the `[ Preview ]` chip lines
/// up below them. No inline suffix: the mux pane label and
/// captured-time freshness already render in the Mux section
/// above, and a second copy here pushed the chip far to the left.
pub(super) fn preview_divider_line(_app: &App, width: usize, theme: &Theme) -> Line<'static> {
    chip_divider_line("Preview", None, width, theme, ChipAnchor::Left)
}
