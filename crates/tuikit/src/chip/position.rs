//! Where a reader is in what it shows.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// A reader's position: `[ n/m ]`, the first row shown and how many rows
/// there are, counting from 1.
///
/// The row is held inside the content, so a reader scrolled past its end
/// or showing nothing still reads sensibly (`[ 1/1 ]` for an empty one).
/// It is drawn bold. A reader that fills the screen shows this chip; one
/// in a frame shows a scrollbar instead (house rule 18).
///
/// ```
/// use tuikit::chip::PositionChip;
///
/// assert_eq!(PositionChip::new(0, 240).text(), "[ 1/240 ]");
/// assert_eq!(PositionChip::new(500, 240).text(), "[ 240/240 ]");
/// assert_eq!(PositionChip::new(0, 0).text(), "[ 1/1 ]");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionChip {
    offset: usize,
    total: usize,
}

impl PositionChip {
    /// The chip for a reader whose first shown row is `offset` (from 0)
    /// of `total` rows.
    pub fn new(offset: usize, total: usize) -> Self {
        Self { offset, total }
    }

    /// The chip's text.
    pub fn text(&self) -> String {
        let total = self.total.max(1);
        let at = self.offset.saturating_add(1).min(total);
        format!("[ {at}/{total} ]")
    }

    /// The style the chip draws in: bold.
    pub fn look() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// The chip as one styled span.
    pub fn span(&self) -> Span<'static> {
        Span::styled(self.text(), Self::look())
    }
}
