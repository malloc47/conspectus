use ratatui::style::{Modifier, Style};

use super::*;

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn text_of(lines: &[Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

#[test]
fn a_word_that_ends_at_the_edge_stays_on_its_row() {
    assert_eq!(wrap("alpha beta gamma", 10), ["alpha beta", "gamma"]);
    assert_eq!(wrap("hello world", 5), ["hello", "world"]);
    assert_eq!(wrap("a b c", 3), ["a b", "c"]);
}

#[test]
fn spaces_before_a_break_stay_when_they_fit() {
    assert_eq!(wrap("alpha beta gamma", 11), ["alpha beta ", "gamma"]);
    assert_eq!(wrap("aaa  bbb", 4), ["aaa ", "bbb"]);
}

#[test]
fn a_continuation_row_never_starts_with_a_space() {
    assert_eq!(wrap("aaa   bbb", 4), ["aaa ", "bbb"]);
    assert_eq!(wrap("the quick brown fox", 10), ["the quick ", "brown fox"]);
}

#[test]
fn the_first_row_keeps_its_indent() {
    assert_eq!(
        wrap("  the quick brown fox jumps over", 12),
        ["  the quick ", "brown fox ", "jumps over"]
    );
}

#[test]
fn a_long_word_breaks_and_carries_on() {
    assert_eq!(wrap("aaaaaaaaaaaa bb", 5), ["aaaaa", "aaaaa", "aa bb"]);
    assert_eq!(wrap_chars("abcdefgh", 3), ["abc", "def", "gh"]);
}

#[test]
fn line_breaks_end_rows_and_an_empty_text_is_one_row() {
    assert_eq!(wrap("one\ntwo three", 20), ["one", "two three"]);
    assert_eq!(wrap("", 10), [""]);
    assert_eq!(wrap_chars("", 10), [""]);
}

#[test]
fn wide_characters_are_never_split() {
    assert_eq!(wrap("東京都庁", 5), ["東京", "都庁"]);
    assert_eq!(
        wrap_chars("日本語のテキスト", 7),
        ["日本語", "のテキ", "スト"]
    );
    // A two-cell character cannot be drawn in a one-cell row.
    assert_eq!(wrap_chars("a東", 1), ["a", "…"]);
}

#[test]
fn a_width_of_zero_is_one() {
    assert_eq!(wrap("ab", 0), ["a", "b"]);
}

#[test]
fn separator_wrap_keeps_items_whole() {
    let hints = "j/k move · Enter open · Space mark · Esc close";
    assert_eq!(
        wrap_at(hints, " · ", 24),
        ["j/k move · Enter open", "Space mark · Esc close"]
    );
    assert_eq!(wrap_at(hints, " · ", 200), [hints]);
    assert_eq!(wrap_at("", " · ", 10), [""]);
}

#[test]
fn an_item_wider_than_a_row_is_word_wrapped() {
    assert_eq!(
        wrap_at("a very long hint item · x", " · ", 10),
        ["a very ", "long hint ", "item · x"]
    );
}

#[test]
fn styled_wrap_keeps_each_span_style() {
    let line = Line::from(vec![Span::styled("red ", bold()), Span::raw("blue words")]);
    let rows = wrap_line(&line, 9, 0);
    assert_eq!(text_of(&rows), ["red blue ", "words"]);
    assert_eq!(rows[0].spans[0].style, bold());
    assert_eq!(rows[0].spans[1].content, "blue ");
    assert_eq!(rows[1].spans[0].style, Style::default());
}

#[test]
fn a_word_across_two_styles_is_one_word() {
    let line = Line::from(vec![
        Span::raw("go "),
        Span::styled("fast", bold()),
        Span::raw("er now"),
    ]);
    assert_eq!(text_of(&wrap_line(&line, 8, 0)), ["go ", "faster ", "now"]);
}

#[test]
fn a_hanging_indent_starts_every_continuation_row() {
    let line = Line::raw("  the quick brown fox jumps over the lazy dog");
    assert_eq!(
        text_of(&wrap_line(&line, 20, 2)),
        ["  the quick brown ", "  fox jumps over the", "  lazy dog"]
    );
    // An indent as wide as the row is ignored.
    assert_eq!(text_of(&wrap_line(&Line::raw("ab cd"), 2, 4)), ["ab", "cd"]);
}

#[test]
fn rows_keep_the_line_style_and_alignment() {
    let line = Line::styled("one two", bold()).centered();
    for row in wrap_line(&line, 4, 0) {
        assert_eq!(row.style, bold());
        assert_eq!(row.alignment, line.alignment);
    }
}

#[test]
fn styled_character_wrap_fills_rows() {
    let line = Line::from(vec![Span::styled("abcd", bold()), Span::raw("ef")]);
    let rows = wrap_line_chars(&line, 3);
    assert_eq!(text_of(&rows), ["abc", "def"]);
    assert_eq!(rows[1].spans[0].style, bold());
    assert_eq!(rows[1].spans[1].style, Style::default());
}
