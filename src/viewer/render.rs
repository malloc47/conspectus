//! Per-turn rendering: [`TranscriptTurn`] → `ratatui::text::Line`s.
//!
//! `Message` and `CompactionSummary` bodies pass through
//! `tui_markdown::from_str`; tool blocks render plain. Per-turn
//! output begins with a dimmed role header
//! (`you` / `assistant` / `assistant · thinking` / etc.) and ends
//! with one blank spacer line so turns visually separate in the
//! body scroll region.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::viewer::model::{TranscriptTurn, TurnKind};

/// Render one turn into a flat sequence of [`Line`]s. The borrowed
/// returned value shares lifetime with `turn.body`; the widget
/// builds the full body Vec per-frame so the lifetime is bounded
/// by the draw call.
pub fn render_turn<'a>(turn: &'a TranscriptTurn) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    lines.push(role_header_line(turn));

    match turn.kind {
        TurnKind::Message | TurnKind::CompactionSummary => {
            // Markdown body. `tui_markdown::from_str` produces a
            // styled `Text<'_>` that we drain line-by-line into our
            // output Vec.
            let text = tui_markdown::from_str(&turn.body);
            for line in text.lines {
                lines.push(line);
            }
        }
        TurnKind::Thinking | TurnKind::ToolUse | TurnKind::ToolResult => {
            // Plain text — render verbatim with the dim style so
            // tool / thinking blocks fade into the background.
            let style = Style::new().add_modifier(Modifier::DIM);
            for raw_line in turn.body.lines() {
                lines.push(Line::from(Span::styled(raw_line, style)));
            }
        }
    }

    // Spacer between turns.
    lines.push(Line::raw(""));
    lines
}

/// Dimmed `[role · kind]` header for a turn. CompactionSummary gets
/// a distinct banner-style header since the role is always synthetic.
fn role_header_line(turn: &TranscriptTurn) -> Line<'static> {
    let role = turn.role.header_label();
    let header_text = match turn.kind {
        TurnKind::Message => role.to_string(),
        TurnKind::Thinking => format!("{role} · thinking"),
        TurnKind::ToolUse => format!("{role} · tool call"),
        TurnKind::ToolResult => format!("{role} · tool result"),
        TurnKind::CompactionSummary => "— compaction summary —".to_string(),
    };
    Line::from(Span::styled(
        header_text,
        Style::new().add_modifier(Modifier::DIM),
    ))
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

    #[test]
    fn message_renders_header_then_body_then_spacer() {
        let t = turn(TurnRole::User, TurnKind::Message, "hello");
        let lines = render_turn(&t);
        // header + at least one body line + spacer
        assert!(lines.len() >= 3);
        let header_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(header_text, "you");
        // Last line is the empty spacer.
        let last_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(last_text.is_empty());
    }

    #[test]
    fn assistant_header_uses_assistant_label() {
        let t = turn(TurnRole::Assistant, TurnKind::Message, "hi");
        let lines = render_turn(&t);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(header, "assistant");
    }

    #[test]
    fn thinking_header_appends_kind_suffix() {
        let t = turn(TurnRole::Assistant, TurnKind::Thinking, "considering...");
        let lines = render_turn(&t);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(header, "assistant · thinking");
    }

    #[test]
    fn tool_use_and_tool_result_get_distinct_headers() {
        let call_turn = turn(TurnRole::Assistant, TurnKind::ToolUse, "bash: ls");
        let call = render_turn(&call_turn);
        let call_header: String = call[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(call_header, "assistant · tool call");
        let res_turn = turn(TurnRole::Assistant, TurnKind::ToolResult, "a\nb");
        let res = render_turn(&res_turn);
        let res_header: String = res[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(res_header, "assistant · tool result");
    }

    #[test]
    fn compaction_summary_uses_banner_header() {
        let t = turn(TurnRole::User, TurnKind::CompactionSummary, "before...");
        let lines = render_turn(&t);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(header, "— compaction summary —");
    }

    #[test]
    fn plain_text_turn_kinds_split_body_by_line() {
        let t = turn(
            TurnRole::Assistant,
            TurnKind::ToolResult,
            "first\nsecond\nthird",
        );
        let lines = render_turn(&t);
        // header + 3 body + spacer = 5
        assert_eq!(lines.len(), 5);
        let body_1: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(body_1, "first");
        let body_2: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(body_2, "second");
    }
}
