//! Captured terminal output fitted to an area narrower than the screen it
//! was captured from.
//!
//! A capture is laid out for the terminal's width, not the area's.
//! [`layout_capture`] turns the parsed capture into display rows that each
//! fit the area, per [`WrapMode`], so the caller can keep exactly as many
//! of the newest rows as the area has room for.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use crate::text;

/// How a capture's lines are fitted to an area narrower than the screen
/// they were captured from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapMode {
    /// Wrap every line at the area's width, rules and borders included.
    Plain,
    /// Wrap content but truncate formatting (rules, box borders, and
    /// padding), so decorations do not spill onto extra rows; a rule with
    /// a label past the edge shortens around the label. The default.
    #[default]
    Smart,
    /// Keep the source's own layout: lines re-wrapped at the width the
    /// source laid them out at, then each row clipped to the area.
    None,
}

impl WrapMode {
    /// Every mode, the default first.
    pub const ALL: [WrapMode; 3] = [Self::Smart, Self::Plain, Self::None];
}

/// One terminal cell's worth of styled text. Zero-width characters
/// (combining marks) ride along with the cell before them.
#[derive(Clone, Debug, PartialEq)]
struct Cell {
    text: String,
    width: usize,
    style: Style,
}

impl Cell {
    fn space(style: Style) -> Self {
        Self {
            text: " ".to_string(),
            width: 1,
            style,
        }
    }

    fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }

    /// Rules, box borders, and block shading: characters that draw
    /// structure rather than say anything.
    fn is_decoration(&self) -> bool {
        self.text.chars().next().is_some_and(|c| {
            matches!(c,
                '\u{2500}'..='\u{259F}' // box drawing + block elements
                | '\u{2010}'..='\u{2015}' // hyphens and dashes
                | '\u{23AF}' // horizontal line extension
                | '-' | '=' | '_' | '~')
        })
    }
}

/// Lay out a parsed capture as display rows no wider than `width`.
/// Trailing blank lines (an idle screen's empty bottom rows) are dropped
/// first so the latest output sits at the bottom. `source_width` is the
/// width the source laid the capture out at, used by [`WrapMode::None`]
/// to reproduce its own wrapping; without it, that mode clips each line.
///
/// ```
/// use ratatui::text::Line;
/// use tuikit::richtext::{WrapMode, layout_capture};
///
/// let capture = [Line::raw("serving on :8080"), Line::raw("─".repeat(60)), Line::raw("")];
/// let rows = layout_capture(&capture, WrapMode::Smart, None, 20);
/// // The trailing blank row is gone, and the rule is cut at the edge
/// // instead of wrapping onto a row of its own.
/// assert_eq!(rows.len(), 2);
/// assert_eq!(rows[1].width(), 20);
/// ```
pub fn layout_capture(
    lines: &[Line<'_>],
    mode: WrapMode,
    source_width: Option<u16>,
    width: u16,
) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut cells: Vec<Vec<Cell>> = lines.iter().map(line_cells).collect();
    while cells
        .last()
        .is_some_and(|line| line.iter().all(Cell::is_blank))
    {
        cells.pop();
    }
    let mut rows = Vec::new();
    for line in cells {
        match mode {
            WrapMode::Plain => rows.extend(text::wrap_line(&cells_line(line), width, 0)),
            WrapMode::Smart => rows.extend(smart_rows(line, width)),
            WrapMode::None => {
                let line = cells_line(line);
                let source_rows = match source_width.map(usize::from) {
                    Some(source_width) if source_width > 0 => {
                        text::wrap_line_chars(&line, source_width)
                    }
                    _ => vec![line],
                };
                rows.extend(source_rows.iter().map(|row| clip_line(row, width)));
            }
        }
    }
    rows
}

/// `line` cut to `width` cells with no mark, as a terminal clips a row:
/// the first row of a character wrap.
fn clip_line(line: &Line<'_>, width: usize) -> Line<'static> {
    text::wrap_line_chars(line, width)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The fewest cells of one rule character that make a rule. Shorter
/// runs are content (`--flag`, `===`, `->`) and are never shortened.
const MIN_RULE_RUN: usize = 4;

/// Smart wrap for one line: drop trailing padding, truncate when only
/// decoration would spill past the edge, squeeze interior padding or
/// else shorten rules when that makes the line fit, and otherwise
/// word-wrap with a hanging indent.
fn smart_rows(mut line: Vec<Cell>, width: usize) -> Vec<Line<'static>> {
    while line.last().is_some_and(Cell::is_blank) {
        line.pop();
    }
    if line_width(&line) <= width {
        return vec![cells_line(line)];
    }
    let excess = line_width(&line) - width;
    let (kept, overflow) = split_at_width(&line, width);
    if overflow
        .iter()
        .all(|cell| cell.is_blank() || cell.is_decoration())
    {
        let mut kept = kept.to_vec();
        while kept.last().is_some_and(Cell::is_blank) {
            kept.pop();
        }
        return vec![cells_line(kept)];
    }
    // A label in the overflow (a rule ending in the source's name, a
    // banner centered between two rules) is content, so the line is not
    // cut; its padding goes first, then its rules shorten around it.
    let lead = line.iter().take_while(|cell| cell.is_blank()).count();
    let padding = runs_of(&line, lead, 2, |cell| {
        (cell.width == 1 && cell.is_blank()).then_some(())
    });
    let rules = runs_of(&line, 0, MIN_RULE_RUN, |cell| {
        (cell.width == 1 && cell.is_decoration()).then_some(cell.text.as_str())
    });
    if let Some(shortened) =
        shorten_runs(&line, &padding, excess).or_else(|| shorten_runs(&line, &rules, excess))
    {
        return vec![cells_line(shortened)];
    }
    let indent = line
        .iter()
        .take_while(|cell| cell.is_blank())
        .map(|cell| cell.width)
        .sum::<usize>();
    let hanging = if indent * 2 < width { indent } else { 0 };
    text::wrap_line(&cells_line(line), width, hanging)
}

/// The maximal runs, as `(start, len)`, of at least `min` cells from
/// `from` on whose `class` is the same `Some`. A cell of class `None`
/// belongs to no run.
fn runs_of<'a, K: PartialEq>(
    line: &'a [Cell],
    from: usize,
    min: usize,
    class: impl Fn(&'a Cell) -> Option<K>,
) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut index = from;
    while index < line.len() {
        let Some(key) = class(&line[index]) else {
            index += 1;
            continue;
        };
        let start = index;
        while index < line.len() && class(&line[index]).as_ref() == Some(&key) {
            index += 1;
        }
        if index - start >= min {
            runs.push((start, index - start));
        }
    }
    runs
}

/// `line` with its `runs` shortened by `excess` cells in all, widest
/// first, each to no fewer than one cell. A shortened run keeps evenly
/// spaced cells, so a run drawn in two styles (a progress bar) keeps
/// their proportions. `None` when the runs can't give that much, so the
/// caller wraps the line untouched instead of mangling real content.
fn shorten_runs(line: &[Cell], runs: &[(usize, usize)], excess: usize) -> Option<Vec<Cell>> {
    let spare: usize = runs.iter().map(|(_, len)| len - 1).sum();
    if spare < excess {
        return None;
    }
    let mut keep: Vec<usize> = runs.iter().map(|(_, len)| *len).collect();
    for _ in 0..excess {
        let widest = (0..keep.len()).max_by_key(|&i| keep[i])?;
        keep[widest] -= 1;
    }
    let mut out = Vec::with_capacity(line.len());
    let mut cursor = 0;
    for (&(start, len), kept) in runs.iter().zip(keep) {
        out.extend_from_slice(&line[cursor..start]);
        out.extend((0..kept).map(|i| line[start + i * len / kept].clone()));
        cursor = start + len;
    }
    out.extend_from_slice(&line[cursor..]);
    Some(out)
}

/// The longest prefix that fits in `width` cells, and the rest.
fn split_at_width(line: &[Cell], width: usize) -> (&[Cell], &[Cell]) {
    let mut used = 0;
    for (index, cell) in line.iter().enumerate() {
        if used + cell.width > width {
            return line.split_at(index);
        }
        used += cell.width;
    }
    (line, &[])
}

fn line_width(line: &[Cell]) -> usize {
    line.iter().map(|cell| cell.width).sum()
}

fn line_cells(line: &Line<'_>) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    for span in &line.spans {
        let style = line.style.patch(span.style);
        for c in span.content.chars() {
            if c == '\t' {
                // Tab stops every 8 columns, as a terminal renders them.
                let column = line_width(&cells);
                let spaces = 8 - column % 8;
                cells.extend(std::iter::repeat_n(Cell::space(style), spaces));
                continue;
            }
            let width = c.width().unwrap_or(0);
            match cells.last_mut() {
                Some(previous) if width == 0 => previous.text.push(c),
                _ if width == 0 => {}
                _ => cells.push(Cell {
                    text: c.to_string(),
                    width,
                    style,
                }),
            }
        }
    }
    cells
}

fn cells_line(cells: Vec<Cell>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut style = None;
    for cell in cells {
        if style.is_some_and(|current| current != cell.style) {
            spans.push(Span::styled(
                std::mem::take(&mut text),
                style.unwrap_or_default(),
            ));
        }
        style = Some(cell.style);
        text.push_str(&cell.text);
    }
    if let Some(style) = style {
        spans.push(Span::styled(text, style));
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
