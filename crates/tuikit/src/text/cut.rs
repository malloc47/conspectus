//! Cutting and padding text to a cell budget.

use std::borrow::Cow;

use ratatui::text::Span;

use super::measure::{clusters, prefix, spans_width, suffix, width};
use crate::glyphs::marks;

/// `text` cut to at most `width` cells, ending in the ellipsis mark when
/// anything visible was cut.
///
/// The mark takes the last cell, so the result never exceeds `width`: a
/// width of 1 leaves only the mark and a width of 0 leaves nothing. A
/// two-cell character that would straddle the budget is dropped whole,
/// so the result can be a cell short. When only spaces lie past the
/// budget they are dropped without a mark, since no text was lost.
///
/// ```
/// use tuikit::text::truncate_end;
///
/// assert_eq!(truncate_end("conformance", 6), "confo…");
/// assert_eq!(truncate_end("short", 6), "short");
/// assert_eq!(truncate_end("東京都", 4), "東…");
/// ```
pub fn truncate_end(text: &str, width: usize) -> String {
    if self::width(text) <= width {
        return text.to_string();
    }
    let (end, _) = prefix(text, width);
    if text[end..].chars().all(char::is_whitespace) {
        return text[..end].to_string();
    }
    if width == 0 {
        return String::new();
    }
    let (end, _) = prefix(text, width - 1);
    format!("{}{}", &text[..end], marks::ELLIPSIS)
}

/// `text` cut to at most `width` cells by replacing its middle with the
/// ellipsis mark, so both ends stay readable.
///
/// This is the cut for paths and identifiers, whose start and end both
/// carry meaning (`/srv/data/…/report.txt`). The start gets the larger
/// half of an odd budget; a cell the start cannot use (a two-cell
/// character at its edge) goes to the end instead.
///
/// ```
/// use tuikit::text::truncate_middle;
///
/// assert_eq!(truncate_middle("/srv/archive/2026/report.txt", 15), "/srv/ar…ort.txt");
/// assert_eq!(truncate_middle("abc", 1), "…");
/// ```
pub fn truncate_middle(text: &str, width: usize) -> String {
    if self::width(text) <= width {
        return text.to_string();
    }
    match width {
        0 => return String::new(),
        1 => return marks::ELLIPSIS.to_string(),
        _ => {}
    }
    let usable = width - 1;
    let (head, head_cells) = prefix(text, usable.div_ceil(2));
    let (tail, _) = suffix(text, usable - head_cells, head);
    format!("{}{}{}", &text[..head], marks::ELLIPSIS, &text[tail..])
}

/// `text` cut to at most `width` cells with no mark: a hard cut for a
/// fixed-width column whose content is a prefix by nature, such as a
/// short identifier.
pub fn clip(text: &str, width: usize) -> String {
    let (end, _) = prefix(text, width);
    text[..end].to_string()
}

/// The end of `text` that fits in `width` cells, with no mark: the hard
/// cut for the distinctive tail of an identifier (`…a41f` keeps `a41f`).
pub fn clip_start(text: &str, width: usize) -> String {
    let (start, _) = suffix(text, width, 0);
    text[start..].to_string()
}

/// `text` followed by spaces up to `width` cells. Text already as wide or
/// wider is returned as it is; pair it with [`truncate_end`] (or use
/// [`fit`]) when the column must not grow.
pub fn pad(text: &str, width: usize) -> String {
    let gap = width.saturating_sub(self::width(text));
    format!("{text}{}", " ".repeat(gap))
}

/// Spaces up to `width` cells followed by `text`: right alignment.
pub fn pad_start(text: &str, width: usize) -> String {
    let gap = width.saturating_sub(self::width(text));
    format!("{}{text}", " ".repeat(gap))
}

/// `text` made exactly `width` cells: cut with [`truncate_end`] when it
/// is wider, then padded with spaces.
pub fn fit(text: &str, width: usize) -> String {
    pad(&truncate_end(text, width), width)
}

/// `spans` cut to at most `width` cells, with the ellipsis mark in the
/// last cell when anything visible was cut.
///
/// Spans that fit are kept as they are. The span the budget runs out in
/// is cut between graphemes. The mark takes the style of the first span
/// it stands in for, so a cut label ends in the label's style. As with
/// [`truncate_end`], spaces alone past the budget are dropped without a
/// mark.
pub fn fit_spans<'a>(spans: Vec<Span<'a>>, width: usize) -> Vec<Span<'a>> {
    if spans_width(&spans) <= width {
        return spans;
    }
    let marked = width > 0 && !only_spaces_after(&spans, width);
    let room = if marked { width - 1 } else { width };
    let mut out = Vec::with_capacity(spans.len());
    let mut used = 0;
    let mut mark_style = None;
    for span in spans {
        let cells = self::width(&span.content);
        if used + cells <= room {
            used += cells;
            out.push(span);
            continue;
        }
        // The budget runs out inside this span.
        let (end, _) = prefix(&span.content, room - used);
        if end > 0 {
            let kept = span.content[..end].to_string();
            out.push(Span::styled(Cow::Owned(kept), span.style));
        }
        mark_style = Some(span.style);
        break;
    }
    if marked {
        out.push(Span::styled(
            marks::ELLIPSIS,
            mark_style.unwrap_or_default(),
        ));
    }
    out
}

/// Whether everything in `spans` past the first `width` cells is spaces.
fn only_spaces_after(spans: &[Span<'_>], width: usize) -> bool {
    let mut used = 0;
    for span in spans {
        for cluster in clusters(&span.content) {
            used += cluster.width;
            if used > width && !cluster.text.chars().all(char::is_whitespace) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
#[path = "cut_tests.rs"]
mod tests;
