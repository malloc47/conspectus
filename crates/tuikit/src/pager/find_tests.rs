use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;

/// Each unit takes three rows: unit 4 is rows 12 to 14.
fn three_rows(unit: usize) -> Range<usize> {
    unit * 3..unit * 3 + 3
}

fn active(units: &[usize]) -> Find {
    let mut find = Find::default();
    find.set("needle", units.to_vec());
    find
}

#[test]
fn a_query_is_trimmed_and_an_empty_one_finds_nothing() {
    let mut find = Find::default();
    find.set("  needle ", vec![2, 5]);
    assert_eq!(find.query(), "needle");
    assert!(find.is_active());
    assert_eq!(find.current(), None, "a new find is on no match yet");

    find.set("   ", vec![1, 2, 3]);
    assert!(!find.is_active());
    assert!(find.matches().is_empty(), "an empty query matches nothing");
}

#[test]
fn stepping_wraps_at_the_ends_and_says_so() {
    let mut find = active(&[2, 5, 9]);
    find.go(0);
    assert_eq!(find.step(true, None, 0..10), Step::Moved { wrapped: false });
    assert_eq!(find.current_unit(), Some(5));
    find.step(true, None, 0..10);
    assert_eq!(
        find.step(true, None, 0..10),
        Step::Moved { wrapped: true },
        "past the last match to the first"
    );
    assert_eq!(find.current_unit(), Some(2));
    assert_eq!(
        find.step(false, None, 0..10),
        Step::Moved { wrapped: true },
        "back past the first to the last"
    );
    assert_eq!(find.current_unit(), Some(9));
    assert_eq!(
        find.take_landing(),
        Some(9),
        "every step scrolls to its match"
    );
    assert_eq!(find.take_landing(), None, "once");
}

#[test]
fn stepping_with_no_find_or_no_match_moves_nowhere() {
    let mut find = Find::default();
    assert_eq!(find.step(true, None, 0..10), Step::Inactive);
    let mut find = active(&[]);
    assert_eq!(find.step(true, None, 0..10), Step::Nothing);
    assert_eq!(find.current(), None);
}

#[test]
fn the_first_step_starts_from_the_window() {
    let rows: &dyn Fn(usize) -> Range<usize> = &three_rows;
    // The window shows rows 13 to 22: unit 4 is partly in it, unit 8 is
    // below it.
    let mut find = active(&[1, 4, 8]);
    assert_eq!(
        find.step(true, Some(rows), 13..23),
        Step::Moved { wrapped: false }
    );
    assert_eq!(
        find.current_unit(),
        Some(4),
        "the first match not above the top"
    );
    let mut find = active(&[1, 4, 8]);
    find.step(false, Some(rows), 13..23);
    assert_eq!(
        find.current_unit(),
        Some(4),
        "the last match above the bottom"
    );
    // Nothing at or below the top: forward wraps to the first.
    let mut find = active(&[1, 2]);
    assert_eq!(
        find.step(true, Some(rows), 20..30),
        Step::Moved { wrapped: true }
    );
    assert_eq!(find.current_unit(), Some(1));
}

#[test]
fn the_first_match_from_the_top_is_the_one_whose_rows_reach_it() {
    let rows: &dyn Fn(usize) -> Range<usize> = &three_rows;
    let find = active(&[1, 4, 8]);
    assert_eq!(find.first_from(Some(rows), 0), 0);
    assert_eq!(find.first_from(Some(rows), 13), 1, "unit 4 ends at row 15");
    assert_eq!(find.first_from(Some(rows), 15), 2);
    assert_eq!(find.first_from(Some(rows), 99), 0, "none below: the first");
    assert_eq!(find.first_from(None, 13), 0, "not laid out: the first");
}

#[test]
fn the_nearest_match_wins_and_a_tie_goes_to_the_earlier() {
    let find = active(&[3, 10, 17]);
    assert_eq!(find.nearest(9), Some(1));
    assert_eq!(find.nearest(0), Some(0));
    assert_eq!(find.nearest(99), Some(2));
    let tied = active(&[4, 8]);
    assert_eq!(tied.nearest(6), Some(0));
    assert_eq!(active(&[]).nearest(6), None);
}

#[test]
fn staying_on_a_unit_after_the_matches_change() {
    let mut find = active(&[2, 5, 9]);
    find.go(1);
    find.set("needle", vec![1, 5, 7]);
    find.stay_on(Some(5));
    assert_eq!(find.current(), Some(1));
    assert_eq!(find.take_landing(), None, "staying does not scroll");
    find.set("needle", vec![1, 7]);
    find.stay_on(Some(5));
    assert_eq!(find.current(), None, "gone: on no match");
}

#[test]
fn the_chip_says_where_the_find_is_or_that_nothing_matched() {
    assert!(Find::default().chip(None).is_none());
    let mut find = active(&[2, 5, 9]);
    find.go(1);
    assert_eq!(
        find.chip(None).map(|chip| chip.text()).as_deref(),
        Some("find \"needle\" 2/3")
    );
    let lost = active(&[2, 5, 9]);
    assert_eq!(
        lost.chip(None).map(|chip| chip.text()).as_deref(),
        Some("find \"needle\" 3 matches")
    );
    let none = active(&[]);
    let chip = none.chip(Some("4 hidden".into())).expect("a chip");
    assert_eq!(chip.text(), "find \"needle\" no match · 4 hidden");
    assert_eq!(chip.tone(), Tone::Warning);
    let mut long = Find::default();
    long.set("a query far longer than the chip allows", vec![0]);
    let text = long.chip(None).expect("a chip").text();
    assert_eq!(
        text, "find \"a query far long…\" 1 match",
        "17 cells, the mark included"
    );
}

#[test]
fn painting_picks_out_every_match_after_the_gutter() {
    let theme = Theme::default();
    let find = active(&[0]);
    let row = Line::from(vec![
        Span::raw("needle "),
        Span::raw("a needle and a NEEDLE"),
    ]);
    // The gutter's `needle` is skipped; both matches after it are painted.
    let painted = find.paint(&row, 7, false, &theme);
    let looks: Vec<(String, Style)> = painted
        .spans
        .iter()
        .map(|span| (span.content.to_string(), span.style))
        .collect();
    let matched: Vec<&str> = looks
        .iter()
        .filter(|(_, style)| *style == theme.match_style())
        .map(|(text, _)| text.as_str())
        .collect();
    assert_eq!(matched, ["needle", "NEEDLE"]);
    assert_eq!(looks[0].0, "needle ", "the gutter keeps its look");

    let current = find.paint(&row, 7, true, &theme);
    assert!(
        current
            .spans
            .iter()
            .filter(|span| span.content.eq_ignore_ascii_case("needle"))
            .all(|span| span.style.add_modifier.contains(Modifier::REVERSED)),
        "the current match takes the selection look: {current:?}"
    );
    assert_eq!(
        Find::current_style(&theme).add_modifier,
        theme.selection_active
    );
    // With no find, nothing changes.
    assert_eq!(Find::default().paint(&row, 0, true, &theme), row);
}

#[test]
fn the_prompt_opens_on_the_active_query_and_hands_back_its_text() {
    let mut find = active(&[1]);
    find.open_prompt();
    assert_eq!(find.prompt().map(LineEdit::value), Some("needle"));
    find.edit(crate::testkit::keys::typed('s'));
    assert_eq!(find.take_prompt().as_deref(), Some("needles"));
    assert!(find.prompt().is_none());
    find.open_prompt();
    find.close_prompt();
    assert!(find.prompt().is_none());
    assert_eq!(find.query(), "needle", "cancelling keeps the find");
}
