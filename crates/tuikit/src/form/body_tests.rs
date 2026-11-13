use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::keys::{Footer, KeyRow};
use crate::testkit::buffer::rows;
use crate::theme::Theme;

/// A footer of two notes, which take two rows at 20 cells.
const KEYS: &[KeyRow<()>] = &[KeyRow::Note("footer zero"), KeyRow::Note("footer one")];

fn lines(prefix: &str, count: usize) -> Vec<Line<'static>> {
    (0..count)
        .map(|n| Line::from(format!("{prefix} {n}")))
        .collect()
}

fn footer(theme: &Theme) -> ModalFooter<'_> {
    ModalFooter::new(Footer::new(KEYS), theme)
}

/// A body of six rows with every band, the cursor on `cursor`.
fn body(cursor: usize, theme: &Theme) -> FormBody<'_> {
    FormBody::new(lines("row", 6), cursor)
        .top(lines("top", 2))
        .notes(lines("note", 1))
        .error(Some(Line::from("error")))
        .footer(footer(theme))
}

/// What a `body` draws on a screen `height` rows tall, trimmed.
fn drawn(body: FormBody<'_>, height: u16) -> Vec<String> {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, height));
    let _ = body.render(buf.area, &mut buf);
    rows(&buf)
        .into_iter()
        .map(|row| row.trim_end().to_string())
        .collect()
}

#[test]
fn the_body_asks_for_a_row_per_line_of_every_band() {
    let theme = Theme::default();
    // The footer asks for its blank row, the error line, and two key rows.
    assert_eq!(body(0, &theme).measure(20), 2 + 6 + 1 + 4);
    assert_eq!(FormBody::new(lines("row", 3), 0).measure(20), 3);
}

#[test]
fn with_room_every_band_is_drawn_in_order() {
    let theme = Theme::default();
    assert_eq!(
        drawn(body(0, &theme), 13),
        [
            "top 0",
            "top 1",
            "row 0",
            "row 1",
            "row 2",
            "row 3",
            "row 4",
            "row 5",
            "note 0",
            "",
            "error",
            "footer zero",
            "footer one",
        ]
    );
}

#[test]
fn on_a_short_screen_only_the_rows_scroll_and_follow_the_cursor() {
    let theme = Theme::default();
    let fixed = ["note 0", "", "error", "footer zero", "footer one"];
    let mut expected = vec!["top 0", "top 1", "row 3", "row 4", "row 5"];
    expected.extend(fixed);
    assert_eq!(drawn(body(5, &theme), 10), expected);
    let mut expected = vec!["top 0", "top 1", "row 0", "row 1", "row 2"];
    expected.extend(fixed);
    assert_eq!(drawn(body(0, &theme), 10), expected);
}

#[test]
fn a_tiny_screen_keeps_the_cursor_row_the_error_and_the_footer_first() {
    let theme = Theme::default();
    // Seven rows: the footer and its blank row, then the notes, then the
    // top band, before the other rows.
    assert_eq!(
        drawn(body(3, &theme), 7),
        [
            "top 0",
            "row 3",
            "note 0",
            "",
            "error",
            "footer zero",
            "footer one"
        ]
    );
    // Four rows: the blank row has gone, then the notes and the top band.
    assert_eq!(
        drawn(body(3, &theme), 4),
        ["row 3", "error", "footer zero", "footer one"]
    );
    // Three rows: the footer keeps its last row, the keys that leave.
    assert_eq!(drawn(body(3, &theme), 3), ["row 3", "error", "footer one"]);
    // Two rows: the cursor's row and the error, so a refusal still shows.
    assert_eq!(drawn(body(3, &theme), 2), ["row 3", "error"]);
    // One row: the cursor's row.
    assert_eq!(drawn(body(3, &theme), 1), ["row 3"]);
}

#[test]
fn without_an_error_the_footer_keeps_its_last_rows() {
    let theme = Theme::default();
    let body = FormBody::new(lines("row", 3), 0).footer(footer(&theme));
    assert_eq!(
        drawn(body.clone(), 4),
        ["row 0", "", "footer zero", "footer one"]
    );
    assert_eq!(
        drawn(body.clone(), 3),
        ["row 0", "footer zero", "footer one"]
    );
    assert_eq!(drawn(body, 2), ["row 0", "footer one"]);
}

#[test]
fn the_window_it_returns_is_the_rows_it_showed() {
    let theme = Theme::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
    let window = body(5, &theme).render(buf.area, &mut buf);
    assert_eq!(window.visible(), 3..6);
    assert_eq!(window.content(), 6);
}

#[test]
fn a_body_without_rows_draws_its_bands() {
    let theme = Theme::default();
    let body = FormBody::new(Vec::new(), 0).footer(footer(&theme));
    assert_eq!(drawn(body, 4), ["", "", "footer zero", "footer one"]);
}

#[test]
fn a_taller_area_leaves_the_space_between_the_rows_and_the_footer() {
    let theme = Theme::default();
    let body = FormBody::new(lines("row", 2), 0).footer(footer(&theme));
    assert_eq!(
        drawn(body, 6),
        ["row 0", "row 1", "", "", "footer zero", "footer one"]
    );
}

#[test]
fn a_body_without_a_footer_keeps_its_error_line() {
    let body = FormBody::new(lines("row", 2), 0).error(Some(Line::from("error")));
    assert_eq!(drawn(body, 3), ["row 0", "row 1", "error"]);
}
