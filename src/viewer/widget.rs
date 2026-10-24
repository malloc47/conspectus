//! Full-screen Ratatui modal for the transcript viewer.
//!
//! Layout (top → bottom):
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ <harness>:<key> · <cwd> · N turns                  HH:MM:SS │  header
//! │─────────────────────────────────────────────────────────────│  rule
//! │                                                             │
//! │       you │ what's up?                                      │  body
//! │           │                                                 │
//! │       ai │ Not much.                                       │
//! │           │                                                 │
//! │  Thinking │ <italic dim thinking text>                      │
//! │      Tool │ Read: /path                                     │
//! │  ↳ Result │ <dim tool output>                               │
//! │                                                             │
//! │─────────────────────────────────────────────────────────────│  rule
//! │ [ 42/137 ] tools·off Think·on  q close · j/k · g/G · …      │  footer
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! The widget is the only piece of `src/viewer/` that touches
//! `ratatui::*` outside the renderer. Per ADR 0052 the renderer
//! never branches on harness; it consumes only the normalized model.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::viewer::model::{TranscriptDocument, TurnKind};
use crate::viewer::render::{
    ToolCategory, aggregate_tool_phrase, categorize_tool_name, chip_label, chip_label_width,
    into_owned_line, leader_width, render_aggregated_tool_summary, render_turn,
    tool_name_from_body,
};
use crate::viewer::state::{RenderCache, ToolDetail, ViewerState};
use crate::viewer::theme::Theme;

/// Draw the transcript viewer onto `area`. Mutates `state` to
/// record the layout it observed (viewport height, total line
/// count) so the reducer can use those numbers on the next key
/// event.
pub fn draw(state: &mut ViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // separator
        Constraint::Min(1),    // body
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(&state.document, theme, frame, chunks[0]);
    draw_separator(theme, frame, chunks[1]);
    draw_body(state, theme, frame, chunks[2]);
    draw_separator(theme, frame, chunks[3]);
    draw_footer(state, theme, frame, chunks[4]);
}

fn draw_header(document: &TranscriptDocument, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let title = format!("{}:{}", document.meta.harness, document.meta.session_key);
    let title_style = harness_chip_style(&document.meta.harness, theme);

    let cwd_text = document.meta.cwd.as_deref().unwrap_or("(no cwd)");
    let turn_count_text = format!(
        "{} {}",
        document.turns.len(),
        if document.turns.len() == 1 {
            "turn"
        } else {
            "turns"
        }
    );
    let sep = " · ";
    let sep_style = Style::new()
        .fg(theme.secondary_text)
        .add_modifier(Modifier::DIM);

    let spans: Vec<Span<'_>> = vec![
        Span::raw(" "),
        Span::styled(title, title_style),
        Span::styled(sep, sep_style),
        Span::styled(
            cwd_text.to_string(),
            Style::new().fg(theme.cwd_mark).add_modifier(Modifier::DIM),
        ),
        Span::styled(sep, sep_style),
        Span::styled(turn_count_text, sep_style),
    ];

    // Width-aware: if the assembled header would overflow, drop
    // the lowest-priority pieces (cwd first, then turn count) so
    // the title always survives. Cheap implementation: build a
    // single string, measure, and rebuild minimal if needed.
    let total: usize = spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let cap = area.width as usize;
    let line = if total <= cap {
        Line::from(spans)
    } else {
        // Fall back to just the title + ellipsis.
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("{}:{}", document.meta.harness, document.meta.session_key),
                harness_chip_style(&document.meta.harness, theme),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_separator(theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let rule: String = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            rule,
            Style::new()
                .fg(theme.secondary_text)
                .add_modifier(Modifier::DIM),
        ))),
        area,
    );
}

fn draw_body(state: &mut ViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    // Empty-document banner short-circuits before we ever touch the
    // turn list.
    if state.document.is_empty() {
        state.viewport_height = area.height;
        state.total_lines = 0;
        state.scroll_offset = 0;
        let banner = Line::from(Span::styled(
            "transcript unavailable",
            Style::new().add_modifier(Modifier::DIM),
        ));
        frame.render_widget(
            Paragraph::new(vec![banner]).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let stick_to_end = state.stick_to_end;
    let requested_offset = state.scroll_offset;
    let lines = body_lines(state, theme, area.width);
    let total = lines.len();
    let viewport_height = area.height;
    let max_offset = total.saturating_sub(viewport_height as usize);
    let scroll_offset = if stick_to_end {
        max_offset
    } else {
        requested_offset.min(max_offset)
    };

    // The cached lines are already wrapped to `content_width`, so the
    // visible window is a plain slice. Slicing (rather than
    // `Paragraph::scroll`, whose offset is a u16) keeps transcripts
    // longer than 65,535 lines scrollable and clones only what is shown.
    let visible_end = total.min(scroll_offset + viewport_height as usize);
    let paragraph = Paragraph::new(lines[scroll_offset..visible_end].to_vec());
    frame.render_widget(paragraph, area);

    state.viewport_height = viewport_height;
    state.total_lines = total;
    state.scroll_offset = scroll_offset;
}

/// Compose the transcript body at `width` columns, gutter included, and
/// return its lines. The composition is cached on `state` and reused
/// while the width and the display toggles are unchanged, so callers
/// scroll by slicing rather than re-rendering. The full-screen modal
/// and the inline preview (ADR 0112) both draw from it. An empty
/// document composes no lines; callers show their own "unavailable"
/// state.
pub fn body_lines<'s>(
    state: &'s mut ViewerState,
    theme: &Theme,
    width: u16,
) -> &'s [Line<'static>] {
    if state.document.is_empty() {
        return &[];
    }
    let gutter_inner_width = compute_gutter_inner_width(state);
    let content_width = width.saturating_sub(leader_width(gutter_inner_width));

    if !cache_is_current(state, content_width) {
        let owned: Vec<Line<'static>> =
            build_body_lines(state, theme, content_width, gutter_inner_width)
                .into_iter()
                .map(into_owned_line)
                .collect();
        state.rendered = Some(RenderCache {
            content_width,
            tool_detail: state.tool_detail,
            show_thinking: state.show_thinking,
            show_aborted: state.show_aborted,
            show_hidden: state.show_hidden,
            lines: owned,
        });
    }
    &state.rendered.as_ref().expect("just populated").lines
}

/// The body lines [`body_lines`] last composed, when they were composed
/// at `width` with the current display toggles. Lets a caller that only
/// holds `&ViewerState` reuse the cache it warmed earlier in the frame.
pub fn cached_body_lines(state: &ViewerState, width: u16) -> Option<&[Line<'static>]> {
    let content_width = width.saturating_sub(leader_width(compute_gutter_inner_width(state)));
    if !cache_is_current(state, content_width) {
        return None;
    }
    state.rendered.as_ref().map(|cache| cache.lines.as_slice())
}

/// Whether the cached composition matches `content_width` and the
/// current display toggles. This is the difference between scrolling
/// by keystroke being O(turns × markdown render) and being O(1) on a
/// large session with tools shown.
fn cache_is_current(state: &ViewerState, content_width: u16) -> bool {
    state.rendered.as_ref().is_some_and(|cache| {
        cache.content_width == content_width
            && cache.tool_detail == state.tool_detail
            && cache.show_thinking == state.show_thinking
            && cache.show_aborted == state.show_aborted
            && cache.show_hidden == state.show_hidden
    })
}

fn draw_footer(state: &ViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let position = format!(
        "[ {}/{} ]",
        state
            .scroll_offset
            .saturating_add(1)
            .min(state.total_lines.max(1)),
        state.total_lines.max(1),
    );

    let chip_style = Style::new().add_modifier(Modifier::BOLD);
    let off_style = Style::new()
        .fg(theme.secondary_text)
        .add_modifier(Modifier::DIM);
    let on_style = Style::new().fg(theme.success).add_modifier(Modifier::BOLD);
    let pipe = Span::styled(
        " · ",
        Style::new()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::DIM),
    );

    // Toggle chips inline the binding letter so the key + label
    // travel together (`tools·sum (t)`). Keeps the operator's eyes
    // on one column instead of cross-referencing a separate
    // shortcut hint.
    let tools_text = tool_chip_label(state.tool_detail);
    let thinking_text = toggle_chip("think", state.show_thinking, 'T');
    let aborted_text = abort_chip_label(state.show_aborted);
    let hidden_text = hidden_chip_label(state.show_hidden);

    // Long hint trails. `?` for help is its own chip so it stands
    // out; `q close` is the last item per the operator-preferred
    // ordering (close should sit where the muscle memory lands
    // when ready to exit).
    let help_chip = "(?) help";
    let close_chip = "(q) close";
    let scroll_hint = "j/k scroll · g/G top/end · PgUp/PgDn";

    // The hidden chip only appears for transcripts whose parser marked
    // turns hidden; for the rest it would toggle nothing.
    let has_hidden = state.document.turns.iter().any(|turn| turn.hidden);

    let tools_on = state.tool_detail.is_visible();
    let mut spans: Vec<Span<'_>> = vec![
        Span::styled(position, chip_style),
        pipe.clone(),
        Span::styled(tools_text, if tools_on { on_style } else { off_style }),
        pipe.clone(),
        Span::styled(
            thinking_text,
            if state.show_thinking {
                on_style
            } else {
                off_style
            },
        ),
        pipe.clone(),
        Span::styled(
            aborted_text,
            if state.show_aborted {
                on_style
            } else {
                off_style
            },
        ),
        pipe.clone(),
    ];
    if has_hidden {
        spans.push(Span::styled(
            hidden_text,
            if state.show_hidden {
                on_style
            } else {
                off_style
            },
        ));
        spans.push(pipe.clone());
    }
    spans.extend([
        Span::styled(help_chip, off_style),
        pipe.clone(),
        Span::styled(scroll_hint, off_style),
        pipe,
        Span::styled(close_chip, off_style),
    ]);

    // Width-aware truncate. Tiers, picked by what fits:
    //   1. Full assembly above.
    //   2. Drop scroll hint + help chip, keep position · tools · think
    //      · aborted · hidden · close.
    //   3. Keep one filter chip: hidden when the transcript has hidden
    //      turns and no aborted ones, aborted otherwise.
    //   4. Drop that chip too. Those filters are rarely flipped, and the
    //      tools / think chips reflect ongoing state — those stay until
    //      close itself is at risk.
    let total: usize = spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let cap = area.width as usize;
    let line = if total <= cap {
        Line::from(spans)
    } else {
        let position_span = Span::styled(
            format!(
                "[ {}/{} ]",
                state
                    .scroll_offset
                    .saturating_add(1)
                    .min(state.total_lines.max(1)),
                state.total_lines.max(1)
            ),
            chip_style,
        );
        let tools_span = Span::styled(
            format!(" · {}", tool_chip_label(state.tool_detail)),
            if tools_on { on_style } else { off_style },
        );
        let think_span = Span::styled(
            format!(" · {}", toggle_chip("think", state.show_thinking, 'T')),
            if state.show_thinking {
                on_style
            } else {
                off_style
            },
        );
        let aborted_span = Span::styled(
            format!(" · {}", abort_chip_label(state.show_aborted)),
            if state.show_aborted {
                on_style
            } else {
                off_style
            },
        );
        let hidden_span = Span::styled(
            format!(" · {}", hidden_chip_label(state.show_hidden)),
            if state.show_hidden {
                on_style
            } else {
                off_style
            },
        );
        let close_span = Span::styled(format!(" · {close_chip}"), off_style);
        let base = [position_span, tools_span, think_span];
        let mut tiers: Vec<Vec<Span<'_>>> = Vec::new();
        if has_hidden {
            tiers.push(
                base.iter()
                    .cloned()
                    .chain([
                        aborted_span.clone(),
                        hidden_span.clone(),
                        close_span.clone(),
                    ])
                    .collect(),
            );
        }
        // With room for one filter chip, keep the one that filters
        // something in this transcript.
        let has_aborted = state.document.turns.iter().any(|turn| turn.aborted);
        let single = if has_hidden && !has_aborted {
            hidden_span
        } else {
            aborted_span
        };
        tiers.push(
            base.iter()
                .cloned()
                .chain([single, close_span.clone()])
                .collect(),
        );
        let width = |spans: &[Span<'_>]| -> usize {
            spans
                .iter()
                .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                .sum()
        };
        let chosen = tiers
            .into_iter()
            .find(|tier| width(tier) <= cap)
            .unwrap_or_else(|| base.into_iter().chain([close_span]).collect());
        Line::from(chosen)
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// `tools·sum (t)` / `tools·off (t)` style chip. Carries the
/// current ToolDetail value so the operator sees both the level
/// and the cycle key together.
fn tool_chip_label(detail: ToolDetail) -> String {
    let label = detail.footer_label();
    // Pad to 5 cells so the chip width is stable as the state
    // cycles (Hidden=3 char, Summary=3, Truncated=5, Full=3).
    format!("tools·{label:<5} (t)")
}

/// `think·on  (T)` / `think·off (T)` style toggle chip. The key
/// binding is inlined so the operator doesn't need a separate
/// hint to know how to flip it.
fn toggle_chip(name: &str, on: bool, key: char) -> String {
    let state_label = if on { "on " } else { "off" };
    format!("{name}·{state_label} ({key})")
}

/// `aborted·hide (I)` / `aborted·show (I)` chip. Uses `hide`/`show`
/// instead of `on`/`off` because the operator question isn't "is the
/// flag on" but "am I looking at the on-disk reality or the
/// harness-faithful view". Pad so the chip width stays stable.
fn abort_chip_label(showing: bool) -> String {
    let state_label = if showing { "show" } else { "hide" };
    format!("aborted·{state_label} (I)")
}

/// `hidden·hide (H)` / `hidden·show (H)` chip for records the
/// harness's UI never showed (ADR 0122). Same `hide`/`show` wording as
/// the aborted chip.
fn hidden_chip_label(showing: bool) -> String {
    let state_label = if showing { "show" } else { "hide" };
    format!("hidden·{state_label} (H)")
}

/// Compute the inner gutter chip-label width from the widest
/// label among visible turns. This allows the gutter to shrink
/// when wider labels (e.g. `thinking`) are toggled off.
fn compute_gutter_inner_width(state: &ViewerState) -> usize {
    let mut max_width = 0usize;
    for turn in &state.document.turns {
        if !passes_turn_filters(turn, state) || !is_visible(turn.kind, state) {
            continue;
        }
        let label = chip_label(turn);
        max_width = max_width.max(chip_label_width(label));
    }
    max_width.max(1)
}

/// Build the flat body line list. Filters out tool turns at
/// `ToolDetail::Hidden` and thinking turns when `show_thinking`
/// is off. At `ToolDetail::Summary` consecutive runs of tool
/// turns are aggregated into a single claude-history-style line
/// ("Read 2 files, ran 4 shell commands, edited 2 files") rather
/// than emitting one chip per call. Other tool-detail levels
/// affect how each turn renders — that's `render_turn`'s job.
fn build_body_lines<'a>(
    state: &'a ViewerState,
    theme: &Theme,
    content_width: u16,
    gutter_inner_width: usize,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();
    let turns = &state.document.turns;
    let mut i = 0;
    while i < turns.len() {
        let turn = &turns[i];
        if !passes_turn_filters(turn, state) || !is_visible(turn.kind, state) {
            i += 1;
            continue;
        }
        // Summary mode: walk the contiguous tool-turn run and
        // emit one aggregated line for the whole run.
        if state.tool_detail == ToolDetail::Summary && is_tool_kind(turn.kind) {
            let (counts, run_len) = collect_tool_run(&turns[i..], state);
            if !counts.is_empty() {
                let phrase = aggregate_tool_phrase(&counts);
                let owned: Vec<Line<'static>> = render_aggregated_tool_summary(
                    &phrase,
                    theme,
                    content_width,
                    gutter_inner_width,
                );
                lines.extend(owned);
            }
            i += run_len.max(1);
            continue;
        }
        lines.extend(render_turn(
            turn,
            theme,
            content_width,
            state.tool_detail,
            gutter_inner_width,
        ));
        i += 1;
    }
    lines
}

fn is_tool_kind(k: TurnKind) -> bool {
    matches!(k, TurnKind::ToolUse | TurnKind::ToolResult)
}

/// Count consecutive tool turns by category. Only [`TurnKind::ToolUse`]
/// turns contribute to the count (one call = one ToolUse in our
/// model); the paired ToolResult lives in the same run but isn't
/// double-counted.
fn collect_tool_run(
    turns: &[crate::viewer::model::TranscriptTurn],
    state: &ViewerState,
) -> (std::collections::BTreeMap<ToolCategory, usize>, usize) {
    use crate::viewer::model::TurnKind as TK;
    let mut counts: std::collections::BTreeMap<ToolCategory, usize> =
        std::collections::BTreeMap::new();
    let mut end = 0usize;
    for turn in turns {
        if !is_tool_kind(turn.kind) {
            break;
        }
        // Skip aborted tool turns when the filter is on so the
        // aggregate phrase reflects what the user saw, not the
        // on-disk count.
        if !passes_turn_filters(turn, state) {
            end += 1;
            continue;
        }
        if matches!(turn.kind, TK::ToolUse) {
            let name = tool_name_from_body(&turn.body);
            *counts.entry(categorize_tool_name(name)).or_insert(0) += 1;
        }
        end += 1;
    }
    (counts, end)
}

fn is_visible(kind: TurnKind, state: &ViewerState) -> bool {
    match kind {
        TurnKind::Message | TurnKind::CompactionSummary => true,
        TurnKind::ToolUse | TurnKind::ToolResult => state.tool_detail.is_visible(),
        TurnKind::Thinking => state.show_thinking,
    }
}

/// Per-turn filter applied before any kind-based visibility check.
/// With `show_aborted` and `show_hidden` off (the default), turns the
/// parser tagged `aborted` or `hidden` drop out — matching what the
/// agent's UI showed the user. The toggles restore the full on-disk
/// transcript.
fn passes_turn_filters(turn: &crate::viewer::model::TranscriptTurn, state: &ViewerState) -> bool {
    (state.show_aborted || !turn.aborted) && (state.show_hidden || !turn.hidden)
}

/// Pick a harness-identity color for the title chip. Delegates to
/// [`Theme::harness_color`] so the per-harness color
/// lookup lives in one place (the theme + adapter registry) rather
/// than a match table here.
fn harness_chip_style(harness: &str, theme: &Theme) -> Style {
    Style::new()
        .fg(theme.harness_color(harness))
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
pub fn render_to_buffer(
    state: &mut ViewerState,
    theme: &Theme,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| draw(state, theme, frame, frame.area()))
        .expect("draw on test backend");
    terminal.backend().buffer().clone()
}

#[cfg(test)]
pub fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let mut out =
        String::with_capacity((buffer.area.width as usize + 1) * buffer.area.height as usize);
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
#[path = "widget_tests.rs"]
mod tests;
