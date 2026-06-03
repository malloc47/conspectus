//! Full-screen Ratatui modal for the transcript viewer
//! (`H-VIEWER-NATIVE-006`, restyled by `H-VIEWER-NATIVE-011`).
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
//! │ assistant │ Not much.                                       │
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
use crate::viewer::render::{LEADER_WIDTH, render_turn};
use crate::viewer::state::ViewerState;
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
    if state.show_help {
        draw_help_overlay(theme, frame, area);
    }
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

    let content_width = area.width.saturating_sub(LEADER_WIDTH);
    let lines = build_body_lines(state, theme, content_width);
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
    // travel together (`tools·on (t)`). Keeps the operator's eyes
    // on one column instead of cross-referencing a separate
    // shortcut hint.
    let tools_text = toggle_chip("tools", state.show_tools);
    let thinking_text = toggle_chip("think", state.show_thinking);

    // Long hint trails. `?` for help is its own chip so it stands
    // out; `q close` is the last item per the operator-preferred
    // ordering (close should sit where the muscle memory lands
    // when ready to exit).
    let help_chip = "(?) help";
    let close_chip = "(q) close";
    let scroll_hint = "j/k scroll · g/G top/end · PgUp/PgDn";

    let spans: Vec<Span<'_>> = vec![
        Span::styled(position, chip_style),
        pipe.clone(),
        Span::styled(
            tools_text,
            if state.show_tools {
                on_style
            } else {
                off_style
            },
        ),
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
        Span::styled(help_chip, off_style),
        pipe.clone(),
        Span::styled(scroll_hint, off_style),
        pipe,
        Span::styled(close_chip, off_style),
    ];

    // Width-aware truncate: if the assembled footer exceeds the
    // area, drop the scroll hint first (it's the longest), then
    // the help chip. Position + toggle chips + close stay
    // visible at every width that fits them.
    let total: usize = spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let cap = area.width as usize;
    let line = if total <= cap {
        Line::from(spans)
    } else {
        // Build a minimal version: position · tools · think · close.
        let minimal: Vec<Span<'_>> = vec![
            Span::styled(
                format!(
                    "[ {}/{} ]",
                    state
                        .scroll_offset
                        .saturating_add(1)
                        .min(state.total_lines.max(1)),
                    state.total_lines.max(1)
                ),
                chip_style,
            ),
            Span::styled(
                format!(" · {}", toggle_chip("tools", state.show_tools)),
                if state.show_tools {
                    on_style
                } else {
                    off_style
                },
            ),
            Span::styled(
                format!(" · {}", toggle_chip("think", state.show_thinking)),
                if state.show_thinking {
                    on_style
                } else {
                    off_style
                },
            ),
            Span::styled(format!(" · {close_chip}"), off_style),
        ];
        Line::from(minimal)
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// `tools·on (t)` / `tools·off (t)` style toggle chip. The key
/// binding is inlined so the operator doesn't need a separate
/// hint to know how to flip it.
fn toggle_chip(name: &str, on: bool) -> String {
    let state_label = if on { "on " } else { "off" };
    let key = match name {
        "tools" => 't',
        "think" => 'y',
        _ => '?',
    };
    format!("{name}·{state_label} ({key})")
}

fn draw_help_overlay(theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    use ratatui::widgets::{Block, Borders, Clear};
    // Build the body content first so we can size the panel
    // exactly to fit it.
    let entries: &[(&str, &str)] = &[
        ("q / Esc / Ctrl-C", "Close viewer"),
        ("j / Down", "Scroll one line down"),
        ("k / Up", "Scroll one line up"),
        ("Space / PgDn", "Page down"),
        ("PgUp", "Page up"),
        ("Ctrl-D / Ctrl-U", "Half-page down / up"),
        ("g / Home", "Jump to start of transcript"),
        ("G / End", "Jump to end (open default)"),
        ("t", "Toggle tool-use / tool-result turns"),
        ("y", "Toggle thinking turns"),
        ("?", "Toggle this help panel"),
    ];

    // Pick the longest key cell so columns align.
    let key_col_width = entries
        .iter()
        .map(|(k, _)| unicode_width::UnicodeWidthStr::width(*k))
        .max()
        .unwrap_or(0);
    let desc_col_width = entries
        .iter()
        .map(|(_, d)| unicode_width::UnicodeWidthStr::width(*d))
        .max()
        .unwrap_or(0);
    let inner_width = (key_col_width + 3 + desc_col_width) as u16;
    // +4 for the two borders + two cells of inner padding.
    let panel_width = (inner_width + 4).min(area.width.saturating_sub(2));
    // +2 for the top + bottom borders, +1 for a title line.
    let panel_height = (entries.len() as u16 + 3).min(area.height.saturating_sub(2));

    let panel_x = area.x + area.width.saturating_sub(panel_width) / 2;
    let panel_y = area.y + area.height.saturating_sub(panel_height) / 2;
    let panel_area = Rect::new(panel_x, panel_y, panel_width, panel_height);

    let lines: Vec<Line<'_>> = entries
        .iter()
        .map(|(key, desc)| {
            let key_pad = key_col_width.saturating_sub(unicode_width::UnicodeWidthStr::width(*key));
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{}{} ", " ".repeat(key_pad), key),
                    Style::new()
                        .fg(theme.harness_codex)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled((*desc).to_string(), Style::new()),
            ])
        })
        .collect();

    let title_line = Line::from(Span::styled(
        " viewer keybindings ",
        Style::new()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
            .add_modifier(theme.badge),
    ));

    frame.render_widget(Clear, panel_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.success))
        .title(title_line);
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, panel_area);
}

/// Build the flat body line list. Filters out tool/thinking turns
/// when the corresponding state flags are off.
fn build_body_lines<'a>(
    state: &'a ViewerState,
    theme: &Theme,
    content_width: u16,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();
    for turn in &state.document.turns {
        if !is_visible(turn.kind, state) {
            continue;
        }
        lines.extend(render_turn(turn, theme, content_width));
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

/// Pick a harness-identity color for the title chip. Falls back to
/// `harness_unknown` for harnesses outside the v1 set.
fn harness_chip_style(harness: &str, theme: &Theme) -> Style {
    let color = match harness {
        "claude-code" => theme.harness_claude,
        "codex" => theme.harness_codex,
        "opencode" => theme.harness_opencode,
        "aider" => theme.harness_aider,
        _ => theme.harness_unknown,
    };
    Style::new().fg(color).add_modifier(Modifier::BOLD)
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

    fn document_with_tools() -> TranscriptDocument {
        TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "tools".to_string(),
                cwd: Some("/p".to_string()),
            },
            turns: vec![
                TranscriptTurn {
                    role: TurnRole::User,
                    kind: TurnKind::Message,
                    body: "what files are here?".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolUse,
                    body: "Glob: **/*.rs".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::ToolResult,
                    body: "src/lib.rs\nsrc/main.rs".to_string(),
                    timestamp: None,
                },
                TranscriptTurn {
                    role: TurnRole::Assistant,
                    kind: TurnKind::Message,
                    body: "two Rust files in src/.".to_string(),
                    timestamp: None,
                },
            ],
        }
    }

    /// Drop the trailing whitespace each row pads to width — keeps
    /// snapshots terminal-width-agnostic to padding noise.
    fn snapshot_string(buf: &ratatui::buffer::Buffer) -> String {
        let raw = buffer_to_string(buf);
        raw.lines()
            .map(|l| l.trim_end_matches(' '))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn normal_open_lands_on_last_turn_with_gutter_chips() {
        let doc = sample_document();
        let theme = Theme::default();
        let mut state = ViewerState::new(doc);
        let buf = render_to_buffer(&mut state, &theme, 60, 12);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn jump_to_start_shows_top_of_long_document() {
        let theme = Theme::default();
        let mut state = ViewerState::new(long_document());
        let _ = render_to_buffer(&mut state, &theme, 60, 10);
        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
        let buf = render_to_buffer(&mut state, &theme, 60, 10);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn empty_document_renders_unavailable_banner() {
        let theme = Theme::default();
        let mut state = ViewerState::new(unavailable_document());
        let buf = render_to_buffer(&mut state, &theme, 60, 8);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn compaction_summary_turn_renders_with_compact_chip() {
        let theme = Theme::default();
        let mut state = ViewerState::new(document_with_compaction());
        let buf = render_to_buffer(&mut state, &theme, 60, 14);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn tools_visible_when_toggled_show_call_and_result_chips() {
        let theme = Theme::default();
        let state = ViewerState::new(document_with_tools());
        // Toggle tools on so the ToolUse + ToolResult turns render.
        let (state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleTools);
        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
        let buf = render_to_buffer(&mut state, &theme, 60, 16);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn narrow_terminal_wraps_body_but_keeps_chip_alignment() {
        let theme = Theme::default();
        let mut state = ViewerState::new(TranscriptDocument {
            meta: TranscriptMeta {
                harness: "claude-code".to_string(),
                session_key: "wrap".to_string(),
                cwd: Some("/p".to_string()),
            },
            turns: vec![TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "one two three four five six seven eight nine ten eleven twelve thirteen"
                    .to_string(),
                timestamp: None,
            }],
        });
        let buf = render_to_buffer(&mut state, &theme, 40, 12);
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn footer_shows_position_and_toggle_state() {
        let theme = Theme::default();
        let state = ViewerState::new(sample_document());
        let (state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleTools);
        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleThinking);
        let buf = render_to_buffer(&mut state, &theme, 80, 8);
        let s = buffer_to_string(&buf);
        assert!(s.contains("tools·on"), "footer reports tools state: {s}");
        assert!(s.contains("think·on"), "footer reports thinking state: {s}");
        assert!(s.contains("[ "), "footer carries position counter");
    }

    #[test]
    fn help_overlay_renders_keybindings_panel_when_show_help_is_set() {
        let theme = Theme::default();
        let mut state = ViewerState::new(sample_document());
        state.show_help = true;
        let buf = render_to_buffer(&mut state, &theme, 70, 18);
        let s = buffer_to_string(&buf);
        assert!(
            s.contains("viewer keybindings"),
            "panel title visible:\n{s}"
        );
        assert!(s.contains("q / Esc / Ctrl-C"), "close binding visible");
        assert!(s.contains("Toggle this help panel"), "? binding listed");
        assert!(s.contains("Jump to end"), "G/End binding listed");
        insta::assert_snapshot!(snapshot_string(&buf));
    }

    #[test]
    fn draw_writes_viewport_height_and_total_lines_to_state() {
        let theme = Theme::default();
        let mut state = ViewerState::new(sample_document());
        let _ = render_to_buffer(&mut state, &theme, 40, 12);
        assert_eq!(
            state.viewport_height, 8,
            "12 total - 4 chrome (header/2 seps/footer)"
        );
        assert!(state.total_lines > 0);
    }

    #[test]
    fn toggle_tools_makes_tool_turns_visible() {
        let theme = Theme::default();
        let mut state = ViewerState::new(document_with_tools());
        let before = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
        assert!(!before.contains("Glob:"), "tools hidden by default");

        let (mut state, _) =
            crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleTools);
        let after = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
        assert!(after.contains("Glob:"), "tool turns visible after toggle");
        assert!(after.contains("Tool"), "tool chip label visible");
    }
}
