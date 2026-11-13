use super::*;
use crate::viewer::model::{
    TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use tuikit::theme::Theme;

fn state() -> TranscriptPreview {
    let turns = (0..12)
        .map(|i| TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: if [2, 9].contains(&i) {
                format!("turn {i} has the Needle")
            } else {
                format!("turn {i} is plain")
            },
            timestamp: None,
            aborted: false,
            hidden: false,
        })
        .collect();
    let mut state = TranscriptPreview::new(TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".into(),
            session_key: "s".into(),
            cwd: None,
        },
        turns,
    });
    state.transcript.lines(&Theme::default(), 60);
    state
}

fn search(turn: Option<usize>) -> PreviewSearch {
    PreviewSearch {
        query: "needle".into(),
        turn,
    }
}

#[test]
fn a_search_marks_its_matches_and_lands_on_the_hit_turn() {
    let mut state = state();

    apply(&mut state, Some(&search(Some(9))));

    assert_eq!(state.marks.matches, [2, 9]);
    assert_eq!(state.marks.current, Some(9));
}

#[test]
fn a_search_without_a_hit_turn_marks_but_has_no_current_match() {
    let mut state = state();

    apply(&mut state, Some(&search(None)));

    assert_eq!(state.marks.matches, [2, 9]);
    assert_eq!(state.marks.current, None);
    assert_eq!(match_top(&state), None);
}

#[test]
fn a_hit_turn_the_transcript_hides_is_not_current() {
    let mut state = state();

    apply(&mut state, Some(&search(Some(4))));

    assert_eq!(state.marks.current, None);
}

#[test]
fn the_next_hit_moves_the_current_match_without_recomputing() {
    let mut state = state();
    apply(&mut state, Some(&search(Some(2))));

    apply(&mut state, Some(&search(Some(9))));

    assert_eq!(state.marks.current, Some(9));
}

#[test]
fn no_search_clears_the_marks() {
    let mut state = state();
    apply(&mut state, Some(&search(Some(2))));

    apply(&mut state, None);

    assert!(!state.marks.is_active());
    assert!(state.marks.matches.is_empty());
}

#[test]
fn the_window_starts_a_line_above_the_current_turn() {
    let mut state = state();
    apply(&mut state, Some(&search(Some(9))));

    let cache = state.transcript.rendered.as_ref().unwrap();
    let line = cache
        .turn_starts
        .iter()
        .find(|start| start.turn_index == 9)
        .unwrap()
        .line;
    assert_eq!(match_top(&state), Some(line - 1));
}

fn rows(count: usize, matching: &[usize]) -> Vec<Line<'static>> {
    (0..count)
        .map(|i| {
            if matching.contains(&i) {
                Line::raw(format!("row {i} NEEDLE"))
            } else {
                Line::raw(format!("row {i}"))
            }
        })
        .collect()
}

fn text_of(text: &Text<'static>) -> Vec<String> {
    text.lines.iter().map(row_text).collect()
}

#[test]
fn a_pane_shows_its_newest_rows_when_one_matches_there() {
    let style = Style::new().bold();

    let text = mark_rows(rows(30, &[28]), "needle", style, 5);

    assert_eq!(text.lines.len(), 5);
    assert_eq!(text_of(&text)[0], "row 25");
    let marked = &text.lines[3];
    assert!(marked.spans.iter().any(|span| span.style == style));
}

#[test]
fn a_pane_scrolls_back_to_a_match_above_its_newest_rows() {
    let text = mark_rows(rows(40, &[10]), "needle", Style::new().bold(), 6);

    let shown = text_of(&text);
    assert!(
        shown.iter().any(|row| row.contains("row 10 NEEDLE")),
        "{shown:?}"
    );
    assert_eq!(shown.len(), 6);
}

#[test]
fn a_pane_without_a_match_keeps_the_newest_rows() {
    let text = mark_rows(rows(20, &[]), "needle", Style::new().bold(), 4);

    assert_eq!(text_of(&text), ["row 16", "row 17", "row 18", "row 19"]);
}
