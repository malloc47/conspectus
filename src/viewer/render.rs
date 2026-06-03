//! Gutter-and-chip per-turn rendering (H-VIEWER-NATIVE-011).
//!
//! Inspired by `claude-history`: a right-aligned colored "chip" in
//! a fixed-width left gutter identifies the turn (you / assistant /
//! Thinking / Tool / ↳ Result / compaction), separated from the
//! body by a vertical rule. Wrapped body lines repeat the gutter
//! width (blank) + separator on each visual line so the body's
//! left edge is always at the same column.
//!
//! Theme integration goes through [`crate::viewer::theme::Theme`]
//! (the ADR 0052 carve-out re-export); chip colours come from the
//! palette so operators can override via `[tui.theme]`.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::viewer::model::{TranscriptTurn, TurnKind, TurnRole};
use crate::viewer::theme::Theme;

/// Width (in cells) of the gutter that carries the right-aligned
/// chip. Wide enough for `assistant` (9 chars) + a one-cell pad
/// for the chip glyph (`↳`) when used.
pub const GUTTER_WIDTH: u16 = 10;

/// The separator between gutter and body. Three cells:
/// ` │ ` — leading space, dim rule, trailing space.
pub const SEPARATOR_WIDTH: u16 = 3;

/// Total leader cells consumed before the body content begins.
pub const LEADER_WIDTH: u16 = GUTTER_WIDTH + SEPARATOR_WIDTH;

/// Render one turn into a flat sequence of [`Line`]s sized for the
/// given `content_width` (cells available *after* the gutter +
/// separator). Caller is responsible for picking the content_width
/// against the viewport width.
pub fn render_turn<'a>(
    turn: &'a TranscriptTurn,
    theme: &Theme,
    content_width: u16,
) -> Vec<Line<'a>> {
    let body_lines = build_body_lines(turn, content_width);
    if body_lines.is_empty() {
        // Even an empty turn deserves its chip — render the chip
        // alone so the operator sees the role marker.
        return vec![compose_line(turn, theme, Line::raw(""), true)];
    }
    let mut out = Vec::with_capacity(body_lines.len() + 1);
    for (i, body) in body_lines.into_iter().enumerate() {
        out.push(compose_line(turn, theme, body, i == 0));
    }
    // Spacer between turns. The blank line carries the rule colour
    // so the eye follows the gutter unbroken between adjacent turns.
    out.push(Line::raw(""));
    out
}

/// Build a `gutter + separator + body` line, with the chip drawn
/// only on the first line of a turn.
fn compose_line<'a>(
    turn: &TranscriptTurn,
    theme: &Theme,
    body: Line<'a>,
    show_chip: bool,
) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(2 + body.spans.len());
    if show_chip {
        let label = chip_label(turn);
        let style = chip_style(turn, theme);
        let label_width = UnicodeWidthStr::width(label) as u16;
        let pad = GUTTER_WIDTH.saturating_sub(label_width);
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad as usize)));
        }
        spans.push(Span::styled(label.to_string(), style));
    } else {
        spans.push(Span::raw(" ".repeat(GUTTER_WIDTH as usize)));
    }
    spans.push(Span::styled(
        " │ ",
        Style::new()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::DIM),
    ));
    spans.extend(body.spans);
    Line::from(spans)
}

/// The chip label as it appears in the gutter. Tool-result has a
/// leading `↳` so the call/result pair reads as connected.
fn chip_label(turn: &TranscriptTurn) -> &'static str {
    match (turn.role, turn.kind) {
        (_, TurnKind::CompactionSummary) => "compact",
        (_, TurnKind::Thinking) => "Thinking",
        (_, TurnKind::ToolUse) => "Tool",
        (_, TurnKind::ToolResult) => "↳ Result",
        (TurnRole::User, _) => "you",
        (TurnRole::Assistant, _) => "assistant",
        (TurnRole::System, _) => "system",
    }
}

/// Chip styling — palette routed through the viewer's [`Theme`]
/// re-export so users can override via `[tui.theme]` config.
fn chip_style(turn: &TranscriptTurn, theme: &Theme) -> Style {
    match (turn.role, turn.kind) {
        (_, TurnKind::CompactionSummary) => Style::new()
            .fg(theme.pr_merged)
            .add_modifier(Modifier::BOLD),
        (_, TurnKind::Thinking) => Style::new()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::ITALIC),
        (_, TurnKind::ToolUse) => Style::new().fg(theme.warning).add_modifier(Modifier::BOLD),
        (_, TurnKind::ToolResult) => Style::new().fg(theme.warning).add_modifier(Modifier::DIM),
        (TurnRole::User, _) => Style::new()
            .fg(theme.harness_codex)
            .add_modifier(Modifier::BOLD),
        (TurnRole::Assistant, _) => Style::new().fg(theme.success).add_modifier(Modifier::BOLD),
        (TurnRole::System, _) => Style::new()
            .fg(theme.secondary_text)
            .add_modifier(Modifier::BOLD),
    }
}

/// Build the per-turn body as a list of pre-wrapped, styled
/// [`Line`]s ready for gutter composition.
fn build_body_lines<'a>(turn: &'a TranscriptTurn, content_width: u16) -> Vec<Line<'a>> {
    match turn.kind {
        TurnKind::Message | TurnKind::CompactionSummary => {
            // Markdown body. `tui_markdown::from_str` returns a
            // pre-styled `Text`; we wrap each rendered line to the
            // available width so the body never spills past the
            // gutter+separator on continuation lines.
            let text = tui_markdown::from_str(&turn.body);
            let mut out = Vec::with_capacity(text.lines.len());
            for line in text.lines {
                wrap_styled_line(line, content_width, &mut out);
            }
            out
        }
        TurnKind::Thinking => {
            // Plain text, italicised for visual hint that the
            // content is private reasoning.
            let style = Style::new()
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::DIM);
            wrap_plain(&turn.body, content_width, style)
        }
        TurnKind::ToolUse | TurnKind::ToolResult => {
            // Plain text, dim — tool content reads as supporting
            // material rather than primary prose.
            let style = Style::new().add_modifier(Modifier::DIM);
            wrap_plain(&turn.body, content_width, style)
        }
    }
}

/// Wrap a plain-text body at `width` cells with one fixed style.
/// Splits on existing newlines first, then word-wraps each chunk.
fn wrap_plain<'a>(body: &'a str, width: u16, style: Style) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    for raw in body.lines() {
        for wrapped in word_wrap(raw, width) {
            out.push(Line::from(Span::styled(wrapped, style)));
        }
    }
    out
}

/// Wrap a single line of plain text at `width` cells, breaking at
/// whitespace where possible. Words longer than `width` are
/// hard-broken so they fit. Returns at least one segment (possibly
/// the empty string) per input line.
fn word_wrap(input: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    if input.is_empty() {
        return vec![String::new()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in input.split_inclusive(char::is_whitespace) {
        let w = UnicodeWidthStr::width(word);
        if w > width {
            // Long word: flush, then hard-break.
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            for piece in hard_break(word, width) {
                out.push(piece);
            }
            continue;
        }
        if current_width + w > width {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push_str(word);
        current_width += w;
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Split a single string at `width` cells regardless of word
/// boundaries. Used as the fallback for very long tokens.
fn hard_break(input: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut buf_width = 0usize;
    for ch in input.chars() {
        let cw = UnicodeWidthStr::width(ch.to_string().as_str());
        if buf_width + cw > width {
            out.push(std::mem::take(&mut buf));
            buf_width = 0;
        }
        buf.push(ch);
        buf_width += cw;
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Wrap a *styled* line at `width`. Each output span preserves its
/// source span's style; breaks happen between spans when possible
/// and inside long spans when not. Continuation lines own the same
/// style chain as the source span chain.
fn wrap_styled_line<'a>(line: Line<'a>, width: u16, out: &mut Vec<Line<'a>>) {
    let width = width.max(1);
    let mut current_spans: Vec<Span<'a>> = Vec::new();
    let mut current_width: u16 = 0;
    let max = width as usize;

    for span in line.spans {
        let span_width = UnicodeWidthStr::width(span.content.as_ref());
        if span_width == 0 {
            current_spans.push(span);
            continue;
        }
        // Fast path: whole span fits.
        if (current_width as usize) + span_width <= max {
            current_width += span_width as u16;
            current_spans.push(span);
            continue;
        }
        // Slow path: split the span's content by word-wrap and emit
        // sub-spans inheriting the parent style.
        let style = span.style;
        let content = span.content;
        let pieces = word_wrap(content.as_ref(), width);
        for (i, piece) in pieces.into_iter().enumerate() {
            let piece_width = UnicodeWidthStr::width(piece.as_str());
            let needs_break = i > 0 || (current_width as usize) + piece_width > max;
            if needs_break && (!current_spans.is_empty() || current_width > 0) {
                out.push(Line::from(std::mem::take(&mut current_spans)));
                current_width = 0;
            }
            if piece.is_empty() {
                continue;
            }
            current_spans.push(Span::styled(piece, style));
            current_width += piece_width as u16;
            if current_width >= width {
                out.push(Line::from(std::mem::take(&mut current_spans)));
                current_width = 0;
            }
        }
    }
    if !current_spans.is_empty() {
        out.push(Line::from(current_spans));
    } else if out.is_empty() {
        out.push(Line::raw(""));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::model::{TranscriptTurn, TurnKind, TurnRole};

    fn turn(role: TurnRole, kind: TurnKind, body: &str) -> TranscriptTurn {
        TranscriptTurn {
            role,
            kind,
            body: body.to_string(),
            timestamp: None,
        }
    }

    fn flat_text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn first_line_carries_right_aligned_chip_subsequent_blank_gutter() {
        // Use a paragraph break (blank line) so the Markdown
        // renderer keeps the two lines separate — a single \n is a
        // CommonMark soft break and collapses to a space.
        let t = turn(TurnRole::User, TurnKind::Message, "hello\n\nworld");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        let texts = flat_text(&out);
        // First content line should start with right-aligned "you"
        // padded inside GUTTER_WIDTH, then " │ hello".
        let expected_pad = " ".repeat((GUTTER_WIDTH - 3) as usize);
        assert_eq!(texts[0], format!("{expected_pad}you │ hello"));
        // A "world" line shows up with blank-gutter prefix.
        let blank_pad = " ".repeat(GUTTER_WIDTH as usize);
        assert!(
            texts.iter().any(|l| l == &format!("{blank_pad} │ world")),
            "expected blank-gutter `world` line, got {texts:?}"
        );
        // Last line is the inter-turn spacer.
        assert_eq!(texts.last().unwrap(), "");
    }

    #[test]
    fn assistant_role_uses_assistant_chip() {
        let t = turn(TurnRole::Assistant, TurnKind::Message, "hi");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        let texts = flat_text(&out);
        assert!(texts[0].contains("assistant │ hi"), "got {:?}", texts[0]);
    }

    #[test]
    fn thinking_chip_label() {
        let t = turn(TurnRole::Assistant, TurnKind::Thinking, "musing");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        assert!(flat_text(&out)[0].contains("Thinking │"));
    }

    #[test]
    fn tool_use_and_tool_result_chips() {
        let theme = Theme::default();
        let call_turn = turn(TurnRole::Assistant, TurnKind::ToolUse, "ls");
        let call = render_turn(&call_turn, &theme, 40);
        assert!(flat_text(&call)[0].contains("Tool │ ls"));
        let res_turn = turn(TurnRole::Assistant, TurnKind::ToolResult, "ok");
        let res = render_turn(&res_turn, &theme, 40);
        // Tool result uses the corner-arrow glyph.
        assert!(
            flat_text(&res)[0].contains("↳ Result │ ok"),
            "got {:?}",
            flat_text(&res)[0]
        );
    }

    #[test]
    fn compaction_summary_uses_compact_chip() {
        let t = turn(TurnRole::User, TurnKind::CompactionSummary, "summary");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        assert!(flat_text(&out)[0].contains("compact │ summary"));
    }

    #[test]
    fn plain_text_wraps_at_content_width() {
        // 60-char body, content_width=20, should wrap.
        let body = "one two three four five six seven eight nine ten";
        let t = turn(TurnRole::Assistant, TurnKind::ToolResult, body);
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 20);
        let texts = flat_text(&out);
        // Drop the spacer line.
        let body_lines: Vec<&String> = texts.iter().take(texts.len() - 1).collect();
        assert!(
            body_lines.len() >= 3,
            "expected wrapped lines, got {body_lines:?}"
        );
        // Every line begins with the leader (GUTTER_WIDTH + " │ ").
        for line in &body_lines {
            let leader_cells = LEADER_WIDTH as usize;
            assert!(
                line.chars().count() >= leader_cells,
                "line shorter than leader: {line:?}"
            );
        }
    }

    #[test]
    fn hard_break_handles_oversized_tokens() {
        // 30-char "word" with no whitespace; content_width=10.
        let body = "abcdefghijklmnopqrstuvwxyz1234";
        let t = turn(TurnRole::Assistant, TurnKind::ToolResult, body);
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 10);
        let texts = flat_text(&out);
        // Should produce ≥ 3 body lines (30/10 = 3). +1 spacer = 4.
        assert!(texts.len() >= 4, "got {texts:?}");
    }

    #[test]
    fn empty_body_still_renders_the_chip() {
        let t = turn(TurnRole::User, TurnKind::Message, "");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        // Empty body yields just the chip line; no spacer (the
        // alternative would waste vertical space for what is
        // already a degenerate turn).
        assert_eq!(out.len(), 1);
        assert!(flat_text(&out)[0].contains("you │"));
    }
}
