use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::*;
use crate::testkit::render::{render, render_widget};

#[test]
fn text_ends_every_row_with_a_line_break() {
    let buf = render_widget(Paragraph::new("ab\ncd"), 3, 2);
    assert_eq!(text(&buf), "ab \ncd \n");
    assert_eq!(rows(&buf), ["ab ", "cd "]);
    assert_eq!(region_rows(&buf, Rect::new(1, 1, 5, 5)), ["d "]);
}

#[test]
fn tokens_carry_the_columns_they_cover() {
    let found = tokens(" one  two three ");
    let spans: Vec<(usize, usize, &str)> = found
        .iter()
        .map(|token| (token.start, token.end, token.text.as_str()))
        .collect();
    assert_eq!(spans, [(1, 4, "one"), (6, 9, "two"), (10, 15, "three")]);
    assert_eq!(tokens("tail")[0].end, 4);
    assert!(tokens("   ").is_empty());
}

#[test]
fn find_places_text_by_cell_even_after_a_wide_character() {
    let buf = render_widget(Paragraph::new("\u{6771}x find me"), 12, 1);
    assert_eq!(find(&buf, "find"), Some(Position::new(4, 0)));
    assert_eq!(find(&buf, "x"), Some(Position::new(2, 0)));
    assert_eq!(find(&buf, "absent"), None);
    assert_eq!(find(&buf, ""), None);
}

#[test]
fn find_all_lists_every_appearance_in_reading_order() {
    let buf = render_widget(Paragraph::new("a.a\n.a."), 3, 2);
    assert_eq!(
        find_all(&buf, "a"),
        [
            Position::new(0, 0),
            Position::new(2, 0),
            Position::new(1, 1)
        ]
    );
    assert_eq!(
        find_in(&buf, Rect::new(1, 0, 2, 2), "a"),
        Some(Position::new(2, 0))
    );
}

#[test]
fn the_probes_read_the_cell_text_starts_in() {
    let line = Line::from(vec![
        Span::raw("plain "),
        Span::styled("bold", Style::new().add_modifier(Modifier::BOLD)),
    ]);
    let buf = render_widget(Paragraph::new(line), 12, 1);
    assert_eq!(cell_at(&buf, "bold").map(Cell::symbol), Some("b"));
    let style = style_at(&buf, "bold").expect("a style");
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(
        !style_at(&buf, "plain")
            .unwrap()
            .add_modifier
            .contains(Modifier::BOLD)
    );
    let bold = count_cells(&buf, buf.area, |cell| {
        cell.modifier.contains(Modifier::BOLD)
    });
    assert_eq!(bold, 4);
}

#[test]
fn every_border_type_has_corners_and_sides_the_checks_know() {
    for kind in [
        BorderType::Plain,
        BorderType::Rounded,
        BorderType::Double,
        BorderType::Thick,
    ] {
        let buf = render_widget(Block::bordered().border_type(kind), 4, 3);
        for (x, y) in [(0, 0), (3, 0), (0, 2), (3, 2)] {
            assert!(is_corner(buf[(x, y)].symbol()), "{kind:?} at {x},{y}");
        }
        assert!(is_side(buf[(0, 1)].symbol()), "{kind:?}");
        assert!(!is_corner("x") && !is_side("x"));
    }
}

#[test]
fn frame_rect_finds_only_a_frame_the_modal_drew() {
    let screen = |frame: &mut ratatui::Frame<'_>| {
        frame.render_widget(Block::bordered(), frame.area());
    };
    let bare = render(20, 8, screen);
    let with = render(20, 8, |frame| {
        screen(frame);
        frame.render_widget(Block::bordered().title("modal"), Rect::new(4, 2, 9, 4));
    });
    assert_eq!(frame_rect(&with, &bare), Some(Rect::new(4, 2, 9, 4)));
    assert_eq!(
        frame_rect(&bare, &bare),
        None,
        "the screen's own border is not it"
    );
}

#[test]
fn inner_rows_read_a_scrollbar_cell_as_a_space() {
    let bar = ratatui::symbols::scrollbar::DOUBLE_VERTICAL;
    let buf = render(8, 4, |frame| {
        frame.render_widget(Block::bordered(), frame.area());
        frame.render_widget(Paragraph::new("words"), Rect::new(1, 1, 5, 1));
        frame.buffer_mut()[(6, 1)].set_symbol(bar.thumb);
        frame.buffer_mut()[(6, 2)].set_symbol("x");
    });
    assert_eq!(inner_rows(&buf, buf.area), ["words ", "     x"]);
}

#[test]
fn restyled_rows_are_the_rows_a_selection_repainted() {
    let draw = |selected: u16| {
        render(6, 3, move |frame| {
            frame.render_widget(Paragraph::new("a\nb\nc"), frame.area());
            let row = Rect::new(0, selected, 6, 1);
            frame
                .buffer_mut()
                .set_style(row, Style::new().add_modifier(Modifier::BOLD));
        })
    };
    assert_eq!(restyled_rows(&draw(0), &draw(1)), [0, 1]);
    assert!(restyled_rows(&draw(2), &draw(2)).is_empty());
}
