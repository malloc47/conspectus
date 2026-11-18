//! Captured terminal output fitted to an area narrower than the screen it
//! was captured from.
//!
//! A capture is laid out for the terminal's width, not the area's.
//! [`layout_capture`] turns the parsed capture into display rows that each
//! fit the area, per [`WrapMode`], so the caller can keep exactly as many
//! of the newest rows as the area has room for. [`layout_capture_with`]
//! takes the smart wrap's settings ([`SmartWrap`]) and a [`LineFit`] for
//! each line, for a host that knows its sources' quirks.

use std::ops::RangeInclusive;

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

/// What [`WrapMode::Smart`] counts as decoration, and how it treats rules.
/// [`Default`] is the framework's choice; a host may let its user tune it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmartWrap {
    /// Characters that draw structure rather than say anything (rules,
    /// box borders, block shading), as inclusive ranges. A line whose
    /// overflow is only these and blanks is cut at the edge.
    pub decoration: Vec<RangeInclusive<char>>,
    /// The fewest cells of one decoration character that make a rule.
    /// Shorter runs are content (`--flag`, `===`, `->`) and keep their
    /// length.
    pub min_rule_run: usize,
    /// Whether a line whose overflow carries a label shortens its rules
    /// to fit on one row, rather than wrapping.
    pub shorten_rules: bool,
}

impl Default for SmartWrap {
    fn default() -> Self {
        Self {
            decoration: vec![
                '\u{2500}'..='\u{259F}', // box drawing + block elements
                '\u{2010}'..='\u{2015}', // hyphens and dashes
                '\u{23AF}'..='\u{23AF}', // horizontal line extension
                '-'..='-',
                '='..='=',
                '_'..='_',
                '~'..='~',
            ],
            min_rule_run: 4,
            shorten_rules: true,
        }
    }
}

impl SmartWrap {
    /// Whether `c` is one of the [`decoration`](Self::decoration)
    /// characters.
    pub fn is_decoration(&self, c: char) -> bool {
        self.decoration.iter().any(|range| range.contains(&c))
    }

    fn is_decoration_cell(&self, cell: &Cell) -> bool {
        cell.text
            .chars()
            .next()
            .is_some_and(|c| self.is_decoration(c))
    }
}

/// How one line of a capture fits the area under [`WrapMode::Smart`]. The
/// host picks it per line, so a source's known quirk (a status line it
/// right-aligns, a hint not worth a row) fits as the host means it to.
/// The other modes lay every line out their own way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineFit {
    /// The smart wrap's own fit, by its [`SmartWrap`] settings.
    #[default]
    Smart,
    /// Cut at the edge, as a terminal clips a row.
    Cut,
    /// Word-wrap the line as it is, as [`WrapMode::Plain`] does.
    Wrap,
    /// Shrink the leading padding so the line ends at the edge, on one
    /// row; when its text alone is wider, the text without the padding
    /// fits the smart way.
    RightAlign,
    /// Leave the line out.
    Drop,
}

impl LineFit {
    /// Every fit, the default first.
    pub const ALL: [LineFit; 5] = [
        Self::Smart,
        Self::Cut,
        Self::Wrap,
        Self::RightAlign,
        Self::Drop,
    ];
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
}

/// Lay out a parsed capture as display rows no wider than `width`.
/// Trailing blank lines (an idle screen's empty bottom rows) are dropped
/// first so the latest output sits at the bottom. `source_width` is the
/// width the source laid the capture out at, used by [`WrapMode::None`]
/// to reproduce its own wrapping; without it, that mode clips each line.
/// Smart wrap uses the [`SmartWrap`] defaults and fits every line its own
/// way; [`layout_capture_with`] sets both.
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
    layout_capture_with(
        lines,
        mode,
        source_width,
        width,
        &SmartWrap::default(),
        |_| LineFit::Smart,
    )
}

/// [`layout_capture`] with the smart wrap's settings and a fit for each
/// line. Under [`WrapMode::Smart`], `fit` is asked once per line of the
/// capture, before trailing blank lines are dropped, so a dropped line
/// leaves no blank rows behind it at the bottom; the other modes never
/// ask it and ignore `smart`.
///
/// ```
/// use ratatui::text::Line;
/// use tuikit::richtext::{LineFit, SmartWrap, WrapMode, layout_capture_with};
///
/// let hint = format!("{}press ? for help", " ".repeat(50));
/// let capture = [Line::raw("output"), Line::raw(hint)];
/// let rows = layout_capture_with(&capture, WrapMode::Smart, None, 30, &SmartWrap::default(), |line| {
///     if line.to_string().ends_with("for help") {
///         LineFit::RightAlign
///     } else {
///         LineFit::Smart
///     }
/// });
/// // The hint keeps its place at the right edge, on one row.
/// assert_eq!(rows.len(), 2);
/// assert_eq!(rows[1].to_string(), format!("{}press ? for help", " ".repeat(14)));
/// ```
pub fn layout_capture_with(
    lines: &[Line<'_>],
    mode: WrapMode,
    source_width: Option<u16>,
    width: u16,
    smart: &SmartWrap,
    fit: impl Fn(&Line<'_>) -> LineFit,
) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut cells: Vec<(LineFit, Vec<Cell>)> = lines
        .iter()
        .map(|line| {
            let fit = if mode == WrapMode::Smart {
                fit(line)
            } else {
                LineFit::Smart
            };
            (fit, line_cells(line))
        })
        .filter(|(fit, _)| *fit != LineFit::Drop)
        .collect();
    while cells
        .last()
        .is_some_and(|(_, line)| line.iter().all(Cell::is_blank))
    {
        cells.pop();
    }
    let mut rows = Vec::new();
    for (fit, line) in cells {
        match mode {
            WrapMode::Plain => rows.extend(text::wrap_line(&cells_line(line), width, 0)),
            WrapMode::Smart => rows.extend(fitted_rows(line, fit, width, smart)),
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

/// One line under smart wrap, fitted as `fit` says.
fn fitted_rows(
    mut line: Vec<Cell>,
    fit: LineFit,
    width: usize,
    smart: &SmartWrap,
) -> Vec<Line<'static>> {
    match fit {
        LineFit::Smart => smart_rows(line, width, smart),
        LineFit::Wrap => text::wrap_line(&cells_line(line), width, 0),
        LineFit::Cut => {
            let mut kept = split_at_width(&line, width).0.to_vec();
            while kept.last().is_some_and(Cell::is_blank) {
                kept.pop();
            }
            vec![cells_line(kept)]
        }
        LineFit::RightAlign => {
            while line.last().is_some_and(Cell::is_blank) {
                line.pop();
            }
            // Leading padding goes from the left until the line fits, or
            // all of it when the text alone is wider.
            let mut excess = line_width(&line).saturating_sub(width);
            let lead = line.iter().take_while(|cell| cell.is_blank()).count();
            let mut cut = 0;
            while excess > 0 && cut < lead {
                excess = excess.saturating_sub(line[cut].width);
                cut += 1;
            }
            let line = line.split_off(if excess > 0 { lead } else { cut });
            if excess > 0 {
                smart_rows(line, width, smart)
            } else {
                vec![cells_line(line)]
            }
        }
        LineFit::Drop => Vec::new(),
    }
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
/// decoration would spill past the edge, squeeze interior padding or
/// else shorten rules when that makes the line fit, and otherwise
/// word-wrap with a hanging indent.
fn smart_rows(mut line: Vec<Cell>, width: usize, smart: &SmartWrap) -> Vec<Line<'static>> {
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
        .all(|cell| cell.is_blank() || smart.is_decoration_cell(cell))
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
    let rules = if smart.shorten_rules {
        runs_of(&line, 0, smart.min_rule_run, |cell| {
            (cell.width == 1 && smart.is_decoration_cell(cell)).then_some(cell.text.as_str())
        })
    } else {
        Vec::new()
    };
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
