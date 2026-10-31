//! Measuring text in terminal cells, one grapheme at a time.

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// How many terminal cells `text` takes.
///
/// A CJK character or an emoji takes two cells, a combining mark none,
/// and an emoji written with a variation selector (`U+FE0F`) or a joiner
/// sequence takes the two cells of the one picture it draws. This is the
/// count ratatui uses when it lays a span out, so a width measured here
/// agrees with [`Span::width`] and [`ratatui::text::Line::width`].
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// How many terminal cells `spans` take, laid end to end.
pub fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| width(&span.content)).sum()
}

/// One grapheme of a text: where it starts, its characters, and its
/// cells.
///
/// A cut, a wrap, or a restyle that lands inside a grapheme separates a
/// letter from its accent or half an emoji from the other half, and the
/// terminal then draws the pieces apart (or drops the accent). Every
/// operation in the module therefore moves in whole graphemes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cluster<'a> {
    /// The byte offset of the grapheme in its text.
    pub at: usize,
    /// The grapheme's characters.
    pub text: &'a str,
    /// The cells it takes.
    pub width: usize,
}

impl Cluster<'_> {
    /// The byte offset just past the grapheme.
    pub fn end(&self) -> usize {
        self.at + self.text.len()
    }

    /// Whether the grapheme is a space a wrap may break at. The no-break
    /// spaces are spaces that say "do not break here", so they are not.
    pub fn is_blank(&self) -> bool {
        self.text
            .chars()
            .all(|c| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}'))
    }

    /// Whether the grapheme takes two cells: a CJK character, a
    /// full-width form, or an emoji.
    pub fn is_wide(&self) -> bool {
        self.width > 1
    }
}

/// The graphemes of `text`, in order, covering every byte.
///
/// The segmentation is ratatui's own (it draws one grapheme per cell, or
/// two for a wide one), so a cut made here falls where the terminal draws
/// a boundary. Ratatui skips control characters when it draws; they are
/// kept here, one character at a time, so a cut text is always a prefix
/// of its source and its byte offsets stay valid.
pub(crate) fn clusters(text: &str) -> Vec<Cluster<'_>> {
    let span = Span::raw(text);
    let mut out = Vec::with_capacity(text.len());
    let mut next = 0;
    for grapheme in span.styled_graphemes(Style::default()) {
        let symbol = grapheme.symbol;
        // Only control characters lie between two graphemes the
        // segmenter returns.
        while next < text.len() && !text[next..].starts_with(symbol) {
            let len = text[next..].chars().next().map_or(1, char::len_utf8);
            push(&mut out, text, next, next + len);
            next += len;
        }
        push(&mut out, text, next, next + symbol.len());
        next += symbol.len();
    }
    while next < text.len() {
        let len = text[next..].chars().next().map_or(1, char::len_utf8);
        push(&mut out, text, next, next + len);
        next += len;
    }
    out
}

fn push<'a>(out: &mut Vec<Cluster<'a>>, text: &'a str, at: usize, end: usize) {
    let end = end.min(text.len());
    let piece = &text[at..end];
    out.push(Cluster {
        at,
        text: piece,
        width: width(piece),
    });
}

/// The byte length of the longest run of whole graphemes at the start of
/// `text` that fits in `cells`, and the cells it takes.
pub(crate) fn prefix(text: &str, cells: usize) -> (usize, usize) {
    let mut used = 0;
    for cluster in clusters(text) {
        if used + cluster.width > cells {
            return (cluster.at, used);
        }
        used += cluster.width;
    }
    (text.len(), used)
}

/// The byte offset where the longest run of whole graphemes at the end of
/// `text` that fits in `cells` starts, and the cells it takes. The run
/// never starts before `floor`.
pub(crate) fn suffix(text: &str, cells: usize, floor: usize) -> (usize, usize) {
    let mut used = 0;
    let mut start = text.len();
    for cluster in clusters(text).iter().rev() {
        if cluster.at < floor || used + cluster.width > cells {
            break;
        }
        used += cluster.width;
        start = cluster.at;
    }
    (start, used)
}
