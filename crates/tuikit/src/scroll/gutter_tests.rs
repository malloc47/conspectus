use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::*;

const THUMB: &str = "\u{2588}";

fn column(buf: &Buffer, x: u16) -> Vec<String> {
    (buf.area.y..buf.area.bottom())
        .map(|y| buf[(x, y)].symbol().to_string())
        .collect()
}

fn drawn(area: Rect, window: ScrollWindow) -> Buffer {
    let gutter = ScrollGutter::layout(area, window.content());
    let mut buf = Buffer::empty(area);
    gutter.render(&window, &mut buf);
    buf
}

#[test]
fn rows_that_fit_keep_every_column() {
    let area = Rect::new(2, 1, 20, 6);
    let gutter = ScrollGutter::layout(area, 6);
    assert_eq!(gutter.content(), area);
    assert_eq!(gutter.bar(), None);
    assert_eq!(ScrollGutter::layout(area, 0).bar(), None);
}

#[test]
fn rows_that_overflow_give_up_the_right_column() {
    let area = Rect::new(2, 1, 20, 6);
    let gutter = ScrollGutter::layout(area, 7);
    assert_eq!(gutter.content(), Rect::new(2, 1, 19, 6));
    assert_eq!(gutter.bar(), Some(Rect::new(21, 1, 1, 6)));
}

#[test]
fn an_area_narrower_than_two_columns_has_no_gutter() {
    let area = Rect::new(0, 0, 1, 4);
    assert_eq!(ScrollGutter::layout(area, 40).bar(), None);
    assert_eq!(ScrollGutter::layout(area, 40).content(), area);
}

#[test]
fn the_bar_draws_only_in_the_gutter_inside_its_margins() {
    let area = Rect::new(0, 0, 10, 8);
    let buf = drawn(area, ScrollWindow::new(8, 30).at(11));
    for x in 0..9 {
        assert!(
            column(&buf, x).iter().all(|cell| cell == " "),
            "column {x} was drawn on"
        );
    }
    let bar = column(&buf, 9);
    assert_eq!(bar[0], " ", "the top margin stays clear");
    assert_eq!(bar[7], " ", "the bottom margin stays clear");
    assert!(bar[1..7].iter().all(|cell| cell != " "), "{bar:?}");
}

#[test]
fn the_thumb_reaches_the_bottom_at_the_last_offset() {
    let area = Rect::new(0, 0, 4, 12);
    let window = ScrollWindow::new(12, 40);
    let bottom = drawn(area, window.at(window.last_offset()));
    let thumb: Vec<usize> = column(&bottom, 3)
        .iter()
        .enumerate()
        .filter_map(|(y, cell)| (cell == THUMB).then_some(y))
        .collect();
    // The track is rows 1..=10; its last row before the end arrow is 9.
    assert_eq!(thumb.last(), Some(&9), "{:?}", column(&bottom, 3));
    let top = drawn(area, window);
    let first = column(&top, 3).iter().position(|cell| cell == THUMB);
    assert_eq!(first, Some(2), "{:?}", column(&top, 3));
}

#[test]
fn a_window_that_fits_draws_no_bar() {
    let area = Rect::new(0, 0, 10, 8);
    let gutter = ScrollGutter::layout(area, 30);
    let mut buf = Buffer::empty(area);
    gutter.render(&ScrollWindow::new(8, 8), &mut buf);
    gutter.render(&ScrollWindow::new(0, 30), &mut buf);
    assert_eq!(buf, Buffer::empty(area));
}

#[test]
fn the_bar_drops_a_reversed_highlight_beneath_it() {
    let area = Rect::new(0, 0, 6, 8);
    let mut buf = Buffer::empty(area);
    buf.set_style(area, Style::default().add_modifier(Modifier::REVERSED));
    let window = ScrollWindow::new(8, 20).at(4);
    ScrollGutter::layout(area, 20).render(&window, &mut buf);
    for y in 1..7 {
        assert!(
            !buf[(5, y)].modifier.contains(Modifier::REVERSED),
            "row {y} of the bar kept the highlight"
        );
    }
    assert!(buf[(4, 3)].modifier.contains(Modifier::REVERSED));
}
