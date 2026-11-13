use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// A theme that draws no color and no modifier at all: what a list looks
/// like on a terminal that shows neither.
fn colorless() -> Theme {
    let mut theme = Theme::default();
    theme.selection_active = Modifier::empty();
    theme.selection_inactive = Modifier::empty();
    theme.muted = Modifier::empty();
    theme.inert = Color::Reset;
    theme
}

/// A list of `items` with the cursor on `cursor`, drawn into a buffer.
fn draw(items: &[&str], cursor: usize, focused: bool, theme: &Theme) -> Buffer {
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            row(
                [Span::raw(*item)],
                Cursor::of(index == cursor, focused),
                theme,
            )
        })
        .collect();
    let area = Rect::new(0, 0, 20, u16::try_from(items.len()).unwrap_or(0));
    let mut buf = Buffer::empty(area);
    Paragraph::new(lines).render(area, &mut buf);
    buf
}

/// The symbols of one buffer row, as plain text.
fn text_of(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn the_gutter_is_two_cells_with_the_pointer_only_on_the_cursor_row() {
    assert_eq!(gutter(Cursor::Off).content, "  ");
    for cursor in [Cursor::Active, Cursor::Inactive] {
        let marked = gutter(cursor);
        assert!(marked.content.starts_with(marks::POINTER), "{cursor:?}");
        assert_eq!(text::width(&marked.content), GUTTER_WIDTH, "{cursor:?}");
    }
    assert_eq!(text::width(&gutter(Cursor::Off).content), GUTTER_WIDTH);
    assert_eq!(gutter(Cursor::Active).style, Style::default());
}

#[test]
fn a_cursor_takes_the_active_pair_when_focused_and_the_inactive_one_when_not() {
    let theme = Theme::default();
    assert_eq!(
        Cursor::of(true, true).modifier(&theme),
        theme.selection_active
    );
    assert_eq!(
        Cursor::of(true, false).modifier(&theme),
        theme.selection_inactive
    );
    assert_eq!(Cursor::of(false, true).modifier(&theme), Modifier::empty());
    assert_eq!(Cursor::of(false, false), Cursor::Off);
    assert!(!Cursor::Off.is_on());
    assert_eq!(
        Cursor::Active.style(&theme),
        Style::default().add_modifier(theme.selection_active)
    );
}

#[test]
fn the_highlight_follows_the_theme() {
    let mut theme = Theme::default();
    theme.selection_active = Modifier::UNDERLINED;
    theme.selection_inactive = Modifier::ITALIC;
    let active = row([Span::raw("a")], Cursor::Active, &theme);
    let inactive = row([Span::raw("a")], Cursor::Inactive, &theme);
    for span in &active.spans {
        assert_eq!(span.style.add_modifier, Modifier::UNDERLINED);
    }
    for span in &inactive.spans {
        assert_eq!(span.style.add_modifier, Modifier::ITALIC);
    }
}

#[test]
fn a_row_off_the_cursor_is_left_as_it_was() {
    let theme = Theme::default();
    let styled = Span::styled("name", Style::default().fg(Color::Green));
    let line = row([styled.clone()], Cursor::Off, &theme);
    assert_eq!(line.spans, vec![Span::raw("  "), styled]);
}

#[test]
fn the_highlight_covers_every_span_but_keeps_their_colors_and_filled_spans() {
    let theme = Theme::default();
    let colored = Span::styled("name", Style::default().fg(Color::Green));
    let muted = Span::styled(" note", Style::default().add_modifier(theme.muted));
    let chip = Span::styled(
        " tag ",
        Style::default().fg(Color::Blue).add_modifier(theme.badge),
    );
    let fill = Span::styled(
        "value",
        Style::default().fg(Color::White).bg(Color::DarkGray),
    );
    let line = row(
        [colored, muted, chip.clone(), fill.clone()],
        Cursor::Active,
        &theme,
    );
    let mods = |index: usize| line.spans[index].style.add_modifier;
    assert!(
        mods(0).contains(theme.selection_active),
        "the gutter is highlighted"
    );
    assert!(mods(1).contains(theme.selection_active));
    assert_eq!(
        line.spans[1].style.fg,
        Some(Color::Green),
        "colors are kept"
    );
    assert!(
        mods(2).contains(theme.selection_active | theme.muted),
        "muted text stays muted"
    );
    assert_eq!(line.spans[3], chip, "a reversed chip keeps its look");
    assert_eq!(line.spans[4], fill, "a background fill keeps its look");
}

#[test]
fn highlight_alone_restyles_spans_without_a_gutter() {
    let theme = Theme::default();
    let mut spans = vec![Span::raw("a"), Span::raw("b")];
    highlight(&mut spans, Cursor::Inactive, &theme);
    assert_eq!(spans.len(), 2);
    for span in &spans {
        assert_eq!(span.style.add_modifier, theme.selection_inactive);
    }
    let mut untouched = vec![Span::raw("a")];
    highlight(&mut untouched, Cursor::Off, &theme);
    assert_eq!(untouched, vec![Span::raw("a")]);
}

#[test]
fn moving_the_cursor_moves_no_text() {
    let theme = Theme::default();
    let items = ["first", "second", "third"];
    let before = draw(&items, 0, true, &theme);
    let after = draw(&items, 2, true, &theme);
    for y in 0..3 {
        let (a, b) = (text_of(&before, y), text_of(&after, y));
        assert_eq!(
            a.chars().skip(GUTTER_WIDTH).collect::<String>(),
            b.chars().skip(GUTTER_WIDTH).collect::<String>(),
            "row {y}"
        );
    }
}

#[test]
fn the_selected_row_is_highlighted_from_its_gutter_to_its_last_cell() {
    let theme = Theme::default();
    let buf = draw(&["first", "second", "third"], 1, true, &theme);
    let text_cells = GUTTER_WIDTH + "second".len();
    for x in 0..text_cells {
        let cell = &buf[(u16::try_from(x).unwrap_or(0), 1)];
        assert!(
            cell.modifier.contains(theme.selection_active),
            "cell {x} of the cursor row"
        );
    }
    for y in [0, 2] {
        for x in 0..buf.area.width {
            assert!(buf[(x, y)].modifier.is_empty(), "row {y} cell {x}");
        }
    }
}

#[test]
fn an_unfocused_list_still_marks_its_cursor_row_with_the_inactive_pair() {
    let theme = Theme::default();
    let buf = draw(&["first", "second"], 1, false, &theme);
    assert!(text_of(&buf, 1).starts_with(marks::POINTER));
    assert_eq!(buf[(2, 1)].modifier, theme.selection_inactive);
    assert!(!buf[(2, 1)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn without_color_or_modifiers_the_marker_alone_shows_the_cursor_row() {
    let theme = colorless();
    for focused in [true, false] {
        let buf = draw(&["alpha", "beta", "gamma"], 1, focused, &theme);
        let marked: Vec<u16> = (0..3)
            .filter(|y| text_of(&buf, *y).starts_with(marks::POINTER))
            .collect();
        assert_eq!(marked, vec![1], "focused: {focused}");
        for y in 0..3 {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                assert!(cell.modifier.is_empty() && cell.fg == Color::Reset);
            }
        }
    }
}

#[test]
fn a_wrapping_list_goes_round_at_both_ends() {
    assert_eq!(Ends::Wrap.step(0, 5, -1), 4);
    assert_eq!(Ends::Wrap.step(4, 5, 1), 0);
    assert_eq!(Ends::Wrap.step(2, 5, 1), 3);
    assert_eq!(Ends::Wrap.step(0, 5, -6), 4);
    assert_eq!(Ends::Wrap.step(0, 1, 1), 0);
    assert_eq!(Ends::Wrap.step(3, 0, 1), 0, "an empty list");
}

#[test]
fn a_clamping_list_stops_at_both_ends() {
    assert_eq!(Ends::Clamp.step(0, 5, -1), 0);
    assert_eq!(Ends::Clamp.step(4, 5, 1), 4);
    assert_eq!(Ends::Clamp.step(2, 5, 1), 3);
    assert_eq!(Ends::Clamp.step(2, 5, 40), 4, "a page past the end");
    assert_eq!(Ends::Clamp.step(2, 5, -40), 0);
    assert_eq!(
        Ends::Clamp.step(9, 5, 0),
        4,
        "an index past the end comes back"
    );
    assert_eq!(Ends::Clamp.step(3, 0, 1), 0, "an empty list");
}

#[test]
fn the_ghost_look_dims_text_and_keeps_a_chip_filled() {
    let theme = Theme::default();
    assert_eq!(
        ghost_style(&theme),
        Style::default()
            .fg(theme.inert)
            .add_modifier(Modifier::ITALIC)
    );
    let mut spans = vec![
        Span::styled(
            "name",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " tag ",
            Style::default().fg(Color::Blue).add_modifier(theme.badge),
        ),
        Span::styled("fill", Style::default().bg(Color::DarkGray)),
    ];
    ghost(&mut spans, &theme);
    assert_eq!(spans[0].style.fg, Some(theme.inert));
    assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    assert!(spans[0].style.sub_modifier.contains(Modifier::BOLD));
    assert_eq!(
        spans[1].style.fg,
        Some(Color::Blue),
        "the chip keeps its color"
    );
    assert!(
        spans[1]
            .style
            .add_modifier
            .contains(theme.badge | theme.muted | Modifier::ITALIC)
    );
    assert_eq!(spans[2].style.bg, Some(Color::DarkGray));
}

#[test]
fn a_selected_ghost_row_is_still_a_ghost() {
    let theme = Theme::default();
    let mut spans = vec![Span::raw("gone")];
    ghost(&mut spans, &theme);
    highlight(&mut spans, Cursor::Active, &theme);
    assert_eq!(spans[0].style.fg, Some(theme.inert));
    assert!(
        spans[0]
            .style
            .add_modifier
            .contains(Modifier::ITALIC | theme.selection_active)
    );
}

#[test]
fn the_pending_look_recedes_and_keeps_colors() {
    let theme = Theme::default();
    let mut spans = vec![
        Span::styled("name", Style::default().fg(Color::Green)),
        Span::styled(" tag ", Style::default().bg(Color::Blue)),
    ];
    pending(&mut spans, &theme);
    for span in &spans {
        assert!(span.style.add_modifier.contains(theme.muted));
    }
    assert_eq!(spans[0].style.fg, Some(Color::Green));
    assert_eq!(spans[1].style.bg, Some(Color::Blue));
}
