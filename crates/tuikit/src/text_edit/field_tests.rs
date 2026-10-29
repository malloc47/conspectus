use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::*;
use crate::testkit::golden::assert_golden;

fn theme() -> Theme {
    Theme::default()
}

fn text_of(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

/// The text of the span drawn in the caret style, if any.
fn caret(spans: &[Span<'_>]) -> Option<String> {
    let caret = theme().field_cursor.into_style();
    spans
        .iter()
        .find(|span| span.style == caret)
        .map(|span| span.content.to_string())
}

/// A form's field: filled, with marks, in the focused field's style.
fn boxed(edit: &LineEdit) -> TextField<'_> {
    TextField::new(edit)
        .marks(true)
        .fill(true)
        .style(theme().field_active.into_style())
}

const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

#[test]
fn a_long_value_scrolls_so_the_cursor_rides_the_right_edge() {
    let edit = LineEdit::new(ALPHABET).with_cursor(20);
    let spans = boxed(&edit).spans(12, &theme());
    assert_eq!(text_of(&spans), "< nopqrstu >");
    assert_eq!(caret(&spans).as_deref(), Some("u"));
    let edit = LineEdit::new(ALPHABET).with_cursor(0);
    let spans = boxed(&edit).spans(12, &theme());
    assert_eq!(text_of(&spans), "  abcdefgh >", "nothing hidden before");
    assert_eq!(caret(&spans).as_deref(), Some("a"));
}

#[test]
fn the_caret_sits_on_a_blank_cell_after_the_text() {
    let edit = LineEdit::new("abc");
    let spans = TextField::new(&edit).spans(10, &theme());
    assert_eq!(
        text_of(&spans),
        "abc ",
        "an unfilled field ends with its text"
    );
    assert_eq!(caret(&spans).as_deref(), Some(" "));
    let filled = TextField::new(&edit).fill(true).spans(10, &theme());
    assert_eq!(text_of(&filled), "abc       ");
}

#[test]
fn an_unfocused_field_draws_no_caret() {
    let edit = LineEdit::new("abc").with_cursor(1);
    let field = TextField::new(&edit)
        .focused(false)
        .style(theme().field.into_style());
    let spans = field.spans(10, &theme());
    assert_eq!(caret(&spans), None);
    assert!(
        spans
            .iter()
            .all(|span| span.style == theme().field.into_style())
    );
}

#[test]
fn wide_characters_scroll_by_their_cells() {
    // Five two-cell characters with the cursor at the end: what fits in
    // seven cells is three of them and the caret.
    let edit = LineEdit::new("東京タワー");
    let spans = TextField::new(&edit).spans(7, &theme());
    assert_eq!(text_of(&spans), "タワー ");
    assert_eq!(text::spans_width(&spans), 7);
    // On a wide character the caret covers both of its cells.
    let edit = LineEdit::new("東京タワー").with_cursor(1);
    let spans = TextField::new(&edit).spans(7, &theme());
    assert_eq!(caret(&spans).as_deref(), Some("京"));
    assert_eq!(
        text_of(&spans),
        "東京タ",
        "the fourth would straddle the edge"
    );
}

#[test]
fn a_wide_character_that_would_straddle_the_edge_leaves_the_cell_blank() {
    let edit = LineEdit::new("a東京").with_cursor(0);
    let spans = TextField::new(&edit).fill(true).spans(4, &theme());
    assert_eq!(text_of(&spans), "a東 ");
    assert_eq!(text::spans_width(&spans), 4);
    let marked = boxed(&edit).spans(7, &theme());
    assert_eq!(text_of(&marked), "  a東 >", "more follows");
}

#[test]
fn a_character_wider_than_the_field_is_drawn_as_the_ellipsis() {
    let edit = LineEdit::new("東").with_cursor(0);
    let spans = TextField::new(&edit).spans(1, &theme());
    assert_eq!(text_of(&spans), marks::ELLIPSIS);
}

#[test]
fn the_prefix_stays_put_while_the_text_scrolls() {
    let edit = LineEdit::new("needle in a haystack");
    let accent = Style::new().fg(theme().accent);
    let spans = TextField::new(&edit)
        .prefix(Span::styled("/", accent))
        .spans(12, &theme());
    assert_eq!(text_of(&spans), "/a haystack ");
    assert_eq!(spans[0].style, accent);
    let none = TextField::new(&edit).prefix("//").spans(1, &theme());
    assert_eq!(text_of(&none), "/", "a prefix wider than the row is cut");
}

#[test]
fn an_empty_field_shows_its_placeholder_under_the_caret() {
    let edit = LineEdit::default();
    let spans = boxed(&edit).placeholder("-").spans(10, &theme());
    assert_eq!(text_of(&spans), "  -       ");
    assert_eq!(caret(&spans).as_deref(), Some("-"));
    let typed = LineEdit::new(" ");
    let spans = boxed(&typed).placeholder("-").spans(10, &theme());
    assert_eq!(text_of(&spans), "          ", "a typed space is text");
}

#[test]
fn a_field_too_narrow_for_its_marks_gives_every_cell_to_the_text() {
    let edit = LineEdit::new(ALPHABET).with_cursor(3);
    assert_eq!(text_of(&boxed(&edit).spans(4, &theme())), "abcd");
    assert_eq!(text_of(&boxed(&edit).spans(5, &theme())), "< d >");
    assert!(boxed(&edit).spans(0, &theme()).is_empty());
}

#[test]
fn the_marks_take_the_accent_over_the_fields_style() {
    let edit = LineEdit::new(ALPHABET).with_cursor(20);
    let spans = boxed(&edit).spans(12, &theme());
    let mark = spans
        .iter()
        .find(|span| span.content == "< ")
        .expect("mark");
    assert_eq!(
        mark.style,
        Style::new()
            .fg(Color::Cyan)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    );
    let blank = boxed(&LineEdit::new("ab")).spans(12, &theme());
    assert_eq!(blank[0], Span::raw("  "), "an empty slot is unstyled");
}

/// Every field over every value, cursor, and width keeps its width and
/// shows the cursor's grapheme.
#[test]
fn every_window_fits_and_holds_the_cursor() {
    let values = [
        "",
        "plain ascii text",
        "東京タワーと富士山",
        "a\u{2764}\u{fe0f}b\u{1f469}\u{200d}\u{1f4bb}c",
        "cafe\u{301} de\u{301}ja\u{300} vu",
        "mixed 東京 text 🙂 here",
    ];
    for value in values {
        let graphemes = clusters(value);
        for (index, cursor_cluster) in graphemes.iter().map(Some).chain([None]).enumerate() {
            let cursor = value[..cursor_cluster.map_or(value.len(), |c| c.at)]
                .chars()
                .count();
            let edit = LineEdit::new(value).with_cursor(cursor);
            for width in 0..24 {
                for marks in [false, true] {
                    let field = TextField::new(&edit).marks(marks);
                    let open = field.spans(width, &theme());
                    let filled = field.clone().fill(true).spans(width, &theme());
                    let what = format!("{value:?} at {index}, width {width}, marks {marks}");
                    assert!(text::spans_width(&open) <= width, "{what}");
                    assert_eq!(text::spans_width(&filled), width, "{what}");
                    if width == 0 {
                        continue;
                    }
                    let expected = match cursor_cluster {
                        Some(cluster) if cluster.width > width => marks::ELLIPSIS.to_string(),
                        Some(cluster) => cluster.text.to_string(),
                        None => " ".to_string(),
                    };
                    if !marks
                        || width < MIN_MARKED
                        || cursor_cluster.is_none_or(|c| c.width <= width - 4)
                    {
                        assert_eq!(caret(&open), Some(expected), "{what}");
                    }
                }
            }
        }
    }
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/text_edit/goldens")
        .join(name)
}

#[test]
fn the_field_looks_as_its_golden_says() {
    let theme = theme();
    let mut buf = Buffer::empty(Rect::new(0, 0, 24, 6));
    let row = |y| Rect::new(0, y, 24, 1);
    let long = LineEdit::new(ALPHABET).with_cursor(20);
    boxed(&long).render(row(0), &mut buf, &theme);
    let idle = LineEdit::new("idle value").with_cursor(2);
    TextField::new(&idle)
        .marks(true)
        .fill(true)
        .focused(false)
        .style(theme.field.into_style())
        .render(row(1), &mut buf, &theme);
    let empty = LineEdit::default();
    boxed(&empty)
        .placeholder("-")
        .render(row(2), &mut buf, &theme);
    let query = LineEdit::new("東京タワーと富士山の写真");
    TextField::new(&query)
        .prefix(Span::styled("/", Style::new().fg(theme.accent)))
        .render(row(3), &mut buf, &theme);
    let mid = LineEdit::new("東京タワーと富士山の写真").with_cursor(3);
    TextField::new(&mid)
        .prefix(Span::styled("/", Style::new().fg(theme.accent)))
        .render(row(4), &mut buf, &theme);
    let emoji = LineEdit::new("a🙂b").with_cursor(1);
    boxed(&emoji).render(row(5), &mut buf, &theme);
    assert_golden(&golden_path("field.golden"), &buf, |path, text| {
        std::fs::write(path, text)
    });
}
