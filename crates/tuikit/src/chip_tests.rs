//! The width rule of the filled chip (default, themed, overridden, and
//! wide text), each chip form's text and look, and the spinner frames.

use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};

use super::*;
use crate::glyphs::marks;

fn theme_with_badge_width(cells: usize) -> Theme {
    let mut theme = Theme::default();
    theme.badge_width = cells;
    theme
}

#[test]
fn a_chip_is_the_label_width_and_two_cells_whatever_its_label() {
    let theme = Theme::default();
    assert_eq!(width(&theme), 10);
    for label in [
        "",
        "a",
        "eight ch",
        "longer than eight",
        "東京",
        "東京大阪名古屋",
    ] {
        let chip = Chip::new(label).span(&theme);
        assert_eq!(text::width(&chip.content), width(&theme), "{label:?}");
        assert!(chip.content.starts_with(' ') && chip.content.ends_with(' '));
    }
}

#[test]
fn a_chip_pads_short_labels_and_cuts_long_ones_in_cells() {
    let theme = Theme::default();
    let text = |label| Chip::new(label).text(&theme);
    assert_eq!(text("alpha"), " alpha    ");
    assert_eq!(text("exactly8"), " exactly8 ");
    assert_eq!(text("ninechars"), " ninecha… ");
    assert_eq!(
        text("東京"),
        " 東京     ",
        "two two-cell characters take four cells"
    );
    assert_eq!(
        text("東京大阪名古屋"),
        " 東京大…  ",
        "a cut a cell short of the width is made up by the padding"
    );
}

#[test]
fn a_chip_follows_the_theme_badge_width_and_its_minimum() {
    let narrow = theme_with_badge_width(5);
    assert_eq!(Chip::new("alpha").text(&narrow), " alpha ");
    assert_eq!(Chip::new("alphabet").text(&narrow), " alph… ");
    assert_eq!(width(&narrow), 7);
    let wide = theme_with_badge_width(12);
    assert_eq!(Chip::new("alphabet").text(&wide), " alphabet     ");
    let below = theme_with_badge_width(0);
    assert_eq!(label_width(&below), MIN_BADGE_WIDTH);
    assert_eq!(Chip::new("alpha").text(&below), " a… ");
}

#[test]
fn a_chip_label_width_overrides_the_theme() {
    let theme = Theme::default();
    assert_eq!(Chip::new("alpha").label_width(6).text(&theme), " alpha  ");
    assert_eq!(Chip::new("alpha").label_width(1).text(&theme), " a… ");
}

#[test]
fn a_chip_aligns_a_short_label() {
    let theme = Theme::default();
    let aligned = |align| Chip::new("abc").align(align).text(&theme);
    assert_eq!(aligned(Alignment::Left), " abc      ");
    assert_eq!(aligned(Alignment::Center), "   abc    ");
    assert_eq!(aligned(Alignment::Right), "      abc ");
}

#[test]
fn a_chip_takes_the_badge_look_over_its_color_or_a_style_of_the_callers() {
    let theme = Theme::default();
    let plain = Chip::new("x").span(&theme);
    assert_eq!(plain.style, Style::default().add_modifier(theme.badge));
    let colored = Chip::new("x").color(Color::Magenta).span(&theme);
    assert_eq!(
        colored.style,
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    );
    let mut custom = Theme::default();
    custom.badge = Modifier::UNDERLINED;
    assert_eq!(
        Chip::new("x").color(Color::Cyan).look(&custom),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    );
    let own = Style::default().fg(Color::White).bg(Color::Black);
    let styled = Chip::new("x").style(own).span(&theme);
    assert_eq!(styled.style, own);
    assert_eq!(text::width(&styled.content), width(&theme));
}

#[test]
fn a_chip_keeps_its_label() {
    assert_eq!(Chip::new(String::from("owned")).label(), "owned");
}

#[test]
fn a_toggle_chip_pads_its_state_to_the_widest_it_can_show() {
    let states = ["off", "sum", "trunc", "all"];
    let texts: Vec<String> = states
        .iter()
        .map(|state| {
            ToggleChip::new("tools", state, *state != "off")
                .states(&states)
                .key("t")
                .text()
        })
        .collect();
    assert_eq!(
        texts,
        [
            "tools·off   (t)",
            "tools·sum   (t)",
            "tools·trunc (t)",
            "tools·all   (t)"
        ]
    );
    assert!(texts.iter().all(|text| text::width(text) == 15));
    assert!(texts[0].contains(marks::SEPARATOR));
}

#[test]
fn a_toggle_chip_without_states_or_key_is_just_its_words() {
    let chip = ToggleChip::new("wrap", "on", true);
    assert_eq!(chip.text(), "wrap·on");
    assert!(chip.is_on());
    let wider = ToggleChip::new("wrap", "a long state", false).states(&["on"]);
    assert_eq!(
        wider.text(),
        "wrap·a long state",
        "the shown state is never cut"
    );
}

#[test]
fn a_toggle_chip_is_bold_success_when_on_and_recedes_when_off() {
    let theme = Theme::default();
    let on = ToggleChip::new("wrap", "on", true).span(&theme);
    assert_eq!(
        on.style,
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    );
    let off = ToggleChip::new("wrap", "off", false).span(&theme);
    assert_eq!(
        off.style,
        Style::default().fg(theme.inert).add_modifier(theme.muted)
    );
}

#[test]
fn a_segmented_title_brackets_the_chosen_word_and_keeps_its_width() {
    let theme = Theme::default();
    let line = |segments: &[(&str, bool)]| -> String {
        SegmentedTitle::new(segments)
            .spans(&theme)
            .iter()
            .map(|span| span.content.to_string())
            .collect()
    };
    let first = line(&[("One", true), ("Two", false)]);
    let second = line(&[("One", false), ("Two", true)]);
    assert_eq!(first, "[ One ] Two");
    assert_eq!(second, "One [ Two ]");
    assert_eq!(text::width(&first), text::width(&second));
    assert_eq!(
        line(&[("A", false), ("B", false), ("C", true)]),
        "A B [ C ]"
    );
}

#[test]
fn a_segmented_title_draws_the_chosen_word_in_the_accent_chip_look() {
    let mut theme = Theme::default();
    theme.accent = Color::Magenta;
    let spans = SegmentedTitle::new(&[("One", false), ("Two", true)]).spans(&theme);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].style, Style::default());
    assert_eq!(
        spans[2].style,
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(theme.badge)
    );
}

#[test]
fn a_position_chip_counts_from_one_inside_the_content() {
    assert_eq!(PositionChip::new(0, 10).text(), "[ 1/10 ]");
    assert_eq!(PositionChip::new(9, 10).text(), "[ 10/10 ]");
    assert_eq!(PositionChip::new(42, 10).text(), "[ 10/10 ]");
    assert_eq!(PositionChip::new(3, 0).text(), "[ 1/1 ]");
    assert_eq!(PositionChip::new(usize::MAX, 5).text(), "[ 5/5 ]");
    let span = PositionChip::new(0, 2).span();
    assert_eq!(span.style, Style::default().add_modifier(Modifier::BOLD));
}

#[test]
fn each_tone_styles_from_its_theme_token() {
    let theme = Theme::default();
    let tones = [
        (Tone::Muted, Style::default().add_modifier(theme.muted)),
        (
            Tone::Inert,
            Style::default().fg(theme.inert).add_modifier(theme.muted),
        ),
        (Tone::Accent, Style::default().fg(theme.accent)),
        (Tone::Success, Style::default().fg(theme.success)),
        (Tone::Warning, Style::default().fg(theme.warning)),
        (Tone::Error, Style::default().fg(theme.error)),
    ];
    for (tone, style) in tones {
        assert_eq!(tone.style(&theme), style, "{tone:?}");
        assert_eq!(StatusChip::new("x", tone).span(&theme).style, style);
    }
}

#[test]
fn a_status_chip_takes_a_mark_bold_and_padding() {
    let theme = Theme::default();
    let chip = StatusChip::new("2 unread", Tone::Error)
        .mark(marks::WARNING)
        .bold()
        .pad(0, 2);
    assert_eq!(chip.text(), format!("{} 2 unread  ", marks::WARNING));
    assert_eq!(
        chip.span(&theme).style,
        Style::default()
            .fg(theme.error)
            .add_modifier(Modifier::BOLD)
    );
    assert_eq!(chip.tone(), Tone::Error);
    assert_eq!(
        StatusChip::new("note", Tone::Inert).pad(2, 1).text(),
        "  note "
    );
}

#[test]
fn a_busy_chip_leads_with_the_spinner_frame_in_bold_accent() {
    let theme = Theme::default();
    let chip = StatusChip::busy("saving", Duration::from_millis(130));
    assert_eq!(chip.text(), format!("{} saving", marks::SPINNER[1]));
    assert_eq!(
        chip.span(&theme).style,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    );
}

#[test]
fn the_spinner_advances_every_120_milliseconds_and_starts_over() {
    let frame = |ms| spinner_frame(Duration::from_millis(ms));
    assert_eq!(frame(0), marks::SPINNER[0]);
    assert_eq!(frame(119), marks::SPINNER[0]);
    assert_eq!(frame(120), marks::SPINNER[1]);
    assert_eq!(frame(7 * 120), marks::SPINNER[7]);
    assert_eq!(frame(8 * 120), marks::SPINNER[0]);
    assert_eq!(frame(8 * 120 * 1000 + 360), marks::SPINNER[3]);
}
