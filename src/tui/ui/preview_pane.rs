//! Preview pane: pane captures, session previews, and pin diagnostics.

use super::*;

pub(super) fn draw_detail_preview(
    app: &App,
    _detail: &NodeDetail,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    let preview = preview_text_for_selection(app, area.width, area.height as usize);
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
/// - Muxed agent session, mux candidate, or mux row: the tmux pane
///   capture, or the privacy banner with `--no-live-preview`.
/// - Pin placeholder: the pin's live pane when its mux exists, else
///   the pin's diagnostics.
/// - Other rows: a "no preview" placeholder.
pub(super) fn preview_text_for_selection(app: &App, width: u16, height: usize) -> Text<'static> {
    match latest_failure_for_selection(app) {
        Some(entry) => {
            // The pane body gets the rows the banner leaves, so its
            // newest output stays on screen below the banner.
            let mut text = failure_banner(entry, app.theme());
            let banner_rows = wrapped_line_count(&text.lines, width);
            let body_height = height.saturating_sub(banner_rows).max(1);
            text.extend(selection_preview_body(app, width, body_height));
            text
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

fn selection_preview_body(app: &App, width: u16, height: usize) -> Text<'static> {
    let Some(selection) = app.selection() else {
        return Text::raw("");
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return Text::raw("");
    };
    let live_preview = app.config().live_preview_enabled;
    if let RowId::Pin { pin_id } = selection {
        return pin_placeholder_preview(app, pin_id, live_preview, width, height);
    }
    match &row.kind {
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => {
                mux_preview_text(app, live_preview, width, height)
            }
            MuxIndicator::Unmuxed => Text::raw(
                session
                    .preview
                    .clone()
                    .unwrap_or_else(|| "no preview available".to_string()),
            ),
        },
        RowKind::AgentSessionMuxCandidate(_) => mux_preview_text(app, live_preview, width, height),
        RowKind::Pin(_) => {
            let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
            if diagnostics.is_empty() {
                Text::raw("pin diagnostic unavailable — try `r` to refresh")
            } else {
                Text::raw(render_pin_diagnostics(&diagnostics))
            }
        }
        _ => match selection {
            RowId::Group(NodeId::MuxSession(_)) => {
                mux_preview_text(app, live_preview, width, height)
            }
            RowId::MuxSession(NodeId::MuxSession(_)) => {
                mux_preview_text(app, live_preview, width, height)
            }
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

/// Preview for a pin placeholder row: the live pane when the pin's
/// mux exists (a stale mux, or a pin running something other than an
/// agent harness, like `conspectus serve`), else the pin's
/// diagnostics, which say what `Enter` will do.
fn pin_placeholder_preview(
    app: &App,
    pin_id: &str,
    live_preview: bool,
    width: u16,
    height: usize,
) -> Text<'static> {
    let live_mux = app
        .snapshot_handle()
        .and_then(|handle| crate::tui::actions::pin_live_mux(handle.snapshot(), pin_id));
    if live_mux.is_some() {
        return mux_preview_text(app, live_preview, width, height);
    }
    let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
    if diagnostics.is_empty() {
        Text::raw("pin diagnostic unavailable — try `r` to refresh")
    } else {
        Text::raw(render_pin_diagnostics(&diagnostics))
    }
}

/// Compose the preview body for a row whose preview source is a
/// tmux pane capture. Pulls from the per-mux cache; if no cache
/// entry exists yet (the runtime hasn't refreshed for this
/// selection), shows a "loading" placeholder. With
/// `--no-live-preview`, swaps in the privacy banner instead.
pub(super) fn mux_preview_text(
    app: &App,
    live_preview: bool,
    width: u16,
    height: usize,
) -> Text<'static> {
    if !live_preview {
        return Text::raw("preview disabled (--no-live-preview)");
    }
    let Some(target) = resolve_attach_target(app).ok().map(|t| t.mux) else {
        return Text::raw("no mux target for this row");
    };
    format_preview_for_mux(app, &target, width, height, app.config().color)
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
