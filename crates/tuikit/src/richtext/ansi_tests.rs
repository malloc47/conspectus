use super::*;

#[test]
fn with_color_parses_ansi_into_styled_spans() {
    // ESC[31m makes "red", ESC[0m resets.
    let raw = "\x1b[31mred\x1b[0m  plain";
    let text = ansi_text(raw, true);
    // Flattened content matches the visible characters.
    assert_eq!(text.to_string(), "red  plain");
    // The first line's first span carries red foreground style.
    let first_line = text.lines.first().expect("at least one line");
    let first_span = first_line.spans.first().expect("at least one span");
    assert_eq!(first_span.content, "red");
    assert_eq!(
        first_span.style.fg,
        Some(ratatui::style::Color::Red),
        "expected red fg on the red span"
    );
}

#[test]
fn without_color_strips_styling() {
    let raw = "\x1b[31mred\x1b[0m  plain";
    let text = ansi_text(raw, false);
    assert_eq!(text.to_string(), "red  plain");
    // With colour disabled every span should land styled
    // identically to a plain `Text::raw` — i.e. default fg.
    for line in &text.lines {
        for span in &line.spans {
            assert_eq!(
                span.style.fg, None,
                "expected colour to be stripped, got span={span:?}"
            );
        }
    }
}

#[test]
fn falls_back_to_plain_text_on_malformed_input() {
    // Lone ESC byte: the parser either reads it harmlessly or fails;
    // either way the text comes back printable rather than panicking.
    let raw = "before\x1bafter";
    let text = ansi_text(raw, true);
    let flattened = text.to_string();
    // The visible characters either side of the rogue ESC must survive,
    // so a reader loses no output to a single bad byte.
    assert!(
        flattened.contains("before"),
        "expected 'before' in output, got: {flattened:?}"
    );
    assert!(
        flattened.contains("after"),
        "expected 'after' in output, got: {flattened:?}"
    );
}
