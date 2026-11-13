//! Counting the rows wrapped lines take, from the rows themselves.

use std::ops::Range;

use ratatui::text::Line;

use super::ScrollGutter;
use crate::text::{fit_spans, wrap_line};

/// Lines wrapped to a width, with the rows each line took.
///
/// A scrolled body needs its row count before it draws: for the window's
/// range, the scrollbar's length, a height budget, and the span a wrapped
/// cursor line takes. Estimating the count apart from the wrap lets the
/// two disagree, and the cursor then sits a row off the screen. These
/// rows are wrapped by [`text::wrap_line`](crate::text::wrap_line) and
/// drawn as they are (a paragraph without its own wrap), so the count is
/// the count drawn.
///
/// Every line takes at least one row; an empty line is one empty row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WrappedRows {
    rows: Vec<Line<'static>>,
    /// One past the last row of each line, in line order.
    ends: Vec<usize>,
}

impl WrappedRows {
    /// `lines` wrapped to `width` cells by the
    /// [wrapping rules](crate::text#wrapping-rules), with no hanging
    /// indent. A width of 0 is treated as 1.
    pub fn new(lines: &[Line<'_>], width: usize) -> Self {
        Self::hanging(lines, width, 0)
    }

    /// `lines` wrapped to `width` cells with a hanging indent: every row
    /// after a line's first starts `indent` cells in, under a gutter the
    /// line's first row draws (a line number).
    pub fn hanging(lines: &[Line<'_>], width: usize, indent: usize) -> Self {
        let mut rows = Vec::with_capacity(lines.len());
        let mut ends = Vec::with_capacity(lines.len());
        for line in lines {
            rows.extend(wrap_line(line, width.max(1), indent));
            ends.push(rows.len());
        }
        Self { rows, ends }
    }

    /// `lines` one row each, cut to `width` cells with the ellipsis mark
    /// where they are longer: for a reader that shows lines as they are
    /// instead of wrapping them.
    pub fn cut(lines: &[Line<'_>], width: usize) -> Self {
        let rows: Vec<Line<'static>> = lines
            .iter()
            .map(|line| {
                let spans = line
                    .spans
                    .iter()
                    .map(|span| ratatui::text::Span::styled(span.content.to_string(), span.style))
                    .collect();
                Line {
                    spans: fit_spans(spans, width),
                    style: line.style,
                    alignment: line.alignment,
                }
            })
            .collect();
        let ends = (1..=rows.len()).collect();
        Self { rows, ends }
    }

    /// `lines` wrapped to fit an area `width` cells wide and `height` rows
    /// tall: one cell narrower when they overflow and
    /// [`ScrollGutter::layout`](super::ScrollGutter::layout) gives that
    /// column to the scrollbar ([`ScrollGutter::carves`]), and otherwise
    /// at the full width, including an area too short for the bar. Lay
    /// the area out with the returned row count; its content area is then
    /// as wide as the rows.
    pub fn fit(lines: &[Line<'_>], width: usize, height: usize) -> Self {
        let rows = Self::new(lines, width);
        let cells = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        if ScrollGutter::carves(cells(width), cells(height), rows.len()) {
            Self::new(lines, width - 1)
        } else {
            rows
        }
    }

    /// How many rows the lines took.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no rows, which happens only for no lines.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// How many lines were wrapped.
    pub fn lines(&self) -> usize {
        self.ends.len()
    }

    /// The rows line `index` took. Past the last line, the empty range at
    /// the end.
    pub fn rows_of(&self, index: usize) -> Range<usize> {
        let Some(&end) = self.ends.get(index) else {
            return self.len()..self.len();
        };
        let start = index
            .checked_sub(1)
            .and_then(|previous| self.ends.get(previous))
            .copied()
            .unwrap_or(0);
        start..end
    }

    /// The line row `row` belongs to, or `None` past the last row.
    pub fn line_of(&self, row: usize) -> Option<usize> {
        let line = self.ends.partition_point(|&end| end <= row);
        (line < self.ends.len()).then_some(line)
    }

    /// The rows, top to bottom.
    pub fn rows(&self) -> &[Line<'static>] {
        &self.rows
    }

    /// The rows, for a paragraph that draws them without wrapping again.
    pub fn into_rows(self) -> Vec<Line<'static>> {
        self.rows
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
