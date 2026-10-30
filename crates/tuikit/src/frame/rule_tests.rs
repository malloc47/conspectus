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
    let narrow = rule_line.line(10, &theme);
    assert_eq!(
        pieces(&narrow),
        [" Links ".to_string(), " 3 shown ".into(), rule(2)],
        "nothing else gives way"
    );
    assert_eq!(narrow.width(), 18, "wider than asked below that");
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
