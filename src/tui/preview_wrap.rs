//! Fit a captured mux pane into the preview pane (ADR 0106).
//!
//! A pane capture is laid out for tmux's width, not the preview's.
//! [`layout_capture`] turns the parsed capture into display rows that
//! each fit the preview width, per [`PreviewWrap`] mode, so the caller
//! can bottom-anchor exactly as many rows as the pane has room for.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use tuikit::text;
use unicode_width::UnicodeWidthChar;

use super::PreviewWrap;

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

/// Lay out a parsed pane capture as display rows no wider than
/// `width`. Trailing blank lines (an idle pane's empty bottom rows)
/// are dropped first so the latest output sits at the bottom.
/// `pane_width` is the width tmux laid the capture out at, used by
/// [`PreviewWrap::None`] to reproduce tmux's own wrapping.
pub fn layout_capture(
    lines: &[Line<'_>],
    mode: PreviewWrap,
    pane_width: Option<u16>,
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
            PreviewWrap::Plain => rows.extend(text::wrap_line(&cells_line(line), width, 0)),
            PreviewWrap::Smart => rows.extend(smart_rows(line, width)),
            PreviewWrap::None => {
                let line = cells_line(line);
                let tmux_rows = match pane_width.map(usize::from) {
                    Some(pane_width) if pane_width > 0 => text::wrap_line_chars(&line, pane_width),
                    _ => vec![line],
                };
                rows.extend(tmux_rows.iter().map(|row| clip_line(row, width)));
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

/// Smart wrap for one line: drop trailing padding, truncate when only
/// decoration would spill past the edge, squeeze interior padding when
/// that makes the line fit, and otherwise word-wrap with a hanging
/// indent.
fn smart_rows(mut line: Vec<Cell>, width: usize) -> Vec<Line<'static>> {
    while line.last().is_some_and(Cell::is_blank) {
        line.pop();
    }
    if line_width(&line) <= width {
        return vec![cells_line(line)];
    }
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
    if let Some(squeezed) = squeeze_padding(&line, width) {
        return vec![cells_line(squeezed)];
    }
    let indent = line
        .iter()
        .take_while(|cell| cell.is_blank())
        .map(|cell| cell.width)
        .sum::<usize>();
    let hanging = if indent * 2 < width { indent } else { 0 };
    text::wrap_line(&cells_line(line), width, hanging)
}

/// Shrink interior runs of two or more spaces (never the leading
/// indent), widest first, until the line fits `width`. `None` when
/// even single spaces everywhere wouldn't fit, so the caller wraps the
/// line untouched instead of mangling real content.
fn squeeze_padding(line: &[Cell], width: usize) -> Option<Vec<Cell>> {
    let mut overflow = line_width(line).checked_sub(width)?;
    let indent = line.iter().take_while(|cell| cell.is_blank()).count();
    // Interior blank runs as (start, len), longer than one cell.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut index = indent;
    while index < line.len() {
        if line[index].is_blank() && line[index].width == 1 {
            let start = index;
            while index < line.len() && line[index].is_blank() && line[index].width == 1 {
                index += 1;
            }
            if index - start > 1 {
                runs.push((start, index - start));
            }
        } else {
            index += 1;
        }
    }
    let spare: usize = runs.iter().map(|(_, len)| len - 1).sum();
    if spare < overflow {
        return None;
    }
    let mut keep: Vec<usize> = runs.iter().map(|(_, len)| *len).collect();
    while overflow > 0 {
        let widest = (0..keep.len()).max_by_key(|&i| keep[i])?;
        keep[widest] -= 1;
        overflow -= 1;
    }
    let mut out = Vec::with_capacity(line.len());
    let mut cursor = 0;
    for ((start, len), kept) in runs.iter().zip(keep) {
        out.extend_from_slice(&line[cursor..*start]);
        out.extend_from_slice(&line[*start..*start + kept]);
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
                // Tab stops every 8 columns, as tmux renders them.
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
#[path = "preview_wrap_tests.rs"]
mod tests;
