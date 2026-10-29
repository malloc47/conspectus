//! The History view's Clean up modal: the records `history reap`
//! would remove, all preselected, each of which the operator can
//! deselect before confirming (ADR 0140). With nothing to remove, Clean
//! up is the framework's notice dialog instead ([`empty_notice`]).

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::age::Age;
use tuikit::dialog::Dialog;

use crate::history::reap::Candidate;
use crate::tui::Msg;
use crate::tui::theme::Theme;
use crate::tui::widgets::multi_select::{MultiSelectOutcome, MultiSelectState, MultiSelectWidget};
use tuikit::overlay::Outcome;

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
                    "{:<8} {} · last seen {}",
                    c.kind.as_str(),
                    c.label,
                    Age::between(c.last_seen, now).ago()
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
}

impl CleanUpState {
    /// One key: move, toggle, confirm the reap, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        match self.select.handle_key(key) {
            MultiSelectOutcome::Continue => Outcome::Consumed,
            MultiSelectOutcome::Cancel => Outcome::Close,
            MultiSelectOutcome::Confirm(indices) => {
                let ids: Vec<String> = indices
                    .into_iter()
                    .filter_map(|i| self.candidates.get(i).map(|c| c.id.clone()))
                    .collect();
                if ids.is_empty() {
                    Outcome::Close
                } else {
                    Outcome::Commit(Msg::ReapRecords(ids))
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
        MultiSelectWidget::new(&self.state.select, &self.state.labels, self.theme)
            .render(area, buf);
    }
}

/// What Clean up shows when nothing is past its reap age: a notice that
/// says why the list is empty, which any of the dialog keys dismisses.
pub fn empty_notice() -> Dialog<Msg> {
    Dialog::notice("Clean up").body([
        "Nothing to clean up.",
        "Stopped and removed records are kept until they pass their \
         `[history.reap_after]` age, and records a pin targets are never listed.",
    ])
}

#[cfg(test)]
#[path = "clean_up_tests.rs"]
mod tests;
