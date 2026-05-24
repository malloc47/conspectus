//! `?` help overlay (F8-011).
//!
//! Minimal reference card for the current keymap. Render-only — no
//! mutation, no sub-editors — so the state struct is empty today and
//! exists mainly so the runtime can use the same open/close pattern
//! it uses for the controls, rename, and search overlays. As more
//! capabilities accumulate this can grow tabs or scrolling without
//! changing its boundary with the runtime.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// Pure state for the help overlay. Carries no settings today — the
/// renderer reads the static keymap below.
#[derive(Debug, Clone, Default)]
pub struct HelpOverlayState;

impl HelpOverlayState {
    pub fn new() -> Self {
        Self
    }

    /// Dispatch a crossterm key event. Esc / Ctrl-C / `q` close the
    /// overlay; everything else is swallowed so navigation keys
    /// don't accidentally affect the row tree behind it.
    pub fn handle_key(&mut self, event: KeyEvent) -> HelpOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return HelpOutcome::Close;
        }
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => HelpOutcome::Close,
            _ => HelpOutcome::Continue,
        }
    }
}

/// What the host should do after passing a key event through the
/// help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOutcome {
    Continue,
    Close,
}

/// Centered modal that renders the keymap reference. Lays out two
/// columns of `key · action` pairs grouped into sections so the
/// operator can scan for the action they want.
pub struct HelpOverlayWidget;

impl HelpOverlayWidget {
    pub fn new(_state: &HelpOverlayState) -> Self {
        Self
    }
}

impl Widget for HelpOverlayWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Help "));
        let inner = block.inner(modal);
        block.render(modal, buf);
        let para = Paragraph::new(body_lines());
        para.render(inner, buf);
    }
}

/// Build the static keymap. The first column is the key, the second
/// is a one-line description. Sections are bold; bindings are plain
/// text.
fn body_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    section(&mut lines, "Discoverable controls (ADR 0031)");
    binding(
        &mut lines,
        "v",
        "Open the controls overlay (view / grouping / filters / sort)",
    );
    binding(&mut lines, "?", "This help");
    blank(&mut lines);

    section(&mut lines, "View switching");
    binding(
        &mut lines,
        "1 – 5",
        "Switch directly to view N (sessions, mux, union, prs, forks)",
    );
    binding(&mut lines, "] / [", "Cycle to next / previous view");
    blank(&mut lines);

    section(&mut lines, "Filters & grouping");
    binding(
        &mut lines,
        "f",
        "Jump into the controls overlay's Filters section",
    );
    binding(
        &mut lines,
        "F",
        "Clear all active filters for the visible view",
    );
    binding(
        &mut lines,
        "Ctrl-G",
        "Cycle grouping forward for the active view",
    );
    blank(&mut lines);

    section(&mut lines, "Search");
    binding(
        &mut lines,
        "/",
        "Open the search overlay (ranks within the active filter set)",
    );
    blank(&mut lines);

    section(&mut lines, "Navigation");
    binding(&mut lines, "j / k / arrows", "Move selection down / up");
    binding(&mut lines, "PgDn / PgUp", "Page through the row tree");
    binding(&mut lines, "g / G", "First / last row");
    binding(&mut lines, "Enter", "Expand / collapse a parent row");
    binding(
        &mut lines,
        "Tab",
        "Cycle focus between left tree and right panel",
    );
    binding(&mut lines, "J / K", "Scroll the right-panel preview");
    blank(&mut lines);

    section(&mut lines, "Actions");
    binding(&mut lines, "a", "Attach to the selected mux");
    binding(&mut lines, "R", "Rename the selected agent session");
    binding(&mut lines, "r", "Refresh discovery now");
    binding(&mut lines, "q / Ctrl-C", "Quit");
    blank(&mut lines);

    lines.push(Line::from(Span::styled(
        "Press Esc, q, or ? to close.",
        Style::default().add_modifier(Modifier::DIM),
    )));
    lines
}

fn section(lines: &mut Vec<Line<'static>>, title: &str) {
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
}

fn binding(lines: &mut Vec<Line<'static>>, key: &str, desc: &str) {
    lines.push(Line::from(vec![
        Span::styled(format!("  {key:<14}"), Style::default().fg(Color::Cyan)),
        Span::raw(desc.to_string()),
    ]));
}

fn blank(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(""));
}

/// Centered modal sized to roughly two thirds of the terminal,
/// capped so it stays readable on wide screens.
pub fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(40);
    let max_height = area.height.saturating_sub(2);
    let desired = 26;
    let height = (desired as u16).clamp(10, max_height.max(10));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn esc_closes_help_overlay() {
        let mut state = HelpOverlayState::new();
        assert_eq!(state.handle_key(key(KeyCode::Esc)), HelpOutcome::Close);
    }

    #[test]
    fn q_and_question_mark_close_help_overlay() {
        let mut state = HelpOverlayState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('q'))),
            HelpOutcome::Close
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('?'))),
            HelpOutcome::Close
        );
    }

    #[test]
    fn ctrl_c_closes_help_overlay() {
        let mut state = HelpOverlayState::new();
        let event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(state.handle_key(event), HelpOutcome::Close);
    }

    #[test]
    fn other_keys_are_swallowed_not_propagated() {
        let mut state = HelpOverlayState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j'))),
            HelpOutcome::Continue
        );
        assert_eq!(state.handle_key(key(KeyCode::Enter)), HelpOutcome::Continue);
    }

    #[test]
    fn body_lines_cover_every_documented_key() {
        let rendered: String = body_lines()
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<_>>()
            .join(" ");
        // Spot-check the new ADR 0031 / T8-017 keys.
        for needle in [
            "v ", "f ", "F ", "Ctrl-G", "] / [", "/ ", "1 – 5", "Tab", "?",
        ] {
            assert!(
                rendered.contains(needle),
                "help text missing `{needle}` reference; full text:\n{rendered}"
            );
        }
    }
}
