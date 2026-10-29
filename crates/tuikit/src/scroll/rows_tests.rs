use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::*;
use crate::scroll::ScrollGutter;

fn texts(rows: &WrappedRows) -> Vec<String> {
    rows.rows().iter().map(ToString::to_string).collect()
}

#[test]
fn each_line_takes_the_rows_it_wraps_to() {
    let lines = [
        Line::from("one two three four"),
        Line::from(""),
        Line::from("five"),
    ];
    let rows = WrappedRows::new(&lines, 8);
    assert_eq!(texts(&rows), ["one two ", "three ", "four", "", "five"]);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows.lines(), 3);
    assert_eq!(rows.rows_of(0), 0..3);
    assert_eq!(rows.rows_of(1), 3..4);
    assert_eq!(rows.rows_of(2), 4..5);
    assert_eq!(rows.rows_of(3), 5..5);
}

#[test]
fn no_lines_are_no_rows() {
    let rows = WrappedRows::new(&[], 10);
    assert!(rows.is_empty());
    assert_eq!(rows.rows_of(0), 0..0);
}

#[test]
fn a_width_of_zero_is_one() {
    let rows = WrappedRows::new(&[Line::from("abc")], 0);
    assert_eq!(texts(&rows), ["a", "b", "c"]);
}

#[test]
fn rows_keep_their_styles() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let line = Line::from(vec![Span::styled("label ", bold), Span::raw("value")]);
    let rows = WrappedRows::new(&[line], 6).into_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].spans[0].style, bold);
}

#[test]
fn fitting_narrows_only_when_the_rows_overflow() {
    // Exactly ten cells: one row at 10, two at 9.
    let lines = [Line::from("abcde fghi"), Line::from("x")];
    let fits = WrappedRows::fit(&lines, 10, 2);
    assert_eq!(fits.len(), 2);
    let overflows = WrappedRows::fit(&lines, 10, 1);
    assert_eq!(texts(&overflows), ["abcde ", "fghi", "x"]);
    // Too narrow for a gutter: nothing to narrow to.
    assert_eq!(
        WrappedRows::fit(&lines, 1, 1).len(),
        WrappedRows::new(&lines, 1).len()
    );
}

#[test]
fn fitted_rows_are_as_wide_as_the_gutter_leaves() {
    let lines: Vec<Line> = (0..12)
        .map(|n| Line::from(format!("entry {n} with words that wrap")))
        .collect();
    for width in 1..30_u16 {
        for height in 1..20_u16 {
            let area = Rect::new(0, 0, width, height);
            let rows = WrappedRows::fit(&lines, usize::from(width), usize::from(height));
            let gutter = ScrollGutter::layout(area, rows.len());
            let content = usize::from(gutter.content().width);
            assert!(
                rows.rows().iter().all(|row| row.width() <= content.max(1)),
                "{width}x{height}: a row is wider than the content area"
            );
            assert_eq!(
                rows,
                WrappedRows::new(&lines, content),
                "{width}x{height}: the rows were not wrapped at the content width"
            );
        }
    }
}

/// A fixed-seed xorshift over one-cell words, some with accents, one
/// space apart.
struct Gen(u64);

impl Gen {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).unwrap_or(0)
    }

    fn line(&mut self) -> Line<'static> {
        const WORDS: &[&str] = &[
            "a",
            "to",
            "row",
            "word",
            "longer",
            "wrapping",
            "unbreakable",
            "e\u{301}t\u{e9}",
        ];
        let count = self.below(12);
        let text: String = (0..count)
            .map(|_| WORDS[self.below(WORDS.len())])
            .collect::<Vec<_>>()
            .join(" ");
        if self.below(3) == 0 {
            let bold = Style::default().add_modifier(Modifier::BOLD);
            let split = text.char_indices().nth(text.chars().count() / 2);
            if let Some((at, _)) = split {
                return Line::from(vec![
                    Span::styled(text[..at].to_string(), bold),
                    Span::raw(text[at..].to_string()),
                ]);
            }
        }
        Line::from(text)
    }
}

/// The text of every cell, row by row.
fn drawn_text(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// For one-cell text with words one space apart, the rows drawn
/// unwrapped show the same text, row for row, as the lines drawn by
/// ratatui's own wrap (keeping whitespace), so the count is the count
/// such a paragraph draws. Elsewhere the text module's rules differ on
/// purpose: a continuation row never starts with a space, and a wide
/// character after a broken word goes to the next row instead of
/// running past the edge. Spaces at a break also keep their span's
/// style, where the paragraph leaves them plain, so only text is
/// compared.
#[test]
fn the_rows_draw_what_a_wrapping_paragraph_draws() {
    let mut generator = Gen(0x5eed_0000_0000_0724);
    for case in 0..300 {
        let lines: Vec<Line> = (0..1 + generator.below(5))
            .map(|_| generator.line())
            .collect();
        let width = 2 + generator.below(24);
        let rows = WrappedRows::new(&lines, width);
        let height = u16::try_from(rows.len() + 2).expect("a small test");
        let area = Rect::new(0, 0, u16::try_from(width).expect("a small test"), height);
        let mut ours = Buffer::empty(area);
        Paragraph::new(rows.clone().into_rows()).render(area, &mut ours);
        let mut theirs = Buffer::empty(area);
        Paragraph::new(lines.clone())
            .wrap(Wrap { trim: false })
            .render(area, &mut theirs);
        assert_eq!(
            drawn_text(&ours),
            drawn_text(&theirs),
            "case {case} at width {width}: {lines:?}"
        );
    }
}

#[test]
fn a_hanging_indent_starts_every_row_after_a_lines_first() {
    let lines = [Line::from(vec![
        Span::raw("  1 "),
        Span::raw("one two three four"),
    ])];
    let rows = WrappedRows::hanging(&lines, 12, 4);
    assert_eq!(texts(&rows), ["  1 one two ", "    three ", "    four"]);
    assert_eq!(rows.rows_of(0), 0..3);
}

#[test]
fn cut_rows_keep_one_row_per_line_and_mark_the_cut() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let lines = [
        Line::from(vec![
            Span::styled("  1 ", bold),
            Span::raw("a line too long"),
        ]),
        Line::from("fits"),
    ];
    let rows = WrappedRows::cut(&lines, 10);
    assert_eq!(texts(&rows), ["  1 a lin…", "fits"]);
    assert_eq!(
        rows.rows()[0].spans[0].style,
        bold,
        "the spans keep their styles"
    );
    assert_eq!(rows.rows_of(1), 1..2);
}

#[test]
fn each_row_knows_its_line() {
    let lines = [
        Line::from("one two three"),
        Line::from(""),
        Line::from("four"),
    ];
    let rows = WrappedRows::new(&lines, 8);
    let owners: Vec<Option<usize>> = (0..rows.len() + 1).map(|row| rows.line_of(row)).collect();
    assert_eq!(owners, [Some(0), Some(0), Some(1), Some(2), None]);
}
