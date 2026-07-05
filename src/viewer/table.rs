//! Markdown-table extraction + rendering for message bodies
//! (`H-VIEWER-NATIVE-017`, ADR 0054).
//!
//! `tui-markdown` 0.3 doesn't enable pulldown-cmark's
//! `ENABLE_TABLES`, so a GFM pipe-table in a message body would
//! otherwise flow through as literal pipe-text. This module owns
//! the pre-pass that splits a body into Markdown segments and
//! table segments before tui-markdown sees it, then renders each
//! table via [`comfy_table`] with column-aware wrap-to-fit.
//!
//! Per ADR 0054 the v1 trade-offs:
//!
//! - In-cell Markdown styling (bold, code spans, links) is dropped
//!   — cells render as plain wrapped text. comfy-table's
//!   `custom_styling` upgrade path is documented for when this
//!   becomes a real complaint.
//! - When even comfy-table's minimum-width allocation overflows the
//!   area, we let comfy-table emit its overflowing output rather
//!   than swap in a key/value transpose. Codex's
//!   `table_key_value.rs` is the documented follow-on for that
//!   tail.
//!
//! The segmentation state machine is intentionally small: it
//! recognises the GFM pipe-table shape (`|...|` row + `|---|...|`
//! separator + body rows), exempts fenced code blocks (` ``` ` or
//! `~~~`), and falls through to ordinary Markdown for anything
//! else.

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table, presets};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::viewer::theme::Theme;

/// One piece of a message body. Either a slice of source Markdown
/// the caller should run through `tui-markdown`, or a parsed
/// table the caller should render via [`render_table`].
#[derive(Debug, Clone)]
pub enum BodySegment<'a> {
    Markdown(&'a str),
    Table(ParsedTable),
}

/// A GFM pipe-table parsed out of the source. Cells are stored as
/// raw strings; v1 renders them plain. Column alignments are
/// derived from the `:---:` markers on the separator row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTable {
    pub header: Vec<String>,
    pub alignments: Vec<TableAlign>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
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

/// Split `body` into ordered segments, alternating Markdown text
/// and parsed tables. Fenced code blocks are preserved verbatim
/// inside Markdown segments — a `|` row inside a ```` ``` ```` is
/// not a table.
///
/// Adjacent Markdown segments are coalesced; empty Markdown
/// segments are dropped. The concatenation of every segment's
/// source text equals the input (modulo per-segment parsing of
/// tables, which captures the same lines but in structured form).
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
    // Coalesce with the prior segment if it's also Markdown. Should
    // not happen in normal flow but defends against off-by-one.
    if let Some(BodySegment::Markdown(prior)) = out.last() {
        // Build a contiguous slice when possible; in practice the
        // two slices are not guaranteed to be adjacent in memory.
        // The fallback is to push a new segment — tui-markdown
        // handles back-to-back paragraphs the same way it handles a
        // single one.
        let _ = prior;
    }
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
        // long ones, to the header width. Matches pulldown-cmark's
        // forgiving behaviour.
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
            (true, false) | (false, false) => TableAlign::Left,
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

/// Render a [`ParsedTable`] into Ratatui [`Line`]s using comfy-table's
/// dynamic-width column allocator. Borders are dimmed with the
/// theme's `secondary_text` colour so the table reads as
/// structurally distinct without competing with role chips.
pub fn render_table(parsed: &ParsedTable, content_width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut table = Table::new();
    // UTF8_NO_BORDERS gives a header rule (`═`) and dotted vertical
    // separators (`┆`) between columns but no outer box, which fits
    // the viewer's existing gutter-only aesthetic. UTF8_BORDERS_ONLY
    // (full outer box) was tried first; the box competed visually
    // with the gutter rule on the left.
    table.load_preset(presets::UTF8_NO_BORDERS);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    // comfy-table interprets `set_width` as the total table width,
    // *including* outer borders + per-column gutters. We pass the
    // available content_width directly; the allocator handles the
    // border accounting.
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
    let border_style = Style::new()
        .fg(theme.secondary_text)
        .add_modifier(Modifier::DIM);
    let body_style = Style::new();

    rendered
        .split('\n')
        .map(|raw| style_table_line(raw, border_style, body_style))
        .collect()
}

/// Split a comfy-table-rendered line into a border span + body
/// span(s) so border chars dim independently of cell content. The
/// detection is character-class based — UTF8_BORDERS_ONLY uses a
/// fixed alphabet of box-drawing glyphs and the cell content (in
/// v1) is plain text, so the split is unambiguous.
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

/// `true` when `c` is one of the box-drawing glyphs used by the
/// preset above. Covers the single-line set (`│ ─ ┌ ┐ └ ┘ ├ ┤ ┬
/// ┴ ┼`), the double-line header-rule set (`═ ╞ ╡ ╪`), and the
/// dotted variant (`┆ ╌`) emitted by `UTF8_NO_BORDERS`. Plain
/// ASCII spaces are NOT borders so inter-cell whitespace stays in
/// the body span.
fn is_box_drawing(c: char) -> bool {
    matches!(
        c,
        '│' | '─'
            | '┌'
            | '┐'
            | '└'
            | '┘'
            | '├'
            | '┤'
            | '┬'
            | '┴'
            | '┼'
            | '═'
            | '╞'
            | '╡'
            | '╪'
            | '╤'
            | '╧'
            | '╠'
            | '╣'
            | '╬'
            | '║'
            | '┆'
            | '╌'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_body_with_no_table_returns_one_markdown_segment() {
        let body = "just prose\n\nwith two paragraphs";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 1);
        match &segs[0] {
            BodySegment::Markdown(s) => assert_eq!(*s, body),
            BodySegment::Table(_) => panic!("expected markdown segment"),
        }
    }

    #[test]
    fn segment_body_splits_prose_around_a_table() {
        let body = "intro line\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\noutro";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 3, "expected md + table + md, got {segs:?}");
        match &segs[0] {
            BodySegment::Markdown(s) => assert!(s.starts_with("intro line"), "got {s:?}"),
            BodySegment::Table(_) => panic!(),
        }
        match &segs[1] {
            BodySegment::Table(t) => {
                assert_eq!(t.header, vec!["a", "b"]);
                assert_eq!(t.rows, vec![vec!["1".to_string(), "2".to_string()]]);
            }
            BodySegment::Markdown(_) => panic!(),
        }
        match &segs[2] {
            BodySegment::Markdown(s) => assert!(s.trim_start().starts_with("outro"), "got {s:?}"),
            BodySegment::Table(_) => panic!(),
        }
    }

    #[test]
    fn segment_body_ignores_pipes_inside_fenced_code_block() {
        let body = "```\n| not | a | table |\n|---|---|---|\n| x | y | z |\n```\n";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 1, "fenced pipes stay as markdown: {segs:?}");
        assert!(matches!(segs[0], BodySegment::Markdown(_)));
    }

    #[test]
    fn segment_body_handles_table_at_end_of_body() {
        let body = "intro\n\n| a | b |\n|---|---|\n| 1 | 2 |";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 2);
        assert!(matches!(segs[1], BodySegment::Table(_)));
    }

    #[test]
    fn segment_body_handles_table_at_start_of_body() {
        let body = "| a | b |\n|---|---|\n| 1 | 2 |\n\ntail";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 2, "got {segs:?}");
        assert!(matches!(segs[0], BodySegment::Table(_)));
    }

    #[test]
    fn segment_body_rejects_separator_row_without_dashes() {
        // A row of all colons isn't a separator; it's body content.
        let body = "| a | b |\n|:::|:::|\n| 1 | 2 |";
        let segs = segment_body(body);
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0], BodySegment::Markdown(_)));
    }

    #[test]
    fn split_pipe_row_strips_outer_pipes_and_trims_cells() {
        let cells = split_pipe_row("|  a  |  b  |").unwrap();
        assert_eq!(cells, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn split_pipe_row_handles_missing_trailing_pipe() {
        let cells = split_pipe_row("| a | b").unwrap();
        assert_eq!(cells, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parse_separator_row_extracts_alignment_markers() {
        // |:--|---|---:|:---:|
        let aligns = parse_separator_row("|:--|---|---:|:---:|", 4).unwrap();
        assert_eq!(
            aligns,
            vec![
                TableAlign::Left,
                TableAlign::Left,
                TableAlign::Right,
                TableAlign::Center
            ]
        );
    }

    #[test]
    fn parse_separator_row_rejects_count_mismatch() {
        assert!(parse_separator_row("|---|---|", 3).is_none());
    }

    #[test]
    fn try_parse_pipe_table_tolerates_column_count_drift() {
        let lines = vec![
            "| a | b | c |",
            "|---|---|---|",
            "| 1 | 2 |",         // short row — should pad
            "| x | y | z | w |", // long row — should truncate
        ];
        let (parsed, consumed) = try_parse_pipe_table(&lines, 0).expect("parse");
        assert_eq!(consumed, 4);
        assert_eq!(parsed.header, vec!["a", "b", "c"]);
        assert_eq!(
            parsed.rows[0],
            vec!["1".to_string(), "2".to_string(), String::new()]
        );
        assert_eq!(
            parsed.rows[1],
            vec!["x".to_string(), "y".to_string(), "z".to_string()]
        );
    }

    #[test]
    fn render_table_produces_borders_at_target_width() {
        let parsed = ParsedTable {
            header: vec!["col1".into(), "col2".into()],
            alignments: vec![TableAlign::Left, TableAlign::Left],
            rows: vec![
                vec!["alpha".into(), "beta".into()],
                vec!["gamma".into(), "delta".into()],
            ],
        };
        let theme = Theme::default();
        let lines = render_table(&parsed, 40, &theme);
        let text: Vec<String> = lines.iter().map(line_to_string).collect();
        assert!(
            text.iter()
                .any(|l| l.contains("col1") && l.contains("col2")),
            "header row present: {text:?}"
        );
        assert!(
            text.iter()
                .any(|l| l.contains("alpha") && l.contains("beta")),
            "body row present: {text:?}"
        );
        assert!(
            text.iter().any(|l| l.contains('┆')),
            "column separators present (UTF8_NO_BORDERS uses ┆): {text:?}"
        );
    }

    #[test]
    fn render_table_wraps_wide_cells_to_fit_target_width() {
        let parsed = ParsedTable {
            header: vec!["short".into(), "long content column".into()],
            alignments: vec![TableAlign::Left, TableAlign::Left],
            rows: vec![vec![
                "x".into(),
                "this is a very long sentence that needs wrapping across multiple lines".into(),
            ]],
        };
        let theme = Theme::default();
        let lines = render_table(&parsed, 30, &theme);
        for line in &lines {
            let width: usize = line
                .spans
                .iter()
                .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                .sum();
            assert!(
                width <= 30,
                "every line stays within target width 30; got {width} from {:?}",
                line_to_string(line)
            );
        }
    }

    fn line_to_string(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }
}
