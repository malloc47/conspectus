use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::*;
use crate::text::strip_ansi;

#[test]
fn a_style_change_writes_its_sequence_and_every_row_ends_in_a_reset() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 2));
    buffer.set_string(0, 0, "abc", Style::default().fg(Color::Red));
    buffer.set_string(0, 1, "xyz", Style::default());
    assert_eq!(
        to_ansi(&buffer, buffer.area),
        "\x1b[0m\x1b[31mabc\x1b[0m\n\x1b[0mxyz\x1b[0m"
    );
}

#[test]
fn a_region_is_clipped_to_the_buffer() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
    buffer.set_string(0, 0, "abcd", Style::default());
    buffer.set_string(0, 1, "efgh", Style::default());
    let out = to_ansi(&buffer, Rect::new(2, 1, 9, 9));
    assert_eq!(strip_ansi(&out), "gh");
}

#[test]
fn named_colors_take_their_own_codes_and_others_the_long_forms() {
    let cases = [
        (Color::Black, "\x1b[30m", "\x1b[40m"),
        (Color::Gray, "\x1b[37m", "\x1b[47m"),
        (Color::DarkGray, "\x1b[90m", "\x1b[100m"),
        (Color::White, "\x1b[97m", "\x1b[107m"),
        (Color::LightCyan, "\x1b[96m", "\x1b[106m"),
        (Color::Indexed(208), "\x1b[38;5;208m", "\x1b[48;5;208m"),
        (Color::Rgb(1, 2, 3), "\x1b[38;2;1;2;3m", "\x1b[48;2;1;2;3m"),
    ];
    for (color, fg, bg) in cases {
        assert_eq!(sequence(Style::new().fg(color)), format!("\x1b[0m{fg}"));
        assert_eq!(sequence(Style::new().bg(color)), format!("\x1b[0m{bg}"));
    }
    assert_eq!(sequence(Style::new().fg(Color::Reset)), "\x1b[0m");
}

#[test]
fn every_modifier_is_written_in_code_order() {
    let all = Modifier::all();
    assert_eq!(
        sequence(Style::new().add_modifier(all)),
        "\x1b[0m\x1b[1m\x1b[2m\x1b[3m\x1b[4m\x1b[5m\x1b[6m\x1b[7m\x1b[8m\x1b[9m"
    );
    assert_eq!(
        sequence(
            Style::new()
                .fg(Color::Yellow)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC)
        ),
        "\x1b[0m\x1b[33m\x1b[44m\x1b[1m\x1b[3m"
    );
    for (name, _) in all.iter_names() {
        assert!(
            MODIFIER_CODES.iter().any(|(known, _)| *known == name),
            "{name} has no code"
        );
    }
}

#[test]
fn a_run_of_one_style_writes_one_sequence() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
    let bold = Style::default().add_modifier(Modifier::BOLD);
    buffer.set_string(0, 0, "aaa", bold);
    buffer.set_string(3, 0, "bbb", bold);
    let out = to_ansi(&buffer, buffer.area);
    assert_eq!(out.matches("\x1b[1m").count(), 1, "{out:?}");
}
