use super::*;
use ratatui::style::Color;

fn lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::raw(line.to_string()))
        .collect()
}

fn rows(text: &str, mode: PreviewWrap, pane_width: Option<u16>, width: u16) -> Vec<String> {
    layout_capture(&lines(text), mode, pane_width, width)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn trailing_blank_lines_are_dropped_in_every_mode() {
    let capture = "serving on :8080\nGET /\n\n   \n\n";
    for mode in PreviewWrap::ALL {
        assert_eq!(
            rows(capture, mode, Some(80), 40),
            ["serving on :8080", "GET /"],
            "{mode:?}"
        );
    }
}

#[test]
fn trailing_lines_with_only_styled_spaces_count_as_blank() {
    let mut capture = lines("output");
    capture.push(Line::from(Span::styled(
        "      ",
        Style::default().bg(Color::Blue),
    )));
    let laid_out = layout_capture(&capture, PreviewWrap::Smart, None, 20);
    assert_eq!(laid_out.len(), 1);
}

#[test]
fn plain_wraps_rules_and_content_alike() {
    let rule = "─".repeat(12);
    assert_eq!(
        rows(&rule, PreviewWrap::Plain, Some(12), 8),
        ["─".repeat(8), "─".repeat(4)]
    );
    assert_eq!(
        rows("alpha beta gamma", PreviewWrap::Plain, None, 11),
        ["alpha beta ", "gamma"]
    );
}

#[test]
fn smart_truncates_rules_and_box_borders_instead_of_wrapping() {
    let rule = "─".repeat(120);
    assert_eq!(rows(&rule, PreviewWrap::Smart, None, 40), ["─".repeat(40)]);

    let top = format!("╭{}╮", "─".repeat(60));
    assert_eq!(
        rows(&top, PreviewWrap::Smart, None, 20),
        [format!("╭{}", "─".repeat(19))]
    );

    // Content fits; only padding and the closing border spill.
    let boxed = format!("│ > fix the preview{}│", " ".repeat(50));
    assert_eq!(
        rows(&boxed, PreviewWrap::Smart, None, 30),
        ["│ > fix the preview"]
    );

    let dashes = format!("{}  ", "-".repeat(50));
    assert_eq!(
        rows(&dashes, PreviewWrap::Smart, None, 10),
        ["-".repeat(10)]
    );
}

#[test]
fn smart_squeezes_padding_to_keep_a_status_line_on_one_row() {
    let status = format!("  ⏵⏵ accept edits on{}ctx: 40%", " ".repeat(40));
    assert_eq!(
        rows(&status, PreviewWrap::Smart, None, 40),
        [format!("  ⏵⏵ accept edits on{}ctx: 40%", " ".repeat(12))]
    );
}

#[test]
fn smart_word_wraps_content_with_a_hanging_indent() {
    assert_eq!(
        rows(
            "  ⎿  the quick brown fox jumps over the lazy dog",
            PreviewWrap::Smart,
            None,
            24
        ),
        [
            "  ⎿  the quick brown ",
            "  fox jumps over the ",
            "  lazy dog"
        ]
    );
}

#[test]
fn smart_wraps_lines_whose_overflow_carries_content() {
    // Squeezing every gap to one space still doesn't fit, so the line
    // wraps untouched rather than losing the tail.
    let text = "aaaa  bbbb  cccc  dddd";
    assert_eq!(
        rows(text, PreviewWrap::Smart, None, 12),
        ["aaaa  bbbb  ", "cccc  dddd"]
    );
}

#[test]
fn smart_drops_trailing_padding_before_measuring() {
    let text = format!("short{}", " ".repeat(100));
    assert_eq!(rows(&text, PreviewWrap::Smart, None, 10), ["short"]);
}

#[test]
fn none_rewraps_at_the_pane_width_and_clips_to_the_preview() {
    // tmux joined a 30-column line that it showed wrapped at 20.
    let joined = format!("{}{}", "a".repeat(20), "b".repeat(10));
    assert_eq!(
        rows(&joined, PreviewWrap::None, Some(20), 12),
        ["a".repeat(12), "b".repeat(10)]
    );
    // A preview wider than the pane shows tmux's rows as they are.
    assert_eq!(
        rows(&joined, PreviewWrap::None, Some(20), 40),
        ["a".repeat(20), "b".repeat(10)]
    );
}

#[test]
fn none_without_a_pane_width_clips_each_line() {
    assert_eq!(
        rows(&"x".repeat(30), PreviewWrap::None, None, 12),
        ["x".repeat(12)]
    );
}

#[test]
fn layout_keeps_span_styles_across_wraps() {
    let line = Line::from(vec![
        Span::styled("red ", Style::default().fg(Color::Red)),
        Span::styled("blue words", Style::default().fg(Color::Blue)),
    ]);
    let laid_out = layout_capture(&[line], PreviewWrap::Plain, None, 9);
    assert_eq!(laid_out.len(), 2);
    assert_eq!(laid_out[0].spans[0].style.fg, Some(Color::Red));
    assert_eq!(laid_out[0].spans[1].content, "blue ");
    assert_eq!(laid_out[0].spans[1].style.fg, Some(Color::Blue));
    assert_eq!(laid_out[1].to_string(), "words");
    assert_eq!(laid_out[1].spans[0].style.fg, Some(Color::Blue));
}

#[test]
fn wide_characters_count_two_columns() {
    assert_eq!(
        rows("日本語のテキスト", PreviewWrap::None, None, 7),
        ["日本語"]
    );
}

#[test]
fn tabs_expand_to_eight_column_stops() {
    assert_eq!(rows("a\tb", PreviewWrap::None, None, 20), ["a       b"]);
}

#[test]
fn every_row_fits_the_preview_width() {
    let capture = format!(
        "{}\n│ prompt{}│\nplain words that go on and on and on\n\t\tindented\n{}",
        "═".repeat(90),
        " ".repeat(70),
        "x".repeat(200)
    );
    for mode in PreviewWrap::ALL {
        for width in [1u16, 5, 17, 40] {
            for row in layout_capture(&lines(&capture), mode, Some(90), width) {
                assert!(
                    row.width() <= usize::from(width),
                    "{mode:?} at {width}: {row:?}"
                );
            }
        }
    }
}
