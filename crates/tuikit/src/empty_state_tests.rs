use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::*;
use crate::testkit::buffer::rows;

fn drawn(empty: &EmptyState, width: u16, height: u16) -> (Buffer, Vec<String>) {
    let theme = Theme::default();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    empty.render(area, &mut buf, &theme);
    let text = rows(&buf);
    (buf, text)
}

#[test]
fn a_message_alone_is_one_wrapped_block_from_the_top_left() {
    let empty = EmptyState::new("Nothing has loaded yet.");
    assert_eq!(empty.rows(80), ["Nothing has loaded yet."]);
    let (_, text) = drawn(&empty, 12, 3);
    assert_eq!(text, ["Nothing has ", "loaded yet. ", "            "]);
}

#[test]
fn the_hint_starts_on_its_own_row_after_the_message() {
    let empty = EmptyState::new("No rows.").hint("Press r to refresh.");
    assert_eq!(empty.rows(80), ["No rows.", "Press r to refresh."]);
    assert_eq!(empty.text(), "No rows.\nPress r to refresh.");
    assert_eq!(empty.message(), "No rows.");
    assert_eq!(empty.hint_text(), Some("Press r to refresh."));
    // Each wraps on its own.
    assert_eq!(empty.rows(10), ["No rows.", "Press r to", "refresh."]);
}

#[test]
fn rows_wrap_at_word_edges_and_never_run_past_the_width() {
    let empty = EmptyState::new("Looking for \"abc\"…")
        .hint("Entries that match appear as they are found. Press `F` to clear.");
    for width in 1..=80 {
        for row in empty.rows(width) {
            assert!(text::width(&row) <= width, "{row:?} at {width}");
        }
    }
    // The same rows the one word wrapper gives the text.
    assert_eq!(empty.rows(24), text::wrap(&empty.text(), 24));
}

#[test]
fn the_whole_slot_recedes_in_the_muted_look() {
    let theme = Theme::default();
    let empty = EmptyState::new("Empty.");
    let (buf, _) = drawn(&empty, 10, 2);
    for y in 0..2 {
        for x in 0..10 {
            assert_eq!(buf[(x, y)].modifier, theme.muted, "cell {x},{y}");
        }
    }
    let lines = empty.hint("Add one.").lines(10, &theme);
    let muted = Style::default().add_modifier(theme.muted);
    assert!(lines.iter().all(|line| line.style == muted));
    assert_eq!(lines.len(), 2);
}

#[test]
fn a_centered_state_sits_in_the_middle_of_its_slot() {
    let empty = EmptyState::new("No items.")
        .hint("Press n to add one.")
        .centered();
    assert_eq!(empty.rows(21), ["      No items.", " Press n to add one."]);
    let (_, text) = drawn(&empty, 21, 6);
    assert_eq!(
        text,
        [
            " ".repeat(21),
            " ".repeat(21),
            format!("      No items.{}", " ".repeat(6)),
            " Press n to add one. ".to_string(),
            " ".repeat(21),
            " ".repeat(21),
        ]
    );
}

#[test]
fn a_slot_too_short_draws_the_rows_that_fit() {
    let empty = EmptyState::new("one two three four").centered();
    let (_, text) = drawn(&empty, 5, 2);
    assert_eq!(text, [" one ", " two "]);
    let (_, text) = drawn(&EmptyState::new("one two three"), 5, 1);
    assert_eq!(text, ["one  "]);
    // No room at all draws nothing and does not fail.
    let (_, text) = drawn(&empty, 0, 0);
    assert!(text.is_empty());
}

#[test]
fn rows_are_measured_in_cells() {
    // Two cells a character.
    let empty = EmptyState::new("東京 大阪").centered();
    assert_eq!(empty.rows(11), [" 東京 大阪"]);
    assert_eq!(empty.rows(4), ["東京", "大阪"]);
}

#[test]
fn a_string_is_an_empty_state_with_no_hint() {
    assert_eq!(EmptyState::from("Nothing."), EmptyState::new("Nothing."));
    assert_eq!(
        EmptyState::from(String::from("Nothing.")),
        EmptyState::new("Nothing.")
    );
    assert_eq!(
        EmptyState::new("x").align(Align::Center),
        EmptyState::new("x").centered()
    );
    assert_eq!(EmptyState::default().align, Align::Left);
}

#[test]
fn the_look_is_the_muted_token_and_nothing_else() {
    let mut theme = Theme::default();
    theme.muted = Modifier::ITALIC;
    let lines = EmptyState::new("Empty.").lines(10, &theme);
    assert_eq!(
        lines[0].style,
        Style::default().add_modifier(Modifier::ITALIC)
    );
}
