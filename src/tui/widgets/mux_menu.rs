//! Mux action menu overlay (H-MUX-LAUNCH-001 / ADR 0096).
//!
//! `m`-keyed menu that fronts the mux-specific verbs. Discharges the
//! ADR 0095 follow-up ("when a second bare-mux-shape verb lands, ship
//! the `m`-keyed Mux action menu"): the second verb is
//! `mux launch` (ADR 0096), and this overlay is that menu.
//!
//! Entries:
//! - `New tmux session` — commits [`Msg::OpenNewMuxForm`], opening
//!   the ADR 0095 bare-mux form.
//! - `Launch harness in new mux…` — commits [`Msg::OpenMuxLaunchForm`],
//!   opening the ADR 0096 ephemeral harness launch form.
//!
//! Attach (`a` / `Enter`) and rename (`R`) stay on their polymorphic
//! global bindings; a follow-up story may surface them here for
//! discoverability once operator muscle memory settles.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::popup_frame;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MuxMenuChoice {
    NewBareMux,
    LaunchHarnessMux,
}

impl MuxMenuChoice {
    fn label(self) -> &'static str {
        match self {
            Self::NewBareMux => "New tmux session (bare shell)",
            Self::LaunchHarnessMux => "Launch harness in new mux… (no pin)",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::NewBareMux => "ADR 0095 · shell only, no harness, no pin",
            Self::LaunchHarnessMux => "ADR 0096 · spawn harness, no pin persistence",
        }
    }
}

const MENU_ENTRIES: [MuxMenuChoice; 2] =
    [MuxMenuChoice::NewBareMux, MuxMenuChoice::LaunchHarnessMux];

#[derive(Debug, Clone, Default)]
pub struct MuxMenuState {
    cursor: usize,
}

impl MuxMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn entries(&self) -> &'static [MuxMenuChoice] {
        &MENU_ENTRIES
    }

    fn selected(&self) -> MuxMenuChoice {
        MENU_ENTRIES[self.cursor.min(MENU_ENTRIES.len() - 1)]
    }
}

impl Overlay for MuxMenuState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Esc => OverlayOutcome::Close,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                OverlayOutcome::Close
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                OverlayOutcome::Consumed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(MENU_ENTRIES.len() - 1);
                OverlayOutcome::Consumed
            }
            KeyCode::Enter => match self.selected() {
                MuxMenuChoice::NewBareMux => OverlayOutcome::Commit(Box::new(Msg::OpenNewMuxForm)),
                MuxMenuChoice::LaunchHarnessMux => {
                    OverlayOutcome::Commit(Box::new(Msg::OpenMuxLaunchForm))
                }
            },
            _ => OverlayOutcome::Consumed,
        }
    }
}

pub struct MuxMenuWidget<'a> {
    state: &'a MuxMenuState,
    theme: &'a Theme,
}

impl<'a> MuxMenuWidget<'a> {
    pub fn new(state: &'a MuxMenuState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for MuxMenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::styled("Mux actions", Style::default().add_modifier(Modifier::BOLD)),
            Line::from(""),
        ];
        for (idx, entry) in MENU_ENTRIES.iter().enumerate() {
            let focused = idx == self.state.cursor;
            let marker = if focused { "▸" } else { " " };
            let label = format!("{marker} {}", entry.label());
            if focused {
                lines.push(Line::styled(
                    label,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            } else {
                lines.push(Line::from(label));
            }
            lines.push(Line::from(format!("     {}", entry.subtitle())));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("↑/↓ move · Enter pick · Esc close"));

        let height = (lines.len() as u16)
            .saturating_add(2)
            .clamp(10, area.height);
        let width = 62_u16.min(area.width);
        let rect = popup_frame::centered_rect(area, width, height);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Mux ")
            .border_style(Style::default().fg(self.theme.panel_focus_accent));
        let inner = block.inner(rect);
        block.render(rect, buf);
        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn enter_on_first_entry_opens_new_mux_form() {
        let mut state = MuxMenuState::new();
        match state.handle((), key(KeyCode::Enter)) {
            OverlayOutcome::Commit(msg) => match *msg {
                Msg::OpenNewMuxForm => {}
                other => panic!("expected OpenNewMuxForm, got {other:?}"),
            },
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn enter_on_second_entry_opens_launch_form() {
        let mut state = MuxMenuState::new();
        state.handle((), key(KeyCode::Down));
        match state.handle((), key(KeyCode::Enter)) {
            OverlayOutcome::Commit(msg) => match *msg {
                Msg::OpenMuxLaunchForm => {}
                other => panic!("expected OpenMuxLaunchForm, got {other:?}"),
            },
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn cursor_clamps_at_last_entry() {
        let mut state = MuxMenuState::new();
        for _ in 0..10 {
            state.handle((), key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), MENU_ENTRIES.len() - 1);
    }
}
