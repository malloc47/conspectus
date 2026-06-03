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

/// Inner label width inside the chip pill. Wide enough for
/// `assistant` (9 chars); shorter labels right-justify inside it
/// so the colored pill is uniform.
const CHIP_INNER_WIDTH: usize = 9;

/// Total chip pill width in cells: 1 lead space + inner label +
/// 1 trail space.
pub const GUTTER_WIDTH: u16 = (CHIP_INNER_WIDTH + 2) as u16;

/// The separator between gutter and body. Three cells:
/// ` │ ` — leading space, rule, trailing space. On the chip line
/// (and every continuation line of the same turn) the rule picks
/// up the chip's color so each turn reads as a colored vertical
/// thread.
pub const SEPARATOR_WIDTH: u16 = 3;

/// Total leader cells consumed before the body content begins.
pub const LEADER_WIDTH: u16 = GUTTER_WIDTH + SEPARATOR_WIDTH;

/// Convert a borrowed [`Line`] into an owned `Line<'static>` by
/// cloning every span's content. Used by the widget when it caches
/// composed body lines across draws.
pub fn into_owned_line(line: Line<'_>) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| Span::styled(s.content.into_owned(), s.style))
        .collect();
    Line::from(spans)
}

/// Render one turn into a flat sequence of [`Line`]s sized for the
/// given `content_width` (cells available *after* the gutter +
/// separator). Caller is responsible for picking the content_width
/// against the viewport width.
pub fn render_turn<'a>(
    turn: &'a TranscriptTurn,
    theme: &Theme,
    content_width: u16,
) -> Vec<Line<'a>> {
    let chip_color = chip_color(turn, theme);
    let body_lines = build_body_lines(turn, content_width);
    if body_lines.is_empty() {
        // Even an empty turn deserves its chip — render the chip
        // alone so the operator sees the role marker.
        return vec![compose_line(turn, theme, chip_color, Line::raw(""), true)];
    }
    let mut out = Vec::with_capacity(body_lines.len() + 1);
    for (i, body) in body_lines.into_iter().enumerate() {
        out.push(compose_line(turn, theme, chip_color, body, i == 0));
    }
    // Spacer between turns. The blank line is intentionally blank
    // (no separator) so adjacent turns don't look like the same
    // turn continuing.
    out.push(Line::raw(""));
    out
}

/// Build a `gutter + separator + body` line. The chip is drawn
/// only on the first line of a turn; the separator picks up the
/// chip's color on every line so each turn reads as a colored
/// vertical thread.
fn compose_line<'a>(
    turn: &TranscriptTurn,
    theme: &Theme,
    chip_color: ratatui::style::Color,
    body: Line<'a>,
    show_chip: bool,
) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(3 + body.spans.len());
    if show_chip {
        // Filled pill: fixed-width ` <label:>9> ` (11 cells)
        // rendered with `theme.badge` (REVERSED+BOLD by default)
        // over the chip's foreground color. Matches the session-
        // list badge primitive (`crate::tui::widgets::badge`).
        let label = chip_label(turn);
        let pill = format!(" {label:>CHIP_INNER_WIDTH$} ");
        spans.push(Span::styled(
            pill,
            Style::new().fg(chip_color).add_modifier(theme.badge),
        ));
    } else {
        spans.push(Span::raw(" ".repeat(GUTTER_WIDTH as usize)));
    }
    spans.push(Span::styled(" │ ", Style::new().fg(chip_color)));
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

/// Color associated with a turn's chip (and therefore its
/// separator thread). Routed through the viewer's [`Theme`]
/// re-export so users can override via `[tui.theme]` config.
fn chip_color(turn: &TranscriptTurn, theme: &Theme) -> ratatui::style::Color {
    match (turn.role, turn.kind) {
        (_, TurnKind::CompactionSummary) => theme.pr_merged,
        (_, TurnKind::Thinking) => theme.secondary_text,
        (_, TurnKind::ToolUse) => theme.warning,
        (_, TurnKind::ToolResult) => theme.warning,
        (TurnRole::User, _) => theme.harness_codex,
        (TurnRole::Assistant, _) => theme.success,
        (TurnRole::System, _) => theme.secondary_text,
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
            // material rather than primary prose. Replace tabs
            // with two spaces so `cat -n`-style line-numbered
            // output (e.g. Claude's `Read` tool result) gets a
            // visible gap between the line number and content
            // instead of collapsing in the terminal.
            let style = Style::new().add_modifier(Modifier::DIM);
            let detabbed = turn.body.replace('\t', "  ");
            wrap_plain_owned(detabbed, content_width, style)
        }
    }
}

/// Wrap a plain-text body at `width` cells with one fixed style.
/// Splits on existing newlines first, then word-wraps each chunk.
/// Returns owned spans (`'static`) so callers can pass either
/// borrowed input from the turn or a locally-built `String` (e.g.
/// detabbed tool output) without lifetime contortions.
fn wrap_plain(body: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for raw in body.lines() {
        for wrapped in word_wrap(raw, width) {
            out.push(Line::from(Span::styled(wrapped, style)));
        }
    }
    out
}

/// Sibling for the owned-input case; kept distinct so the
/// detabbed-string path reads obviously at the call site.
fn wrap_plain_owned(body: String, width: u16, style: Style) -> Vec<Line<'static>> {
    wrap_plain(&body, width, style)
}

/// Wrap a single line of plain text at `width` cells, breaking at
/// whitespace where possible. Convenience wrapper over
/// [`word_wrap_with_budgets`] when first-line and subsequent-line
/// budgets are the same.
fn word_wrap(input: &str, width: u16) -> Vec<String> {
    word_wrap_with_budgets(input, width, width)
}

/// Wrap with a distinct first-line cap. The first returned chunk
/// fits in `first_budget` cells; subsequent chunks fit in `rest`.
/// This is what keeps the styled wrapper from orphaning leading
/// content (like a `1. ` list marker) onto its own line: the
/// caller passes the remaining cells on the current line as
/// `first_budget` so the wrap picks a body chunk that joins
/// cleanly.
fn word_wrap_with_budgets(input: &str, first_budget: u16, rest: u16) -> Vec<String> {
    let first = first_budget.max(1) as usize;
    let rest = rest.max(1) as usize;
    if input.is_empty() {
        return vec![String::new()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut current_cap = first;

    for word in input.split_inclusive(char::is_whitespace) {
        let w = UnicodeWidthStr::width(word);
        if w > current_cap {
            // Word too long for the current line.
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_width = 0;
                current_cap = rest;
            }
            if w > current_cap {
                // Still too long after switching to rest budget:
                // hard-break inside this single word.
                for piece in hard_break(word, current_cap) {
                    out.push(piece);
                }
                continue;
            }
            current.push_str(word);
            current_width = w;
            continue;
        }
        if current_width + w > current_cap {
            out.push(std::mem::take(&mut current));
            current_width = 0;
            current_cap = rest;
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
        // Slow path: split the span's content by word-wrap. The
        // first chunk gets a budget equal to the remaining cells
        // on the current line so leading content (a `1. ` list
        // marker, an inline-code open, ...) doesn't orphan onto
        // its own line. Subsequent chunks land on fresh lines.
        let style = span.style;
        let content = span.content;
        let first_budget = width.saturating_sub(current_width);
        let pieces = word_wrap_with_budgets(content.as_ref(), first_budget, width);
        for (i, piece) in pieces.into_iter().enumerate() {
            if i > 0 && (!current_spans.is_empty() || current_width > 0) {
                out.push(Line::from(std::mem::take(&mut current_spans)));
                current_width = 0;
            }
            if piece.is_empty() {
                continue;
            }
            let piece_width = UnicodeWidthStr::width(piece.as_str()) as u16;
            current_spans.push(Span::styled(piece, style));
            current_width += piece_width;
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

    /// Visible-text expectation of the chip pill for a given label,
    /// matching `format!(" {label:>CHIP_INNER_WIDTH$} ")`. The pill
    /// is followed by ` │ ` (no extra space — the pill's trailing
    /// space + separator's leading space give two cells between
    /// the label and the rule).
    fn chip_pill(label: &str) -> String {
        format!(" {label:>CHIP_INNER_WIDTH$} ")
    }

    #[test]
    fn first_line_carries_chip_pill_subsequent_blank_gutter() {
        // Use a paragraph break (blank line) so the Markdown
        // renderer keeps the two lines separate — a single \n is a
        // CommonMark soft break and collapses to a space.
        let t = turn(TurnRole::User, TurnKind::Message, "hello\n\nworld");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        let texts = flat_text(&out);
        // First content line: ` <you right-aligned in 9> ` + ` │ ` + body.
        let expected_first = format!("{} │ hello", chip_pill("you"));
        assert_eq!(texts[0], expected_first);
        // A "world" line shows up with blank-gutter prefix
        // (GUTTER_WIDTH spaces + ` │ ` + body).
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
        assert!(
            texts[0].contains(&format!("{} │ hi", chip_pill("assistant"))),
            "got {:?}",
            texts[0]
        );
    }

    #[test]
    fn thinking_chip_label() {
        let t = turn(TurnRole::Assistant, TurnKind::Thinking, "musing");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        assert!(
            flat_text(&out)[0].contains(&format!("{} │", chip_pill("Thinking"))),
            "got {:?}",
            flat_text(&out)[0]
        );
    }

    #[test]
    fn tool_use_and_tool_result_chips() {
        let theme = Theme::default();
        let call_turn = turn(TurnRole::Assistant, TurnKind::ToolUse, "ls");
        let call = render_turn(&call_turn, &theme, 40);
        assert!(
            flat_text(&call)[0].contains(&format!("{} │ ls", chip_pill("Tool"))),
            "got {:?}",
            flat_text(&call)[0]
        );
        let res_turn = turn(TurnRole::Assistant, TurnKind::ToolResult, "ok");
        let res = render_turn(&res_turn, &theme, 40);
        // Tool result uses the corner-arrow glyph.
        assert!(
            flat_text(&res)[0].contains(&format!("{} │ ok", chip_pill("↳ Result"))),
            "got {:?}",
            flat_text(&res)[0]
        );
    }

    #[test]
    fn compaction_summary_uses_compact_chip() {
        let t = turn(TurnRole::User, TurnKind::CompactionSummary, "summary");
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 40);
        assert!(
            flat_text(&out)[0].contains(&format!("{} │ summary", chip_pill("compact"))),
            "got {:?}",
            flat_text(&out)[0]
        );
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
    fn list_marker_does_not_orphan_to_its_own_line() {
        // Reproduces the orphan: a Markdown numbered list item
        // whose body text is long enough to exceed content_width.
        // tui-markdown emits the marker `1. ` as a small span and
        // the body as a separate (larger) span; the wrap slow path
        // used to flush the marker onto its own line before the
        // body could attach.
        let body = "1. some text that is just long enough to need wrapping";
        let t = turn(TurnRole::Assistant, TurnKind::Message, body);
        let theme = Theme::default();
        let out = render_turn(&t, &theme, 30);
        let texts = flat_text(&out);
        // The list marker must share a line with at least the
        // first body word — no `"        you │ 1."` then
        // `"            │ some text..."`.
        let first = &texts[0];
        let last_pipe = first.rfind('│').expect("separator present");
        let after = &first[last_pipe + '│'.len_utf8()..];
        assert!(
            after.trim().starts_with("1.")
                && after
                    .trim()
                    .chars()
                    .skip("1.".len())
                    .any(|c| !c.is_whitespace()),
            "marker should not be alone on the first line: {first:?}"
        );
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
        assert!(
            flat_text(&out)[0].contains(&format!("{} │", chip_pill("you"))),
            "got {:?}",
            flat_text(&out)[0]
        );
    }
}
