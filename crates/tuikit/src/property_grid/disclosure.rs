//! The disclosure row: a section header that opens and closes the rows
//! under it.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::grid::Intent;
use crate::glyphs::marks;
use crate::theme::Theme;

/// A section header the cursor can stand on: the disclosure mark, a
/// label, and a summary in parentheses of how many rows the section
/// holds and how many of them carry a mark (`▶ Other  (3 · 1 ▲)`).
///
/// The mark is [`marks::disclosure`] in the theme's `disclosure` color,
/// the arrow and the look a tree's folding rows have, so a section reads
/// as foldable in a grid and in a tree alike. The label and summary are
/// in the `inert` color, bold. Whether the rows under it are drawn is the
/// host's: activating the row asks for [`Intent::Toggle`].
///
/// ```
/// use tuikit::glyphs::marks;
/// use tuikit::property_grid::{DisclosureRow, Intent};
///
/// let header = DisclosureRow::new("Other", false)
///     .count(3)
///     .tally(1, marks::WARNING)
///     .tally(0, marks::NO_VALUE);
/// assert_eq!(header.summary(), "(3 · 1 ▲)", "a tally of nothing is left out");
/// assert_eq!(header.activate(), Some(Intent::Toggle));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisclosureRow {
    label: String,
    open: bool,
    count: Option<usize>,
    tallies: Vec<(usize, &'static str)>,
}

impl DisclosureRow {
    /// A header for the section `label`, `open` or closed.
    pub fn new(label: impl Into<String>, open: bool) -> Self {
        Self {
            label: label.into(),
            open,
            count: None,
            tallies: Vec::new(),
        }
    }

    /// How many rows the section holds, first in the summary.
    pub fn count(mut self, rows: usize) -> Self {
        self.count = Some(rows);
        self
    }

    /// How many of the section's rows carry `mark`, after the count. A
    /// tally of none is left out of the summary.
    pub fn tally(mut self, rows: usize, mark: &'static str) -> Self {
        self.tallies.push((rows, mark));
        self
    }

    /// Whether the section is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The summary as drawn: the count and every tally of some rows,
    /// joined by the separator mark in parentheses, or nothing when there
    /// is neither.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .count
            .map(|count| count.to_string())
            .into_iter()
            .chain(
                self.tallies
                    .iter()
                    .filter(|(rows, _)| *rows > 0)
                    .map(|(rows, mark)| format!("{rows} {mark}")),
            )
            .collect();
        if parts.is_empty() {
            String::new()
        } else {
            format!("({})", parts.join(marks::SPACED_SEPARATOR))
        }
    }

    /// What `Enter` on the header asks for: opening or closing it.
    pub fn activate(&self) -> Option<Intent> {
        Some(Intent::Toggle)
    }

    /// The row's spans after the cursor gutter.
    pub(super) fn draw(&self, theme: &Theme) -> Vec<Span<'static>> {
        let summary = self.summary();
        let text = if summary.is_empty() {
            format!(" {}", self.label)
        } else {
            format!(" {}  {summary}", self.label)
        };
        vec![
            Span::styled(
                marks::disclosure(self.open),
                Style::default().fg(theme.disclosure),
            ),
            Span::styled(
                text,
                Style::default()
                    .fg(theme.inert)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    }
}
