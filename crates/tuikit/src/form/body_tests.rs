use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::testkit::buffer::rows;

fn lines(prefix: &str, count: usize) -> Vec<Line<'static>> {
    (0..count)
        .map(|n| Line::from(format!("{prefix} {n}")))
        .collect()
}

/// A body of six rows with every band, the cursor on `cursor`.
fn body(cursor: usize) -> FormBody<'static> {
    FormBody::new(lines("row", 6), cursor)
        .top(lines("top", 2))
        .notes(lines("note", 1))
        .error(Some(Line::from("error")))
        .footer(lines("footer", 2))
}

/// What a `body` draws on a screen `height` rows tall, trimmed.
fn drawn(body: FormBody<'static>, height: u16) -> Vec<String> {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, height));
    let _ = body.render(buf.area, &mut buf);
    rows(&buf)
        .into_iter()
        .map(|row| row.trim_end().to_string())
        .collect()
}

#[test]
fn the_body_asks_for_a_row_per_line_of_every_band() {
    assert_eq!(body(0).measure(20), 2 + 6 + 1 + 1 + 2);
    assert_eq!(FormBody::new(lines("row", 3), 0).measure(20), 3);
}

#[test]
fn with_room_every_band_is_drawn_in_order() {
    assert_eq!(
        drawn(body(0), 12),
        [
            "top 0", "top 1", "row 0", "row 1", "row 2", "row 3", "row 4", "row 5", "note 0",
            "error", "footer 0", "footer 1",
        ]
    );
}

#[test]
fn on_a_short_screen_only_the_rows_scroll_and_follow_the_cursor() {
    assert_eq!(
        drawn(body(5), 9),
        [
            "top 0", "top 1", "row 3", "row 4", "row 5", "note 0", "error", "footer 0", "footer 1",
        ]
    );
    assert_eq!(
        drawn(body(0), 9),
        [
            "top 0", "top 1", "row 0", "row 1", "row 2", "note 0", "error", "footer 0", "footer 1",
        ]
    );
}

#[test]
fn a_tiny_screen_keeps_the_cursor_row_the_error_and_the_footer_first() {
    // Six rows: the notes come before the top band, and the top band
    // before the other rows.
    assert_eq!(
        drawn(body(3), 6),
        ["top 0", "row 3", "note 0", "error", "footer 0", "footer 1"]
    );
    // Four rows: the cursor's row, the error, and both footer lines.
    assert_eq!(
        drawn(body(3), 4),
        ["row 3", "error", "footer 0", "footer 1"]
    );
    // Two rows: the cursor's row and the error, so a refusal still shows.
    assert_eq!(drawn(body(3), 2), ["row 3", "error"]);
    // One row: the cursor's row.
    assert_eq!(drawn(body(3), 1), ["row 3"]);
}

#[test]
fn the_window_it_returns_is_the_rows_it_showed() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 9));
    let window = body(5).render(buf.area, &mut buf);
    assert_eq!(window.visible(), 3..6);
    assert_eq!(window.content(), 6);
}

#[test]
fn a_body_without_rows_draws_its_bands() {
    let body = FormBody::new(Vec::new(), 0).footer(lines("footer", 1));
    assert_eq!(drawn(body, 3), ["", "", "footer 0"]);
}

#[test]
fn a_taller_area_leaves_the_space_between_the_rows_and_the_footer() {
    let body = FormBody::new(lines("row", 2), 0).footer(lines("footer", 1));
    assert_eq!(drawn(body, 5), ["row 0", "row 1", "", "", "footer 0"]);
}
