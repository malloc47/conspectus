//! Full-value inspection modal (T8-030).
//!
//! Surfaces the untruncated text of a long detail-pane value (`cwd`,
//! `command`, `url`, `last_message_preview`, …) in a centered modal
//! so the operator can read or copy the full content without forcing
//! it into the main right-panel layout. Reuses the same centered
//! modal frame as the help overlay (`centered_modal_rect`) per the
//! existing modal conventions and avoids new dependencies.
//!
//! Input handling:
//! - `Esc` / `q` / `o` close the modal.
//! - `j` / `k` / `Down` / `Up` scroll the value body by one row.
//! - `PageDown` / `PageUp` / `Space` jump by a viewport.
//! - `g` / `G` jump to the top / bottom.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::tui::Theme;
use crate::tui::widgets::help::centered_modal_rect;

/// Pure state for the full-value modal.
#[derive(Debug, Clone)]
pub struct ValueModalState {
    /// Static label rendered in the title bar (`cwd`, `command`, …).
    pub label: String,
    /// The full untruncated value being inspected.
    pub value: String,
    /// Vertical scroll offset in rendered rows.
    pub scroll: u16,
}

impl ValueModalState {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            scroll: 0,
        }
    }

    /// Dispatch a crossterm key event. Returns
    /// [`ValueModalOutcome::Close`] when the modal should close,
    /// otherwise [`ValueModalOutcome::Continue`].
    pub fn handle_key(&mut self, event: KeyEvent) -> ValueModalOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return ValueModalOutcome::Close;
        }
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => ValueModalOutcome::Close,
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_by(1);
                ValueModalOutcome::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_by(-1);
                ValueModalOutcome::Continue
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_by(8);
                ValueModalOutcome::Continue
            }
            KeyCode::PageUp => {
                self.scroll_by(-8);
                ValueModalOutcome::Continue
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.scroll = 0;
                ValueModalOutcome::Continue
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.scroll = u16::MAX;
                ValueModalOutcome::Continue
            }
            _ => ValueModalOutcome::Continue,
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        let current = i32::from(self.scroll);
        let next = current.saturating_add(delta).max(0);
        self.scroll = u16::try_from(next.min(i32::from(u16::MAX))).unwrap_or(u16::MAX);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueModalOutcome {
    Continue,
    Close,
}

/// Centered modal that renders [`ValueModalState`]. Uses the same
/// `centered_modal_rect` frame as the help overlay so all modal
/// surfaces line up consistently.
pub struct ValueModalWidget<'a> {
    state: &'a ValueModalState,
    theme: &'a Theme,
}

impl<'a> ValueModalWidget<'a> {
    pub fn new(state: &'a ValueModalState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for ValueModalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area);
        // Clear the modal region so we don't blend with what's behind.
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
        let title_text = format!(" {} ", self.state.label);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    title_text,
                    Style::default()
                        .fg(self.theme.panel_focus_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ]));
        let inner = block.inner(modal);
        block.render(modal, buf);
        let body = Paragraph::new(self.state.value.clone())
            .wrap(Wrap { trim: false })
            .scroll((self.state.scroll, 0));
        body.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_closes_the_modal() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        assert_eq!(
            state.handle_key(press(KeyCode::Esc)),
            ValueModalOutcome::Close
        );
    }

    #[test]
    fn q_closes_the_modal() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        assert_eq!(
            state.handle_key(press(KeyCode::Char('q'))),
            ValueModalOutcome::Close
        );
    }

    #[test]
    fn o_toggles_the_modal_off() {
        // Mirrors the open key so operators can press `o` twice to
        // pop the modal back closed.
        let mut state = ValueModalState::new("cwd", "/long/path");
        assert_eq!(
            state.handle_key(press(KeyCode::Char('o'))),
            ValueModalOutcome::Close
        );
    }

    #[test]
    fn j_and_k_scroll_one_row() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        state.handle_key(press(KeyCode::Char('j')));
        assert_eq!(state.scroll, 1);
        state.handle_key(press(KeyCode::Char('j')));
        assert_eq!(state.scroll, 2);
        state.handle_key(press(KeyCode::Char('k')));
        assert_eq!(state.scroll, 1);
        // Scrolling above zero clamps.
        state.handle_key(press(KeyCode::Char('k')));
        state.handle_key(press(KeyCode::Char('k')));
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn page_down_and_up_jump_by_a_viewport() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        state.handle_key(press(KeyCode::PageDown));
        assert_eq!(state.scroll, 8);
        state.handle_key(press(KeyCode::PageUp));
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn g_and_capital_g_jump_to_top_and_bottom() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        state.handle_key(press(KeyCode::Char('G')));
        assert_eq!(state.scroll, u16::MAX);
        state.handle_key(press(KeyCode::Char('g')));
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn ctrl_c_closes_modal() {
        let mut state = ValueModalState::new("cwd", "/long/path");
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(state.handle_key(event), ValueModalOutcome::Close);
    }
}
