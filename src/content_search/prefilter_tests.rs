use super::prefilter::{contains_all, contains_ignore_ascii_case, file_may_match, safe_runs};
use super::test_support::{BODIES, corpus, synthesized_corpus};
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
fn assert_prefilter_admits_every_match(corpus: &super::test_support::Corpus) -> usize {
    let kinds = ContentKinds::default();
    let mut checked = 0usize;
    for session in &corpus.sessions {
        let Ok(document) = session.parser.read(&session.locator) else {
            continue;
        };
        let index = session.parser.index(&session.locator.state_root);
        let source = index.locate(&session.key.session_key);
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
                    if let Some(crate::viewer::parser::TranscriptSource::File(path)) = &source {
                        assert!(
                            file_may_match(path, &runs).expect("read transcript"),
                            "{}: prefilter dropped a file that matches {needle:?} (runs {runs:?})",
                            session.key.session_key
                        );
                    } else {
                        let candidates = session
                            .parser
                            .candidates(&session.locator.state_root, &runs, &|| false)
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
    checked
}

#[test]
fn the_prefilter_never_drops_a_file_the_matcher_matches() {
    let checked = assert_prefilter_admits_every_match(&corpus());
    assert!(
        checked > 2000,
        "the property was exercised ({checked} queries)"
    );
}

/// The same property over transcripts whose searchable text is not the
/// file's plain text: a branch-summary heading the parser prepends, a
/// message split across blocks, and ASCII spelled as `\u00XX` escapes.
#[test]
fn the_prefilter_admits_synthesized_split_and_ascii_escaped_text() {
    let checked = assert_prefilter_admits_every_match(&synthesized_corpus());
    assert!(
        checked > 100,
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

fn write_file(
    dir: &std::path::Path,
    name: &str,
    head: usize,
    middle: &str,
    tail: &str,
) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut bytes = vec![b'x'; head];
    bytes.extend_from_slice(middle.as_bytes());
    bytes.extend_from_slice(tail.as_bytes());
    std::fs::write(&path, bytes).expect("write");
    path
}

#[test]
fn a_run_straddling_a_read_chunk_is_still_found() {
    // The chunk size is 1 MiB; sweep a needle across that boundary.
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = vec!["alphabeta".to_string()];
    for back in 0..=9 {
        let path = write_file(
            dir.path(),
            &format!("f{back}"),
            (1 << 20) - back,
            "AlphaBeta",
            " rest",
        );
        assert!(
            file_may_match(&path, &runs).expect("read"),
            "needle starting {back} bytes before the boundary"
        );
    }
    let absent = write_file(dir.path(), "absent", 3 << 20, "alphabet", "!");
    assert!(!file_may_match(&absent, &runs).expect("read"));
}

#[test]
fn every_run_must_be_somewhere_in_a_large_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = vec!["first".to_string(), "second".to_string()];
    let path = dir.path().join("big");
    let mut bytes = b"first".to_vec();
    bytes.extend(vec![b'x'; 3 << 20]);
    std::fs::write(&path, &bytes).expect("write");
    assert!(
        !file_may_match(&path, &runs).expect("read"),
        "second is missing"
    );
    bytes.extend_from_slice(b"SECOND");
    std::fs::write(&path, &bytes).expect("write");
    assert!(file_may_match(&path, &runs).expect("read"));
}

#[test]
fn an_escaped_letter_makes_a_file_a_candidate_whatever_it_contains() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = vec!["needle".to_string()];
    // `e` and `N` written as escapes hide the word from a byte comparison.
    for escape in ["\\u0065", "\\u004e", "\\u005f", "\\u002e", "\\u0039"] {
        let path = write_file(dir.path(), "escaped", 100, &format!("ne{escape}dle"), "");
        assert!(file_may_match(&path, &runs).expect("read"), "{escape}");
    }
    // Escapes of characters the filter never relies on do not.
    for escape in ["\\u0022", "\\u0020", "\\u00e9", "\\u001f", "\\u002f"] {
        let path = write_file(dir.path(), "other", 100, &format!("ne{escape}dle"), "");
        assert!(!file_may_match(&path, &runs).expect("read"), "{escape}");
    }
}

#[test]
fn an_escape_straddling_a_chunk_boundary_is_still_seen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = vec!["needle".to_string()];
    for back in 0..=7 {
        let path = write_file(
            dir.path(),
            &format!("e{back}"),
            (1 << 20) - back,
            "\\u0065",
            " tail",
        );
        assert!(
            file_may_match(&path, &runs).expect("read"),
            "{back} bytes before"
        );
    }
}
