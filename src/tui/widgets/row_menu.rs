//! Row action menu (`.`): the verbs that apply to the selected row,
//! picked from a list so they're discoverable before they're
//! memorized.
//!
//! The reducer builds the entries for the row under the cursor
//! ([`crate::tui::actions::row_menu_for`]); each entry commits a
//! [`Msg`]. An entry marked `confirm` needs a second `Enter` before it
//! commits, which is how destructive verbs (forget) are confirmed.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::popup_frame;

/// One verb in the menu.
#[derive(Debug, Clone, PartialEq)]
pub struct RowMenuEntry {
    pub label: String,
    pub subtitle: String,
    /// What picking the entry does.
    pub msg: Msg,
    /// Ask for a second `Enter` before committing.
    pub confirm: bool,
}

impl RowMenuEntry {
    pub fn new(label: impl Into<String>, subtitle: impl Into<String>, msg: Msg) -> Self {
        Self {
            label: label.into(),
            subtitle: subtitle.into(),
            msg,
            confirm: false,
        }
    }

    #[must_use]
    pub fn confirming(mut self) -> Self {
        self.confirm = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct RowMenuState {
    title: String,
    entries: Vec<RowMenuEntry>,
    cursor: usize,
    /// The entry waiting for its confirming `Enter`.
    armed: Option<usize>,
}

impl RowMenuState {
    /// `entries` must not be empty.
    pub fn new(title: impl Into<String>, entries: Vec<RowMenuEntry>) -> Self {
        debug_assert!(!entries.is_empty());
        Self {
            title: title.into(),
            entries,
            cursor: 0,
            armed: None,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn entries(&self) -> &[RowMenuEntry] {
        &self.entries
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn armed(&self) -> Option<usize> {
        self.armed
    }
}

impl Overlay for RowMenuState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        use crate::tui::keybindings::overlay::{MENU_KEYS, MenuKey, resolve};
        let last = self.entries.len().saturating_sub(1);
        match resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort | MenuKey::Close) => OverlayOutcome::Close,
            Some(MenuKey::Up) => {
                self.cursor = self.cursor.saturating_sub(1);
                self.armed = None;
                OverlayOutcome::Consumed
            }
            Some(MenuKey::Down) => {
                self.cursor = (self.cursor + 1).min(last);
                self.armed = None;
                OverlayOutcome::Consumed
            }
            Some(MenuKey::Pick) => {
                let entry = &self.entries[self.cursor.min(last)];
                if entry.confirm && self.armed != Some(self.cursor) {
                    self.armed = Some(self.cursor);
                    return OverlayOutcome::Consumed;
                }
                OverlayOutcome::Commit(Box::new(entry.msg.clone()))
            }
            None => OverlayOutcome::Consumed,
        }
    }
}

pub struct RowMenuWidget<'a> {
    state: &'a RowMenuState,
    theme: &'a Theme,
}

impl<'a> RowMenuWidget<'a> {
    pub fn new(state: &'a RowMenuState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for RowMenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![Line::from("")];
        for (idx, entry) in self.state.entries.iter().enumerate() {
            let focused = idx == self.state.cursor;
            let marker = if focused { "►" } else { " " };
            let armed = self.state.armed == Some(idx);
            let label = if armed {
                format!("{marker} {} — press Enter again to confirm", entry.label)
            } else {
                format!("{marker} {}", entry.label)
            };
            let style = if armed {
                Style::default()
                    .fg(self.theme.error)
                    .add_modifier(Modifier::BOLD)
            } else if focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::styled(label, style));
            lines.push(Line::from(format!("     {}", entry.subtitle)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(crate::tui::keybindings::overlay::footer(
            crate::tui::keybindings::overlay::MENU_KEYS,
        )));

        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(8, area.height);
        let width = 72_u16.min(area.width);
        let rect = popup_frame::centered_rect(area, width, height);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.state.title))
            .border_style(Style::default().fg(self.theme.panel_focus_accent));
        let inner = block.inner(rect);
        block.render(rect, buf);
        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
