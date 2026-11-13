use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::frame::SectionRule;
use crate::glyphs::marks;
use crate::testkit::buffer::rows as buffer_rows;
use crate::theme::Theme;

fn pane(width: u16, height: u16) -> Rect {
    Rect::new(4, 2, width, height)
}

/// The list's, the divider's, and the detail's heights.
fn heights(layout: StackedMasterDetail, area: Rect, rows: usize) -> (u16, u16, u16) {
    let (list, divider, detail) = layout.areas(area, rows);
    (list.height, divider.height, detail.height)
}

/// `count` one-row lines, `item 0` and on.
fn items(count: usize) -> Vec<Line<'static>> {
    (0..count)
        .map(|n| Line::from(format!("item {n}")))
        .collect()
}

fn draw(
    area: Rect,
    lines: Vec<Line<'static>>,
    cursor_line: Option<usize>,
    window: ScrollWindow,
) -> (Buffer, Stacked) {
    let theme = Theme::default();
    let mut buf = Buffer::empty(area);
    let divider = SectionRule::new("Detail").line(usize::from(area.width), &theme);
    let drawn = StackedMasterDetail::new().render(
        area,
        &mut buf,
        GridLines { lines, cursor_line },
        divider,
        window,
    );
    (buf, drawn)
}

#[test]
fn the_detail_keeps_its_minimum_when_the_list_is_long() {
    let layout = StackedMasterDetail::new();
    for height in 10..60 {
        let (list, divider, detail) = heights(layout, pane(40, height), 500);
        assert_eq!(detail, 6, "at {height} rows");
        assert_eq!((list, divider), (height - 7, 1), "at {height} rows");
    }
}

#[test]
fn a_list_that_fits_takes_its_rows_and_the_detail_the_rest() {
    let layout = StackedMasterDetail::new();
    assert_eq!(heights(layout, pane(40, 30), 5), (5, 1, 24));
    assert_eq!(heights(layout, pane(40, 30), 23), (23, 1, 6));
    assert_eq!(
        heights(layout, pane(40, 30), 24),
        (23, 1, 6),
        "one more scrolls"
    );
}

#[test]
fn the_list_takes_its_minimum_however_few_its_rows() {
    let layout = StackedMasterDetail::new();
    for rows in 0..=3 {
        assert_eq!(
            heights(layout, pane(40, 30), rows),
            (3, 1, 26),
            "{rows} rows"
        );
    }
    assert_eq!(
        heights(layout.min_list(1), pane(40, 30), 0),
        (1, 1, 28),
        "a minimum of one"
    );
}

#[test]
fn a_short_pane_gives_the_detail_what_the_list_minimum_and_the_divider_leave() {
    let layout = StackedMasterDetail::new();
    // Ten rows hold the list's three, the divider, and the detail's six.
    assert_eq!(heights(layout, pane(40, 10), 500), (3, 1, 6));
    for (height, detail) in [(9, 5), (8, 4), (6, 2), (5, 1), (4, 0)] {
        assert_eq!(
            heights(layout, pane(40, height), 500),
            (3, 1, detail),
            "at {height} rows"
        );
        assert_eq!(
            heights(layout, pane(40, height), 1),
            (3, 1, detail),
            "a short list at {height} rows"
        );
    }
}

#[test]
fn an_area_shorter_than_the_list_minimum_keeps_the_divider_after_one_list_row() {
    let layout = StackedMasterDetail::new();
    assert_eq!(heights(layout, pane(40, 3), 50), (2, 1, 0));
    assert_eq!(heights(layout, pane(40, 2), 50), (1, 1, 0));
    assert_eq!(heights(layout, pane(40, 1), 50), (1, 0, 0));
    assert_eq!(heights(layout, pane(40, 0), 50), (0, 0, 0));
}

#[test]
fn the_minimums_are_parameters() {
    let layout = StackedMasterDetail::new().min_list(2).min_detail(3);
    assert_eq!(heights(layout, pane(40, 20), 500), (16, 1, 3));
    assert_eq!(heights(layout, pane(40, 20), 0), (2, 1, 17));
    assert_eq!(heights(layout, pane(40, 5), 500), (2, 1, 2));
    assert_eq!(StackedMasterDetail::default(), StackedMasterDetail::new());
}

#[test]
fn the_areas_stack_down_the_pane_at_its_width() {
    let area = pane(40, 30);
    let (list, divider, detail) = StackedMasterDetail::new().areas(area, 8);
    assert_eq!(list, Rect::new(4, 2, 40, 8));
    assert_eq!(divider, Rect::new(4, 10, 40, 1));
    assert_eq!(detail, Rect::new(4, 11, 40, 21));
}

#[test]
fn the_layout_width_leaves_the_scrollbar_column() {
    assert_eq!(StackedMasterDetail::layout_width(pane(40, 10)), 39);
    assert_eq!(StackedMasterDetail::layout_width(pane(2, 10)), 1);
    assert_eq!(StackedMasterDetail::layout_width(pane(1, 10)), 1);
    assert_eq!(StackedMasterDetail::layout_width(pane(0, 10)), 1);
}

#[test]
fn a_rule_laid_out_at_the_layout_width_stays_one_row_when_the_bar_appears() {
    let area = pane(30, 16);
    let theme = Theme::default();
    let width = StackedMasterDetail::layout_width(area);
    let mut lines = vec![
        SectionRule::new("Links")
            .summary("20 shown")
            .line(width, &theme),
    ];
    lines.extend(items(20));
    let (buf, drawn) = draw(area, lines, Some(0), ScrollWindow::default());
    let rows = buffer_rows(&buf);
    assert!(drawn.window.overflows(), "the list scrolls");
    let content = |row: &str| row.chars().take(width).collect::<String>();
    assert_eq!(
        content(&rows[0]),
        format!("{} Links  20 shown ──", marks::RULE.repeat(10)),
        "the rule fills the columns the bar leaves"
    );
    assert_eq!(
        content(&rows[1]).trim_end(),
        "item 0",
        "the rule took one row"
    );
    assert_ne!(rows[1].chars().last(), Some(' '), "the bar: {rows:?}");
}

#[test]
fn a_line_as_wide_as_the_area_takes_one_row_while_the_list_fits() {
    let area = pane(12, 16);
    let lines = vec![Line::from("a".repeat(12)), Line::from("b")];
    let (buf, drawn) = draw(area, lines, Some(1), ScrollWindow::default());
    let rows = buffer_rows(&buf);
    assert_eq!(rows[0], "a".repeat(12));
    assert_eq!(rows[1].trim_end(), "b");
    assert_eq!(drawn.list.height, 3, "two rows, the minimum");
    assert!(!drawn.window.overflows());
}

#[test]
fn lines_that_overflow_wrap_one_column_narrower() {
    let area = pane(12, 12);
    // Twelve cells wrap onto two rows at the eleven the gutter leaves.
    let mut lines = vec![Line::from("a".repeat(12))];
    lines.extend(items(8));
    let (buf, drawn) = draw(area, lines, Some(0), ScrollWindow::default());
    let rows = buffer_rows(&buf);
    assert_eq!(drawn.list.height, 5, "the room six detail rows leave");
    assert_eq!(drawn.window.content(), 10, "the long line took two rows");
    assert_eq!(&rows[0][..11], "a".repeat(11));
    assert_eq!(rows[1].chars().next(), Some('a'));
}

#[test]
fn the_window_follows_every_row_of_the_cursor_line() {
    let area = pane(20, 16);
    let mut lines = items(12);
    // A last line as wide as the area, which wraps onto two rows at the
    // width the gutter leaves.
    lines.push(Line::from(format!("{} tail", "w".repeat(15))));
    let (buf, drawn) = draw(area, lines, Some(12), ScrollWindow::default());
    let rows = buffer_rows(&buf);
    assert_eq!(drawn.list.height, 9);
    assert_eq!(drawn.window.content(), 14);
    assert_eq!(
        drawn.window.visible(),
        5..14,
        "both rows of the cursor line"
    );
    assert_eq!(rows[0].trim_end(), "item 5");
    assert!(rows[8].starts_with("tail"), "{rows:?}");
}

#[test]
fn the_window_starts_where_the_last_frame_left_it() {
    let area = pane(20, 16);
    let kept = ScrollWindow::new(9, 30).at(10);
    let (buf, drawn) = draw(area, items(30), Some(12), kept);
    assert_eq!(
        drawn.window.offset(),
        10,
        "the cursor is in view: no motion"
    );
    assert_eq!(buffer_rows(&buf)[0].trim_end(), "item 10");
    // The list shrank under the window: it comes back inside.
    let (_, drawn) = draw(area, items(12), Some(11), kept);
    assert_eq!(drawn.window.offset(), 3);
}

#[test]
fn without_a_cursor_the_window_shows_the_first_row() {
    let area = pane(20, 16);
    let kept = ScrollWindow::new(9, 30).at(10);
    let (buf, drawn) = draw(area, items(30), None, kept);
    assert_eq!(drawn.window.offset(), 0);
    assert_eq!(buffer_rows(&buf)[0].trim_end(), "item 0");
}

#[test]
fn the_divider_draws_on_its_row_and_the_detail_is_left_alone() {
    let area = pane(24, 14);
    let (buf, drawn) = draw(area, items(2), Some(0), ScrollWindow::default());
    let rows = buffer_rows(&buf);
    let divider = usize::from(drawn.divider.y - area.y);
    assert_eq!(divider, 3);
    assert_eq!(
        rows[divider],
        format!("{} Detail ──", marks::RULE.repeat(14))
    );
    for row in &rows[divider + 1..] {
        assert_eq!(row.trim(), "", "the detail is the host's: {rows:?}");
    }
    assert!(
        rows[2].trim().is_empty(),
        "the list's minimum row stays blank"
    );
}

#[test]
fn a_list_that_fits_draws_no_scrollbar() {
    let area = pane(20, 16);
    let (buf, drawn) = draw(area, items(9), Some(8), ScrollWindow::default());
    assert!(!drawn.window.overflows());
    let last_column: String = (area.y..area.y + drawn.list.height)
        .map(|y| buf[(area.right() - 1, y)].symbol().to_string())
        .collect();
    assert_eq!(last_column.trim(), "", "fade on fit");
}
