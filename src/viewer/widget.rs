//! Full-screen Ratatui modal for the transcript viewer
//! (`H-VIEWER-NATIVE-006`).
//!
//! Layout (top → bottom):
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  <harness>:<session-key>             cwd: <cwd>          │  ← header (1 line)
//! │──────────────────────────────────────────────────────────│  ← separator (1 line)
//! │                                                          │
//! │  you                                                     │  ← body (flex)
//! │  hello                                                   │
//! │                                                          │
//! │  assistant                                               │
//! │  hi back                                                 │
//! │                                                          │
//! │──────────────────────────────────────────────────────────│  ← separator (1 line)
//! │  q close · j/k scroll · g/G top/bottom · t tools · y …   │  ← footer (1 line)
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! The widget is the only piece of `src/viewer/` that touches
//! `ratatui::*`. Per ADR 0052 the renderer never branches on
//! harness; it consumes only the normalized model.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::viewer::model::{TranscriptDocument, TurnKind};
use crate::viewer::render::render_turn;
use crate::viewer::state::ViewerState;

/// Draw the transcript viewer onto `area`. Mutates `state` to
/// record the layout it observed (viewport height, total line
/// count) so the reducer can use those numbers on the next key
/// event.
pub fn draw(state: &mut ViewerState, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // separator
        Constraint::Min(1),    // body
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(area);

    draw_header(&state.document, frame, chunks[0]);
    draw_separator(frame, chunks[1]);
    draw_body(state, frame, chunks[2]);
    draw_separator(frame, chunks[3]);
    draw_footer(state, frame, chunks[4]);
}

fn draw_header(document: &TranscriptDocument, frame: &mut Frame<'_>, area: Rect) {
    let title = format!("{}:{}", document.meta.harness, document.meta.session_key);
    let cwd_text = document.meta.cwd.as_deref().unwrap_or("(no cwd)");
    let cwd_label = format!("cwd: {cwd_text}");

    // Title left-justified, cwd right-justified, computed so the
    // two never overlap on narrow terminals.
    let title_width = title.chars().count();
    let cwd_width = cwd_label.chars().count();
    let available = area.width as usize;
    let gap = available.saturating_sub(title_width + cwd_width);
    let line = if gap == 0 || title_width + cwd_width >= available {
        Line::from(Span::styled(
            title,
            Style::new().add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(vec![
            Span::styled(title, Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(gap)),
            Span::styled(cwd_label, Style::new().add_modifier(Modifier::DIM)),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_separator(frame: &mut Frame<'_>, area: Rect) {
    let rule: String = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            rule,
            Style::new().add_modifier(Modifier::DIM),
        ))),
        area,
    );
}

fn draw_body(state: &mut ViewerState, frame: &mut Frame<'_>, area: Rect) {
    // Empty-document banner short-circuits before we ever touch the
    // turn list. Capture the layout metrics into state so navigation
    // messages don't see stale numbers.
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

    // `lines` borrows from `state.document.turns[*].body` so we
    // cannot mutate `state` until the borrow ends with the
    // `render_widget` call below. Compute everything we'll write
    // back here, then move `lines` into `Paragraph`.
    let lines = build_body_lines(state);
    let total = lines.len();
    let viewport_height = area.height;
    let max_offset = total.saturating_sub(viewport_height as usize);
    let scroll_offset = if state.stick_to_end {
        max_offset
    } else {
        state.scroll_offset.min(max_offset)
    };

    let paragraph = Paragraph::new(lines).scroll((scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);

    state.viewport_height = viewport_height;
    state.total_lines = total;
    state.scroll_offset = scroll_offset;
}

fn draw_footer(state: &ViewerState, frame: &mut Frame<'_>, area: Rect) {
    let tools_label = if state.show_tools {
        "tools on"
    } else {
        "tools off"
    };
    let thinking_label = if state.show_thinking {
        "thinking on"
    } else {
        "thinking off"
    };
    let hint = format!(
        "q close · j/k scroll · g/G top/end · PgUp/PgDn · t {tools_label} · y {thinking_label}"
    );
    let mut chars = hint.chars().count();
    let cap = area.width as usize;
    let display = if chars <= cap {
        hint
    } else {
        // Truncate with ellipsis on narrow terminals.
        let mut buf = String::new();
        for c in hint.chars() {
            if chars + 1 > cap {
                break;
            }
            buf.push(c);
            chars += 1;
        }
        buf.push('…');
        buf
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            display,
            Style::new().add_modifier(Modifier::DIM),
        ))),
        area,
    );
}

/// Build the flat body line list. Filters out tool/thinking turns
/// when the corresponding state flags are off.
fn build_body_lines<'a>(state: &'a ViewerState) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();
    for turn in &state.document.turns {
        if !is_visible(turn.kind, state) {
            continue;
        }
        lines.extend(render_turn(turn));
    }
    lines
}

fn is_visible(kind: TurnKind, state: &ViewerState) -> bool {
    match kind {
        TurnKind::Message | TurnKind::CompactionSummary => true,
        TurnKind::ToolUse | TurnKind::ToolResult => state.show_tools,
        TurnKind::Thinking => state.show_thinking,
    }
}

#[cfg(test)]
pub fn render_to_buffer(
    state: &mut ViewerState,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| draw(state, frame, frame.area()))
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
mod tests {
    use super::*;
    use crate::viewer::model::{
        SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
    };
    use std::path::PathBuf;

    fn sample_document() -> TranscriptDocument {
        TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "0b34e59c".to_string(),
                cwd: Some("/p/proj".to_string()),
            },
            turns: vec![
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    body: "what's up?".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::Message,
                    body: "not much".to_string(),
                    timestamp: None,
                },
            ],
        }
    }

    fn unavailable_document() -> TranscriptDocument {
        TranscriptDocument::unavailable(&SessionLocator::ClaudeCode {
            state_root: PathBuf::from("/x"),
            session_key: "missing".to_string(),
        })
    }

    fn long_document() -> TranscriptDocument {
        let mut doc = TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "long".to_string(),
                cwd: Some("/p".to_string()),
            },
            turns: Vec::new(),
        };
        for i in 0..30 {
            doc.turns.push(TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: format!("turn {i}"),
                timestamp: None,
            });
        }
        doc
    }

    fn document_with_compaction() -> TranscriptDocument {
        TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "compact".to_string(),
                cwd: Some("/p".to_string()),
            },
            turns: vec![
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    body: "before".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::CompactionSummary,
                    body: "summary of prior turns".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::Message,
                    body: "after".to_string(),
                    timestamp: None,
                },
            ],
        }
    }

    /// Stronger normalization for snapshot tests: collapse trailing
    /// space at the end of each line (frames pad to the full width
    /// with spaces). Keeps the snapshot terminal-width-agnostic to
    /// rendering differences in how widgets handle padding.
    fn snapshot_string(buf: &ratatui::buffer::Buffer) -> String {
        let raw = buffer_to_string(buf);
        raw.lines()
            .map(|l| l.trim_end_matches(' '))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn normal_open_lands_on_last_turn_and_renders_header() {
        let doc = sample_document();
        let mut state = ViewerState::new(doc);
        let buf = render_to_buffer(&mut state, 60, 12);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn jump_to_start_shows_top_of_long_document() {
        let mut state = ViewerState::new(long_document());
        // First draw to populate viewport_height + total_lines.
        let _ = render_to_buffer(&mut state, 60, 10);
        // Then jump to start and redraw.
        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
        let buf = render_to_buffer(&mut state, 60, 10);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn empty_document_renders_unavailable_banner() {
        let mut state = ViewerState::new(unavailable_document());
        let buf = render_to_buffer(&mut state, 60, 8);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn compaction_summary_turn_renders_with_banner_header() {
        let mut state = ViewerState::new(document_with_compaction());
        let buf = render_to_buffer(&mut state, 60, 14);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn footer_reports_current_tool_and_thinking_toggle_state() {
        let state = ViewerState::new(sample_document());
        // Toggle both on so the footer should say "tools on" /
        // "thinking on".
        let (state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleTools);
        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleThinking);
        let buf = render_to_buffer(&mut state, 80, 8);
        let s = buffer_to_string(&buf);
        assert!(
            s.contains("tools on"),
            "footer should advertise tools on, got: {s}"
        );
        assert!(
            s.contains("thinking on"),
            "footer should advertise thinking on"
        );
    }

    #[test]
    fn draw_writes_viewport_height_and_total_lines_to_state() {
        let mut state = ViewerState::new(sample_document());
        let _ = render_to_buffer(&mut state, 40, 12);
        assert_eq!(
            state.viewport_height, 8,
            "12 total - 4 chrome (header/2 seps/footer)"
        );
        assert!(state.total_lines > 0);
    }

    #[test]
    fn toggle_tools_makes_tool_turns_visible() {
        let mut state = ViewerState::new(TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "tools".to_string(),
                cwd: None,
            },
            turns: vec![
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    body: "what is in this dir?".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolUse,
                    body: "ls /".to_string(),
                    timestamp: None,
                },
            ],
        });
        let before = buffer_to_string(&render_to_buffer(&mut state, 60, 10));
        assert!(!before.contains("tool call"), "tools hidden by default");

        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleTools);
        let after = buffer_to_string(&render_to_buffer(&mut state, 60, 10));
        assert!(
            after.contains("tool call"),
            "tool turns visible after toggle"
        );
    }
}
