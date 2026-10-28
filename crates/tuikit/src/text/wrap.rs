//! Wrapping text into rows of a cell width, by the wrapping rules in the
//! module documentation of [`crate::text`]. One row breaker serves the
//! plain and the styled wrappers, so the two cannot disagree.

use std::borrow::Cow;

use ratatui::text::{Line, Span};

use super::measure::{Cluster, clusters, width};
use crate::glyphs::marks;

/// One row of a wrap: the graphemes `start..end` of the source, after
/// `indent` spaces. `wide` marks a row that holds a single grapheme too
/// wide to draw, which the ellipsis mark replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    start: usize,
    end: usize,
    indent: usize,
    wide: bool,
}

/// Where word wrap breaks a run of graphemes into rows `width` cells
/// wide, continuation rows indented by `hanging`.
fn word_rows(cells: &[(usize, bool)], width: usize, hanging: usize) -> Vec<Row> {
    let width = width.max(1);
    let hanging = if hanging < width { hanging } else { 0 };
    let mut rows = Vec::new();
    let mut start = 0;
    let mut indent = 0;
    loop {
        let room = width - indent;
        let rest: usize = cells[start..].iter().map(|(cells, _)| cells).sum();
        if rest <= room {
            rows.push(Row {
                start,
                end: cells.len(),
                indent,
                wide: false,
            });
            return rows;
        }
        let mut fit = start;
        let mut used = 0;
        while fit < cells.len() && used + cells[fit].0 <= room {
            used += cells[fit].0;
            fit += 1;
        }
        if fit == start {
            rows.push(Row {
                start,
                end: start + 1,
                indent,
                wide: true,
            });
            start += 1;
        } else {
            // A head that ends where a space begins ends a word; otherwise
            // break after the last space that follows some text, or inside
            // the word when there is none.
            let end = if fit == cells.len() || cells[fit].1 {
                fit
            } else {
                (start..fit)
                    .rev()
                    .find(|&i| cells[i].1 && cells[start..i].iter().any(|cell| !cell.1))
                    .map_or(fit, |space| space + 1)
            };
            rows.push(Row {
                start,
                end,
                indent,
                wide: false,
            });
            start = end;
        }
        while start < cells.len() && cells[start].1 {
            start += 1;
        }
        if start >= cells.len() {
            return rows;
        }
        indent = hanging;
    }
}

/// Where character wrap breaks a run of graphemes: rows of up to `width`
/// cells, filled left to right as a terminal wraps.
fn char_rows(cells: &[(usize, bool)], width: usize) -> Vec<Row> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut start = 0;
    while start < cells.len() {
        let mut end = start;
        let mut used = 0;
        while end < cells.len() && used + cells[end].0 <= width {
            used += cells[end].0;
            end += 1;
        }
        let wide = end == start;
        rows.push(Row {
            start,
            end: if wide { start + 1 } else { end },
            indent: 0,
            wide,
        });
        start = if wide { start + 1 } else { end };
    }
    if rows.is_empty() {
        rows.push(Row {
            start: 0,
            end: 0,
            indent: 0,
            wide: false,
        });
    }
    rows
}

fn shape(clusters: &[Cluster<'_>]) -> Vec<(usize, bool)> {
    clusters
        .iter()
        .map(|cluster| (cluster.width, cluster.is_blank()))
        .collect()
}

fn plain_rows(line: &str, rows: &[Row], clusters: &[Cluster<'_>]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let mut out = " ".repeat(row.indent);
            if row.wide {
                out.push_str(marks::ELLIPSIS);
            } else if row.start < row.end {
                out.push_str(&line[clusters[row.start].at..clusters[row.end - 1].end()]);
            }
            out
        })
        .collect()
}

/// `text` word-wrapped into rows at most `width` cells wide, by the
/// [wrapping rules](crate::text#wrapping-rules). A line break in `text`
/// ends a row; an empty text is one empty row.
///
/// ```
/// use tuikit::text::wrap;
///
/// assert_eq!(wrap("the quick brown fox", 10), ["the quick ", "brown fox"]);
/// assert_eq!(wrap("alpha beta gamma", 10), ["alpha beta", "gamma"]);
/// ```
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let clusters = clusters(line);
        let rows = word_rows(&shape(&clusters), width, 0);
        out.extend(plain_rows(line, &rows, &clusters));
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// `text` cut into rows of at most `width` cells regardless of words, the
/// way a terminal wraps a long line. A line break in `text` ends a row.
pub fn wrap_chars(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let clusters = clusters(line);
        let rows = char_rows(&shape(&clusters), width);
        out.extend(plain_rows(line, &rows, &clusters));
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// `text` wrapped into rows at most `width` cells wide, breaking only
/// where `separator` stands; the separator at a break is dropped.
///
/// This is the wrap for a list of short items joined by a separator,
/// such as key hints (`o open · q quit`), whose items must not
/// be split. An item wider than a row is word-wrapped on its own rows.
///
/// ```
/// use tuikit::text::wrap_at;
///
/// let hints = "o open · m mark · q quit";
/// assert_eq!(wrap_at(hints, " · ", 16), ["o open · m mark", "q quit"]);
/// ```
pub fn wrap_at(text: &str, separator: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let gap = self::width(separator);
    let mut out = Vec::new();
    let mut current = String::new();
    for item in text.split(separator) {
        let item_cells = self::width(item);
        if !current.is_empty() && self::width(&current) + gap + item_cells <= width {
            current.push_str(separator);
            current.push_str(item);
            continue;
        }
        if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if item_cells <= width {
            current.push_str(item);
        } else {
            let mut rows = wrap(item, width);
            current = rows.pop().unwrap_or_default();
            out.extend(rows);
        }
    }
    if !current.is_empty() || out.is_empty() {
        out.push(current);
    }
    out
}

/// One grapheme of a styled line, with the style it is drawn in.
struct Styled<'a> {
    cluster: Cluster<'a>,
    span: usize,
}

fn styled_clusters<'a>(line: &'a Line<'_>) -> Vec<Styled<'a>> {
    line.spans
        .iter()
        .enumerate()
        .flat_map(|(span, piece)| {
            clusters(&piece.content)
                .into_iter()
                .map(move |cluster| Styled { cluster, span })
        })
        .collect()
}

fn styled_rows(line: &Line<'_>, cells: &[Styled<'_>], rows: &[Row]) -> Vec<Line<'static>> {
    rows.iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            if row.indent > 0 {
                spans.push(Span::raw(" ".repeat(row.indent)));
            }
            if row.wide {
                let style = line.spans[cells[row.start].span].style;
                spans.push(Span::styled(marks::ELLIPSIS, style));
            } else {
                let mut index = row.start;
                while index < row.end {
                    let span = cells[index].span;
                    let from = cells[index].cluster.at;
                    while index + 1 < row.end && cells[index + 1].span == span {
                        index += 1;
                    }
                    let to = cells[index].cluster.end();
                    let piece = &line.spans[span];
                    spans.push(Span::styled(
                        Cow::Owned(piece.content[from..to].to_string()),
                        piece.style,
                    ));
                    index += 1;
                }
            }
            Line {
                spans,
                style: line.style,
                alignment: line.alignment,
            }
        })
        .collect()
}

/// `line` word-wrapped into rows at most `width` cells wide, keeping each
/// span's style, with continuation rows indented by `hanging` unstyled
/// spaces. The rows follow the [wrapping rules](crate::text#wrapping-rules);
/// a word may run across spans of different styles and is still one
/// word. Each row keeps the line's own style and alignment.
///
/// A `hanging` indent that leaves no room on a row is ignored.
pub fn wrap_line(line: &Line<'_>, width: usize, hanging: usize) -> Vec<Line<'static>> {
    let cells = styled_clusters(line);
    let shape: Vec<(usize, bool)> = cells
        .iter()
        .map(|cell| (cell.cluster.width, cell.cluster.is_blank()))
        .collect();
    styled_rows(line, &cells, &word_rows(&shape, width, hanging))
}

/// `line` cut into rows of at most `width` cells regardless of words,
/// keeping each span's style: the styled form of [`wrap_chars`].
pub fn wrap_line_chars(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    let cells = styled_clusters(line);
    let shape: Vec<(usize, bool)> = cells
        .iter()
        .map(|cell| (cell.cluster.width, cell.cluster.is_blank()))
        .collect();
    styled_rows(line, &cells, &char_rows(&shape, width))
}

#[cfg(test)]
#[path = "wrap_tests.rs"]
mod tests;
