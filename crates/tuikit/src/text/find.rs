//! Finding matches and painting them.

use std::borrow::Cow;
use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::measure::clusters;

/// The byte offset of the first occurrence of `needle` in `haystack`,
/// comparing ASCII letters without case; `None` when there is none or
/// `needle` is empty.
///
/// Other characters compare exactly. Folding only ASCII keeps the match
/// the same length in bytes as `needle`, so `offset..offset +
/// needle.len()` is the match's range and a highlight built from it
/// lines up. The offset is always a character boundary. Nothing is
/// allocated.
///
/// ```
/// use tuikit::text::find_ci;
///
/// assert_eq!(find_ci("Rebuild the Index", "index"), Some(12));
/// assert_eq!(find_ci("Rebuild", ""), None);
/// ```
pub fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let hay = haystack.as_bytes();
    let pat = needle.as_bytes();
    let first = *pat.first()?;
    if pat.len() > hay.len() {
        return None;
    }
    let first = first.to_ascii_lowercase();
    let last_start = hay.len() - pat.len();
    (0..=last_start).find(|&start| {
        hay[start].to_ascii_lowercase() == first
            && hay[start + 1..start + pat.len()]
                .iter()
                .zip(&pat[1..])
                .all(|(h, p)| h.eq_ignore_ascii_case(p))
    })
}

/// Every occurrence of `needle` in `haystack`, left to right and not
/// overlapping, as byte ranges; compared as [`find_ci`] compares.
pub fn find_all_ci<'a>(haystack: &'a str, needle: &'a str) -> FindAll<'a> {
    FindAll {
        haystack,
        needle,
        from: 0,
    }
}

/// The iterator [`find_all_ci`] returns.
#[derive(Clone, Debug)]
pub struct FindAll<'a> {
    haystack: &'a str,
    needle: &'a str,
    from: usize,
}

impl Iterator for FindAll<'_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        let found = self.from + find_ci(self.haystack.get(self.from..)?, self.needle)?;
        let end = found + self.needle.len();
        self.from = end;
        Some(found..end)
    }
}

/// `spans` with the text in `ranges` restyled: the one match painter.
///
/// `ranges` are byte ranges into the spans' text laid end to end, in any
/// order (an `Option` of one range works too); overlapping ranges merge. Each painted grapheme takes
/// `restyle(style)`, where `style` is the style of the span it is in, so
/// the caller chooses what a match looks like: `|base| base.patch(m)`
/// keeps the surrounding look and adds the match style `m` on top,
/// `|_| m` replaces it. A range edge inside a grapheme moves outward to
/// the grapheme's edge, so a highlight never separates a letter from its
/// accent. Spans are split only where a painted range starts or ends.
///
/// ```
/// use ratatui::style::{Modifier, Style};
/// use ratatui::text::Span;
/// use tuikit::text::{find_all_ci, paint};
///
/// let text = "rebuild the index and the cache";
/// let bold = Style::default().add_modifier(Modifier::BOLD);
/// let spans = paint(&[Span::raw(text)], find_all_ci(text, "the"), |base| base.patch(bold));
/// assert_eq!(spans.len(), 5);
/// assert_eq!(spans[1].content, "the");
/// assert_eq!(spans[1].style, bold);
/// ```
pub fn paint(
    spans: &[Span<'_>],
    ranges: impl IntoIterator<Item = Range<usize>>,
    restyle: impl Fn(Style) -> Style,
) -> Vec<Span<'static>> {
    let mut painted: Vec<Range<usize>> = ranges
        .into_iter()
        .filter(|range| range.start < range.end)
        .collect();
    painted.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(painted.len());
    for range in painted {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }

    let mut out = Vec::with_capacity(spans.len() + merged.len() * 2);
    let mut offset = 0;
    for span in spans {
        let content = span.content.as_ref();
        let len = content.len();
        // Grapheme starts in this span, so cuts land between graphemes.
        let edges: Vec<usize> = clusters(content).iter().map(|c| c.at).collect();
        let snap_down = |at: usize| edges[..edges.partition_point(|&e| e <= at)].last().copied();
        let mut pieces: Vec<(Range<usize>, bool)> = Vec::new();
        let mut cursor = 0;
        for range in &merged {
            if range.end <= offset || range.start >= offset + len {
                continue;
            }
            let start = snap_down(range.start.saturating_sub(offset)).unwrap_or(0);
            let end = range.end - offset;
            let end = if end >= len {
                len
            } else {
                // Move a cut inside a grapheme to the grapheme's end.
                edges
                    .get(edges.partition_point(|&e| e < end))
                    .copied()
                    .unwrap_or(len)
            };
            let start = start.max(cursor);
            if start >= end {
                continue;
            }
            if cursor < start {
                pieces.push((cursor..start, false));
            }
            pieces.push((start..end, true));
            cursor = end;
        }
        if pieces.is_empty() {
            out.push(Span::styled(Cow::Owned(content.to_string()), span.style));
        } else {
            if cursor < len {
                pieces.push((cursor..len, false));
            }
            for (piece, marked) in pieces {
                let style = if marked {
                    restyle(span.style)
                } else {
                    span.style
                };
                out.push(Span::styled(Cow::Owned(content[piece].to_string()), style));
            }
        }
        offset += len;
    }
    out
}

/// `line` with every occurrence of `query` after its first `skip` cells
/// (a gutter of line numbers or labels) patched with `look`, compared as
/// [`find_ci`] compares: the row painter readers share. A match a wrap
/// splits across two rows is on neither, so neither is painted.
///
/// ```
/// use ratatui::style::{Modifier, Style};
/// use ratatui::text::Line;
/// use tuikit::text::paint_matches;
///
/// let bold = Style::default().add_modifier(Modifier::BOLD);
/// let row = paint_matches(&Line::raw("12 │ the index, the cache"), 5, "the", bold);
/// let painted: Vec<&str> = row
///     .spans
///     .iter()
///     .filter(|span| span.style == bold)
///     .map(|span| span.content.as_ref())
///     .collect();
/// assert_eq!(painted, ["the", "the"]);
/// ```
pub fn paint_matches(line: &Line<'_>, skip: usize, query: &str, look: Style) -> Line<'static> {
    let joined: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    let from = super::clip(&joined, skip).len();
    let ranges =
        find_all_ci(&joined[from..], query).map(|range| from + range.start..from + range.end);
    Line {
        spans: paint(&line.spans, ranges, |base| base.patch(look)),
        style: line.style,
        alignment: line.alignment,
    }
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
