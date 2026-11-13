//! What a column shows on one row: typed cells, each with one way of
//! being cut to the cells its column gives it.

use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::Span;

use crate::age::{self, Recency};
use crate::text;
use crate::theme::Theme;

/// How a text cell is cut when its column gives it fewer cells than its
/// text needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cut {
    /// Keep the start and end with the ellipsis mark: labels, titles.
    #[default]
    End,
    /// Keep the start and the end around the mark: paths, identifiers.
    Middle,
    /// Keep the start with no mark: a prefix that names, such as the
    /// first cells of an identifier the column always shortens.
    Clip,
}

/// What a cell holds.
#[derive(Clone, Debug, PartialEq)]
enum Content {
    /// Nothing: the cell is blank.
    Empty,
    /// One run of text in one style.
    Text { text: String, style: Style },
    /// Spans the host styled (a chip, a mark with its word).
    Spans(Vec<Span<'static>>),
    /// A list of chips: as many whole chips as fit, then a count of the
    /// rest.
    Chips {
        chips: Vec<Span<'static>>,
        rest: Style,
    },
    /// An age: its label, right-aligned in the cell and tinted by its
    /// recency.
    Age {
        label: Option<String>,
        recency: Option<Recency>,
    },
    /// An excerpt around a match, cut so the match stays in view.
    Snippet {
        text: String,
        highlight: Range<usize>,
        style: Style,
        matched: Style,
        drop: &'static [char],
    },
}

/// What one column shows on one row.
///
/// A cell knows its natural width (what it takes when nothing is short)
/// and how to cut itself to fewer cells; where it sits, how wide its
/// column is, and how it is padded are the schema's. Each form cuts one
/// way:
///
/// | Form | Built with | Cut to fewer cells |
/// | --- | --- | --- |
/// | text | [`text`](Self::text), [`secondary`](Self::secondary) | by its [`Cut`]: at the end with `…` (the default), in the middle, or clipped |
/// | styled spans, a chip, a mark | [`spans`](Self::spans), [`chip`](Self::chip) | at the end with `…`, in the style of the span it stands for |
/// | a chip list | [`chips`](Self::chips) | whole chips only, then `+N` for the ones left out |
/// | an age | [`age`](Self::age) | the framework's age cell, right-aligned in the column |
/// | a snippet | [`snippet`](Self::snippet) | around its match, which stays in view |
///
/// A text cell may carry a [`tail`](Self::tail): spans kept whole after
/// the text, which is cut first to leave room for them (a path and the
/// note after it).
///
/// ```
/// use ratatui::style::Style;
/// use tuikit::columns::Cell;
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let path = Cell::text("/srv/data/archive", Style::default()).middle();
/// assert_eq!(path.width(), 17);
/// assert_eq!(Cell::plain(&path.render(10, &theme)), "/srv/…hive");
/// assert!(Cell::empty().is_empty());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    content: Content,
    cut: Cut,
    tail: Vec<Span<'static>>,
}

impl Default for Cell {
    fn default() -> Self {
        Self::empty()
    }
}

impl Cell {
    const fn of(content: Content) -> Self {
        Self {
            content,
            cut: Cut::End,
            tail: Vec::new(),
        }
    }

    /// A blank cell.
    pub const fn empty() -> Self {
        Self::of(Content::Empty)
    }

    /// `text` in `style`, cut at the end with `…` when it is too wide
    /// (see [`middle`](Self::middle) and [`clip`](Self::clip)).
    pub fn text(text: impl Into<String>, style: Style) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::empty();
        }
        Self::of(Content::Text { text, style })
    }

    /// Secondary text (a path, an identifier, a note that matters less
    /// than its row's label): the theme's `muted` look, cut in the middle.
    pub fn secondary(text: impl Into<String>, theme: &Theme) -> Self {
        Self::text(text, Style::default().add_modifier(theme.muted)).middle()
    }

    /// Spans the host styled, as one cell: a mark and its word, a label in
    /// two styles. Cut at the end with `…`.
    pub fn spans(spans: Vec<Span<'static>>) -> Self {
        if text::spans_width(&spans) == 0 {
            return Self::empty();
        }
        Self::of(Content::Spans(spans))
    }

    /// A filled chip (see [`crate::chip`]), drawn as the host built it.
    pub fn chip(chip: Span<'static>) -> Self {
        Self::spans(vec![chip])
    }

    /// A list of chips, one cell apart: as many whole chips as the column
    /// holds, then ` +N` in `rest` for the ones left out. A chip is never
    /// cut in half while the first one fits.
    pub fn chips(chips: Vec<Span<'static>>, rest: Style) -> Self {
        if chips.is_empty() {
            return Self::empty();
        }
        Self::of(Content::Chips { chips, rest })
    }

    /// An age: `label` (from [`crate::age::Age::label`]) right-aligned in
    /// the column and tinted by `recency`, or the no-value mark when the
    /// age is unknown ([`crate::age::cell`]).
    pub const fn age(label: Option<String>, recency: Option<Recency>) -> Self {
        Self::of(Content::Age { label, recency })
    }

    /// An excerpt of `text` around the match at byte range `highlight`,
    /// cut so the match stays in view ([`crate::text::fit_snippet`]),
    /// drawn in `style` with the match in `matched`.
    pub fn snippet(
        text: impl Into<String>,
        highlight: Range<usize>,
        style: Style,
        matched: Style,
    ) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::empty();
        }
        Self::of(Content::Snippet {
            text,
            highlight,
            style,
            matched,
            drop: &[],
        })
    }

    /// Cuts the text in the middle, keeping its start and its end.
    #[must_use]
    pub const fn middle(mut self) -> Self {
        self.cut = Cut::Middle;
        self
    }

    /// Cuts the text with no mark, keeping its start.
    #[must_use]
    pub const fn clip(mut self) -> Self {
        self.cut = Cut::Clip;
        self
    }

    /// Keeps `spans` whole after the cell's text, which is cut first to
    /// leave room for them.
    #[must_use]
    pub fn tail(mut self, spans: Vec<Span<'static>>) -> Self {
        self.tail = spans;
        self
    }

    /// For a snippet: the characters to leave out of the excerpt (see
    /// [`crate::text::fit_snippet`]). Other cells ignore it.
    #[must_use]
    pub fn dropping(mut self, chars: &'static [char]) -> Self {
        if let Content::Snippet { drop, .. } = &mut self.content {
            *drop = chars;
        }
        self
    }

    /// Whether the cell is blank.
    pub fn is_empty(&self) -> bool {
        matches!(self.content, Content::Empty) && self.tail.is_empty()
    }

    /// The cells the cell takes when nothing is short.
    pub fn width(&self) -> usize {
        let body = match &self.content {
            Content::Empty => 0,
            Content::Text { text, .. } | Content::Snippet { text, .. } => text::width(text),
            Content::Spans(spans) => text::spans_width(spans),
            Content::Chips { chips, .. } => chips_width(chips, chips.len()),
            Content::Age { label, .. } => label.as_deref().map_or(1, text::width),
        };
        body + text::spans_width(&self.tail)
    }

    /// The cell cut to at most `cells` cells, unpadded. Its column pads it
    /// to the column's width.
    pub fn render(&self, cells: usize, theme: &Theme) -> Vec<Span<'static>> {
        let tail = text::spans_width(&self.tail);
        if tail > 0 {
            if tail > cells {
                let mut all = self.body(self.width() - tail, theme);
                all.extend(self.tail.iter().cloned());
                return text::fit_spans(all, cells);
            }
            let mut body = self.body(cells - tail, theme);
            body.extend(self.tail.iter().cloned());
            return body;
        }
        self.body(cells, theme)
    }

    /// The cell's own content cut to `cells`, its tail left out.
    fn body(&self, cells: usize, theme: &Theme) -> Vec<Span<'static>> {
        match &self.content {
            Content::Empty => Vec::new(),
            Content::Text { text, style } => {
                let cut = match self.cut {
                    Cut::End => text::truncate_end(text, cells),
                    Cut::Middle => text::truncate_middle(text, cells),
                    Cut::Clip => text::clip(text, cells),
                };
                if cut.is_empty() {
                    Vec::new()
                } else {
                    vec![Span::styled(cut, *style)]
                }
            }
            Content::Spans(spans) => text::fit_spans(spans.clone(), cells),
            Content::Chips { chips, rest } => chips_in(chips, *rest, cells),
            Content::Age { label, recency } => {
                vec![age::cell(label.as_deref(), *recency, cells, theme)]
            }
            Content::Snippet {
                text,
                highlight,
                style,
                matched,
                drop,
            } => {
                let fitted = text::fit_snippet(text, highlight.clone(), cells, drop);
                text::paint(
                    &[Span::styled(fitted.text, *style)],
                    fitted.highlight,
                    |_| *matched,
                )
                .into_iter()
                .filter(|span| !span.content.is_empty())
                .collect()
            }
        }
    }

    /// The style a pad beside the cell takes: the text's own, so a padded
    /// text cell is one run of one style; nothing for other forms, whose
    /// fill (a chip's) must not run on into the pad.
    pub(super) fn pad_style(&self) -> Style {
        match (&self.content, self.tail.is_empty()) {
            (Content::Text { style, .. }, true) => *style,
            _ => Style::default(),
        }
    }

    /// The text of `spans`, for a test or a reader that wants the words.
    pub fn plain(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }
}

/// The cells the first `shown` of `chips` take, one cell apart, with the
/// count of the rest.
fn chips_width(chips: &[Span<'_>], shown: usize) -> usize {
    let shown = shown.min(chips.len());
    let body = chips[..shown]
        .iter()
        .map(|chip| text::width(&chip.content))
        .sum::<usize>()
        + shown.saturating_sub(1);
    body + rest_width(chips.len() - shown)
}

/// The cells ` +N` takes for `hidden` chips left out; none for none.
fn rest_width(hidden: usize) -> usize {
    if hidden == 0 {
        0
    } else {
        text::width(&rest_text(hidden))
    }
}

fn rest_text(hidden: usize) -> String {
    format!(" +{hidden}")
}

/// As many whole chips as fit in `cells`, then the count of the rest. When
/// not even the first chip fits, it is cut like any span.
fn chips_in(chips: &[Span<'static>], rest: Style, cells: usize) -> Vec<Span<'static>> {
    let Some(shown) = (1..=chips.len())
        .rev()
        .find(|&shown| chips_width(chips, shown) <= cells)
    else {
        return text::fit_spans(vec![chips[0].clone()], cells);
    };
    let mut spans = Vec::with_capacity(shown * 2 + 1);
    for (index, chip) in chips[..shown].iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(chip.clone());
    }
    let hidden = chips.len() - shown;
    if hidden > 0 {
        spans.push(Span::styled(rest_text(hidden), rest));
    }
    spans
}
