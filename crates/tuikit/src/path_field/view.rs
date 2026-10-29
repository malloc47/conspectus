//! Drawing a path field: the text, its validity mark, and the one
//! completion hint rule.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::PathField;
use crate::keys::{KeyRow, PATH_FIELD_KEYS, PathKey, key_label, rows};
use crate::text;
use crate::text_edit::TextField;
use crate::theme::Theme;

/// Cells the validity mark takes after the text: a space and the mark.
const MARK_CELLS: usize = 2;
/// The most cells the completion hint takes, its leading space included.
const HINT_CELLS: usize = 30;
/// The fewest cells a hint is drawn in: a space, the key and its colon
/// and space, and two cells of the remainder.
const MIN_HINT_CELLS: usize = 8;

/// A [`PathField`] drawn in one row: the one look for a path field, in
/// every form.
///
/// The row is exactly as wide as asked, in three parts:
///
/// 1. **The text**, a filled [`TextField`] with the knobs a host sets
///    here (focus, scroll marks, placeholder, style), so the mark after
///    it stays in one column whatever is typed.
/// 2. **The validity mark**, a space and [`Validity::mark`] in
///    [`Validity::style`]: `?`, the success mark, or the warning mark.
/// 3. **The completion hint**, only while the field is focused and a
///    completion exists: a space and `Tab: <remainder>` in the `accent`
///    color, the remainder cut in the middle to fit. It takes at most 30
///    cells and at most half the row, and the text gives up those cells
///    only while the hint shows. A row too narrow for a hint of 8 cells
///    shows none.
///
/// The hint shows exactly what `Tab` adds, since a completion only ever
/// extends the typed text.
///
/// [`Validity::mark`]: super::Validity::mark
/// [`Validity::style`]: super::Validity::style
#[derive(Clone, Debug)]
pub struct PathFieldView<'a> {
    field: &'a PathField,
    focused: bool,
    marks: bool,
    placeholder: &'a str,
    style: Style,
}

impl<'a> PathFieldView<'a> {
    /// A focused view of `field`, unstyled, with no scroll marks or
    /// placeholder.
    pub(super) fn new(field: &'a PathField) -> Self {
        Self {
            field,
            focused: true,
            marks: false,
            placeholder: "",
            style: Style::default(),
        }
    }

    /// Whether the field has the keys: its caret is drawn and its
    /// completion hint shown.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the text keeps a slot at each end for the `<` and `>`
    /// scroll marks ([`TextField::marks`]).
    #[must_use]
    pub fn marks(mut self, marks: bool) -> Self {
        self.marks = marks;
        self
    }

    /// What empty text shows instead of nothing.
    #[must_use]
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// The style of the text and its fill: a form's field fill.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// The cells the hint takes in a row of `width`: none unless the
    /// field is focused, a completion exists, and the row has room.
    fn hint_width(&self, width: usize) -> usize {
        if !self.focused || self.field.completion().is_none() {
            return 0;
        }
        let reserve = HINT_CELLS.min(width / 2);
        let text = width.saturating_sub(MARK_CELLS + reserve);
        if reserve < MIN_HINT_CELLS || text == 0 {
            0
        } else {
            reserve
        }
    }

    /// The field drawn into exactly `width` cells. A row of two cells or
    /// fewer has no room for the mark and shows the text alone.
    pub fn spans(&self, width: usize, theme: &Theme) -> Vec<Span<'static>> {
        let text_field = TextField::new(self.field.edit())
            .focused(self.focused)
            .marks(self.marks)
            .fill(true)
            .placeholder(self.placeholder)
            .style(self.style);
        if width <= MARK_CELLS {
            return text_field.spans(width, theme);
        }
        let hint = self.hint_width(width);
        let mut spans = text_field.spans(width - MARK_CELLS - hint, theme);
        let validity = self.field.validity();
        spans.push(Span::raw(" "));
        spans.push(Span::styled(validity.mark(), validity.style(theme)));
        if let Some(completion) = self.field.completion().filter(|_| hint > 0) {
            // The space before the hint is a gap, not part of it, so a
            // highlighted row does not color it.
            let lead = format!("{}: ", complete_label());
            let room = hint.saturating_sub(1 + text::width(&lead));
            let shown = format!(
                "{lead}{}",
                text::truncate_middle(completion.remainder(), room)
            );
            let used = 1 + text::width(&shown);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(shown, Style::default().fg(theme.accent)));
            spans.push(Span::raw(" ".repeat(hint.saturating_sub(used))));
        }
        spans
    }

    /// The field drawn into `width` cells, as a line.
    pub fn line(&self, width: usize, theme: &Theme) -> Line<'static> {
        Line::from(self.spans(width, theme))
    }
}

/// The key that completes, as the path field's table writes it.
fn complete_label() -> String {
    rows(PATH_FIELD_KEYS)
        .find_map(|row| match row {
            KeyRow::Bind {
                key,
                action: PathKey::Complete,
                ..
            } => Some(key_label(key)),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
