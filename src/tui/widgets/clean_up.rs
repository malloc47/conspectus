//! The History view's Clean up modal: the records `history reap`
//! would remove, all preselected, each of which the operator can
//! deselect before confirming (ADR 0140).

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::history::reap::Candidate;
use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::multi_select::{MultiSelectOutcome, MultiSelectState, MultiSelectWidget};
use crate::tui::widgets::popup_frame;

#[derive(Debug, Clone)]
pub struct CleanUpState {
    candidates: Vec<Candidate>,
    labels: Vec<String>,
    select: MultiSelectState,
}

impl CleanUpState {
    /// `now` dates the "last seen" ages.
    pub fn new(candidates: Vec<Candidate>, now: i64) -> Self {
        let labels = candidates
            .iter()
            .map(|c| {
                format!(
                    "{:<8} {} · last seen {} ago",
                    c.kind.as_str(),
                    c.label,
                    crate::output::render::format_relative_age(c.last_seen, now)
                )
            })
            .collect();
        let all: Vec<usize> = (0..candidates.len()).collect();
        let select =
            MultiSelectState::new("Clean up: forget these records", candidates.len(), &all);
        Self {
            candidates,
            labels,
            select,
        }
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    pub fn selected(&self) -> Vec<usize> {
        self.select.selected_indices()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

impl Overlay for CleanUpState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        if self.candidates.is_empty() {
            // Nothing to choose: any menu key dismisses the notice.
            use crate::tui::keybindings::overlay::{MENU_KEYS, MenuKey, resolve};
            return match resolve(MENU_KEYS, key) {
                Some(MenuKey::Abort | MenuKey::Close | MenuKey::Pick) => OverlayOutcome::Close,
                _ => OverlayOutcome::Consumed,
            };
        }
        match self.select.handle_key(key) {
            MultiSelectOutcome::Continue => OverlayOutcome::Consumed,
            MultiSelectOutcome::Cancel => OverlayOutcome::Close,
            MultiSelectOutcome::Confirm(indices) => {
                let ids: Vec<String> = indices
                    .into_iter()
                    .filter_map(|i| self.candidates.get(i).map(|c| c.id.clone()))
                    .collect();
                if ids.is_empty() {
                    OverlayOutcome::Close
                } else {
                    OverlayOutcome::Commit(Box::new(Msg::ReapRecords(ids)))
                }
            }
        }
    }
}

pub struct CleanUpWidget<'a> {
    state: &'a CleanUpState,
    theme: &'a Theme,
}

impl<'a> CleanUpWidget<'a> {
    pub fn new(state: &'a CleanUpState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for CleanUpWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.state.is_empty() {
            MultiSelectWidget::new(&self.state.select, &self.state.labels)
                .theme(self.theme)
                .render(area, buf);
            return;
        }
        let lines = vec![
            Line::from(""),
            Line::from("Nothing to clean up."),
            Line::from(
                "Stopped and removed records are kept until they pass their `[history.reap_after]`",
            ),
            Line::from("age, and records a pin targets are never listed."),
            Line::from(""),
            Line::from("Esc/Enter close"),
        ];
        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .min(area.height);
        let rect = popup_frame::centered_rect(area, 68_u16.min(area.width), height);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Clean up ")
            .border_style(Style::default().fg(self.theme.panel_focus_accent));
        let inner = block.inner(rect);
        block.render(rect, buf);
        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
#[path = "clean_up_tests.rs"]
mod tests;
