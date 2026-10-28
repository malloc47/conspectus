//! Property tests over generated text: ASCII, CJK, emoji (alone, with a
//! presentation selector, and as a joiner sequence), and combining
//! accents, at every budget from 0 to past the text's width.
//!
//! The generator is a fixed-seed xorshift, so a failure names the case
//! and reproduces on every run.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;

/// The pieces generated text is made of. Each is one grapheme or a run
/// of spaces.
const ATOMS: &[&str] = &[
    "a",
    "b",
    "q",
    "Z",
    "7",
    "-",
    "/",
    " ",
    "  ",
    "東",
    "京",
    "語",
    "\u{1f600}",
    "\u{26a0}\u{fe0f}",
    "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
    "e\u{301}",
    "a\u{308}",
    "n\u{303}",
];

/// Characters that extend the grapheme before them in [`ATOMS`].
fn extends(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36f}' | '\u{fe0f}' | '\u{200d}')
        || matches!(c, '\u{1f469}' | '\u{1f467}')
}

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).unwrap_or(0)
    }

    fn text(&mut self) -> String {
        let len = self.below(20);
        (0..len).map(|_| ATOMS[self.below(ATOMS.len())]).collect()
    }
}

/// A few hundred generated texts plus the fixed edge cases.
fn samples() -> Vec<String> {
    let mut generator = Gen(0x5eed_1234_abcd_0001);
    let mut out: Vec<String> = [
        "",
        " ",
        "plain ascii words in a row",
        "東京都庁舎前の交差点",
        "\u{1f600}\u{1f600}\u{1f600}",
        "cafe\u{301} na\u{308}ive",
        "   leading and trailing   ",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    out.extend((0..400).map(|_| generator.text()));
    out
}

/// Whether `piece` sits in `source` at whole-grapheme edges: it does not
/// start with a character that extends a grapheme, and the character
/// after it in `source` (at `end`) does not extend it either.
fn whole_graphemes(source: &str, piece: &str, end: usize) -> bool {
    let starts_clean = piece.chars().next().is_none_or(|c| !extends(c));
    let ends_clean = source[end..].chars().next().is_none_or(|c| !extends(c));
    starts_clean && ends_clean
}

#[test]
fn cuts_never_exceed_the_budget_and_are_idempotent() {
    for text in samples() {
        for budget in 0..=width(&text) + 2 {
            for (name, cut) in [
                ("truncate_end", truncate_end as fn(&str, usize) -> String),
                ("truncate_middle", truncate_middle),
                ("clip", clip),
            ] {
                let once = cut(&text, budget);
                assert!(
                    width(&once) <= budget,
                    "{name}({text:?}, {budget}) = {once:?} is {} cells",
                    width(&once)
                );
                assert_eq!(cut(&once, budget), once, "{name}({text:?}, {budget}) moved");
                if width(&text) <= budget {
                    assert_eq!(once, text, "{name}({text:?}, {budget}) cut text that fit");
                }
            }
        }
    }
}

#[test]
fn end_cuts_and_clips_keep_a_prefix_of_whole_graphemes() {
    for text in samples() {
        for budget in 0..=width(&text) {
            let clipped = clip(&text, budget);
            assert!(text.starts_with(&clipped), "clip({text:?}, {budget})");
            assert!(
                whole_graphemes(&text, &clipped, clipped.len()),
                "clip({text:?}, {budget}) = {clipped:?} split a grapheme"
            );
            let cut = truncate_end(&text, budget);
            let kept = cut.strip_suffix('…').unwrap_or(&cut);
            assert!(text.starts_with(kept), "truncate_end({text:?}, {budget})");
            assert!(whole_graphemes(&text, kept, kept.len()));
        }
    }
}

#[test]
fn fits_are_exactly_the_budget() {
    for text in samples() {
        for budget in 0..=width(&text) + 2 {
            assert_eq!(
                width(&fit(&text, budget)),
                budget,
                "fit({text:?}, {budget})"
            );
            assert!(width(&pad(&text, budget)) >= budget);
            assert!(width(&pad_start(&text, budget)) >= budget);
        }
    }
}

#[test]
fn span_fits_never_exceed_the_budget_and_are_idempotent() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut generator = Gen(0x0dd_ba11);
    for _ in 0..300 {
        let spans: Vec<Span<'static>> = (0..1 + generator.below(4))
            .map(|i| {
                let style = if i % 2 == 0 { Style::default() } else { bold };
                Span::styled(generator.text(), style)
            })
            .collect();
        for budget in 0..=spans_width(&spans) + 1 {
            let once = fit_spans(spans.clone(), budget);
            assert!(
                spans_width(&once) <= budget,
                "{spans:?} at {budget}: {once:?}"
            );
            assert_eq!(
                fit_spans(once.clone(), budget),
                once,
                "{spans:?} at {budget}"
            );
        }
    }
}

/// The text of `rows` with every space removed, for comparing content.
fn squeezed<'a>(rows: impl IntoIterator<Item = &'a str>) -> String {
    rows.into_iter()
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect()
}

#[test]
fn word_wrap_never_exceeds_the_width_or_splits_a_grapheme() {
    for text in samples() {
        for limit in 1..=width(&text) + 1 {
            let rows = wrap(&text, limit);
            let mut from = 0;
            for row in &rows {
                assert!(width(row) <= limit, "wrap({text:?}, {limit}): {row:?}");
                if row.is_empty() || row.contains('…') {
                    continue;
                }
                let at = from + text[from..].find(row.as_str()).expect("a row is a slice");
                assert!(
                    whole_graphemes(&text, row, at + row.len()),
                    "wrap({text:?}, {limit}): {row:?} split a grapheme"
                );
                from = at + row.len();
            }
            // Nothing but spaces is lost once every grapheme fits a row.
            if limit >= 2 {
                assert_eq!(
                    squeezed(rows.iter().map(String::as_str)),
                    squeezed([text.as_str()]),
                    "wrap({text:?}, {limit}) lost text: {rows:?}"
                );
            }
        }
    }
}

#[test]
fn character_wrap_fills_rows_without_losing_anything() {
    for text in samples() {
        for limit in 2..=width(&text) + 1 {
            let rows = wrap_chars(&text, limit);
            for row in &rows {
                assert!(
                    width(row) <= limit,
                    "wrap_chars({text:?}, {limit}): {row:?}"
                );
            }
            assert_eq!(rows.concat(), text, "wrap_chars({text:?}, {limit})");
        }
    }
}

#[test]
fn styled_wrap_agrees_with_plain_wrap_and_keeps_styles() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut generator = Gen(0xfeed_f00d);
    for _ in 0..200 {
        let pieces: Vec<String> = (0..1 + generator.below(3))
            .map(|_| generator.text())
            .collect();
        let joined = pieces.concat();
        let line = Line::from(
            pieces
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    Span::styled(
                        text.clone(),
                        if i % 2 == 0 { Style::default() } else { bold },
                    )
                })
                .collect::<Vec<_>>(),
        );
        for limit in 1..=width(&joined) + 1 {
            let styled: Vec<String> = wrap_line(&line, limit, 0)
                .iter()
                .map(ToString::to_string)
                .collect();
            assert_eq!(styled, wrap(&joined, limit), "{pieces:?} at {limit}");
            let hung = wrap_line(&line, limit, 2);
            for row in &hung {
                assert!(
                    row.width() <= limit,
                    "{pieces:?} at {limit} hanging: {row:?}"
                );
            }
            let chars: Vec<String> = wrap_line_chars(&line, limit)
                .iter()
                .map(ToString::to_string)
                .collect();
            assert_eq!(chars, wrap_chars(&joined, limit), "{pieces:?} at {limit}");
        }
    }
}

#[test]
fn separator_wrap_never_exceeds_the_width_or_splits_an_item_that_fits() {
    let mut generator = Gen(0xab_cdef);
    for _ in 0..200 {
        let items: Vec<String> = (0..1 + generator.below(6))
            .map(|_| generator.text().trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
        let joined = items.join(" | ");
        for limit in 2..=width(&joined) + 1 {
            let rows = wrap_at(&joined, " | ", limit);
            for row in &rows {
                assert!(width(row) <= limit, "{joined:?} at {limit}: {row:?}");
            }
            for item in items.iter().filter(|item| width(item) <= limit) {
                assert!(
                    rows.iter()
                        .any(|row| row.split(" | ").any(|piece| piece == item)),
                    "{item:?} was split at {limit}: {rows:?}"
                );
            }
        }
    }
}

#[test]
fn snippets_never_exceed_the_budget_and_keep_the_match() {
    let mut generator = Gen(0x1357_9bdf);
    for _ in 0..300 {
        let before = generator.text();
        let target = format!("x{}y", generator.text().trim());
        let after = generator.text();
        let text = format!("{before}{target}{after}");
        let range = before.len()..before.len() + target.len();
        for budget in 0..=width(&text) + 1 {
            let snippet = fit_snippet(&text, range.clone(), budget, &[]);
            assert!(
                width(&snippet.text) <= budget,
                "{text:?} at {budget}: {snippet:?}"
            );
            if let Some(shown) = snippet.highlight.clone() {
                let shown = &snippet.text[shown];
                assert!(target.starts_with(shown), "{text:?} at {budget}: {shown:?}");
                if width(&target) + 1 < budget {
                    assert_eq!(shown, target, "{text:?} at {budget}");
                }
            }
        }
    }
}
