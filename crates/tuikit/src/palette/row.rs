//! What a palette lists: the row trait, the built-in item, and the
//! built-in word filter.

use std::borrow::Cow;

use ratatui::style::Style;
use ratatui::text::Span;

use crate::glyphs::marks;
use crate::selection::Cursor;
use crate::theme::Theme;

/// What a row is told when the palette lays it out: the cells it has,
/// whether the cursor is on it, the query, and the theme.
///
/// The palette draws the cursor gutter and the highlight itself
/// ([`selection::row`](crate::selection::row)), so a row only lays out
/// its own cells; a row of several cells lays a
/// [`ColumnSchema`](crate::columns::ColumnSchema) out for the width, with
/// the gutter ([`GUTTER_WIDTH`](crate::selection::GUTTER_WIDTH)) as its
/// lead. More may be told later, which is why the struct is built with
/// [`RowContext::new`] outside the crate.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct RowContext<'a> {
    /// The cells the row has after the cursor gutter, and after the
    /// scrollbar's column when the list overflows (or always, in a
    /// palette that reserves it). Cells past it are cut with `…`.
    pub width: usize,
    /// Whether the cursor is on the row, and whether the list has the
    /// keys. The highlight is the palette's; a row may still want to
    /// know, to show more on the cursor's row.
    pub cursor: Cursor,
    /// The query as typed, for a row that marks what matched.
    pub query: &'a str,
    /// The theme the palette is drawn with.
    pub theme: &'a Theme,
}

impl<'a> RowContext<'a> {
    /// What a row `width` cells wide is told.
    pub fn new(width: usize, cursor: Cursor, query: &'a str, theme: &'a Theme) -> Self {
        Self {
            width,
            cursor,
            query,
            theme,
        }
    }

    /// Whether the cursor is on the row.
    pub const fn selected(&self) -> bool {
        self.cursor.is_on()
    }
}

/// One row of a palette: how it draws its cells, and what the built-in
/// filter matches against.
///
/// A host's row type says how it looks at a width, so a palette can list
/// anything: a name and a note ([`Item`]), a row of columns, a command
/// and its key. The palette puts the cursor gutter before the cells,
/// cuts them to the row with `…`, and highlights the cursor's row, so
/// every palette's rows look and move alike.
pub trait PaletteRow {
    /// The row's cells laid out for `row.width` cells.
    fn cells(&self, row: &RowContext<'_>) -> Vec<Span<'static>>;

    /// The text [`every_word`] matches the query against. Empty unless a
    /// row says otherwise, so a row whose palette ranks it some other way
    /// (or whose results a provider sends) need not say.
    fn haystack(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
}

/// A row that is a label, an optional detail, and the host's value: what
/// a palette over known candidates lists.
///
/// It draws the label, then the separator mark and the detail receding
/// (the theme's `muted`); the detail is cut before the label is. The
/// built-in filter matches the label and the detail.
///
/// ```
/// use tuikit::palette::{Item, PaletteRow};
///
/// let item = Item::new("north", 7).detail("cold");
/// assert_eq!(item.value, 7);
/// assert_eq!(item.haystack(), "north cold");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Item<T = ()> {
    /// The row's name, after the cursor gutter.
    pub label: String,
    /// Context after the label, receding: where the thing lives, its
    /// state.
    pub detail: Option<String>,
    /// What picking the row hands the host.
    pub value: T,
}

impl<T> Item<T> {
    /// An item labeled `label` that stands for `value`, with no detail.
    pub fn new(label: impl Into<String>, value: T) -> Self {
        Self {
            label: label.into(),
            detail: None,
            value,
        }
    }

    /// Sets the context after the label.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl<T> PaletteRow for Item<T> {
    fn cells(&self, row: &RowContext<'_>) -> Vec<Span<'static>> {
        let mut spans = vec![Span::raw(self.label.clone())];
        if let Some(detail) = self.detail.as_deref().filter(|detail| !detail.is_empty()) {
            let muted = Style::default().add_modifier(row.theme.muted);
            spans.push(Span::styled(
                format!("  {} {detail}", marks::SEPARATOR),
                muted,
            ));
        }
        spans
    }

    fn haystack(&self) -> Cow<'_, str> {
        match self.detail.as_deref() {
            Some(detail) if !detail.is_empty() => Cow::Owned(format!("{} {detail}", self.label)),
            _ => Cow::Borrowed(&self.label),
        }
    }
}

/// The built-in filter: the rows whose [`haystack`](PaletteRow::haystack)
/// holds every word of the query, ignoring case, in the order the host
/// listed them. An empty query lists every row.
///
/// ```
/// use tuikit::palette::{Item, every_word};
///
/// let rows = [
///     Item::new("alpha", ()).detail("north"),
///     Item::new("beta", ()).detail("south"),
///     Item::new("alphabet", ()).detail("south"),
/// ];
/// assert_eq!(every_word("ALPH south", &rows), [2]);
/// assert_eq!(every_word("", &rows), [0, 1, 2]);
/// ```
pub fn every_word<R: PaletteRow>(query: &str, rows: &[R]) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    rows.iter()
        .enumerate()
        .filter(|(_, row)| {
            let haystack = row.haystack().to_lowercase();
            words.iter().all(|word| haystack.contains(word.as_str()))
        })
        .map(|(index, _)| index)
        .collect()
}
