//! The text field: a line editor drawn in one row, windowed so its
//! cursor stays in view.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::LineEdit;
use crate::glyphs::marks;
use crate::text::{self, Cluster, clusters};
use crate::theme::Theme;

/// The mark at the start of a field whose text scrolled out to the left.
const LEFT_MARK: &str = "< ";
/// The mark at the end of a field whose text runs on past the right.
const RIGHT_MARK: &str = " >";
/// Cells each mark's slot takes, drawn blank while nothing is hidden on
/// that side, so the text does not move when a mark appears.
const MARK_WIDTH: usize = 2;
/// The narrowest field that keeps its marks: one cell of text between
/// the two slots. A narrower field gives every cell to the text.
const MIN_MARKED: usize = 2 * MARK_WIDTH + 1;

/// A [`LineEdit`] drawn inline, in one row: the one renderer for every
/// text field (a form row's value, a query line, a find prompt, the line
/// of a prompt).
///
/// The row shows a window onto the text that always holds the cursor.
/// There is one windowing rule, measured in terminal cells: the window
/// starts at the line's start while the text before the cursor and the
/// cell the cursor sits on fit; once they do not, it starts at the first
/// grapheme that lets them fit, so the cursor rides the right edge while
/// typing moves it on. After the cursor the window fills with as much of
/// the text as fits. A wide character is never split: when it would
/// straddle the edge it is left out and the cell stays blank. A
/// character wider than the whole field is drawn as
/// [`marks::ELLIPSIS`].
///
/// - **Caret.** While the field is [`focused`](Self::focused) the cell
///   under the cursor (a blank cell at the end of the text) takes the
///   theme's `field_cursor` style. An unfocused field draws no caret.
/// - **Marks.** With [`marks`](Self::marks), a 2-cell slot at each end
///   shows `<` when text is scrolled out to the left and `>` when more
///   follows, in the `accent` color, bold, over the field's style.
/// - **Prefix.** A [`prefix`](Self::prefix), such as a `/` before a
///   query, is drawn before the field and never scrolls.
/// - **Placeholder.** An empty field shows its
///   [`placeholder`](Self::placeholder) in the field's style, with the
///   caret on its first cell.
/// - **Fill.** With [`fill`](Self::fill) the field takes exactly its
///   width, padded in its own style, so a filled field draws its whole
///   box; without it the row ends where the text does, so text drawn
///   after the field (a hint) follows it.
///
/// ```
/// use ratatui::style::Style;
/// use ratatui::text::Span;
/// use tuikit::text_edit::{LineEdit, TextField};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let query = LineEdit::new("needle in a haystack");
/// let slash = Span::styled("/", Style::new().fg(theme.accent));
/// let line = TextField::new(&query).prefix(slash).line(12, &theme);
/// // The caret sits on the blank cell after the text, so the window
/// // shows the end of the query.
/// let text: String = line.spans.iter().map(|span| span.content.as_ref()).collect();
/// assert_eq!(text, "/a haystack ");
/// ```
#[derive(Clone, Debug)]
pub struct TextField<'a> {
    edit: &'a LineEdit,
    focused: bool,
    marks: bool,
    fill: bool,
    prefix: Option<Span<'a>>,
    placeholder: &'a str,
    style: Style,
}

impl<'a> TextField<'a> {
    /// A focused field over `edit`, unstyled, with no marks, prefix,
    /// placeholder, or fill.
    pub fn new(edit: &'a LineEdit) -> Self {
        Self {
            edit,
            focused: true,
            marks: false,
            fill: false,
            prefix: None,
            placeholder: "",
            style: Style::default(),
        }
    }

    /// Whether the field has the keys, so its caret is drawn.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the field keeps a 2-cell slot at each end for the `<` and
    /// `>` scroll marks.
    #[must_use]
    pub fn marks(mut self, marks: bool) -> Self {
        self.marks = marks;
        self
    }

    /// Whether the field pads to its whole width in its own style.
    #[must_use]
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// What is drawn before the field, in its own style: a `/` before a
    /// query, a label.
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<Span<'a>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// What an empty field shows instead of nothing.
    #[must_use]
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// The style of the field's text, its padding, and the ground of its
    /// marks: a form's field fill, or plain text.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// The field drawn into `width` cells: at most `width`, and exactly
    /// `width` with [`fill`](Self::fill) (a prefix wider than the row is
    /// cut to it).
    pub fn spans(&self, width: usize, theme: &Theme) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut room = width;
        if let Some(prefix) = &self.prefix {
            let shown = text::clip(&prefix.content, room);
            room -= text::width(&shown);
            push(&mut spans, shown, prefix.style);
        }
        let marked = self.marks && room >= MIN_MARKED;
        let content = if marked { room - 2 * MARK_WIDTH } else { room };
        if content == 0 {
            return spans;
        }
        let (shown, cursor) = if self.edit.is_empty() && !self.placeholder.is_empty() {
            (self.placeholder, 0)
        } else {
            (
                self.edit.value(),
                byte_at(self.edit.value(), self.edit.cursor()),
            )
        };
        let window = Window::of(&clusters(shown), cursor, content);
        let caret = if self.focused {
            theme.field_cursor.into_style()
        } else {
            self.style
        };
        let mark = self.style.fg(theme.accent).add_modifier(Modifier::BOLD);
        if marked {
            spans.push(slot(LEFT_MARK, window.hidden_before, mark));
        }
        push(&mut spans, window.before, self.style);
        push(&mut spans, window.at, caret);
        push(&mut spans, window.after, self.style);
        if self.fill {
            push(
                &mut spans,
                " ".repeat(content.saturating_sub(window.used)),
                self.style,
            );
        }
        if marked {
            spans.push(slot(RIGHT_MARK, window.hidden_after, mark));
        }
        spans
    }

    /// The field drawn into `width` cells, as a line.
    pub fn line(&self, width: usize, theme: &Theme) -> Line<'static> {
        Line::from(self.spans(width, theme))
    }

    /// Draw the field in the first row of `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.is_empty() {
            return;
        }
        let line = self.line(usize::from(area.width), theme);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

/// The part of a line a field shows: the text before the cursor, the
/// cell under it, and the text after it, with whether text is hidden on
/// either side.
struct Window {
    before: String,
    at: String,
    after: String,
    /// Cells the three parts take.
    used: usize,
    hidden_before: bool,
    hidden_after: bool,
}

impl Window {
    /// The window onto `clusters` around the cursor at byte `cursor`, in
    /// `cells` cells (at least one).
    fn of(clusters: &[Cluster<'_>], cursor: usize, cells: usize) -> Self {
        let count = clusters.len();
        // The grapheme the cursor is on: the one that holds its byte, or
        // none (a blank cell) at the end.
        let at = clusters
            .iter()
            .position(|cluster| cluster.end() > cursor)
            .unwrap_or(count);
        let (at_text, at_width) = match clusters.get(at) {
            // A control character draws nothing, so its cell is blank.
            Some(cluster) if cluster.width == 0 => (" ".to_string(), 1),
            Some(cluster) if cluster.width > cells => (marks::ELLIPSIS.to_string(), 1),
            Some(cluster) => (cluster.text.to_string(), cluster.width),
            None => (" ".to_string(), 1),
        };
        let mut start = 0;
        let mut before: usize = clusters[..at].iter().map(|cluster| cluster.width).sum();
        while start < at && before + at_width > cells {
            before -= clusters[start].width;
            start += 1;
        }
        let mut used = before + at_width;
        let mut end = (at + 1).min(count);
        while end < count && used + clusters[end].width <= cells {
            used += clusters[end].width;
            end += 1;
        }
        let join = |range: &[Cluster<'_>]| range.iter().map(|cluster| cluster.text).collect();
        Self {
            before: join(&clusters[start..at]),
            at: at_text,
            after: join(&clusters[(at + 1).min(count)..end]),
            used,
            hidden_before: start > 0,
            hidden_after: end < count,
        }
    }
}

/// The byte offset of the character `cursor` characters into `text`.
fn byte_at(text: &str, cursor: usize) -> usize {
    text.char_indices()
        .nth(cursor)
        .map_or(text.len(), |(index, _)| index)
}

/// A mark's slot: the mark in `style` when text is hidden on its side,
/// blank and unstyled otherwise.
fn slot(mark: &'static str, shown: bool, style: Style) -> Span<'static> {
    if shown {
        Span::styled(mark, style)
    } else {
        Span::raw(" ".repeat(MARK_WIDTH))
    }
}

/// Add `content` in `style`, unless it is empty.
fn push(spans: &mut Vec<Span<'static>>, content: impl Into<String>, style: Style) {
    let content = content.into();
    if !content.is_empty() {
        spans.push(Span::styled(content, style));
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
