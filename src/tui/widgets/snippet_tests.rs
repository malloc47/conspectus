use super::*;

fn marked(fitted: &Fitted) -> &str {
    let range = fitted.highlight.clone().expect("a highlight");
    &fitted.text[range]
}

fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

const SENTENCE: &str = "We looked at the release notes and then chose the quokka design for the upgrade steps after review";

#[test]
fn a_snippet_that_fits_is_unchanged() {
    let range = 4..10;
    let fitted = fit("We looked at it", &range, 40);
    assert_eq!(fitted.text, "We looked at it");
    assert_eq!(fitted.highlight, Some(4..10));
}

#[test]
fn the_match_stays_visible_at_every_budget() {
    let start = SENTENCE.find("quokka").unwrap();
    let range = start..start + "quokka".len();
    for budget in 8..width(SENTENCE) {
        let fitted = fit(SENTENCE, &range, budget);
        assert!(width(&fitted.text) <= budget, "{budget}: {:?}", fitted.text);
        assert_eq!(marked(&fitted), "quokka", "{budget}: {:?}", fitted.text);
    }
}

#[test]
fn cuts_fall_on_word_boundaries() {
    let start = SENTENCE.find("quokka").unwrap();
    let range = start..start + "quokka".len();
    let fitted = fit(SENTENCE, &range, 40);
    let body = fitted.text.trim_matches('…');
    for word in body.split_whitespace() {
        assert!(
            SENTENCE.split_whitespace().any(|w| w == word),
            "`{word}` is a fragment in {:?}",
            fitted.text
        );
    }
    assert!(fitted.text.starts_with('…') && fitted.text.ends_with('…'));
}

#[test]
fn a_match_at_the_start_has_no_leading_ellipsis() {
    let fitted = fit(SENTENCE, &(3..9), 20);
    assert!(!fitted.text.starts_with('…'), "{:?}", fitted.text);
    assert!(fitted.text.ends_with('…'));
    assert_eq!(marked(&fitted), &SENTENCE[3..9]);
}

#[test]
fn a_match_at_the_end_has_no_trailing_ellipsis() {
    let start = SENTENCE.len() - "review".len();
    let fitted = fit(SENTENCE, &(start..SENTENCE.len()), 20);
    assert!(fitted.text.starts_with('…'));
    assert!(!fitted.text.ends_with('…'), "{:?}", fitted.text);
    assert_eq!(marked(&fitted), "review");
}

#[test]
fn a_match_wider_than_the_room_is_shown_cut() {
    let fitted = fit("supercalifragilistic", &(0..20), 8);
    assert_eq!(fitted.text, "superca…");
    assert_eq!(fitted.highlight, Some(0..7));
}

#[test]
fn wide_characters_are_counted_by_cells() {
    let text = "前の会話で量子の設計を選んだ理由についての長い説明です";
    let start = text.find("設計").unwrap();
    let fitted = fit(text, &(start..start + "設計".len()), 16);
    assert!(width(&fitted.text) <= 16, "{:?}", fitted.text);
    assert_eq!(marked(&fitted), "設計");
}

#[test]
fn code_ticks_are_dropped_and_the_highlight_follows() {
    let text = "see `feature notes` and more";
    let start = text.find("notes").unwrap();
    let fitted = fit(text, &(start..start + 5), 80);
    assert_eq!(fitted.text, "see feature notes and more");
    assert_eq!(marked(&fitted), "notes");
}

#[test]
fn a_bad_range_falls_back_to_the_head_of_the_text() {
    let fitted = fit(SENTENCE, &(400..500), 10);
    assert_eq!(fitted.highlight, None);
    assert_eq!(width(&fitted.text), 10);
}

#[test]
fn a_zero_budget_shows_nothing() {
    assert_eq!(fit(SENTENCE, &(0..2), 0).text, "");
}
