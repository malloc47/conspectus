//! Bare tmux new-session form overlay (ADR 0095).
//!
//! A minimal two-field modal — name + cwd — that commits a
//! [`Msg::CommitMuxNew`] the runtime turns into a subprocess re-exec of
//! `conspectus mux new`. Modeled on the worktree menu's branch-input
//! sub-mode but with two fields and `Tab` focus cycling.
//!
//! Menu-first principle: attach
//! (`a` / `Enter`) and rename (`R`) are already mux-relevant but
//! polymorphic across node kinds, so today they wouldn't shape a
//! useful mux-specific menu on their own. When a second bare-mux-
//! shape verb lands, the ADR 0095 follow-up notes an `m`-keyed
//! mux action menu will fold New + the future verb + surface
//! attach/rename for discovery. Today discoverability rides on the
//! `?` help overlay and the `n` binding in the KEYBINDINGS table.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::popup_frame;

/// Which field currently holds the cursor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NewMuxField {
    Name,
    Cwd,
}

impl NewMuxField {
    fn toggle(self) -> Self {
        match self {
            Self::Name => Self::Cwd,
            Self::Cwd => Self::Name,
        }
    }
}

/// Overlay state: two text inputs and which one has focus. Constructor
/// takes the pre-seeded defaults from the caller (runtime derives them
/// from the selected row).
#[derive(Debug, Clone)]
pub struct NewMuxFormState {
    name: TextInputState,
    cwd: TextInputState,
    focus: NewMuxField,
}

impl NewMuxFormState {
    /// New form with `name` / `cwd` pre-populated. Focus starts on the
    /// name field so operators can type a session name immediately;
    /// the cwd default is usually derived from the selected row and
    /// good as-is.
    pub fn new(name: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            name: TextInputState::new("Session name", name),
            cwd: TextInputState::new("Cwd", cwd),
            focus: NewMuxField::Name,
        }
    }

    pub fn name_value(&self) -> &str {
        self.name.value()
    }

    pub fn cwd_value(&self) -> &str {
        self.cwd.value()
    }

    pub fn focus(&self) -> NewMuxField {
        self.focus
    }

    fn active_input_mut(&mut self) -> &mut TextInputState {
        match self.focus {
            NewMuxField::Name => &mut self.name,
            NewMuxField::Cwd => &mut self.cwd,
        }
    }

    /// Try to commit. `Enter` on the name field advances focus to cwd
    /// rather than committing (mirrors two-field forms in the pins
    /// overlay); `Enter` on cwd commits when both fields are non-empty.
    /// Empty-field commit stays on that field with no side effect.
    fn try_commit(&mut self) -> OverlayOutcome {
        match self.focus {
            NewMuxField::Name => {
                if self.name.value().trim().is_empty() {
                    return OverlayOutcome::Consumed;
                }
                self.focus = NewMuxField::Cwd;
                OverlayOutcome::Consumed
            }
            NewMuxField::Cwd => {
                let name = self.name.value().trim().to_string();
                let cwd = self.cwd.value().trim().to_string();
                if name.is_empty() {
                    self.focus = NewMuxField::Name;
                    return OverlayOutcome::Consumed;
                }
                if cwd.is_empty() {
                    return OverlayOutcome::Consumed;
                }
                OverlayOutcome::Commit(Box::new(Msg::CommitMuxNew { name, cwd }))
            }
        }
    }
}

impl Overlay for NewMuxFormState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        // Global chords first (Tab / Esc / Ctrl-C) so they can't be
        // stolen by the line editor.
        match key.code {
            KeyCode::Esc => return OverlayOutcome::Close,
            KeyCode::Tab => {
                self.focus = self.focus.toggle();
                return OverlayOutcome::Consumed;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return OverlayOutcome::Close;
            }
            KeyCode::Enter => return self.try_commit(),
            _ => {}
        }
        match self.active_input_mut().handle_key(key) {
            InputOutcome::Continue => OverlayOutcome::Consumed,
            InputOutcome::Cancel => OverlayOutcome::Close,
            // TextInputState's own Enter handling is unreachable here
            // because we intercepted Enter above; kept for safety.
            InputOutcome::Confirm(_) => self.try_commit(),
        }
    }
}

/// Bordered popup rendering — a title line, then a compact two-line
/// stack of `label: value` rows with the focused field highlighted.
pub struct NewMuxFormWidget<'a> {
    state: &'a NewMuxFormState,
    theme: &'a Theme,
}

impl<'a> NewMuxFormWidget<'a> {
    pub fn new(state: &'a NewMuxFormState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for NewMuxFormWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let name_focused = self.state.focus == NewMuxField::Name;
        let cwd_focused = self.state.focus == NewMuxField::Cwd;

        let name_line = format_field_line("name", self.state.name.value(), name_focused);
        let cwd_line = format_field_line("cwd", self.state.cwd.value(), cwd_focused);

        let lines: Vec<Line> = vec![
            Line::styled(
                "New tmux session",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            name_line,
            cwd_line,
            Line::from(""),
            Line::from("Tab switch field · Enter next/commit · Esc cancel"),
        ];

        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(6, area.height);
        let width = 60_u16.min(area.width);
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

fn format_field_line(label: &str, value: &str, focused: bool) -> Line<'static> {
    let marker = if focused { "▸" } else { " " };
    let text = format!("{marker} {label}: {value}");
    if focused {
        Line::styled(text, Style::default().add_modifier(Modifier::BOLD))
    } else {
        Line::from(text)
    }
}

#[cfg(test)]
#[path = "new_mux_tests.rs"]
mod tests;
