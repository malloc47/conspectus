use super::*;

#[test]
fn a_cursor_below_the_window_becomes_its_last_line() {
    assert_eq!(follow_offset(0, 0..1, 5, 20), 0);
    assert_eq!(follow_offset(0, 10..11, 5, 20), 6);
}

#[test]
fn a_cursor_above_the_window_becomes_its_first_line() {
    assert_eq!(follow_offset(6, 4..5, 5, 20), 4);
    assert_eq!(follow_offset(6, 0..1, 5, 20), 0);
}

#[test]
fn a_cursor_inside_the_window_does_not_scroll_it() {
    for row in 6..11 {
        assert_eq!(follow_offset(6, row..row + 1, 5, 20), 6, "row {row}");
    }
}

#[test]
fn the_offset_never_passes_the_last_full_page() {
    // The end of 20 rows in a 5-row window.
    assert_eq!(follow_offset(0, 19..20, 5, 20), 15);
    // The content shrank to 8 rows with the cursor inside the old window.
    assert_eq!(follow_offset(15, 7..8, 5, 8), 3);
    // Content shorter than the window shows from its top.
    assert_eq!(follow_offset(9, 2..3, 5, 3), 0);
    // A stale offset far past the end comes back first.
    assert_eq!(follow_offset(50, 25..26, 10, 30), 20);
}

#[test]
fn a_window_of_no_rows_only_clamps() {
    assert_eq!(follow_offset(6, 99..100, 0, 100), 6);
    assert_eq!(follow_offset(150, 99..100, 0, 100), 100);
}

#[test]
fn a_wrapped_cursor_line_shows_every_row() {
    // Rows 9 and 10 in a 5-row window: both fit from 6.
    assert_eq!(follow_offset(0, 9..11, 5, 20), 6);
    // The one-row cursor at 9 needs only 5.
    assert_eq!(follow_offset(0, 9..10, 5, 20), 5);
}

#[test]
fn a_span_taller_than_the_window_shows_its_start() {
    assert_eq!(follow_offset(0, 3..12, 5, 20), 3);
    assert_eq!(follow_offset(10, 3..12, 5, 20), 3);
    assert_eq!(follow_offset(1, 3..12, 5, 20), 3);
}

#[test]
fn an_empty_span_is_the_row_at_its_start() {
    assert_eq!(follow_offset(0, 10..10, 5, 20), 6);
}

/// The rule the list overlays used before they shared this one: the
/// cursor on the last line once it passes the first screen.
fn cursor_on_the_last_line(cursor: usize, viewport: usize, content: usize) -> usize {
    if viewport == 0 || content <= viewport || cursor < viewport {
        0
    } else {
        cursor + 1 - viewport
    }
}

#[test]
fn following_from_the_top_keeps_the_cursor_on_the_last_line() {
    for content in 0..30 {
        for viewport in 0..12 {
            for cursor in 0..content {
                assert_eq!(
                    follow_offset(0, cursor..cursor + 1, viewport, content),
                    cursor_on_the_last_line(cursor, viewport, content),
                    "cursor {cursor}, viewport {viewport}, content {content}"
                );
            }
        }
    }
}

#[test]
fn a_window_built_around_a_cursor_follows_from_the_top() {
    let window = ScrollWindow::new(5, 20).following(12..13);
    assert_eq!(window.visible(), 8..13);
    assert_eq!(ScrollWindow::new(5, 20).following(3..4).offset(), 0);
    assert_eq!(
        ScrollWindow::new(5, 20).at(10).following(12..13).offset(),
        10
    );
}

#[test]
fn following_a_span_from_the_top_shows_its_end_or_its_start() {
    for content in 0..20 {
        for viewport in 1..10 {
            for start in 0..content {
                for end in start + 1..=content {
                    assert_eq!(
                        follow_offset(0, start..end, viewport, content),
                        end.saturating_sub(viewport).min(start),
                        "span {start}..{end}, viewport {viewport}, content {content}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_new_window_starts_at_the_top() {
    let window = ScrollWindow::new(5, 20);
    assert_eq!(window.offset(), 0);
    assert_eq!(window.viewport(), 5);
    assert_eq!(window.content(), 20);
    assert_eq!(window.visible(), 0..5);
    assert_eq!(ScrollWindow::default().visible(), 0..0);
}

#[test]
fn a_window_set_past_its_end_stops_at_the_last_offset() {
    let window = ScrollWindow::new(5, 20).at(40);
    assert_eq!(window.offset(), 15);
    assert_eq!(window.last_offset(), 15);
    assert_eq!(window.visible(), 15..20);
    assert_eq!(ScrollWindow::new(5, 3).at(2).offset(), 0);
}

#[test]
fn resizing_keeps_an_offset_that_still_fits() {
    let mut window = ScrollWindow::new(5, 20).at(10);
    window.resize(5, 30);
    assert_eq!(window.offset(), 10);
    window.resize(5, 12);
    assert_eq!(window.offset(), 7);
    window.resize(20, 12);
    assert_eq!(window.offset(), 0);
}

#[test]
fn scrolling_stops_at_both_ends() {
    let mut window = ScrollWindow::new(5, 20);
    window.scroll_by(-3);
    assert_eq!(window.offset(), 0);
    window.scroll_by(4);
    assert_eq!(window.offset(), 4);
    window.scroll_by(100);
    assert_eq!(window.offset(), 15);
    window.scroll_by(i32::MIN);
    assert_eq!(window.offset(), 0);
}

#[test]
fn pages_are_the_rows_the_window_shows() {
    let mut window = ScrollWindow::new(5, 23);
    assert_eq!(window.page(), 5);
    window.page_down();
    assert_eq!(window.visible(), 5..10);
    window.page_down();
    window.page_down();
    window.page_down();
    assert_eq!(window.visible(), 18..23);
    window.page_up();
    assert_eq!(window.offset(), 13);
    assert_eq!(ScrollWindow::new(0, 10).page(), 1);
}

#[test]
fn the_ends_are_the_first_and_last_offsets() {
    let mut window = ScrollWindow::new(5, 20);
    window.to_bottom();
    assert_eq!(window.visible(), 15..20);
    window.to_top();
    assert_eq!(window.offset(), 0);
    let mut fits = ScrollWindow::new(5, 3);
    fits.to_bottom();
    assert_eq!(fits.offset(), 0);
}

#[test]
fn a_window_overflows_only_when_the_content_is_taller() {
    assert!(ScrollWindow::new(5, 6).overflows());
    assert!(!ScrollWindow::new(5, 5).overflows());
    assert!(!ScrollWindow::new(5, 0).overflows());
}

#[test]
fn the_paragraph_offset_saturates() {
    assert_eq!(ScrollWindow::new(5, 20).at(7).paragraph_scroll(), (7, 0));
    let huge = ScrollWindow::new(1, 1 << 20).at(1 << 19);
    assert_eq!(huge.paragraph_scroll(), (u16::MAX, 0));
}
