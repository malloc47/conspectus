use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};

use super::*;
use crate::testkit::buffer::rows as drawn_rows;

fn region(anchor: Anchor, keys: &[i32]) -> ScrollRegion {
    let mut region = ScrollRegion::new(anchor);
    for &delta in keys {
        region.scroll_by(delta);
    }
    region
}

fn numbered(count: usize) -> Text<'static> {
    Text::from(
        (0..count)
            .map(|n| Line::from(format!("row {n}")))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn from_the_top_a_step_up_stops_at_the_first_row_at_once() {
    let top = region(Anchor::Top, &[1, 1, -1, -1, -1]);
    assert_eq!(top.moved(), 0);
    assert_eq!(region(Anchor::Top, &[20, -5]).moved(), 15);
    assert_eq!(region(Anchor::Top, &[20, -1000]).moved(), 0);
}

#[test]
fn from_the_bottom_a_step_down_stops_at_the_last_row_at_once() {
    let bottom = region(Anchor::Bottom, &[-12, 40]);
    assert_eq!(bottom.moved(), 0);
    let up = region(Anchor::Bottom, &[1, 1, 1, -1]);
    assert_eq!(up.moved(), -1, "a step down at the end does nothing");
    assert_eq!(up.away(), 1);
}

#[test]
fn a_mark_scrolls_either_way() {
    assert_eq!(region(Anchor::Mark, &[-3]).moved(), -3);
    assert_eq!(region(Anchor::Mark, &[3]).moved(), 3);
}

#[test]
fn placing_from_the_top_shows_the_rows_moved_down() {
    let placed = region(Anchor::Top, &[4]).place(Anchor::Top, 5, 20);
    assert_eq!(placed.window().visible(), 4..9);
    assert_eq!(placed.moved(), 4);
}

#[test]
fn placing_from_the_bottom_keeps_the_distance_from_the_end_as_rows_arrive() {
    let placed = region(Anchor::Bottom, &[-2]).place(Anchor::Bottom, 5, 20);
    assert_eq!(placed.window().visible(), 13..18);
    let grown = placed.place(Anchor::Bottom, 5, 26);
    assert_eq!(grown.window().visible(), 19..24);
    assert_eq!(grown.moved(), -2);
}

#[test]
fn placing_at_a_mark_puts_it_at_the_top_and_moves_from_it() {
    let at = ScrollRegion::new(Anchor::Mark)
        .marked(7)
        .place(Anchor::Mark, 5, 20);
    assert_eq!(at.window().offset(), 7);
    let mut moved = at;
    moved.scroll_by(-2);
    assert_eq!(moved.place(Anchor::Mark, 5, 20).window().offset(), 5);
    moved.scroll_by(5);
    assert_eq!(moved.place(Anchor::Mark, 5, 20).window().offset(), 10);
}

#[test]
fn a_mark_on_the_last_page_opens_at_the_end_and_the_first_step_up_moves() {
    // The mark is past the last offset, so the window shows the end.
    let placed = ScrollRegion::new(Anchor::Mark)
        .marked(18)
        .place(Anchor::Mark, 5, 20);
    assert_eq!(placed.window().offset(), 15);
    let mut up = placed;
    up.scroll_by(-1);
    assert_eq!(up.place(Anchor::Mark, 5, 20).window().offset(), 14);
}

#[test]
fn a_step_past_the_far_end_is_taken_back_by_the_next_placement() {
    let mut past = region(Anchor::Top, &[100]).place(Anchor::Top, 5, 20);
    assert_eq!(past.moved(), 15);
    past.scroll_by(-1);
    assert_eq!(past.place(Anchor::Top, 5, 20).window().offset(), 14);

    let mut above = region(Anchor::Bottom, &[-100]).place(Anchor::Bottom, 5, 20);
    assert_eq!(above.window().offset(), 0);
    above.scroll_by(1);
    assert_eq!(above.place(Anchor::Bottom, 5, 20).window().offset(), 1);

    let mut beyond = ScrollRegion::new(Anchor::Mark)
        .marked(3)
        .place(Anchor::Mark, 5, 20);
    beyond.scroll_by(50);
    let beyond = beyond.place(Anchor::Mark, 5, 20);
    assert_eq!(beyond.window().offset(), 15);
    let mut back = beyond;
    back.scroll_by(-1);
    assert_eq!(back.place(Anchor::Mark, 5, 20).window().offset(), 14);
}

#[test]
fn content_that_fits_shows_from_its_first_row_whatever_the_anchor() {
    for anchor in [Anchor::Top, Anchor::Bottom, Anchor::Mark] {
        let placed = region(anchor, &[-3, 2]).marked(2).place(anchor, 10, 4);
        assert_eq!(placed.window().visible(), 0..4, "{anchor:?}");
    }
}

#[test]
fn another_anchor_starts_at_that_anchor() {
    let mut region = region(Anchor::Bottom, &[-5]);
    region.set_anchor(Anchor::Bottom);
    assert_eq!(region.moved(), -5, "the same anchor keeps its place");
    region.set_anchor(Anchor::Mark);
    assert_eq!((region.anchor(), region.moved()), (Anchor::Mark, 0));

    let placed = self::region(Anchor::Bottom, &[-5]).place(Anchor::Top, 5, 20);
    assert_eq!(placed.anchor(), Anchor::Top);
    assert_eq!(
        placed.window().offset(),
        0,
        "placed from the top, not 5 rows up"
    );
}

#[test]
fn reset_goes_back_to_the_anchor() {
    let mut region = region(Anchor::Bottom, &[-5]);
    region.reset();
    assert_eq!(region.moved(), 0);
    assert_eq!(region.anchor(), Anchor::Bottom);
}

#[test]
fn draw_lines_shows_the_window_and_the_scrollbar() {
    let area = Rect::new(0, 0, 12, 6);
    let mut buf = Buffer::empty(area);
    let drawn = region(Anchor::Top, &[3]).draw_lines(Anchor::Top, area, &mut buf, numbered(10));
    assert_eq!(drawn.window().visible(), 3..9);
    let rows = drawn_rows(&buf);
    assert!(rows[0].starts_with("row 3"), "{rows:?}");
    assert!(rows[5].starts_with("row 8"), "{rows:?}");
    // The rows overflow, so the right column is the scrollbar's.
    assert!(rows.iter().all(|row| row.chars().count() == 12));
    assert_ne!(buf[(11, 2)].symbol(), " ");
}

#[test]
fn draw_lines_from_the_bottom_shows_the_newest_lines() {
    let area = Rect::new(0, 0, 12, 3);
    let mut buf = Buffer::empty(area);
    let drawn =
        ScrollRegion::new(Anchor::Bottom).draw_lines(Anchor::Bottom, area, &mut buf, numbered(10));
    assert_eq!(drawn.window().visible(), 7..10);
    assert!(drawn_rows(&buf)[2].starts_with("row 9"));
}

#[test]
fn draw_lines_counts_a_mark_from_its_lines_first_row() {
    // Each line wraps onto two rows at this width, so line 4 starts at row 8.
    let text = Text::from(
        (0..6)
            .map(|n| Line::from(format!("line {n} wraps")))
            .collect::<Vec<_>>(),
    );
    let area = Rect::new(0, 0, 8, 3);
    let mut buf = Buffer::empty(area);
    let drawn =
        ScrollRegion::new(Anchor::Mark)
            .marked(4)
            .draw_lines(Anchor::Mark, area, &mut buf, text);
    assert_eq!(drawn.window().offset(), 8);
    assert_eq!(drawn.mark(), 4, "the mark stays a line");
    assert!(drawn_rows(&buf)[0].starts_with("line 4"));
}

#[test]
fn draw_lines_scrolls_a_banner_with_the_lines() {
    let mut text = Text::from(Line::from("banner"));
    text.extend(numbered(10));
    let area = Rect::new(0, 0, 12, 4);
    let mut buf = Buffer::empty(area);
    region(Anchor::Top, &[1]).draw_lines(Anchor::Top, area, &mut buf, text);
    assert!(
        drawn_rows(&buf)[0].starts_with("row 0"),
        "the banner scrolled away"
    );
}

#[test]
fn draw_cut_keeps_the_banner_above_the_window_and_tracks_the_window() {
    let placed = region(Anchor::Bottom, &[-2]).place(Anchor::Bottom, 5, 10);
    let mut text = Text::from(Line::from("banner"));
    text.extend(
        placed
            .window()
            .visible()
            .map(|n| Line::from(format!("row {n}"))),
    );
    let area = Rect::new(0, 0, 12, 6);
    let mut buf = Buffer::empty(area);
    let drawn = placed.draw_cut(area, &mut buf, text);
    assert_eq!(drawn, placed);
    let rows = drawn_rows(&buf);
    assert!(rows[0].starts_with("banner"), "{rows:?}");
    assert!(rows[1].starts_with("row 3"), "{rows:?}");
    assert!(rows[5].starts_with("row 7"), "{rows:?}");
    // The window's rows overflow it, so the scrollbar shows.
    assert_ne!(buf[(11, 2)].symbol(), " ");
}

#[test]
fn rows_under_leaves_a_body_at_least_one_row() {
    let banner = [Line::from("a banner that wraps")];
    assert_eq!(rows_under(&banner, 10, 6), 4);
    assert_eq!(rows_under(&banner, 40, 6), 5);
    assert_eq!(rows_under(&banner, 10, 2), 1);
    assert_eq!(rows_under(&[], 10, 6), 6);
}
