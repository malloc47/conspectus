use ratatui::style::{Modifier, Style};

use super::*;

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn texts(spans: &[Span<'_>]) -> Vec<String> {
    spans.iter().map(|span| span.content.to_string()).collect()
}

#[test]
fn an_end_cut_keeps_text_that_fits_and_marks_text_that_does_not() {
    assert_eq!(truncate_end("alpha", 5), "alpha");
    assert_eq!(truncate_end("alpha", 10), "alpha");
    assert_eq!(truncate_end("alphabetagamma", 8), "alphabe…");
    assert_eq!(truncate_end("", 5), "");
}

#[test]
fn an_end_cut_at_the_smallest_budgets_is_empty_or_only_the_mark() {
    assert_eq!(truncate_end("abc", 0), "");
    assert_eq!(truncate_end("abc", 1), "…");
    assert_eq!(truncate_end("a", 1), "a");
}

#[test]
fn an_end_cut_counts_cells_not_characters() {
    assert_eq!(truncate_end("東京都庁", 4), "東…");
    // The second character would straddle the budget: it goes whole.
    assert_eq!(truncate_end("東京都庁", 5), "東京…");
    assert_eq!(truncate_end("ab東京", 3), "ab…");
    // A combining accent rides with its letter and takes no cell.
    assert_eq!(truncate_end("cafe\u{301} society", 5), "cafe\u{301}…");
    // An emoji with a presentation selector is two cells, cut whole.
    assert_eq!(truncate_end("ok \u{26a0}\u{fe0f} warn", 5), "ok …");
}

#[test]
fn spaces_alone_past_the_budget_are_dropped_without_a_mark() {
    assert_eq!(truncate_end("abcd   ", 5), "abcd ");
    assert_eq!(truncate_end("ab   cd", 4), "ab …");
}

#[test]
fn a_middle_cut_keeps_both_ends() {
    assert_eq!(
        truncate_middle("/srv/data/project", 30),
        "/srv/data/project"
    );
    assert_eq!(
        truncate_middle("/srv/archive-demo/data-a", 15),
        "/srv/ar…/data-a"
    );
    // An odd budget gives the start the larger half.
    assert_eq!(truncate_middle("abcdefghij", 6), "abc…ij");
    assert_eq!(truncate_middle("abc", 1), "…");
    assert_eq!(truncate_middle("abc", 0), "");
}

#[test]
fn a_middle_cut_gives_a_cell_the_start_cannot_use_to_the_end() {
    // Five usable cells: the start may take three, but `東` would make
    // it four, so the end takes the spare cell.
    assert_eq!(truncate_middle("ab東京xyz", 6), "ab…xyz");
    assert_eq!(truncate_middle("東京都庁舎前", 7), "東…舎前");
}

#[test]
fn a_hard_cut_has_no_mark() {
    assert_eq!(clip("0123456789abcdef", 8), "01234567");
    assert_eq!(clip("short", 8), "short");
    assert_eq!(clip("東京都", 3), "東");
    assert_eq!(clip("x", 0), "");
}

#[test]
fn padding_fills_a_column_and_never_cuts() {
    assert_eq!(pad("id", 4), "id  ");
    assert_eq!(pad("東", 4), "東  ");
    assert_eq!(pad("toolong", 4), "toolong");
    assert_eq!(pad_start("7", 3), "  7");
    assert_eq!(pad_start("東", 3), " 東");
}

#[test]
fn fitting_makes_a_column_exactly_its_width() {
    assert_eq!(fit("id", 4), "id  ");
    assert_eq!(fit("identifier", 4), "ide…");
    assert_eq!(fit("東京都", 4), "東… ");
    assert_eq!(fit("anything", 0), "");
}

#[test]
fn spans_that_fit_are_kept_as_they_are() {
    let spans = vec![Span::raw("ab"), Span::styled("cd", bold())];
    assert_eq!(fit_spans(spans.clone(), 4), spans);
}

#[test]
fn the_span_the_budget_runs_out_in_is_cut_and_marked_in_its_style() {
    let spans = vec![Span::raw("ab"), Span::styled("cdef", bold())];
    let fitted = fit_spans(spans, 4);
    assert_eq!(texts(&fitted), ["ab", "c", "…"]);
    assert_eq!(fitted[2].style, bold());
}

#[test]
fn a_cut_at_a_span_boundary_still_ends_in_the_mark() {
    let spans = vec![Span::raw("abcd"), Span::styled("ef", bold())];
    let fitted = fit_spans(spans, 4);
    assert_eq!(texts(&fitted), ["abc", "…"]);
    assert_eq!(fitted[1].style, Style::default());
}

#[test]
fn spans_of_spaces_past_the_budget_go_without_a_mark() {
    let spans = vec![Span::raw("abcd"), Span::raw("  ")];
    assert_eq!(texts(&fit_spans(spans, 4)), ["abcd"]);
    let padded = vec![Span::raw("ab"), Span::styled("cd   ", bold())];
    assert_eq!(texts(&fit_spans(padded, 5)), ["ab", "cd "]);
}

#[test]
fn spans_cut_to_nothing_leave_nothing() {
    assert!(fit_spans(vec![Span::raw("abc")], 0).is_empty());
    assert_eq!(texts(&fit_spans(vec![Span::raw("abc")], 1)), ["…"]);
}
