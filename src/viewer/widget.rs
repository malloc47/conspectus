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

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::chip::{PositionChip, StatusChip, ToggleChip, Tone};
use tuikit::glyphs::marks;

use crate::viewer::highlight::highlight_matches;
use crate::viewer::model::{TranscriptDocument, TurnKind};
use crate::viewer::render::{
    ToolCategory, aggregate_tool_phrase, categorize_tool_name, chip_label, chip_label_width,
    into_owned_line, leader_width, render_aggregated_tool_summary, render_turn,
    tool_name_from_body,
};
use crate::viewer::state::{RenderCache, ToolDetail, TurnStart, ViewerState};
use crate::viewer::theme::Theme;

/// Draw the transcript viewer onto `area`. Mutates `state` to
/// record the layout it observed (viewport height, total line
/// count) so the reducer can use those numbers on the next key
/// event.
pub fn draw(state: &mut ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // separator
        Constraint::Min(1),    // body
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(&state.document, theme, buf, chunks[0]);
    draw_separator(theme, buf, chunks[1]);
    draw_body(state, theme, buf, chunks[2]);
    draw_separator(theme, buf, chunks[3]);
    draw_footer(state, theme, buf, chunks[4]);
}

fn draw_header(document: &TranscriptDocument, theme: &Theme, buf: &mut Buffer, area: Rect) {
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
    let sep_style = Style::new().fg(theme.inert).add_modifier(theme.muted);

    let spans: Vec<Span<'_>> = vec![
        Span::raw(" "),
        Span::styled(title, title_style),
        Span::styled(sep, sep_style),
        Span::styled(
            cwd_text.to_string(),
            Style::new().fg(theme.cwd_mark()).add_modifier(theme.muted),
        ),
        Span::styled(sep, sep_style),
        Span::styled(turn_count_text, sep_style),
    ];

    // Width-aware: if the assembled header would overflow, drop
    // the lowest-priority pieces (cwd first, then turn count) so
    // the title always survives. Cheap implementation: build a
    // single string, measure, and rebuild minimal if needed.
    let total = tuikit::text::spans_width(&spans);
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
    Widget::render(Paragraph::new(line), area, buf);
}

fn draw_separator(theme: &Theme, buf: &mut Buffer, area: Rect) {
    let rule: String = marks::RULE.repeat(area.width as usize);
    Widget::render(
        Paragraph::new(Line::from(Span::styled(
            rule,
            Style::new().fg(theme.inert).add_modifier(theme.muted),
        ))),
        area,
        buf,
    );
}

fn draw_body(state: &mut ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    // Empty-document banner short-circuits before we ever touch the
    // turn list.
    if state.document.is_empty() {
        state.viewport_height = area.height;
        state.total_lines = 0;
        state.scroll_offset = 0;
        let banner = Line::from(Span::styled(
            "transcript unavailable",
            Style::new().add_modifier(theme.muted),
        ));
        let rows = tuikit::text::wrap_line(&banner, usize::from(area.width), 0);
        Widget::render(Paragraph::new(rows), area, buf);
        return;
    }

    let total = body_lines(state, theme, area.width).len();
    // A find that asked to scroll to a turn lands on its first line, one
    // line down so the turn's neighbor above shows.
    if let Some(turn) = state.find.pending_scroll.take()
        && let Some(cache) = state.rendered.as_ref()
        && let Some(start) = cache.turn_starts.iter().find(|s| s.turn_index == turn)
    {
        state.scroll_offset = start.line.saturating_sub(1);
        state.stick_to_end = false;
    }
    let stick_to_end = state.stick_to_end;
    let requested_offset = state.scroll_offset;
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
    let cache = state.rendered.as_ref().expect("composed above");
    let mut visible = cache.lines[scroll_offset..visible_end].to_vec();
    highlight_matches(&mut visible, scroll_offset, cache, &state.find, theme);
    Widget::render(Paragraph::new(visible), area, buf);

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
        let mut turn_starts = Vec::new();
        let owned: Vec<Line<'static>> = build_body_lines(
            state,
            theme,
            content_width,
            gutter_inner_width,
            &mut turn_starts,
        )
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
            turn_starts,
            leader: usize::from(leader_width(gutter_inner_width)),
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

fn draw_footer(state: &ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    if let Some(prompt) = &state.find.prompt {
        draw_find_prompt(prompt, &state.find_hint, theme, buf, area);
        return;
    }
    let position = PositionChip::new(state.scroll_offset, state.total_lines);

    let off_style = Tone::Inert.style(theme);
    let pipe = Span::styled(" · ", off_style);

    // Toggle chips inline the binding letter so the key + label
    // travel together (`tools·sum (t)`). Keeps the operator's eyes
    // on one column instead of cross-referencing a separate
    // shortcut hint.
    let tool_states = ToolDetail::ALL.map(ToolDetail::footer_label);
    let tools = tool_chip(state.tool_detail, &tool_states);
    let thinking = think_chip(state.show_thinking);
    let aborted = abort_chip(state.show_aborted);
    let hidden = hidden_chip(state.show_hidden);

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

    let find_tone = if state.find.is_active() && state.find.matches.is_empty() {
        Tone::Warning
    } else {
        Tone::Success
    };
    let find = find_chip_text(state).map(|text| StatusChip::new(text, find_tone).bold());
    let mut spans: Vec<Span<'_>> = vec![
        position.span(),
        pipe.clone(),
        tools.span(theme),
        pipe.clone(),
        thinking.span(theme),
        pipe.clone(),
        aborted.span(theme),
        pipe.clone(),
    ];
    if has_hidden {
        spans.push(hidden.span(theme));
        spans.push(pipe.clone());
    }
    if let Some(find) = &find {
        spans.splice(2..2, [find.span(theme), pipe.clone()]);
    }
    spans.extend([
        Span::styled("(/) find", off_style),
        pipe.clone(),
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
    let total = tuikit::text::spans_width(&spans);
    let cap = area.width as usize;
    let line = if total <= cap {
        Line::from(spans)
    } else {
        // The tiers lead each chip with its separator, in the chip's
        // own style.
        let after_pipe = |text: String, style: Style| Span::styled(format!(" · {text}"), style);
        let tools_span = after_pipe(tools.text(), tools.look(theme));
        let think_span = after_pipe(thinking.text(), thinking.look(theme));
        let aborted_span = after_pipe(aborted.text(), aborted.look(theme));
        let hidden_span = after_pipe(hidden.text(), hidden.look(theme));
        let close_span = Span::styled(format!(" · {close_chip}"), off_style);
        let mut base = vec![position.span()];
        if let Some(find) = &find {
            base.push(after_pipe(find.text(), find.look(theme)));
        }
        base.extend([tools_span, think_span]);
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
        let width = |spans: &[Span<'_>]| -> usize { tuikit::text::spans_width(spans) };
        let chosen = tiers
            .into_iter()
            .find(|tier| width(tier) <= cap)
            .unwrap_or_else(|| base.into_iter().chain([close_span]).collect());
        Line::from(chosen)
    };
    Widget::render(Paragraph::new(line), area, buf);
}

/// The most cells the query takes in the footer chip, the ellipsis of a
/// cut query included.
const FIND_CHIP_QUERY_CELLS: usize = 17;

/// The find chip: the query with its position among the matches, or
/// `no match` (and how many matches the hidden turns hold), then the
/// one-key note if there is one. With no active find, just the note.
fn find_chip_text(state: &ViewerState) -> Option<String> {
    let find = &state.find;
    if !find.is_active() {
        return find.note.clone();
    }
    let query = tuikit::text::truncate_end(&find.query, FIND_CHIP_QUERY_CELLS);
    let mut text = match find.current {
        Some(index) => format!("find \"{query}\" {}/{}", index + 1, find.matches.len()),
        None => format!("find \"{query}\" no match"),
    };
    if find.matches.is_empty() && find.hidden_matches > 0 {
        text.push_str(&format!(" · {} in hidden turns", find.hidden_matches));
    }
    if let Some(note) = &find.note {
        text.push_str(" · ");
        text.push_str(note);
    }
    Some(text)
}

/// Draw the find prompt over the footer: `/` and the text with a block
/// cursor, then the key hint. A long query scrolls so the cursor stays
/// in view.
pub(crate) fn draw_find_prompt(
    prompt: &crate::viewer::find::FindPrompt,
    hint: &str,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
) {
    let hint_text = if hint.is_empty() {
        String::new()
    } else {
        format!("   {hint}")
    };
    let hint_cells = tuikit::text::width(&hint_text);
    let width = area.width as usize;
    // Cells for the text and the cursor after the leading `/`.
    let room = width.saturating_sub(1 + hint_cells).max(4);
    let chars: Vec<char> = prompt.text.chars().collect();
    let cells = |c: char| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
    let cursor = prompt.cursor.min(chars.len());
    let at_char = chars.get(cursor).copied();
    let at_cells = at_char.map_or(1, |c| cells(c).max(1));
    // Scroll so the cursor cell stays visible: drop text from the left
    // until what precedes the cursor and the cursor itself fit, counting
    // display cells (a CJK character is two) rather than characters.
    let mut start = 0usize;
    let mut before_cells: usize = chars[..cursor].iter().map(|&c| cells(c)).sum();
    while start < cursor && before_cells + at_cells > room {
        before_cells -= cells(chars[start]);
        start += 1;
    }
    let before: String = chars[start..cursor].iter().collect();
    let at: String = at_char.map_or_else(|| " ".to_string(), |c| c.to_string());
    let mut after = String::new();
    let mut used = before_cells + at_cells;
    for &c in chars.iter().skip(cursor + 1) {
        if used + cells(c) > room {
            break;
        }
        used += cells(c);
        after.push(c);
    }
    let line = Line::from(vec![
        Span::styled("/", Style::new().add_modifier(Modifier::BOLD)),
        Span::raw(before),
        Span::styled(at, Style::new().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
        Span::styled(
            hint_text,
            Style::new().fg(theme.inert).add_modifier(theme.muted),
        ),
    ]);
    Widget::render(Paragraph::new(line), area, buf);
}

/// `tools·sum (t)` / `tools·off (t)`: the current tool detail level and
/// its cycle key, the level padded to the widest of `states` so the chip
/// keeps its width as it cycles. On while tool turns show any body.
fn tool_chip<'a>(detail: ToolDetail, states: &'a [&'a str]) -> ToggleChip<'a> {
    ToggleChip::new("tools", detail.footer_label(), detail.is_visible())
        .states(states)
        .key("t")
}

/// `think·on  (T)` / `think·off (T)`. The key binding is inlined so the
/// operator doesn't need a separate hint to know how to flip it.
fn think_chip(on: bool) -> ToggleChip<'static> {
    ToggleChip::new("think", if on { "on" } else { "off" }, on)
        .states(&["on", "off"])
        .key("T")
}

/// `aborted·hide (I)` / `aborted·show (I)`. Uses `hide`/`show` instead
/// of `on`/`off` because the operator question isn't "is the flag on"
/// but "am I looking at the on-disk reality or the harness-faithful
/// view".
fn abort_chip(showing: bool) -> ToggleChip<'static> {
    show_hide_chip("aborted", showing, "I")
}

/// `hidden·hide (H)` / `hidden·show (H)` for records the harness's UI
/// never showed (ADR 0122). Same `hide`/`show` wording as the aborted
/// chip.
fn hidden_chip(showing: bool) -> ToggleChip<'static> {
    show_hide_chip("hidden", showing, "H")
}

fn show_hide_chip(name: &'static str, showing: bool, key: &'static str) -> ToggleChip<'static> {
    ToggleChip::new(name, if showing { "show" } else { "hide" }, showing)
        .states(&["show", "hide"])
        .key(key)
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
    turn_starts: &mut Vec<TurnStart>,
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
            for offset in 0..run_len.max(1) {
                if passes_turn_filters(&turns[i + offset], state) {
                    turn_starts.push(TurnStart {
                        turn_index: i + offset,
                        line: lines.len(),
                        collapsed: true,
                    });
                }
            }
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
        turn_starts.push(TurnStart {
            turn_index: i,
            line: lines.len(),
            collapsed: false,
        });
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
        .draw(|frame| {
            let area = frame.area();
            draw(state, theme, frame.buffer_mut(), area);
        })
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
