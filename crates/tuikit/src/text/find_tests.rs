use ratatui::style::{Modifier, Style};

use super::*;

fn underline() -> Style {
    Style::default().add_modifier(Modifier::UNDERLINED)
}

fn italic() -> Style {
    Style::default().add_modifier(Modifier::ITALIC)
}

fn pieces(spans: &[Span<'_>]) -> Vec<(String, Style)> {
    spans
        .iter()
        .map(|span| (span.content.to_string(), span.style))
        .collect()
}

#[test]
fn ascii_letters_match_without_case() {
    assert_eq!(find_ci("Hello World", "world"), Some(6));
    assert_eq!(find_ci("Hello World", "WORLD"), Some(6));
    assert_eq!(find_ci("abc", "abcd"), None);
    assert_eq!(find_ci("abc", ""), None);
    assert_eq!(find_ci("", "a"), None);
}

#[test]
fn other_characters_compare_exactly_and_offsets_stay_on_boundaries() {
    let hay = "Ünïcode: café, CAFÉ";
    let at = find_ci(hay, "café").unwrap();
    assert_eq!(&hay[at..at + "café".len()], "café");
    assert!(hay.is_char_boundary(at));
    // `É` is not folded, so the second spelling does not match `é`.
    assert_eq!(find_ci(&hay[at + 1..], "café"), None);
}

#[test]
fn only_ascii_letters_fold() {
    assert_eq!(
        find_ci("Café au lait", "CAFÉ"),
        None,
        "É is not folded to é"
    );
    assert_eq!(
        find_ci("Café au lait", "CAFé"),
        Some(0),
        "ASCII letters fold"
    );
    assert_eq!(find_ci("naïve Straße", "Straße"), Some(7));
    assert_eq!(find_ci("STRASSE", "straße"), None);
}

#[test]
fn a_match_in_wide_text_starts_and_ends_on_a_character_boundary() {
    let text = "日本語のテキスト and 😀 emoji";
    for needle in ["本語", "😀", "emoji", "テキスト"] {
        let start = find_ci(text, needle).expect(needle);
        assert!(text.is_char_boundary(start), "{needle}");
        assert!(text.is_char_boundary(start + needle.len()), "{needle}");
    }
}

#[test]
fn every_match_is_found_without_overlap() {
    let found: Vec<_> = find_all_ci("aaaa", "aa").collect();
    assert_eq!(found, [0..2, 2..4]);
    let found: Vec<_> = find_all_ci("Needle, needle, NEEDLE", "needle").collect();
    assert_eq!(found, [0..6, 8..14, 16..22]);
    assert_eq!(find_all_ci("abc", "").count(), 0);
}

#[test]
fn painting_splits_spans_only_at_range_edges() {
    let spans = [Span::styled("the index", italic())];
    let painted = paint(&spans, Some(4..9), |_| underline());
    assert_eq!(
        pieces(&painted),
        [
            ("the ".to_string(), italic()),
            ("index".to_string(), underline())
        ]
    );
}

#[test]
fn patching_keeps_the_base_look_and_replacing_drops_it() {
    let spans = [Span::styled("abc", italic())];
    let patched = paint(&spans, Some(1..2), |base| base.patch(underline()));
    assert_eq!(patched[1].style, italic().patch(underline()));
    let replaced = paint(&spans, Some(1..2), |_| underline());
    assert_eq!(replaced[1].style, underline());
}

#[test]
fn a_range_across_spans_paints_each_part() {
    let spans = [Span::raw("ab"), Span::styled("cd", italic())];
    let painted = paint(&spans, Some(1..3), |base| base.patch(underline()));
    assert_eq!(
        pieces(&painted),
        [
            ("a".to_string(), Style::default()),
            ("b".to_string(), underline()),
            ("c".to_string(), italic().patch(underline())),
            ("d".to_string(), italic()),
        ]
    );
}

#[test]
fn overlapping_and_unsorted_ranges_merge() {
    let spans = [Span::raw("abcdef")];
    let painted = paint(&spans, [3..5, 1..4, 2..2], |_| underline());
    assert_eq!(
        pieces(&painted),
        [
            ("a".to_string(), Style::default()),
            ("bcde".to_string(), underline()),
            ("f".to_string(), Style::default()),
        ]
    );
}

#[test]
fn a_range_edge_inside_a_grapheme_moves_out_to_its_edge() {
    let text = "cafe\u{301}!";
    let painted = paint(&[Span::raw(text)], Some(3..4), |_| underline());
    assert_eq!(painted[1].content, "e\u{301}");
    assert_eq!(painted[2].content, "!");
}

#[test]
fn no_range_leaves_the_spans_as_they_were() {
    let spans = [Span::raw("ab"), Span::styled("cd", italic())];
    assert_eq!(
        pieces(&paint(&spans, None, |_| underline())),
        pieces(&spans)
    );
    assert_eq!(
        pieces(&paint(&spans, Some(9..12), |_| underline())),
        pieces(&spans)
    );
}
