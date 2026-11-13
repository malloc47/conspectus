use ratatui::crossterm::event::KeyCode;
use tuikit::overlay::Outcome;
use tuikit::testkit::keys::{ctrl, press as key, script};
use tuikit::theme::Theme;

use super::*;
use crate::viewer::model::{
    TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use crate::viewer::widget::{buffer_to_string, render_to_buffer};

fn turn(role: TurnRole, kind: TurnKind, body: &str) -> TranscriptTurn {
    TranscriptTurn {
        role,
        kind,
        body: body.to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    }
}

fn message(index: usize, body: &str) -> TranscriptTurn {
    let role = if index.is_multiple_of(2) {
        TurnRole::User
    } else {
        TurnRole::Assistant
    };
    turn(role, TurnKind::Message, body)
}

/// Twelve messages; turns 2, 5, and 9 mention "needle".
fn document() -> TranscriptDocument {
    let turns = (0..12)
        .map(|i| {
            let body = if [2, 5, 9].contains(&i) {
                format!("turn {i} has the Needle in it")
            } else {
                format!("turn {i} is plain")
            };
            message(i, &body)
        })
        .collect();
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "find".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns,
    }
}

type Viewer = Pager<Transcript>;

/// The viewer over `document`, opened as the TUI opens it, landed on
/// `open` when given.
fn open(document: TranscriptDocument, open: Option<&ViewerOpen>) -> Viewer {
    viewer(Transcript::new(document), open)
}

/// A fresh viewer that has been drawn once, so the line map exists.
fn drawn(document: TranscriptDocument) -> Viewer {
    let mut viewer = open(document, None);
    draw(&mut viewer, 80, 8);
    viewer
}

fn draw(viewer: &mut Viewer, width: u16, height: u16) -> String {
    buffer_to_string(&render_to_buffer(viewer, &Theme::default(), width, height))
}

/// Press `keys`, a key script, and answer the last key's outcome.
fn press(viewer: &mut Viewer, keys: &str) -> Outcome<()> {
    let mut last = Outcome::Consumed;
    for key in script(keys) {
        last = viewer.handle_key(key);
    }
    last
}

/// Open the prompt, type `query` over what it holds, confirm, and draw
/// so the scroll lands.
fn find(viewer: &mut Viewer, query: &str) {
    press(viewer, &format!("/<C-u>{query}<Enter>"));
    draw(viewer, 80, 8);
}

fn first_line_of(viewer: &Viewer, turn: usize) -> usize {
    viewer.content().rows_of(turn).start
}

/// The query the open prompt holds.
fn prompt_text(viewer: &Viewer) -> Option<&str> {
    viewer
        .find()
        .prompt()
        .map(tuikit::text_edit::LineEdit::value)
}

fn note(viewer: &Viewer) -> Option<&str> {
    viewer.note().map(|note| note.text.as_ref())
}

fn open_at(document: TranscriptDocument, query: &str, turn_index: usize) -> Viewer {
    let at = ViewerOpen {
        query: query.to_string(),
        turn_index,
    };
    open(document, Some(&at))
}

#[test]
fn the_prompt_edits_like_every_text_field() {
    let mut viewer = drawn(document());
    press(&mut viewer, "/alpha beta<A-b>X");
    assert_eq!(prompt_text(&viewer), Some("alpha Xbeta"));
    press(&mut viewer, "<C-w>");
    assert_eq!(prompt_text(&viewer), Some("alpha beta"));
    // `Ctrl-U` deletes the whole line, wherever the cursor is.
    press(&mut viewer, "<Left><C-u>");
    assert_eq!(prompt_text(&viewer), Some(""));
    press(&mut viewer, "<C-y>");
    assert_eq!(prompt_text(&viewer), Some("alpha beta"));
    press(&mut viewer, "q");
    assert_eq!(prompt_text(&viewer), Some("alpha betaq"), "q types");
}

#[test]
fn confirming_jumps_to_the_first_match_below_the_top() {
    let mut viewer = drawn(document());
    press(&mut viewer, "g");
    find(&mut viewer, "needle");
    assert_eq!(viewer.find().matches(), [2, 5, 9]);
    assert_eq!(viewer.find().current(), Some(0));
    assert_eq!(
        viewer.window().offset(),
        first_line_of(&viewer, 2).saturating_sub(1)
    );
    assert!(viewer.find().prompt().is_none(), "the prompt closed");
}

#[test]
fn the_search_starts_from_the_viewport_not_the_top() {
    let mut viewer = drawn(document());
    press(&mut viewer, "g");
    let row = first_line_of(&viewer, 6);
    viewer.scroll_by(i32::try_from(row).unwrap());
    draw(&mut viewer, 80, 8);
    find(&mut viewer, "needle");
    assert_eq!(
        viewer.find().current_unit(),
        Some(9),
        "first match at or after"
    );
}

#[test]
fn a_search_past_the_last_match_goes_to_the_first() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    assert_eq!(viewer.find().current_unit(), Some(2));
}

#[test]
fn n_and_capital_n_step_and_wrap_around() {
    let mut viewer = drawn(document());
    press(&mut viewer, "g");
    find(&mut viewer, "needle");
    press(&mut viewer, "n");
    assert_eq!(viewer.find().current_unit(), Some(5));
    press(&mut viewer, "n");
    assert_eq!(viewer.find().current_unit(), Some(9));
    press(&mut viewer, "n");
    assert_eq!(viewer.find().current_unit(), Some(2), "wraps to the first");
    assert_eq!(note(&viewer), Some("wrapped to the first match"));
    press(&mut viewer, "N");
    assert_eq!(viewer.find().current_unit(), Some(9), "wraps to the last");
    assert_eq!(note(&viewer), Some("wrapped to the last match"));
    press(&mut viewer, "N");
    assert_eq!(viewer.find().current_unit(), Some(5));
    assert_eq!(note(&viewer), None, "the note lasts one key");
}

#[test]
fn stepping_scrolls_the_match_a_row_below_the_top() {
    let mut viewer = drawn(document());
    press(&mut viewer, "g");
    find(&mut viewer, "needle");
    press(&mut viewer, "n");
    draw(&mut viewer, 80, 8);
    assert_eq!(viewer.window().offset(), first_line_of(&viewer, 5) - 1);
}

#[test]
fn n_without_a_find_says_there_is_nothing_to_step() {
    let mut viewer = drawn(document());
    press(&mut viewer, "n");
    assert!(note(&viewer).is_some_and(|n| n.contains("nothing to find")));
}

#[test]
fn escape_cancels_the_prompt_and_keeps_the_earlier_find() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    press(&mut viewer, "/");
    assert_eq!(
        prompt_text(&viewer),
        Some("needle"),
        "the prompt starts from the active query"
    );
    press(&mut viewer, "zzz<Esc>");
    assert!(viewer.find().prompt().is_none());
    assert_eq!(viewer.find().query(), "needle");
    assert_eq!(viewer.find().matches().len(), 3);
}

#[test]
fn escape_clears_the_find_before_it_closes_the_viewer() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    assert_eq!(viewer.handle_key(key(KeyCode::Esc)), Outcome::Consumed);
    assert!(!viewer.find().is_active(), "the find cleared");
    assert_eq!(viewer.handle_key(key(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_leaves_the_viewer_at_once_even_with_a_find_active() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    assert!(viewer.find().is_active());
    assert_eq!(press(&mut viewer, "q"), Outcome::Dismiss);
}

#[test]
fn q_leaves_the_viewer_even_with_the_help_open() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    assert_eq!(press(&mut viewer, "?q"), Outcome::Dismiss);
}

#[test]
fn esc_with_the_help_open_closes_only_the_help() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    assert_eq!(press(&mut viewer, "?<Esc>"), Outcome::Consumed);
    assert!(!viewer.help_is_open(), "the help closed");
    assert!(viewer.find().is_active(), "the find stayed");
}

#[test]
fn ctrl_c_from_the_find_prompt_leaves_the_viewer() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    press(&mut viewer, "/");
    assert!(viewer.find().prompt().is_some());
    assert_eq!(viewer.handle_key(ctrl('c')), Outcome::Dismiss);
}

#[test]
fn an_empty_query_clears_the_find() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    find(&mut viewer, "");
    assert!(!viewer.find().is_active());
    assert!(viewer.find().matches().is_empty());
}

#[test]
fn a_new_query_replaces_the_old_find() {
    let mut viewer = drawn(document());
    find(&mut viewer, "needle");
    find(&mut viewer, "plain");
    assert_eq!(viewer.find().query(), "plain");
    assert_eq!(viewer.find().matches().len(), 9);
}

#[test]
fn the_footer_shows_the_position_and_no_match() {
    let mut viewer = drawn(document());
    press(&mut viewer, "g");
    find(&mut viewer, "needle");
    let screen = draw(&mut viewer, 100, 8);
    assert!(screen.contains("find \"needle\" 1/3"), "{screen}");
    find(&mut viewer, "zebra");
    let screen = draw(&mut viewer, 100, 8);
    assert!(screen.contains("find \"zebra\" no match"), "{screen}");
}

#[test]
fn the_prompt_replaces_the_footer_and_shows_its_keys() {
    let mut viewer = drawn(document());
    press(&mut viewer, "/abc");
    let screen = draw(&mut viewer, 80, 8);
    let last = screen.lines().last().expect("a footer");
    assert!(last.starts_with("/abc"), "{last:?}");
    assert!(last.contains("Enter find · Esc cancel"), "{last:?}");
}

#[test]
fn the_prompt_draws_its_caret_and_windows_by_cells() {
    use tuikit::testkit::golden;
    let theme = Theme::default();
    let mut viewer = drawn(document());
    press(&mut viewer, "/東京 needle<Home><Right>");
    let buffer = render_to_buffer(&mut viewer, &theme, 60, 8);
    let caret = &buffer[(3, 7)];
    assert_eq!(caret.symbol(), "京", "the caret is on the second character");
    let cue = theme.field_cursor.into_style();
    assert_eq!((Some(caret.fg), Some(caret.bg)), (cue.fg, cue.bg));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/viewer/goldens/find-prompt.golden");
    let footer = ratatui::layout::Rect::new(0, 7, 60, 1);
    golden::assert_golden_text(
        &path,
        &golden::styled_region(&buffer, footer),
        |path, text| std::fs::write(path, text),
    );
    // A query longer than the room before the hint scrolls; the hint
    // stays put.
    press(&mut viewer, &format!("<End>{}", "x".repeat(60)));
    let screen = draw(&mut viewer, 60, 8);
    let last = screen.lines().last().expect("a footer");
    assert!(last.starts_with("/xxx"), "{last:?}");
    assert!(
        last.trim_end().ends_with("Enter find · Esc cancel"),
        "{last:?}"
    );
}

fn tool_document() -> TranscriptDocument {
    let mut doc = document();
    doc.turns.push(turn(
        TurnRole::Assistant,
        TurnKind::ToolResult,
        "output: a needle in tool output",
    ));
    doc.turns.push(turn(
        TurnRole::Assistant,
        TurnKind::Thinking,
        "thinking about the needle",
    ));
    doc
}

#[test]
fn hidden_matches_are_counted_but_not_stepped_to() {
    let mut viewer = drawn(tool_document());
    find(&mut viewer, "needle");
    assert_eq!(viewer.find().matches(), [2, 5, 9]);
    assert_eq!(viewer.content().hidden_matches("needle"), 2);
    find(&mut viewer, "tool output");
    let screen = draw(&mut viewer, 120, 8);
    assert!(screen.contains("no match · 1 in hidden turns"), "{screen}");
}

#[test]
fn showing_a_kind_makes_its_matches_findable() {
    let mut viewer = drawn(tool_document());
    press(&mut viewer, "g");
    find(&mut viewer, "needle");
    press(&mut viewer, "T");
    assert_eq!(viewer.find().matches(), [2, 5, 9, 13]);
    assert_eq!(
        viewer.find().current_unit(),
        Some(2),
        "stays on the current match"
    );
    press(&mut viewer, "ttt");
    assert_eq!(viewer.content().tool_detail, ToolDetail::Full);
    assert_eq!(viewer.find().matches(), [2, 5, 9, 12, 13]);
}

#[test]
fn hiding_the_current_match_leaves_no_current_one_until_n() {
    let mut viewer = drawn(tool_document());
    find(&mut viewer, "needle");
    press(&mut viewer, "T");
    press(&mut viewer, "N");
    assert_eq!(viewer.find().current_unit(), Some(13));
    draw(&mut viewer, 80, 8);
    press(&mut viewer, "T");
    assert_eq!(viewer.find().matches(), [2, 5, 9]);
    assert_eq!(viewer.find().current_unit(), None, "its turn is hidden");
    let screen = draw(&mut viewer, 100, 8);
    assert!(screen.contains("find \"needle\" 3 matches"), "{screen}");
    press(&mut viewer, "N");
    assert!(
        viewer.find().current_unit().is_some(),
        "N goes to one again"
    );
}

#[test]
fn a_collapsed_tool_run_has_one_line_for_all_its_turns() {
    let mut doc = document();
    doc.turns
        .push(turn(TurnRole::Assistant, TurnKind::ToolUse, "Read: /a"));
    doc.turns
        .push(turn(TurnRole::Assistant, TurnKind::ToolResult, "ok"));
    let mut transcript: Transcript = Transcript::new(doc);
    transcript.tool_detail = ToolDetail::Summary;
    transcript.lines(&Theme::default(), 80);
    let starts = &transcript.rendered.as_ref().unwrap().turn_starts;
    let run: Vec<_> = starts.iter().filter(|s| s.turn_index >= 12).collect();
    assert_eq!(run.len(), 2);
    assert!(run.iter().all(|s| s.collapsed && s.line == run[0].line));
    assert!(
        starts
            .iter()
            .filter(|s| s.turn_index < 12)
            .all(|s| !s.collapsed)
    );
}

#[test]
fn opening_at_a_match_lands_on_the_turn_with_the_query_active() {
    let mut viewer = open_at(document(), "needle", 5);
    draw(&mut viewer, 80, 8);
    assert_eq!(viewer.find().current_unit(), Some(5));
    assert_eq!(viewer.find().matches(), [2, 5, 9]);
    assert_eq!(
        viewer.window().offset(),
        first_line_of(&viewer, 5).saturating_sub(1)
    );
    assert!(!viewer.follows_end());
    assert_eq!(note(&viewer), None, "a visible match needs no note");
}

#[test]
fn opening_at_a_hidden_tool_hit_shows_full_tool_output_and_says_so() {
    let mut viewer = open_at(tool_document(), "needle", 12);
    assert_eq!(viewer.content().tool_detail, ToolDetail::Full);
    assert_eq!(viewer.find().current_unit(), Some(12));
    assert_eq!(note(&viewer), Some("showing full tool output"));
    let screen = draw(&mut viewer, 120, 8);
    assert!(screen.contains("showing full tool output"), "{screen}");
}

#[test]
fn opening_at_hidden_thinking_aborted_and_hidden_turns_shows_each() {
    let mut doc = document();
    let mut aborted = message(12, "aborted needle");
    aborted.aborted = true;
    let mut hidden = message(13, "hidden needle");
    hidden.hidden = true;
    let mut both = turn(TurnRole::Assistant, TurnKind::Thinking, "needle thought");
    both.aborted = true;
    doc.turns.extend([aborted, hidden, both]);

    let viewer = open_at(doc.clone(), "needle", 12);
    let shown = viewer.content();
    assert!(shown.show_aborted && !shown.show_hidden && !shown.show_thinking);
    assert_eq!(note(&viewer), Some("showing aborted turns"));

    let viewer = open_at(doc.clone(), "needle", 13);
    assert!(viewer.content().show_hidden && !viewer.content().show_aborted);
    assert_eq!(note(&viewer), Some("showing hidden turns"));

    let viewer = open_at(doc, "needle", 14);
    assert!(viewer.content().show_aborted && viewer.content().show_thinking);
    assert_eq!(
        note(&viewer),
        Some("showing aborted turns, showing thinking")
    );
}

#[test]
fn a_changed_transcript_finds_the_nearest_match() {
    // The scan saw the match at turn 7; it is now at 5 and 9.
    let viewer = open_at(document(), "needle", 7);
    assert_eq!(
        viewer.find().current_unit(),
        Some(5),
        "nearest, earlier first"
    );
    assert!(note(&viewer).is_some_and(|n| n.contains("nearest match")));
}

#[test]
fn a_match_far_from_the_remembered_turn_is_still_found() {
    let mut doc = document();
    doc.turns = (0..44).map(|i| message(i, "filler")).collect();
    doc.turns.push(message(0, "a lonely needle"));
    let viewer = open_at(doc, "needle", 3);
    assert_eq!(
        viewer.find().current_unit(),
        Some(44),
        "falls back to a global search"
    );
}

#[test]
fn a_query_that_is_gone_says_so_and_leaves_the_viewer_at_the_end() {
    let viewer = open_at(document(), "vanished", 3);
    assert!(viewer.follows_end());
    assert!(viewer.find().is_active());
    assert!(viewer.find().matches().is_empty());
    let gone = viewer.note().expect("a note");
    assert!(gone.text.contains("no longer in this transcript"));
    assert_eq!(gone.severity, tuikit::feedback::Severity::Warning);
}

#[test]
fn an_empty_open_query_does_nothing() {
    let viewer = open_at(document(), "  ", 3);
    assert!(!viewer.find().is_active() && viewer.follows_end());
}

#[test]
fn the_find_view_at_80_columns() {
    let mut viewer = open_at(document(), "needle", 5);
    insta::assert_snapshot!("find_at_80_columns", draw(&mut viewer, 80, 12));
}

#[test]
fn the_find_view_at_120_columns() {
    let mut viewer = open_at(tool_document(), "needle", 12);
    insta::assert_snapshot!("find_at_120_columns", draw(&mut viewer, 120, 12));
}
