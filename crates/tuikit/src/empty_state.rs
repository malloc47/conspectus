//! The empty state: what a pane or a region says when it has nothing to
//! show.
//!
//! This is a layer 1 control. A list with no rows, a detail pane with
//! nothing selected, and a preview whose content has not loaded each fill
//! their slot with a sentence instead of leaving it blank: what is missing
//! (`Nothing matches "x".`) and, often, what to do about it (`Press s to
//! search again.`). Written at each site, those sentences drift: one
//! wraps and another is cut by the border, one recedes and another reads
//! like content. `EmptyState` gives them one look.
//!
//! - **A message**, and **an optional hint** on its own rows after it:
//!   the action that would fill the slot, or what happens next.
//! - **Wrapped** to the width at word edges by [`text::wrap`], so no word
//!   is cut by a border; the rows can be counted before they are drawn.
//! - **Muted**: both recede in the theme's `muted` look, so the slot never
//!   reads as content.
//! - **Left-aligned** from the slot's top-left corner, as a list's first
//!   row would be, or **centered** across the slot and down it, for a slot
//!   with nothing around it (a whole screen).
//!
//! The host keeps the words: which message and hint fit which state, in
//! its own vocabulary, with the keys it binds.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use tuikit::empty_state::EmptyState;
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let empty = EmptyState::new("Nothing matches \"draft\".").hint("Press `s` to search again.");
//! let wrapped = empty.rows(15);
//! assert_eq!(wrapped.len(), 4);
//! assert_eq!(wrapped[0], "Nothing matches");
//! assert_eq!(wrapped[1], "\"draft\".");
//! assert_eq!(wrapped[2].trim_end(), "Press `s` to");
//!
//! let area = Rect::new(0, 0, 20, 5);
//! let mut buf = Buffer::empty(area);
//! EmptyState::new("No items.").hint("Press n to add one.").centered().render(area, &mut buf, &theme);
//! assert_eq!(rows(&buf)[1], "     No items.      ");
//! assert_eq!(rows(&buf)[2], "Press n to add one. ");
//! assert_eq!(buf[(5, 1)].modifier, theme.muted);
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::text;
use crate::theme::Theme;

/// Where an [`EmptyState`]'s rows sit in their slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// From the slot's top-left corner, each row at its start.
    #[default]
    Left,
    /// Each row centered across the slot, and the rows centered down it
    /// when drawn in an area.
    Center,
}

/// A message, and an optional hint, standing in for content a slot does
/// not have: wrapped, muted, left-aligned or centered.
///
/// See the [module documentation](self) for the look.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EmptyState {
    message: String,
    hint: Option<String>,
    align: Align,
}

impl EmptyState {
    /// An empty state that says `message`, left-aligned, with no hint.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            align: Align::Left,
        }
    }

    /// Add `hint` on its own rows after the message: the action that
    /// would fill the slot, or what happens next.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Center the rows across the slot, and down it when drawn in an area.
    #[must_use]
    pub fn centered(self) -> Self {
        self.align(Align::Center)
    }

    /// Place the rows by `align`.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// The message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The hint, when there is one.
    pub fn hint_text(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// The message and the hint as one text, a line break between them,
    /// for a reader that takes plain text.
    pub fn text(&self) -> String {
        match &self.hint {
            Some(hint) => format!("{}\n{hint}", self.message),
            None => self.message.clone(),
        }
    }

    /// The rows the state draws at `width` cells: the message wrapped,
    /// then the hint wrapped, each row padded at its start to center it
    /// when the state is centered.
    pub fn rows(&self, width: usize) -> Vec<String> {
        let mut rows = text::wrap(&self.message, width);
        if let Some(hint) = &self.hint {
            rows.extend(text::wrap(hint, width));
        }
        if self.align == Align::Center {
            // A row keeps the spaces before its break; they are not text
            // to center.
            for row in &mut rows {
                let text = row.trim_end();
                let lead = width.saturating_sub(text::width(text)) / 2;
                *row = format!("{}{text}", " ".repeat(lead));
            }
        }
        rows
    }

    /// The rows at `width` cells as lines in the theme's `muted` look, for
    /// a region that lays out lines of its own (a scrolled body, a list's
    /// window).
    pub fn lines(&self, width: usize, theme: &Theme) -> Vec<Line<'static>> {
        let style = Self::look(theme);
        self.rows(width)
            .into_iter()
            .map(|row| Line::styled(row, style))
            .collect()
    }

    /// Draw the state in `area`: the area takes the `muted` look, and the
    /// rows go from its top, or down its middle when centered. Rows past
    /// the area's bottom are not drawn.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let lines = self.lines(usize::from(area.width), theme);
        let top = match self.align {
            Align::Left => 0,
            Align::Center => {
                let rows = u16::try_from(lines.len()).unwrap_or(u16::MAX);
                area.height.saturating_sub(rows) / 2
            }
        };
        let area = Rect {
            y: area.y + top,
            height: area.height - top,
            ..area
        };
        Paragraph::new(lines)
            .style(Self::look(theme))
            .render(area, buf);
    }

    /// The look of an empty state: the theme's `muted` modifier.
    fn look(theme: &Theme) -> Style {
        Style::default().add_modifier(theme.muted)
    }
}

impl From<&str> for EmptyState {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<String> for EmptyState {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

#[cfg(test)]
#[path = "empty_state_tests.rs"]
mod tests;
