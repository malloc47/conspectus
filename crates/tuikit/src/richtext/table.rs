//! Markdown tables: found in a body of Markdown and laid out to a width.
//!
//! The Markdown renderer does not parse GFM pipe tables, so a table in a
//! body would otherwise flow through as literal pipe text. This module
//! owns the pre-pass that splits a body into Markdown segments and table
//! segments before the renderer sees it, and lays each table out with
//! column-aware wrapping to fit.
//!
//! The trade-offs:
//!
//! - In-cell Markdown styling (bold, code spans, links) is dropped: cells
//!   render as plain wrapped text.
//! - When even the narrowest columns overflow the width, the layout's
//!   overflowing output is kept rather than turned into key and value
//!   records.
//!
//! The segmentation state machine is intentionally small: it recognises
//! the GFM pipe-table shape (`|...|` row + `|---|...|` separator + body
//! rows), exempts fenced code blocks (` ``` ` or `~~~`), and falls through
//! to ordinary Markdown for anything else.

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::chip::Tone;
use crate::glyphs::TABLE_BORDERS;
use crate::theme::Theme;

/// One piece of a body of Markdown. Either a slice of source Markdown the
/// caller runs through the Markdown renderer, or a parsed table the caller
/// lays out with [`render_table`].
#[derive(Debug, Clone)]
pub enum BodySegment<'a> {
    /// Markdown source, borrowed from the body.
    Markdown(&'a str),
    /// A table found in the body.
    Table(ParsedTable),
}

/// A GFM pipe-table parsed out of the source. Cells are stored as raw
/// strings and rendered plain. Column alignments are derived from the
/// `:---:` markers on the separator row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTable {
    /// The header row's cells.
    pub header: Vec<String>,
    /// One alignment per column.
    pub alignments: Vec<TableAlign>,
    /// The body rows, each as wide as the header.
    pub rows: Vec<Vec<String>>,
}

/// How a table column's cells are aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlign {
    /// `:---` or `---`.
    Left,
    /// `:---:`.
    Center,
    /// `---:`.
    Right,
}

impl From<TableAlign> for CellAlignment {
    fn from(value: TableAlign) -> Self {
        match value {
            TableAlign::Left => CellAlignment::Left,
            TableAlign::Center => CellAlignment::Center,
            TableAlign::Right => CellAlignment::Right,
        }
    }
}

/// Split `body` into ordered segments, alternating Markdown text and
/// parsed tables. Fenced code blocks are preserved verbatim inside
/// Markdown segments: a `|` row inside a ```` ``` ```` is not a table.
///
/// Empty Markdown segments are dropped. The concatenation of every
/// segment's source text equals the input (modulo per-segment parsing of
/// tables, which captures the same lines but in structured form).
///
/// ```
/// use tuikit::richtext::{BodySegment, segment_body};
///
/// let body = "Before.\n\n| a | b |\n|---|--:|\n| 1 | 2 |\n\nAfter.";
/// let segments = segment_body(body);
/// assert_eq!(segments.len(), 3);
/// assert!(matches!(segments[1], BodySegment::Table(ref table) if table.rows == [["1", "2"]]));
/// ```
pub fn segment_body(body: &str) -> Vec<BodySegment<'_>> {
    let mut out: Vec<BodySegment<'_>> = Vec::new();
    let line_offsets = line_offsets(body);
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;
    let mut md_start: Option<usize> = Some(0);
    let mut in_code_fence = false;
    let mut fence_marker: Option<&str> = None;

    while i < lines.len() {
        let line = lines[i];

        // Fenced-code-block state machine. The marker can be ``` or
        // ~~~; the closing fence must match the opening kind.
        if let Some(marker) = fence_marker {
            if line.trim_start().starts_with(marker)
                && line
                    .trim_start()
                    .trim_start_matches(marker)
                    .trim_end()
                    .is_empty()
            {
                fence_marker = None;
                in_code_fence = false;
            }
            i += 1;
            continue;
        }
        if !in_code_fence {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") {
                fence_marker = Some("```");
                in_code_fence = true;
                i += 1;
                continue;
            }
            if trimmed.starts_with("~~~") {
                fence_marker = Some("~~~");
                in_code_fence = true;
                i += 1;
                continue;
            }
        }

        // Try to match a GFM pipe table starting at line i.
        if let Some((table, consumed)) = try_parse_pipe_table(&lines, i) {
            // Flush the pending Markdown segment up to the byte
            // start of this line.
            let table_byte_start = line_offsets[i];
            if let Some(start) = md_start
                && start < table_byte_start
            {
                push_markdown(&mut out, &body[start..table_byte_start]);
            }
            out.push(BodySegment::Table(table));
            i += consumed;
            md_start = Some(if i < line_offsets.len() {
                line_offsets[i]
            } else {
                body.len()
            });
            continue;
        }

        i += 1;
    }

    // Flush any trailing Markdown.
    if let Some(start) = md_start
        && start < body.len()
    {
        push_markdown(&mut out, &body[start..]);
    }
    out
}

fn push_markdown<'a>(out: &mut Vec<BodySegment<'a>>, src: &'a str) {
    if src.is_empty() {
        return;
    }
    // Two Markdown segments in a row are not joined: the two slices are
    // not guaranteed to be adjacent, and the renderer reads back-to-back
    // paragraphs the same way it reads one.
    out.push(BodySegment::Markdown(src));
}

/// Try to match a GFM pipe-table block starting at `lines[start]`.
/// Returns `(parsed, lines_consumed)` on match. Requires a header
/// row, a separator row, and at least zero body rows (header-only
/// tables are valid GFM).
fn try_parse_pipe_table(lines: &[&str], start: usize) -> Option<(ParsedTable, usize)> {
    if start + 1 >= lines.len() {
        return None;
    }
    let header_raw = lines[start];
    let sep_raw = lines[start + 1];
    let header = split_pipe_row(header_raw)?;
    let alignments = parse_separator_row(sep_raw, header.len())?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut j = start + 2;
    while j < lines.len() {
        let Some(cells) = split_pipe_row(lines[j]) else {
            break;
        };
        // Tolerate column-count drift: pad short rows, truncate
        // long ones, to the header width. Matches the forgiving
        // behaviour of a GFM parser.
        let mut row = cells;
        if row.len() < header.len() {
            row.resize(header.len(), String::new());
        } else if row.len() > header.len() {
            row.truncate(header.len());
        }
        rows.push(row);
        j += 1;
    }
    Some((
        ParsedTable {
            header,
            alignments,
            rows,
        },
        j - start,
    ))
}

/// `| a | b | c |` → `["a", "b", "c"]`. Returns `None` for lines
/// that don't have the pipe-row shape: no leading `|` *or* fewer
/// than 2 cells once the leading/trailing pipes are stripped.
fn split_pipe_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    // Strip exactly one leading + one optional trailing pipe so
    // empty leading cells (`||...`) collapse to one empty cell, not
    // two.
    let mut body = &trimmed[1..];
    if let Some(stripped) = body.strip_suffix('|') {
        body = stripped;
    }
    let cells: Vec<String> = body.split('|').map(|c| c.trim().to_string()).collect();
    if cells.len() < 2 {
        return None;
    }
    Some(cells)
}

/// Parse `|:---|---:|:---:|` etc. Returns one [`TableAlign`] per
/// column iff every cell matches the separator shape and the count
/// equals `expected_cols`. Rejects rows with no `-` chars (those
/// are body rows, not separators).
fn parse_separator_row(line: &str, expected_cols: usize) -> Option<Vec<TableAlign>> {
    let cells = split_pipe_row(line)?;
    if cells.len() != expected_cols {
        return None;
    }
    let mut aligns = Vec::with_capacity(cells.len());
    for cell in &cells {
        let trimmed = cell.trim();
        if !trimmed.contains('-') {
            return None;
        }
        if !trimmed.chars().all(|c| matches!(c, '-' | ':' | ' ' | '\t')) {
            return None;
        }
        let starts = trimmed.starts_with(':');
        let ends = trimmed.ends_with(':');
        aligns.push(match (starts, ends) {
            (true, true) => TableAlign::Center,
            (false, true) => TableAlign::Right,
            (true | false, false) => TableAlign::Left,
        });
    }
    Some(aligns)
}

/// Byte offsets where each line in `s` starts, plus a final entry
/// at `s.len()` so callers can compute `s[lines[i]..lines[i+1]]`
/// without bounds-checking the tail.
fn line_offsets(s: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Lay a [`ParsedTable`] out as lines `content_width` cells wide, with
/// the columns sized to their content and their cells wrapped to fit.
/// The borders take the theme's inert tone, so the table reads as
/// structure without competing with the text around it.
///
/// ```
/// use tuikit::richtext::{BodySegment, render_table, segment_body};
/// use tuikit::theme::Theme;
///
/// let segments = segment_body("| name | note |\n|---|---|\n| a | a longer note than fits |");
/// let BodySegment::Table(table) = &segments[0] else { unreachable!() };
/// let lines = render_table(table, 24, &Theme::default());
/// assert!(lines.iter().all(|line| line.width() <= 24));
/// ```
pub fn render_table(parsed: &ParsedTable, content_width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut table = Table::new();
    // A header rule and dotted column separators, but no outer box: an
    // outer box competes with whatever frames the text beside it.
    table.load_preset(presets::UTF8_NO_BORDERS);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    // The width is the whole table's, outer borders and per-column gutters
    // included; the allocator does the border accounting.
    table.set_width(content_width);

    // Header.
    if !parsed.header.is_empty() {
        let header_cells: Vec<Cell> = parsed
            .header
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mut cell = Cell::new(h);
                if let Some(align) = parsed.alignments.get(i) {
                    cell = cell.set_alignment((*align).into());
                }
                cell
            })
            .collect();
        table.set_header(header_cells);
    }

    // Body.
    for row in &parsed.rows {
        let cells: Vec<Cell> = row
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut cell = Cell::new(c);
                if let Some(align) = parsed.alignments.get(i) {
                    cell = cell.set_alignment((*align).into());
                }
                cell
            })
            .collect();
        table.add_row(cells);
    }

    let rendered = table.to_string();
    let border_style = Tone::Inert.style(theme);
    let body_style = Style::new();

    rendered
        .split('\n')
        .map(|raw| style_table_line(raw, border_style, body_style))
        .collect()
}

/// Split a laid-out table line into border spans and body spans so the
/// border characters recede independently of the cell content. The
/// detection is character-class based: the borders come from a fixed set
/// of box-drawing glyphs and the cell content is plain text, so the split
/// is unambiguous.
fn style_table_line(raw: &str, border_style: Style, body_style: Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_is_border: Option<bool> = None;
    let flush = |buf: &mut String, was_border: Option<bool>, spans: &mut Vec<Span<'static>>| {
        if buf.is_empty() {
            return;
        }
        let style = match was_border {
            Some(true) => border_style,
            _ => body_style,
        };
        spans.push(Span::styled(std::mem::take(buf), style));
    };
    for c in raw.chars() {
        let is_border = is_box_drawing(c);
        match buf_is_border {
            None => buf_is_border = Some(is_border),
            Some(prev) if prev != is_border => {
                flush(&mut buf, buf_is_border, &mut spans);
                buf_is_border = Some(is_border);
            }
            _ => {}
        }
        buf.push(c);
    }
    flush(&mut buf, buf_is_border, &mut spans);
    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }
    Line::from(spans)
}

/// `true` when `c` is one of the box-drawing glyphs a table's borders are
/// made of. Plain spaces are not borders, so the whitespace between cells
/// stays in the body span.
fn is_box_drawing(c: char) -> bool {
    TABLE_BORDERS.contains(&c)
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
