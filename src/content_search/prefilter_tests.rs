use super::prefilter::{contains_all, contains_ignore_ascii_case, safe_runs};
use super::test_support::{BODIES, corpus};
use crate::viewer::search::{ContentKinds, ContentQuery, match_document};

#[test]
fn runs_are_the_longest_unescapable_pieces_longest_first() {
    assert_eq!(
        safe_runs("find the Needle.rs file"),
        ["needle.rs", "find", "file"]
    );
}

#[test]
fn runs_are_capped_and_ignore_short_pieces() {
    assert_eq!(safe_runs("aaaa bbbbb cccccc ddddddd").len(), 3);
    assert!(
        safe_runs("a b c do it").is_empty(),
        "no run of three or more characters"
    );
}

#[test]
fn escapable_and_non_ascii_characters_break_runs() {
    // Quotes, backslashes, slashes, newlines, and non-ASCII may be stored
    // escaped, so they are never part of a run.
    assert_eq!(safe_runs("say \"hello\""), ["hello", "say"]);
    assert_eq!(safe_runs("C:\\Users\\dev"), ["users", "dev"]);
    assert_eq!(safe_runs("path/to/file.rs"), ["file.rs", "path"]);
    assert_eq!(safe_runs("caf\u{e9}s\u{e9}"), ["caf"]);
    assert!(safe_runs("ca\u{e9}").is_empty());
    assert_eq!(safe_runs("one\ntwo three"), ["three", "one", "two"]);
}

#[test]
fn a_query_with_nothing_usable_has_no_runs() {
    assert!(safe_runs("\"\"").is_empty());
    assert!(safe_runs("  ").is_empty());
    assert!(safe_runs("\u{65e5}\u{672c}\u{8a9e}").is_empty());
}

#[test]
fn bytes_match_ignoring_ascii_case_only() {
    assert!(contains_ignore_ascii_case(b"Hello WORLD", b"o wor"));
    assert!(contains_ignore_ascii_case(b"abc", b""));
    assert!(!contains_ignore_ascii_case(b"abc", b"abcd"));
    assert!(!contains_ignore_ascii_case(b"ab", b"abc"));
    // A match that starts at a false first byte still finds the later one.
    assert!(contains_ignore_ascii_case(b"aaab", b"aab"));
    assert!(!contains_ignore_ascii_case(
        "CAF\u{c9}".as_bytes(),
        "caf\u{e9}".as_bytes()
    ));
}

#[test]
fn every_run_must_be_present() {
    let runs = vec!["alpha".to_string(), "beta".to_string()];
    assert!(contains_all(b"BETA then Alpha", &runs));
    assert!(!contains_all(b"only alpha", &runs));
}

/// Every substring of every body in a transcript that the matcher finds
/// is also admitted by the prefilter: a false positive costs a parse, but
/// a false negative would lose a hit.
#[test]
fn the_prefilter_never_drops_a_file_the_matcher_matches() {
    let corpus = corpus();
    let kinds = ContentKinds::default();
    let mut checked = 0usize;
    for session in &corpus.sessions {
        let Ok(document) = session.parser.read(&session.locator) else {
            continue;
        };
        let index = session.parser.index(&session.locator.state_root);
        let source = index.locate(&session.key.session_key);
        let bytes = match &source {
            Some(crate::viewer::parser::TranscriptSource::File(path)) => {
                Some(std::fs::read(path).expect("read transcript"))
            }
            _ => None,
        };
        for turn in &document.turns {
            let body: Vec<char> = turn.body.chars().collect();
            for start in 0..body.len() {
                for end in (start + 1)..=body.len().min(start + 24) {
                    let text: String = body[start..end].iter().collect();
                    let query = ContentQuery::new(text, kinds);
                    let Some(needle) = query.needle() else {
                        continue;
                    };
                    if match_document(&document, &query).is_empty() {
                        continue;
                    }
                    let runs = safe_runs(needle);
                    if runs.is_empty() {
                        continue;
                    }
                    checked += 1;
                    if let Some(bytes) = &bytes {
                        assert!(
                            contains_all(bytes, &runs),
                            "{}: prefilter dropped a file that matches {needle:?} (runs {runs:?})",
                            session.key.session_key
                        );
                    } else {
                        let candidates = session
                            .parser
                            .candidates(&session.locator.state_root, &runs)
                            .expect("opencode can narrow");
                        assert!(
                            candidates.contains(&session.key.session_key),
                            "{}: candidates dropped a session that matches {needle:?} (runs {runs:?})",
                            session.key.session_key
                        );
                    }
                }
            }
        }
    }
    assert!(
        checked > 2000,
        "the property was exercised ({checked} queries)"
    );
}

#[test]
fn the_corpus_covers_every_escaping_case() {
    // Guards the property test: the bodies include each hazard.
    let joined = BODIES.join("\u{0}");
    for hazard in ['"', '\\', '\n', '/', '\t', '\u{e9}', '\u{1f600}'] {
        assert!(joined.contains(hazard), "{hazard:?} is in the corpus");
    }
}
