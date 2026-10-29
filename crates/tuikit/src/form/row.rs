//! The row kit: how each kind of form row is drawn.
//!
//! A [`Form`](super::Form) draws its fields through these functions, and a
//! host form that lays its rows out itself calls the same ones, so every
//! form has one look. A row is the cursor gutter, the label in the label
//! column ([`Cols`]), the gap, and the value: a text field, a path field,
//! a choice with its chips, a row of options, or a line that is only read.
//! In a form with override rows, a text, path, or choice row ends in the
//! source column ([`source_cells`]), blank on a row that is not one.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::keys::{Footer, KeyRow};
use crate::path_field::PathField;
use crate::selection::{self, Cursor};
use crate::text;
use crate::text_edit::{LineEdit, TextField};
use crate::theme::Theme;

/// The narrowest label column, so the values of forms with short labels
/// start in the same column.
pub const LABEL_MIN: usize = 11;
/// The widest label column. A longer label is cut with the ellipsis mark
/// rather than pushing its value out of the column.
pub const LABEL_MAX: usize = 16;
/// Cells between the label column and the value.
pub const LABEL_GAP: usize = 2;
/// Cells between a row's value and its source column.
pub const SOURCE_GAP: usize = 1;
/// What an empty text or path field shows.
pub const EMPTY: &str = "-";
/// What a choice shows after a typed value that is none of its options.
pub const CUSTOM: &str = "[custom]";
/// Cells a choice's value gets before its chips.
pub const CHOICE_SLOT: usize = 18;
/// Cells before the text of a row that is not edited in place (options, a
/// toggle, a line that is only read), so its text starts where a text
/// field's does, after the field's scroll-mark slot.
const TEXT_INDENT: usize = 2;

/// The widths a form's rows are laid out in: the frame's inner width, the
/// label column, and the source column at the right edge that a form
/// with override rows has.
///
/// ```
/// use tuikit::form::Cols;
///
/// let cols = Cols::for_labels(60, ["name", "where"]);
/// assert_eq!(cols.label, 11, "short labels take the narrowest column");
/// assert_eq!(cols.lead(), 15, "the gutter, the label, and the gap");
/// assert_eq!(cols.value(), 45);
/// assert_eq!(cols.with_source(11).value(), 33, "the gap and the source column");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cols {
    /// The width a row is laid out in: the frame's inner width.
    pub inner: usize,
    /// The width of the label column.
    pub label: usize,
    /// The width of the source column after the value, where an override
    /// row says where its value comes from; `0` for a form without one.
    pub source: usize,
}

impl Cols {
    /// Rows `inner` cells wide with the narrowest label column and no
    /// source column.
    pub fn new(inner: usize) -> Self {
        Self {
            inner,
            label: LABEL_MIN,
            source: 0,
        }
    }

    /// Rows `inner` cells wide whose label column fits the longest of
    /// `labels`, between [`LABEL_MIN`] and [`LABEL_MAX`].
    pub fn for_labels<'a>(inner: usize, labels: impl IntoIterator<Item = &'a str>) -> Self {
        let longest = labels.into_iter().map(text::width).max().unwrap_or(0);
        Self {
            label: longest.clamp(LABEL_MIN, LABEL_MAX),
            ..Self::new(inner)
        }
    }

    /// The same rows with a label column `label` cells wide.
    #[must_use]
    pub fn with_label(mut self, label: usize) -> Self {
        self.label = label;
        self
    }

    /// The same rows with a source column `source` cells wide (none when
    /// `0`).
    #[must_use]
    pub fn with_source(mut self, source: usize) -> Self {
        self.source = source;
        self
    }

    /// The cells before a row's value: the cursor gutter, the label
    /// column, and the gap.
    pub fn lead(self) -> usize {
        selection::GUTTER_WIDTH + self.label + LABEL_GAP
    }

    /// The cells after a text, path, or choice row's value: the gap and
    /// the source column, or nothing without one.
    pub fn tail(self) -> usize {
        if self.source == 0 {
            0
        } else {
            SOURCE_GAP + self.source
        }
    }

    /// The cells a text, path, or choice row's value gets: what the lead
    /// and the tail leave, at least one.
    pub fn value(self) -> usize {
        self.inner
            .saturating_sub(self.lead())
            .saturating_sub(self.tail())
            .max(1)
    }
}

/// `label` in a label column `width` cells wide, followed by the gap
/// before the value. A label wider than the column is cut with the
/// ellipsis mark.
pub fn label(label: &str, width: usize) -> String {
    format!("{}{}", text::fit(label, width), " ".repeat(LABEL_GAP))
}

/// A form row: the cursor gutter, then `spans`. The focused field's row
/// carries the list cursor's marker and highlight, as a list's cursor row
/// does; the fills of its value keep their look inside the highlight.
pub fn row(spans: Vec<Span<'static>>, focused: bool, theme: &Theme) -> Line<'static> {
    selection::row(spans, Cursor::of(focused, true), theme)
}

/// A field's fill: `field_active` while it has focus, `field` otherwise.
pub fn field_style(focused: bool, theme: &Theme) -> Style {
    if focused {
        theme.field_active.into_style()
    } else {
        theme.field.into_style()
    }
}

/// A text field over `edit` in exactly `width` cells: the value in its
/// fill, windowed around the cursor with `<` and `>` marking what
/// scrolled out of view, the caret while it has focus, and [`EMPTY`] for
/// an empty value. The cell the cursor sits on is drawn either way, so
/// moving the focus never shifts what follows the field.
pub fn text_cells(
    edit: &LineEdit,
    width: usize,
    focused: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    TextField::new(edit)
        .focused(focused)
        .marks(true)
        .fill(true)
        .placeholder(EMPTY)
        .style(field_style(focused, theme))
        .spans(width.max(1), theme)
}

/// A path field in exactly `width` cells: the value as [`text_cells`]
/// draws it, the validity mark, and, while the field has focus and a
/// completion exists, the `Tab:` hint (the path field's one hint rule).
pub fn path_cells(
    field: &PathField,
    width: usize,
    focused: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    field
        .view()
        .focused(focused)
        .marks(true)
        .placeholder(EMPTY)
        .style(field_style(focused, theme))
        .spans(width, theme)
}

/// A text field's row: the label, then the field over the value column.
pub fn text_row(
    label_text: &str,
    edit: &LineEdit,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(label(label_text, cols.label))];
    spans.extend(text_cells(edit, cols.value(), focused, theme));
    row(spans, focused, theme)
}

/// A path field's row: the label, then the field over the value column.
pub fn path_row(
    label_text: &str,
    field: &PathField,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(label(label_text, cols.label))];
    spans.extend(path_cells(field, cols.value(), focused, theme));
    row(spans, focused, theme)
}

/// A choice in `width` cells: the typed value in a slot of
/// [`CHOICE_SLOT`] cells, then a chip for each option with the chosen one
/// in `accent`; or, for a value that is none of the options, the value
/// over the rest of the width and [`CUSTOM`] in `warning`.
pub fn choice_cells(
    edit: &LineEdit,
    options: &[String],
    width: usize,
    focused: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let value = edit.value().trim();
    if !value.is_empty() && !options.iter().any(|option| option == value) {
        let mark = format!(" {CUSTOM}");
        let value_width = width.saturating_sub(text::width(&mark)).max(1);
        let mut spans = text_cells(edit, value_width, focused, theme);
        spans.push(Span::styled(mark, Style::default().fg(theme.warning)));
        return spans;
    }
    let value_width = CHOICE_SLOT.min(width).max(1);
    let mut spans = text_cells(edit, value_width, focused, theme);
    spans.extend(chips(
        value,
        options,
        width.saturating_sub(value_width),
        theme,
    ));
    spans
}

/// A choice's row: the label, then [`choice_cells`] over the value
/// column.
pub fn choice_row(
    label_text: &str,
    edit: &LineEdit,
    options: &[String],
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(label(label_text, cols.label))];
    spans.extend(choice_cells(edit, options, cols.value(), focused, theme));
    row(spans, focused, theme)
}

/// A chip for each of `options` that fits in `width` cells (` [name]`),
/// `selected` in `accent` and bold and the others `muted`; when the rest
/// do not fit, an ellipsis mark says there are more.
pub fn chips(
    selected: &str,
    options: &[String],
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    if width < 3 {
        return Vec::new();
    }
    let muted = Style::default().add_modifier(theme.muted);
    let mut spans = Vec::new();
    let mut remaining = width;
    for option in options {
        let token = format!(" [{option}]");
        let needed = text::width(&token);
        if needed > remaining {
            if remaining >= 2 {
                spans.push(Span::styled(format!(" {}", marks::ELLIPSIS), muted));
            }
            break;
        }
        let style = if option == selected {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            muted
        };
        spans.push(Span::styled(token, style));
        remaining -= needed;
    }
    spans
}

/// `text` after a checkbox: `[x] text` when `on`, `[ ] text` when not.
pub fn checkbox(text: &str, on: bool) -> String {
    let mark = if on { marks::CHECKED } else { marks::UNCHECKED };
    format!("{mark} {text}")
}

/// `text` after a one-of-N option's box: `(•) text` when it is the
/// chosen one, `( ) text` when not. The shape differs from a checkbox's.
pub fn option(text: &str, chosen: bool) -> String {
    let mark = if chosen {
        marks::CHOSEN
    } else {
        marks::NOT_CHOSEN
    };
    format!("{mark} {text}")
}

/// Text that is not edited in place (a toggle, a row of options) in
/// `width` cells: indented to where a text field's text starts, in the
/// field's fill, cut with the ellipsis mark when it does not fit.
pub fn static_cells(value: &str, width: usize, focused: bool, theme: &Theme) -> Vec<Span<'static>> {
    let room = width.saturating_sub(TEXT_INDENT);
    vec![
        Span::raw(" ".repeat(TEXT_INDENT)),
        Span::styled(text::truncate_end(value, room), field_style(focused, theme)),
    ]
}

/// A row of text that is not edited in place: the label, then
/// [`static_cells`] over the value column.
pub fn static_row(
    label_text: &str,
    value: &str,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(label(label_text, cols.label))];
    spans.extend(static_cells(
        value,
        cols.inner.saturating_sub(cols.lead()),
        focused,
        theme,
    ));
    row(spans, focused, theme)
}

/// A toggle's row: the label, then `text` after a checkbox that is
/// checked when `on`.
pub fn toggle_row(
    label_text: &str,
    on: bool,
    text: &str,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    static_row(label_text, &checkbox(text, on), focused, cols, theme)
}

/// A one-of-N row: the label, then each option after its box, the chosen
/// one marked [`marks::CHOSEN`].
pub fn one_of_row(
    label_text: &str,
    options: &[String],
    chosen: usize,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let value = options
        .iter()
        .enumerate()
        .map(|(index, text)| option(text, index == chosen))
        .collect::<Vec<_>>()
        .join(" ");
    static_row(label_text, &value, focused, cols, theme)
}

/// A row of checkboxes: the label, then each item after its checkbox.
/// While the row has focus and holds several items, the item under its
/// cursor (`at`) takes `field_cursor`, the caret's look, so the item
/// `Space` flips is the one marked; a row of one item needs no cursor.
pub fn check_row(
    label_text: &str,
    items: &[(String, bool)],
    at: Option<usize>,
    focused: bool,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let fill = field_style(focused, theme);
    let caret = theme.field_cursor.into_style();
    let marked = at.filter(|_| focused && items.len() > 1);
    let mut value = vec![Span::raw(" ".repeat(TEXT_INDENT))];
    for (index, (text, on)) in items.iter().enumerate() {
        if index > 0 {
            value.push(Span::styled(" ", fill));
        }
        let style = if marked == Some(index) { caret } else { fill };
        value.push(Span::styled(checkbox(text, *on), style));
    }
    let mut spans = vec![Span::raw(label(label_text, cols.label))];
    spans.extend(text::fit_spans(
        value,
        cols.inner.saturating_sub(cols.lead()),
    ));
    row(spans, focused, theme)
}

/// How a line that is only read sits among the fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    /// A note: the label and the text in the field columns, both
    /// receding (`muted`, italic).
    #[default]
    Note,
    /// Output derived from the fields above it (the command a form will
    /// run): led in two cells deeper by the [`marks::DERIVED`] corner,
    /// receding, its text in the field values' column.
    Derived,
    /// Derived output that needs attention (nothing to run): drawn as
    /// [`Derived`](Self::Derived) with its text in `warning`.
    Warning,
}

/// A line that is only read, in `tone`: never focused, so it carries no
/// cursor marker, and its text starts in the field values' column.
pub fn read_only_row(
    label_text: &str,
    value: &str,
    tone: Tone,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let recede = Style::default().add_modifier(theme.muted | Modifier::ITALIC);
    let width = cols
        .inner
        .saturating_sub(cols.lead())
        .saturating_sub(TEXT_INDENT)
        .max(1);
    let value_style = match tone {
        Tone::Warning => Style::default()
            .fg(theme.warning)
            .add_modifier(Modifier::ITALIC),
        Tone::Note | Tone::Derived => recede,
    };
    let lead = match tone {
        Tone::Note => {
            return row(
                vec![
                    Span::styled(label(label_text, cols.label), recede),
                    Span::raw(" ".repeat(TEXT_INDENT)),
                    Span::styled(text::truncate_end(value, width), value_style),
                ],
                false,
                theme,
            );
        }
        // Two cells deeper than a field's label, so the corner sits under
        // the label above it and the text still starts in the values'
        // column.
        Tone::Derived | Tone::Warning => format!(
            "{}{} {}",
            " ".repeat(selection::GUTTER_WIDTH),
            marks::DERIVED,
            label(label_text, cols.label.saturating_sub(2)),
        ),
    };
    Line::from(vec![
        Span::styled(lead, recede),
        Span::raw(" ".repeat(TEXT_INDENT)),
        Span::styled(text::truncate_end(value, width), value_style),
    ])
}

/// The footer `keys` generate, in rows at most `width` cells wide broken
/// only between hints, receding (`muted`).
pub fn footer_lines<A>(keys: &[KeyRow<A>], width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let muted = Style::default().add_modifier(theme.muted);
    Footer::new(keys)
        .lines(width, theme)
        .into_iter()
        .map(|line| line.patch_style(muted))
        .collect()
}

/// A form's error line: `reason` in `error`, cut with the ellipsis mark
/// to `width` cells.
pub fn error_line(reason: &str, width: usize, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        text::truncate_end(reason, width),
        Style::default().fg(theme.error),
    ))
}

/// A row's source column: the gap, then `word` right-aligned in `width`
/// cells, in `warning` while a change is `pending` and receding
/// (`muted`) otherwise. A row with no word to show gets a blank column,
/// so every row of the form ends in one place.
pub fn source_cells(word: &str, width: usize, pending: bool, theme: &Theme) -> Vec<Span<'static>> {
    let style = if pending {
        Style::default().fg(theme.warning)
    } else {
        Style::default().add_modifier(theme.muted)
    };
    vec![
        Span::raw(" ".repeat(SOURCE_GAP)),
        Span::styled(
            text::pad_start(&text::truncate_end(word, width), width),
            style,
        ),
    ]
}

/// A line that explains the form or its focused field, receding
/// (`muted`): indented by the cursor gutter, so it starts under the
/// labels, and made exactly `width` cells (cut with the ellipsis mark, or
/// padded), so the line keeps its look whatever it says.
pub fn detail_line(detail: &str, width: usize, theme: &Theme) -> Line<'static> {
    let room = width.saturating_sub(selection::GUTTER_WIDTH);
    Line::styled(
        format!(
            "{}{}",
            " ".repeat(selection::GUTTER_WIDTH),
            text::fit(detail, room)
        ),
        Style::default().add_modifier(theme.muted),
    )
}

/// A paragraph that explains the form, wrapped at word boundaries to
/// `width` cells and drawn as [`detail_line`]s.
pub fn paragraph_lines(paragraph: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let room = width.saturating_sub(selection::GUTTER_WIDTH).max(1);
    text::wrap(paragraph, room)
        .iter()
        .map(|line| detail_line(line.trim_end(), width, theme))
        .collect()
}

#[cfg(test)]
#[path = "row_tests.rs"]
mod tests;
