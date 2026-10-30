use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

use super::*;

/// Each span's text, in order.
fn pieces(line: &Line<'_>) -> Vec<String> {
    line.spans
        .iter()
        .map(|span| span.content.to_string())
        .collect()
}

fn rule(cells: usize) -> String {
    marks::RULE.repeat(cells)
}

/// A theme whose rule, chip, and summary looks differ from the defaults,
/// so a test can tell which token drew each piece.
fn marked_theme() -> Theme {
    let mut theme = Theme::default();
    theme.muted = Modifier::ITALIC;
    theme.accent = Color::Magenta;
    theme.badge = Modifier::UNDERLINED;
    theme.inert = Color::Blue;
    theme
}

#[test]
fn the_summary_follows_the_chip_unless_it_comes_first() {
    let theme = Theme::default();
    let after = SectionRule::new("Links").summary("2 shown · 1 more");
    assert_eq!(
        pieces(&after.line(40, &theme)),
        [
            rule(13),
            " Links ".into(),
            " 2 shown · 1 more ".into(),
            rule(2)
        ]
    );
    assert_eq!(
        pieces(&after.summary_first().line(40, &theme)),
        [
            rule(13),
            " 2 shown · 1 more ".into(),
            " Links ".into(),
            rule(2)
        ]
    );
}

#[test]
fn the_chip_keeps_its_column_while_a_first_summary_changes_length() {
    let theme = Theme::default();
    let chip_end = |summary: &str| {
        let line = SectionRule::new("Links")
            .summary(summary)
            .summary_first()
            .line(40, &theme);
        assert_eq!(line.width(), 40, "{summary:?}");
        let text = line.to_string();
        text::width(&text[..text.find(" Links ").expect("chip") + " Links ".len()])
    };
    assert_eq!(chip_end("1 shown"), 38);
    assert_eq!(chip_end("12 shown · 3 more · 1 gone"), 38);
}

#[test]
fn without_a_summary_the_chip_ends_the_line_before_the_closing_rule() {
    let theme = Theme::default();
    let line = SectionRule::new("Detail").line(24, &theme);
    assert_eq!(pieces(&line), [rule(14), " Detail ".into(), rule(2)]);
    assert_eq!(
        line,
        SectionRule::new("Detail").summary_first().line(24, &theme),
        "with no summary, where it would go changes nothing"
    );
    assert_eq!(
        SectionRule::new("Detail").summary("").line(24, &theme),
        line,
        "an empty summary shows nothing"
    );
}

#[test]
fn the_rule_fills_the_width_in_cells() {
    let theme = Theme::default();
    for width in 20..60 {
        let line = SectionRule::new("要約").summary("3 件").line(width, &theme);
        assert_eq!(line.width(), width, "at {width}");
    }
    let line = SectionRule::new("要約").line(12, &theme);
    assert_eq!(pieces(&line), [rule(4), " 要約 ".into(), rule(2)]);
}

#[test]
fn the_leading_rule_drops_out_when_the_chip_and_summary_leave_no_room() {
    let theme = Theme::default();
    let rule_line = SectionRule::new("Links").summary("3 shown");
    // The chip (7 cells), the summary (9), and the closing rule (2).
    assert_eq!(
        pieces(&rule_line.line(18, &theme)),
        [" Links ".to_string(), " 3 shown ".into(), rule(2)]
    );
    assert_eq!(
        pieces(&rule_line.line(19, &theme)),
        [rule(1), " Links ".into(), " 3 shown ".into(), rule(2)]
    );
}

#[test]
fn a_summary_that_does_not_fit_is_cut_before_the_label() {
    let theme = Theme::default();
    let rule_line = SectionRule::new("Links").summary("3 shown · 1 more");
    // The chip (7 cells), the summary (18), and the closing rule (2).
    assert_eq!(rule_line.line(27, &theme).width(), 27);
    assert_eq!(
        pieces(&rule_line.line(26, &theme)),
        [" Links ".to_string(), " 3 shown · 1 mo… ".into(), rule(2)],
        "the summary gives the cells, the label stays whole"
    );
    let first = rule_line.clone().summary_first();
    assert_eq!(
        pieces(&first.line(14, &theme)),
        [rule(1), " 3… ".into(), " Links ".into(), rule(2)],
        "a cut that ends at a space drops it, and the leading rule takes it"
    );
    assert_eq!(
        pieces(&first.line(13, &theme)),
        [" 3… ".to_string(), " Links ".into(), rule(2)],
        "one character and the mark is the least a cut summary shows"
    );
    // Below that the summary leaves with its spaces, and the leading rule
    // takes what it gave.
    assert_eq!(
        pieces(&rule_line.line(12, &theme)),
        [rule(3), " Links ".into(), rule(2)]
    );
    assert_eq!(
        pieces(&rule_line.line(9, &theme)),
        [" Links ".to_string(), rule(2)]
    );
}

#[test]
fn a_label_that_does_not_fit_is_cut_and_then_leaves_a_plain_rule() {
    let theme = Theme::default();
    let rule_line = SectionRule::new("Related").summary("2 validated");
    // The chip alone is 9 cells; with the closing rule, 11.
    assert_eq!(
        pieces(&rule_line.line(10, &theme)),
        [" Relat… ".to_string(), rule(2)],
        "the summary went first; the label is cut"
    );
    assert_eq!(
        pieces(&rule_line.line(6, &theme)),
        [" R… ".to_string(), rule(2)]
    );
    // Too narrow for a character before the mark: the chip leaves, and
    // the line is a rule of its width, never wider.
    assert_eq!(pieces(&rule_line.line(5, &theme)), [rule(5)]);
    assert_eq!(
        rule_line.line(5, &theme).spans[0].style.add_modifier,
        Modifier::DIM
    );
    // A two-cell character that cannot show before the mark leaves too.
    assert_eq!(pieces(&SectionRule::new("要約").line(6, &theme)), [rule(6)]);
    assert_eq!(
        pieces(&SectionRule::new("要約").line(8, &theme)),
        [" 要約 ".to_string(), rule(2)]
    );
}

#[test]
fn below_the_closing_rule_the_line_is_the_rule_it_has_room_for() {
    let theme = Theme::default();
    let rule_line = SectionRule::new("Related")
        .summary("2 validated")
        .summary_first();
    assert_eq!(pieces(&rule_line.line(2, &theme)), [rule(2)]);
    assert_eq!(pieces(&rule_line.line(1, &theme)), [rule(1)]);
    assert_eq!(rule_line.line(0, &theme), Line::default());
}

#[test]
fn a_line_is_never_wider_than_its_width() {
    let theme = Theme::default();
    let rules = [
        SectionRule::new("Related")
            .summary("12 validated · 3 other · 2 ▲ · 1 —")
            .summary_first(),
        SectionRule::new("Preview"),
        SectionRule::new("要約").summary("3 件 · 東京"),
        SectionRule::new("x").summary("y"),
        SectionRule::new("label   ").summary("ends in spaces   "),
    ];
    for rule_line in &rules {
        for width in 0..60 {
            let line = rule_line.line(width, &theme);
            assert_eq!(line.width(), width, "{rule_line:?} at {width}: {line}");
        }
    }
}

#[test]
fn each_piece_draws_in_its_token() {
    let theme = marked_theme();
    let line = SectionRule::new("Links")
        .summary("3 shown")
        .line(30, &theme);
    let styles: Vec<Style> = line.spans.iter().map(|span| span.style).collect();
    let rule_look = Style::default().add_modifier(Modifier::ITALIC);
    assert_eq!(
        styles,
        [
            rule_look,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::UNDERLINED),
            Style::default().fg(Color::Blue),
            rule_look,
        ]
    );
}

#[test]
fn the_default_looks_are_a_dim_rule_a_filled_accent_chip_and_an_inert_summary() {
    let theme = Theme::default();
    let line = SectionRule::new("Links")
        .summary("3 shown")
        .line(30, &theme);
    assert_eq!(line.spans[0].style.add_modifier, Modifier::DIM);
    assert_eq!(line.spans[1].style.fg, Some(theme.accent));
    assert_eq!(
        line.spans[1].style.add_modifier,
        Modifier::REVERSED | Modifier::BOLD
    );
    assert_eq!(line.spans[2].style.fg, Some(theme.inert));
}

#[test]
fn the_label_is_the_one_it_was_given() {
    assert_eq!(SectionRule::new("Detail").label(), "Detail");
}
