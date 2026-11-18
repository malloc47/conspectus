use super::*;
use ratatui::style::Color;

fn lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::raw(line.to_string()))
        .collect()
}

fn rows(text: &str, mode: WrapMode, source_width: Option<u16>, width: u16) -> Vec<String> {
    layout_capture(&lines(text), mode, source_width, width)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn trailing_blank_lines_are_dropped_in_every_mode() {
    let capture = "serving on :8080\nGET /\n\n   \n\n";
    for mode in WrapMode::ALL {
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
    let laid_out = layout_capture(&capture, WrapMode::Smart, None, 20);
    assert_eq!(laid_out.len(), 1);
}

#[test]
fn plain_wraps_rules_and_content_alike() {
    let rule = "─".repeat(12);
    assert_eq!(
        rows(&rule, WrapMode::Plain, Some(12), 8),
        ["─".repeat(8), "─".repeat(4)]
    );
    assert_eq!(
        rows("alpha beta gamma", WrapMode::Plain, None, 11),
        ["alpha beta ", "gamma"]
    );
}

#[test]
fn smart_truncates_rules_and_box_borders_instead_of_wrapping() {
    let rule = "─".repeat(120);
    assert_eq!(rows(&rule, WrapMode::Smart, None, 40), ["─".repeat(40)]);

    let top = format!("╭{}╮", "─".repeat(60));
    assert_eq!(
        rows(&top, WrapMode::Smart, None, 20),
        [format!("╭{}", "─".repeat(19))]
    );

    // Content fits; only padding and the closing border spill.
    let boxed = format!("│ > fix the preview{}│", " ".repeat(50));
    assert_eq!(
        rows(&boxed, WrapMode::Smart, None, 30),
        ["│ > fix the preview"]
    );

    let dashes = format!("{}  ", "-".repeat(50));
    assert_eq!(rows(&dashes, WrapMode::Smart, None, 10), ["-".repeat(10)]);
}

#[test]
fn smart_squeezes_padding_to_keep_a_status_line_on_one_row() {
    let status = format!("  ⏵⏵ accept edits on{}ctx: 40%", " ".repeat(40));
    assert_eq!(
        rows(&status, WrapMode::Smart, None, 40),
        [format!("  ⏵⏵ accept edits on{}ctx: 40%", " ".repeat(12))]
    );
}

#[test]
fn smart_shortens_the_rule_before_a_label_to_keep_it_on_one_row() {
    // A prompt rule 155 columns wide with the source's name near its
    // right end: the label sits in the overflow, so cutting would lose it.
    let rule = format!("{} notes ─", "─".repeat(147));
    assert_eq!(
        rows(&rule, WrapMode::Smart, None, 60),
        [format!("{} notes ─", "─".repeat(52))]
    );
}

#[test]
fn smart_shortens_the_rules_on_both_sides_of_a_centered_label() {
    let banner = format!("{} 3 passed in 0.21s {}", "=".repeat(30), "=".repeat(30));
    assert_eq!(
        rows(&banner, WrapMode::Smart, None, 39),
        [format!(
            "{} 3 passed in 0.21s {}",
            "=".repeat(10),
            "=".repeat(10)
        )]
    );
}

#[test]
fn smart_still_cuts_a_rule_whose_label_fits_before_the_edge() {
    let rule = format!("── notes {}", "─".repeat(140));
    assert_eq!(
        rows(&rule, WrapMode::Smart, None, 30),
        [format!("── notes {}", "─".repeat(21))]
    );
}

#[test]
fn smart_never_shortens_short_dash_runs_in_content() {
    // Runs of fewer than four rule characters are content: shortening
    // them would turn `--all-targets` into `-all-targets`.
    for text in [
        "cargo test --workspace --all-targets --all-features -- --nocapture",
        "if left === right && next --> done ==> fold -> ok ___ ~~~ end",
    ] {
        assert_eq!(
            rows(text, WrapMode::Smart, None, 24),
            rows(text, WrapMode::Plain, None, 24),
            "{text}"
        );
    }
}

#[test]
fn smart_wraps_a_labeled_rule_whose_label_alone_overflows() {
    let rule = format!(
        "{} a label wider than the area {}",
        "─".repeat(20),
        "─".repeat(20)
    );
    assert_eq!(
        rows(&rule, WrapMode::Smart, None, 12),
        rows(&rule, WrapMode::Plain, None, 12)
    );
}

#[test]
fn smart_keeps_the_proportions_of_a_rule_drawn_in_two_styles() {
    // A progress bar half done: one rule character, two colors.
    let done = Style::default().fg(Color::Green);
    let left = Style::default().fg(Color::DarkGray);
    let bar = Line::from(vec![
        Span::styled("━".repeat(20), done),
        Span::styled("━".repeat(20), left),
        Span::raw(" 50% 0:00:03"),
    ]);
    let laid_out = layout_capture(&[bar], WrapMode::Smart, None, 30);
    assert_eq!(laid_out.len(), 1);
    assert_eq!(
        laid_out[0].to_string(),
        format!("{} 50% 0:00:03", "━".repeat(18))
    );
    assert_eq!(laid_out[0].spans[0].content, "━".repeat(9));
    assert_eq!(laid_out[0].spans[0].style, done);
    assert_eq!(laid_out[0].spans[1].content, "━".repeat(9));
    assert_eq!(laid_out[0].spans[1].style, left);
}

#[test]
fn smart_word_wraps_content_with_a_hanging_indent() {
    assert_eq!(
        rows(
            "  ⎿  the quick brown fox jumps over the lazy dog",
            WrapMode::Smart,
            None,
            24
        ),
        // `fox` ends exactly at the edge, so it stays on the first row.
        [
            "  ⎿  the quick brown fox",
            "  jumps over the lazy ",
            "  dog"
        ]
    );
}

#[test]
fn smart_wraps_lines_whose_overflow_carries_content() {
    // Squeezing every gap to one space still doesn't fit, so the line
    // wraps untouched rather than losing the tail.
    let text = "aaaa  bbbb  cccc  dddd";
    assert_eq!(
        rows(text, WrapMode::Smart, None, 12),
        ["aaaa  bbbb  ", "cccc  dddd"]
    );
}

#[test]
fn smart_drops_trailing_padding_before_measuring() {
    let text = format!("short{}", " ".repeat(100));
    assert_eq!(rows(&text, WrapMode::Smart, None, 10), ["short"]);
}

#[test]
fn none_rewraps_at_the_source_width_and_clips_to_the_area() {
    // The source joined a 30-column line that it showed wrapped at 20.
    let joined = format!("{}{}", "a".repeat(20), "b".repeat(10));
    assert_eq!(
        rows(&joined, WrapMode::None, Some(20), 12),
        ["a".repeat(12), "b".repeat(10)]
    );
    // An area wider than the source shows its rows as they are.
    assert_eq!(
        rows(&joined, WrapMode::None, Some(20), 40),
        ["a".repeat(20), "b".repeat(10)]
    );
}

#[test]
fn none_without_a_source_width_clips_each_line() {
    assert_eq!(
        rows(&"x".repeat(30), WrapMode::None, None, 12),
        ["x".repeat(12)]
    );
}

#[test]
fn layout_keeps_span_styles_across_wraps() {
    let line = Line::from(vec![
        Span::styled("red ", Style::default().fg(Color::Red)),
        Span::styled("blue words", Style::default().fg(Color::Blue)),
    ]);
    let laid_out = layout_capture(&[line], WrapMode::Plain, None, 9);
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
        rows("日本語のテキスト", WrapMode::None, None, 7),
        ["日本語"]
    );
}

#[test]
fn tabs_expand_to_eight_column_stops() {
    assert_eq!(rows("a\tb", WrapMode::None, None, 20), ["a       b"]);
}

#[test]
fn every_row_fits_the_area_width() {
    let capture = format!(
        "{}\n│ prompt{}│\nplain words that go on and on and on\n\t\tindented\n{}\n{} label {}",
        "═".repeat(90),
        " ".repeat(70),
        "x".repeat(200),
        "─".repeat(80),
        "─".repeat(3)
    );
    for mode in WrapMode::ALL {
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

fn smart_rows_with(
    text: &str,
    width: u16,
    smart: &SmartWrap,
    fit: impl Fn(&Line<'_>) -> LineFit,
) -> Vec<String> {
    layout_capture_with(&lines(text), WrapMode::Smart, None, width, smart, fit)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_default_settings_are_what_layout_capture_uses() {
    let capture = format!(
        "{}\n{} notes ─\n  ⏵⏵ on{}ctx: 4%",
        "─".repeat(90),
        "─".repeat(80),
        " ".repeat(60)
    );
    for width in [10u16, 30, 60] {
        assert_eq!(
            smart_rows_with(&capture, width, &SmartWrap::default(), |_| LineFit::Smart),
            rows(&capture, WrapMode::Smart, None, width)
        );
    }
}

#[test]
fn the_decoration_setting_decides_what_is_cut() {
    let stars = SmartWrap {
        decoration: std::iter::once('*'..='*').collect(),
        ..SmartWrap::default()
    };
    assert!(stars.is_decoration('*') && !stars.is_decoration('─'));
    assert_eq!(
        smart_rows_with(&"*".repeat(30), 10, &stars, |_| LineFit::Smart),
        ["*".repeat(10)]
    );
    // A box-drawing rule is no longer decoration, so it wraps.
    assert_eq!(
        smart_rows_with(&"─".repeat(30), 10, &stars, |_| LineFit::Smart).len(),
        3
    );
}

#[test]
fn the_shortest_rule_setting_decides_which_runs_shorten() {
    let text = format!("-- {} -- tail", "x".repeat(10));
    // At the default four, the two-dash runs are content: the line wraps.
    assert_eq!(
        smart_rows_with(&text, 20, &SmartWrap::default(), |_| LineFit::Smart).len(),
        2
    );
    let pairs = SmartWrap {
        min_rule_run: 2,
        ..SmartWrap::default()
    };
    assert_eq!(
        smart_rows_with(&text, 20, &pairs, |_| LineFit::Smart),
        [format!("-- {} - tail", "x".repeat(10))]
    );
}

#[test]
fn rules_wrap_when_shortening_is_off() {
    let rule = format!("{} notes ─", "─".repeat(147));
    let off = SmartWrap {
        shorten_rules: false,
        ..SmartWrap::default()
    };
    assert_eq!(
        smart_rows_with(&rule, 60, &off, |_| LineFit::Smart),
        rows(&rule, WrapMode::Plain, None, 60)
    );
}

#[test]
fn a_right_aligned_line_loses_only_the_padding_it_must() {
    let hint = format!("{}new task? /clear", " ".repeat(70));
    let right = |_: &Line<'_>| LineFit::RightAlign;
    assert_eq!(
        smart_rows_with(&hint, 30, &SmartWrap::default(), right),
        [format!("{}new task? /clear", " ".repeat(14))]
    );
    // Already fitting, it keeps all of it.
    let short = format!("{}hint", " ".repeat(6));
    assert_eq!(
        smart_rows_with(&short, 30, &SmartWrap::default(), right),
        std::slice::from_ref(&short)
    );
    // Its text alone wider than the area: the text, wrapped the smart way.
    let long = format!("{}alpha beta gamma delta", " ".repeat(40));
    assert_eq!(
        smart_rows_with(&long, 12, &SmartWrap::default(), right),
        ["alpha beta ", "gamma delta"]
    );
}

#[test]
fn cut_and_wrap_fit_a_line_as_they_say() {
    let text = "alpha beta gamma delta";
    assert_eq!(
        smart_rows_with(text, 12, &SmartWrap::default(), |_| LineFit::Cut),
        ["alpha beta g"]
    );
    assert_eq!(
        smart_rows_with(text, 12, &SmartWrap::default(), |_| LineFit::Wrap),
        rows(text, WrapMode::Plain, None, 12)
    );
}

#[test]
fn a_dropped_line_leaves_no_blank_rows_at_the_bottom() {
    let capture = "output\n\n\nhint to drop\n";
    let fit = |line: &Line<'_>| {
        if line.to_string() == "hint to drop" {
            LineFit::Drop
        } else {
            LineFit::Smart
        }
    };
    assert_eq!(
        smart_rows_with(capture, 30, &SmartWrap::default(), fit),
        ["output"]
    );
}

#[test]
fn only_smart_wrap_asks_for_a_fit_or_reads_its_settings() {
    let capture = format!("{}\nkeep", "─".repeat(40));
    let stars = SmartWrap {
        decoration: std::iter::once('*'..='*').collect(),
        ..SmartWrap::default()
    };
    for mode in [WrapMode::Plain, WrapMode::None] {
        let laid_out = layout_capture_with(&lines(&capture), mode, Some(40), 12, &stars, |_| {
            panic!("{mode:?} asked for a fit")
        });
        assert_eq!(
            laid_out,
            layout_capture(&lines(&capture), mode, Some(40), 12)
        );
    }
}

#[test]
fn every_fit_keeps_rows_within_the_area() {
    let capture = format!(
        "{}\n{}label {}\n{}right\n\t\tindented words that wrap\n{}",
        "═".repeat(90),
        " ".repeat(30),
        "─".repeat(50),
        " ".repeat(80),
        "x".repeat(200)
    );
    for fit in LineFit::ALL {
        for width in [1u16, 5, 17, 40] {
            let laid_out = layout_capture_with(
                &lines(&capture),
                WrapMode::Smart,
                None,
                width,
                &SmartWrap::default(),
                |_| fit,
            );
            for row in laid_out {
                assert!(
                    row.width() <= usize::from(width),
                    "{fit:?} at {width}: {row:?}"
                );
            }
        }
    }
}
