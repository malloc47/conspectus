use super::*;

const TICKS: &[char] = &['`'];

fn marked(fitted: &Snippet) -> &str {
    let range = fitted.highlight.clone().expect("a highlight");
    &fitted.text[range]
}

const SENTENCE: &str = "We looked at the release notes and then chose the quokka design for the upgrade steps after review";

fn quokka() -> Range<usize> {
    let start = SENTENCE.find("quokka").unwrap();
    start..start + "quokka".len()
}

#[test]
fn a_snippet_that_fits_is_unchanged() {
    let fitted = fit_snippet("We looked at it", 3..9, 40, &[]);
    assert_eq!(fitted.text, "We looked at it");
    assert_eq!(fitted.highlight, Some(3..9));
}

#[test]
fn the_match_stays_visible_at_every_budget() {
    for budget in 8..width(SENTENCE) {
        let fitted = fit_snippet(SENTENCE, quokka(), budget, &[]);
        assert!(width(&fitted.text) <= budget, "{budget}: {:?}", fitted.text);
        assert_eq!(marked(&fitted), "quokka", "{budget}: {:?}", fitted.text);
    }
}

#[test]
fn context_after_the_match_is_favored() {
    let fitted = fit_snippet(SENTENCE, quokka(), 40, &[]);
    let range = fitted.highlight.clone().unwrap();
    let before = width(&fitted.text[..range.start]);
    let after = width(&fitted.text[range.end..]);
    assert!(after > before, "{:?}", fitted.text);
}

#[test]
fn cuts_fall_on_word_boundaries() {
    let fitted = fit_snippet(SENTENCE, quokka(), 40, &[]);
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

/// Japanese text with spaces only beside the match.
const SPACED_CJK: &str =
    "日本語のテキストと説明 wombat もっと日本語のテキストと説明がここに続きます";

/// Japanese text with no spaces at all.
const UNSPACED_CJK: &str = "前の会話で量子の設計を選んだ理由についての長い説明ですwombatそしてもっと日本語のテキストと説明がここに続きます";

/// A path and an address beside the match.
const LONG_TOKENS: &str = "see /usr/local/share/project/fixtures/very/long/path/config.toml wombat https://example.com/a/very/long/url/that/keeps/going/on and on";

fn wombat(text: &str) -> Range<usize> {
    let start = text.find("wombat").unwrap();
    start..start + "wombat".len()
}

/// What the fit shows before and after its match, without the marks and
/// the blanks beside the match.
fn sides(fitted: &Snippet) -> (&str, &str) {
    let range = fitted.highlight.clone().expect("a highlight");
    let blank_or_mark = |c: char| c.is_whitespace() || c == '…';
    (
        fitted.text[..range.start].trim_matches(blank_or_mark),
        fitted.text[range.end..].trim_matches(blank_or_mark),
    )
}

/// Checks every budget from `from` to the text's width: the fit stays in
/// the budget, keeps the match whole, shows only a piece of the source,
/// and shows context on both sides.
fn context_on_both_sides(text: &str, from: usize) {
    for budget in from..width(text) {
        let fitted = fit_snippet(text, wombat(text), budget, &[]);
        assert!(width(&fitted.text) <= budget, "{budget}: {:?}", fitted.text);
        assert_eq!(marked(&fitted), "wombat", "{budget}: {:?}", fitted.text);
        let body = fitted.text.trim_matches('…');
        assert!(text.contains(body), "{budget}: {:?}", fitted.text);
        let (before, after) = sides(&fitted);
        assert!(
            !before.is_empty() && !after.is_empty(),
            "{budget}: {:?}",
            fitted.text
        );
    }
}

#[test]
fn text_set_without_spaces_keeps_its_context_beside_a_spaced_match() {
    let fitted = fit_snippet(SPACED_CJK, wombat(SPACED_CJK), 63, &[]);
    assert_eq!(
        fitted.text,
        "…のテキストと説明 wombat もっと日本語のテキストと説明がここに…"
    );
    // A two-cell character and the blank need three cells before the
    // match, which a budget of 16 is the first to leave.
    context_on_both_sides(SPACED_CJK, 16);
}

#[test]
fn text_without_any_spaces_is_cut_between_its_characters() {
    let fitted = fit_snippet(UNSPACED_CJK, wombat(UNSPACED_CJK), 30, &[]);
    assert_eq!(fitted.text, "…明ですwombatそしてもっと日…");
    // A two-cell character needs two cells before the match, which a
    // budget of 13 is the first to leave.
    context_on_both_sides(UNSPACED_CJK, 13);
}

#[test]
fn a_long_token_beside_the_match_keeps_the_part_that_fits() {
    let at = |budget| fit_snippet(LONG_TOKENS, wombat(LONG_TOKENS), budget, &[]).text;
    assert_eq!(at(20), "…oml wombat https:/…");
    assert_eq!(at(40), "…onfig.toml wombat https://example.com/…");
    assert_eq!(
        at(63),
        "…/path/config.toml wombat https://example.com/a/very/long/url/…"
    );
    // A character and the blank need two cells before the match, which a
    // budget of 13 is the first to leave.
    context_on_both_sides(LONG_TOKENS, 13);
}

#[test]
fn a_cut_still_moves_to_a_word_edge_that_keeps_context() {
    let at = |budget| fit_snippet(SENTENCE, quokka(), budget, &[]).text;
    assert_eq!(at(30), "…the quokka design for the…");
    assert_eq!(at(40), "…chose the quokka design for the…");
    assert_eq!(
        at(63),
        "…then chose the quokka design for the upgrade steps after…"
    );
    // The word before the match is wider than the room, and its edge is
    // the one beside the match: the word is cut rather than dropped.
    assert_eq!(at(16), "…he quokka desi…");
}

#[test]
fn a_room_of_one_blank_shows_no_context() {
    // At 12 cells the room before the match holds only the blank.
    let fitted = fit_snippet(SPACED_CJK, wombat(SPACED_CJK), 12, &[]);
    assert_eq!(fitted.text, "…wombat も…");
}

#[test]
fn a_match_at_either_end_has_no_mark_on_that_side() {
    let fitted = fit_snippet(SENTENCE, 3..9, 20, &[]);
    assert!(!fitted.text.starts_with('…'), "{:?}", fitted.text);
    assert!(fitted.text.ends_with('…'));
    assert_eq!(marked(&fitted), &SENTENCE[3..9]);

    let start = SENTENCE.len() - "review".len();
    let fitted = fit_snippet(SENTENCE, start..SENTENCE.len(), 20, &[]);
    assert!(fitted.text.starts_with('…'));
    assert!(!fitted.text.ends_with('…'), "{:?}", fitted.text);
    assert_eq!(marked(&fitted), "review");
}

#[test]
fn a_match_wider_than_the_room_is_shown_cut() {
    let fitted = fit_snippet("supercalifragilistic", 0..20, 8, &[]);
    assert_eq!(fitted.text, "superca…");
    assert_eq!(fitted.highlight, Some(0..7));
}

#[test]
fn wide_characters_are_counted_by_cells() {
    let text = "前の会話で量子の設計を選んだ理由についての長い説明です";
    let start = text.find("設計").unwrap();
    for budget in 6..width(text) {
        let fitted = fit_snippet(text, start..start + "設計".len(), budget, &[]);
        assert!(width(&fitted.text) <= budget, "{budget}: {:?}", fitted.text);
        assert_eq!(marked(&fitted), "設計");
    }
}

#[test]
fn accented_text_is_cut_between_characters() {
    let text = "naïve · approach · brûlée · crème · déjà vu · façade";
    let start = text.find("approach").unwrap();
    for budget in 10..width(text) {
        let fitted = fit_snippet(text, start..start + "approach".len(), budget, &[]);
        assert!(width(&fitted.text) <= budget, "{budget}: {:?}", fitted.text);
        assert_eq!(marked(&fitted), "approach", "{budget}");
    }
}

#[test]
fn a_highlight_inside_a_grapheme_widens_to_the_whole_grapheme() {
    // The match ends between `e` and its combining accent.
    let text = "the cafe\u{301} on the corner";
    let start = text.find("cafe").unwrap();
    let fitted = fit_snippet(text, start..start + 4, 80, &[]);
    assert_eq!(marked(&fitted), "cafe\u{301}");
}

#[test]
fn dropped_characters_go_and_the_highlight_follows() {
    let text = "see `feature notes` and more";
    let start = text.find("notes").unwrap();
    let fitted = fit_snippet(text, start..start + 5, 80, TICKS);
    assert_eq!(fitted.text, "see feature notes and more");
    assert_eq!(marked(&fitted), "notes");
    // Without the parameter the ticks stay.
    let kept = fit_snippet(text, start..start + 5, 80, &[]);
    assert_eq!(kept.text, text);
    assert_eq!(marked(&kept), "notes");
}

#[test]
fn a_bad_range_falls_back_to_the_head_of_the_text() {
    let fitted = fit_snippet(SENTENCE, 400..500, 10, &[]);
    assert_eq!(fitted.highlight, None);
    assert_eq!(fitted.text, "We looked…");
    assert_eq!(fit_snippet("short", 2..2, 10, &[]).highlight, None);
}

#[test]
fn a_zero_budget_shows_nothing() {
    let fitted = fit_snippet(SENTENCE, 0..2, 0, &[]);
    assert_eq!(fitted.text, "");
    assert_eq!(fitted.highlight, None);
}
