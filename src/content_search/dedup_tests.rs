use super::dedup::*;

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|l| normalize_for_compare(l)).collect()
}

fn bodies(text: &[&str]) -> Vec<String> {
    text.iter().map(|b| normalize_for_compare(b)).collect()
}

#[test]
fn normalizing_folds_case_whitespace_and_borders() {
    assert_eq!(
        normalize_for_compare("  The  Retry\tBudget "),
        "the retry budget"
    );
    assert_eq!(
        normalize_for_compare("\u{2502} retry \u{2502} budget \u{2502}"),
        "retry budget"
    );
    assert_eq!(normalize_for_compare("\u{2588}\u{2588}"), "");
    assert_eq!(
        normalize_for_compare("caf\u{e9} \u{c9}"),
        "caf\u{e9} \u{c9}"
    );
}

#[test]
fn a_long_line_inside_a_session_body_is_a_duplicate() {
    let pane = lines(&["the retry budget is exhausted after three attempts"]);
    let session = bodies(&["Well: the retry budget is exhausted after three attempts. Next."]);
    assert!(is_duplicate(&pane, 0, &session));
}

#[test]
fn a_line_the_session_never_said_is_kept() {
    let pane = lines(&["$ grep retry budget src/lib.rs"]);
    let session = bodies(&["the retry budget is exhausted after three attempts"]);
    assert!(!is_duplicate(&pane, 0, &session));
    assert!(!is_duplicate(&pane, 7, &session), "out of range");
}

#[test]
fn borders_and_wrapping_do_not_defeat_the_comparison() {
    let pane = lines(&["\u{2502} the retry budget is   exhausted \u{2502}"]);
    let session = bodies(&["The retry budget is exhausted after three attempts"]);
    assert!(is_duplicate(&pane, 0, &session));
}

#[test]
fn a_short_line_is_judged_with_its_neighbors() {
    let session = bodies(&["we hit the retry budget again"]);
    // Alone, "retry" is short and would match anything; with its
    // neighbors it is the session's sentence.
    let mirrored = lines(&["we hit the", "retry", "budget again"]);
    assert!(is_duplicate(&mirrored, 1, &session));
    let unrelated = lines(&["$ ls", "retry", "$ cd"]);
    assert!(!is_duplicate(&unrelated, 1, &session));
}

#[test]
fn a_line_of_the_threshold_length_stands_alone() {
    assert_eq!("retry budget".chars().count(), SHORT_LINE_CHARS);
    let session = bodies(&["we hit the retry budget again"]);
    let pane = lines(&["$ ls", "retry budget", "$ cd"]);
    assert!(is_duplicate(&pane, 1, &session));
}

#[test]
fn a_blank_neighbor_is_not_joined() {
    let session = bodies(&["we hit the retry budget again"]);
    // Joined with "zzz" the line would not match; the blank line keeps
    // them apart, so "retry" is judged on its own.
    let pane = lines(&["zzz", "", "retry"]);
    assert!(is_duplicate(&pane, 2, &session));
    let joined = lines(&["zzz", "retry"]);
    assert!(!is_duplicate(&joined, 1, &session));
}

#[test]
fn dropping_counts_what_it_removed() {
    let pane = lines(&[
        "the retry budget is exhausted after three attempts",
        "$ cargo test retry",
        "the retry budget is exhausted after three attempts",
    ]);
    let session = bodies(&["the retry budget is exhausted after three attempts"]);
    let (kept, dropped) = drop_duplicates(&pane, &[0, 1, 2], &session);
    assert_eq!(kept, [1]);
    assert_eq!(dropped, 2);
}

#[test]
fn with_no_session_bodies_nothing_is_dropped() {
    let pane = lines(&["a retry budget line"]);
    assert_eq!(drop_duplicates(&pane, &[0], &[]), (vec![0], 0));
}
