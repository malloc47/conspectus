//! Fits of generated English prose, held to a golden.
//!
//! Prose keeps whole words: a cut that would split an ordinary word moves
//! to the word's edge, even when the only edge on its side is the one
//! beside the match and that side comes back empty. The corpus is a fixed
//! set of generated sentences (common, short, and long words, hyphenated
//! words and contractions, figures and abbreviations, ordinary
//! punctuation, quotes and brackets), each with a whole-word match, a
//! match inside a word, and a two-word match, fitted at every budget from
//! 0 to past the text's width.
//!
//! The golden writes each case as a `case` line with the match in
//! brackets and the text between bars, then a line for each run of
//! budgets that give the same fit: the budgets (`16-31`, or one budget),
//! then the fit between bars with its highlight in brackets. A failure
//! lists the fits that differ, budget by budget. When a change to how
//! prose is cut is meant, run the test with `TUIKIT_ACCEPT_GOLDENS=1` and
//! review the golden's diff.

use std::fmt::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::{clusters, fit_snippet, is_token};
use crate::testkit::golden::{accepting, assert_golden_text};
use crate::text::width;

/// A fixed-seed xorshift, so the corpus is the same on every run.
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

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

/// The shortest and most frequent words.
const SHORT: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "at", "by", "or", "and", "but", "if", "so", "we",
    "it", "is", "be", "as", "no", "up", "for", "was", "you", "not", "all", "can", "had", "her",
    "one", "our", "out", "day", "get", "has", "him", "his", "how", "new", "now", "old", "see",
    "two", "way", "who", "did", "its", "let", "put", "say", "she", "too", "use", "I",
];

/// Words of five to nine letters.
const COMMON: &[&str] = &[
    "notes",
    "release",
    "design",
    "review",
    "upgrade",
    "steps",
    "after",
    "before",
    "cache",
    "index",
    "schema",
    "changes",
    "quokka",
    "wombat",
    "garden",
    "river",
    "window",
    "letter",
    "summer",
    "planet",
    "kitchen",
    "morning",
    "journey",
    "careful",
    "quietly",
    "between",
    "against",
    "through",
    "without",
    "nothing",
    "anyone",
    "people",
    "number",
    "simple",
    "rather",
    "little",
    "around",
    "measure",
    "picture",
    "question",
    "together",
    "remember",
    "language",
    "sentence",
    "paragraph",
    "chapter",
    "library",
    "builder",
    "decision",
    "evidence",
    "argument",
    "estimate",
    "balance",
    "pattern",
    "blanket",
    "solution",
    "version",
    "feature",
    "history",
    "context",
    "weather",
    "mountain",
    "village",
    "teacher",
    "student",
    "market",
    "family",
    "minute",
    "country",
    "spring",
    "yellow",
    "orange",
];

/// Long words, up to twenty letters.
const LONG: &[&str] = &[
    "understanding",
    "responsibilities",
    "characteristics",
    "internationalization",
    "uncharacteristically",
    "misunderstanding",
    "extraordinarily",
    "notwithstanding",
    "acknowledgement",
    "straightforward",
    "nevertheless",
    "comprehensive",
];

/// Words with a hyphen or an apostrophe inside.
const JOINED: &[&str] = &[
    "follow-up",
    "well-known",
    "state-of-the-art",
    "up-to-date",
    "end-to-end",
    "long-term",
    "self-evident",
    "twenty-one",
    "mother-in-law's",
    "don't",
    "it's",
    "we'll",
    "they're",
    "isn't",
    "wouldn't",
    "o'clock",
    "rock'n'roll",
];

/// Figures and abbreviations: dots, colons, and slashes inside a word
/// that is neither a path nor an address.
const FIGURES: &[&str] = &[
    "3.14", "$12.99", "10:30", "2026", "42%", "1st", "7th", "24/7", "50/50", "e.g.", "i.e.",
    "U.S.", "a.m.", "Ph.D.", "No.", "vs.", "etc.", "#4",
];

/// Punctuation that closes a word.
const CLOSERS: &[&str] = &[",", ",", ",", ".", ";", ":", "?", "!", "..."];

/// Quotes and brackets around a word.
const PAIRS: &[(&str, &str)] = &[
    ("\"", "\""),
    ("(", ")"),
    ("'", "'"),
    ("\u{201c}", "\u{201d}"),
];

/// One word of prose, with the punctuation around it.
fn word(generator: &mut Gen) -> String {
    let mut word = match generator.below(100) {
        0..30 => generator.pick(SHORT).to_string(),
        30..76 => generator.pick(COMMON).to_string(),
        76..84 => generator.pick(LONG).to_string(),
        84..92 => generator.pick(JOINED).to_string(),
        92..97 => generator.pick(FIGURES).to_string(),
        _ => format!(
            "{}\u{2014}{}",
            generator.pick(COMMON),
            generator.pick(COMMON)
        ),
    };
    if generator.chance(5) {
        let (open, close) = PAIRS[generator.below(PAIRS.len())];
        word = format!("{open}{word}{close}");
    }
    if generator.chance(12) {
        word.push_str(generator.pick(CLOSERS));
    }
    word
}

/// A sentence of 6 to 22 words, and the byte range of each word.
fn sentence(generator: &mut Gen) -> (String, Vec<Range<usize>>) {
    let count = 6 + generator.below(17);
    let mut text = String::new();
    let mut words = Vec::new();
    for index in 0..count {
        if index > 0 {
            // Two spaces after a full stop now and then, as some writers
            // type them.
            let gap = if text.ends_with('.') && generator.chance(30) {
                "  "
            } else {
                " "
            };
            text.push_str(gap);
        }
        let mut word = word(generator);
        if index == 0
            && let Some(first) = word.chars().next()
        {
            word = format!("{}{}", first.to_uppercase(), &word[first.len_utf8()..]);
        }
        let at = text.len();
        text.push_str(&word);
        words.push(at..text.len());
    }
    if !text.ends_with(['.', '?', '!']) {
        text.push('.');
    }
    (text, words)
}

/// `range` without the characters at either end that are not letters or
/// digits: the part of a word a search for it matches.
fn core(text: &str, range: &Range<usize>) -> Range<usize> {
    let piece = &text[range.clone()];
    let start = piece.len()
        - piece
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .len();
    let end = piece.trim_end_matches(|c: char| !c.is_alphanumeric()).len();
    if start < end {
        range.start + start..range.start + end
    } else {
        range.clone()
    }
}

/// Two to three characters from inside the word at `range`, when the
/// word has at least five; the start of a search for part of a word.
fn inside(generator: &mut Gen, text: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let core = core(text, range);
    let chars: Vec<usize> = text[core.clone()]
        .char_indices()
        .map(|(at, _)| core.start + at)
        .chain([core.end])
        .collect();
    let count = chars.len() - 1;
    if count < 5 {
        return None;
    }
    let length = 2 + generator.below(2);
    let from = 1 + generator.below(count - length - 1);
    Some(chars[from]..chars[from + length])
}

/// The corpus: each sentence with a whole-word match, a match inside a
/// word, and a match over two neighboring words.
fn cases() -> Vec<(String, Range<usize>)> {
    let mut generator = Gen(0x7072_6f73_6531_0001);
    let mut out = Vec::new();
    for _ in 0..40 {
        let (text, words) = sentence(&mut generator);
        let one = generator.below(words.len());
        out.push((text.clone(), core(&text, &words[one])));
        let piece = (0..words.len()).find_map(|_| {
            let at = generator.below(words.len());
            inside(&mut generator, &text, &words[at])
        });
        if let Some(piece) = piece {
            out.push((text.clone(), piece));
        }
        let pair = generator.below(words.len() - 1);
        let from = core(&text, &words[pair]).start;
        let to = core(&text, &words[pair + 1]).end;
        out.push((text, from..to));
    }
    out
}

/// A fit with its highlight in brackets.
fn bracketed(text: &str, highlight: Option<&Range<usize>>) -> String {
    match highlight {
        Some(range) => format!(
            "{}[{}]{}",
            &text[..range.start],
            &text[range.clone()],
            &text[range.end..]
        ),
        None => text.to_string(),
    }
}

/// The fits of one case, from budget 0 up.
struct Fits {
    case: String,
    fits: Vec<String>,
}

/// Every fit of the corpus.
fn corpus_fits() -> Vec<Fits> {
    cases()
        .iter()
        .enumerate()
        .map(|(index, (text, range))| Fits {
            case: format!("case {index} [{}] |{text}|", &text[range.clone()]),
            fits: (0..=width(text) + 1)
                .map(|budget| {
                    let fitted = fit_snippet(text, range.clone(), budget, &[]);
                    bracketed(&fitted.text, fitted.highlight.as_ref())
                })
                .collect(),
        })
        .collect()
}

/// `cases` in the golden's form: each case line, then a line for each run
/// of budgets with the same fit (`from-to |fit|`, or `budget |fit|`).
fn written(cases: &[Fits]) -> String {
    let mut out = String::new();
    for case in cases {
        let _ = writeln!(out, "{}", case.case);
        let mut from = 0;
        while from < case.fits.len() {
            let fit = &case.fits[from];
            let to = from
                + case.fits[from..]
                    .iter()
                    .take_while(|next| *next == fit)
                    .count()
                - 1;
            let budgets = if to == from {
                from.to_string()
            } else {
                format!("{from}-{to}")
            };
            let _ = writeln!(out, "{budgets:>9} |{fit}|");
            from = to + 1;
        }
    }
    out
}

/// The cases [`written`] wrote, read back; `None` when a line is in
/// neither form or a run of budgets does not follow on from the last.
fn read(golden: &str) -> Option<Vec<Fits>> {
    let mut cases: Vec<Fits> = Vec::new();
    for line in golden.lines() {
        if line.starts_with("case ") {
            cases.push(Fits {
                case: line.to_string(),
                fits: Vec::new(),
            });
            continue;
        }
        let (budgets, fit) = line.trim_start().split_once(' ')?;
        let fit = fit.strip_prefix('|')?.strip_suffix('|')?;
        let (from, to) = budgets.split_once('-').unwrap_or((budgets, budgets));
        let (from, to): (usize, usize) = (from.parse().ok()?, to.parse().ok()?);
        let case = cases.last_mut()?;
        if from != case.fits.len() || to < from {
            return None;
        }
        case.fits.extend((from..=to).map(|_| fit.to_string()));
    }
    Some(cases)
}

/// A line for each fit where `actual` and `expected` disagree.
fn differences(expected: &[Fits], actual: &[Fits]) -> Vec<String> {
    let mut out = Vec::new();
    if expected.len() != actual.len() {
        out.push(format!(
            "the golden has {} cases and the corpus {}",
            expected.len(),
            actual.len()
        ));
    }
    for (old, new) in expected.iter().zip(actual) {
        if old.case != new.case {
            out.push(format!("golden {}\ncorpus {}", old.case, new.case));
            continue;
        }
        let name = new
            .case
            .split_once(" |")
            .map_or(new.case.as_str(), |(name, _)| name);
        for budget in 0..old.fits.len().max(new.fits.len()) {
            let (was, is) = (old.fits.get(budget), new.fits.get(budget));
            if was != is {
                out.push(format!(
                    "{name} at {budget}: golden |{}|, now |{}|",
                    was.map_or("", String::as_str),
                    is.map_or("", String::as_str)
                ));
            }
        }
    }
    out
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/text/goldens/prose_snippets.golden")
}

#[test]
fn prose_fits_as_its_golden_says() {
    let actual = corpus_fits();
    if !accepting()
        && let Some(expected) = std::fs::read_to_string(golden_path())
            .ok()
            .and_then(|golden| read(&golden))
    {
        let differences = differences(&expected, &actual);
        assert!(
            differences.is_empty(),
            "{} fits differ from the golden; the first:\n{}\n\
             Run with TUIKIT_ACCEPT_GOLDENS=1 to accept them.",
            differences.len(),
            differences[..differences.len().min(20)].join("\n")
        );
    }
    assert_golden_text(&golden_path(), &written(&actual), |path, text| {
        std::fs::write(path, text)
    });
}

#[test]
fn the_corpus_is_ordinary_prose() {
    // Every stretch between blanks is a word, not a token, so every fit
    // in the golden follows the rule for words.
    for (text, _) in cases() {
        for run in text.split_whitespace() {
            assert!(!is_token(&clusters(run)), "`{run}` is a token in {text:?}");
        }
    }
}

#[test]
fn the_golden_form_reads_back_as_written() {
    let cases = corpus_fits();
    let again = read(&written(&cases)).expect("the golden form reads back");
    assert!(differences(&cases, &again).is_empty());
}
