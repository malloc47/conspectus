use super::*;
use crate::viewer::input::{ViewerMsg, reduce};
use crate::viewer::model::{
    TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use crate::viewer::theme::Theme;
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

/// A fresh state that has been drawn once, so the line map exists.
fn drawn(document: TranscriptDocument) -> ViewerState {
    let mut state = ViewerState::new(document);
    draw(&mut state, 80, 8);
    state
}

fn draw(state: &mut ViewerState, width: u16, height: u16) -> String {
    buffer_to_string(&render_to_buffer(state, &Theme::default(), width, height))
}

fn msg(state: ViewerState, msg: ViewerMsg) -> ViewerState {
    reduce(state, msg).0
}

fn typed(mut state: ViewerState, text: &str) -> ViewerState {
    for c in text.chars() {
        state = reduce_find(state, ViewerFindMsg::Insert(c)).0;
    }
    state
}

/// Open the prompt, type `query`, confirm, and draw so the scroll lands.
fn find(state: ViewerState, query: &str) -> ViewerState {
    let state = msg(state, ViewerMsg::FindOpen);
    // The prompt starts from the active query; clear it to type a new one.
    let state = reduce_find(state, ViewerFindMsg::ClearLine).0;
    let state = typed(state, query);
    let mut state = reduce_find(state, ViewerFindMsg::Confirm).0;
    draw(&mut state, 80, 8);
    state
}

fn first_line_of(state: &ViewerState, turn: usize) -> usize {
    state
        .rendered
        .as_ref()
        .expect("drawn")
        .turn_starts
        .iter()
        .find(|start| start.turn_index == turn)
        .expect("turn is shown")
        .line
}

#[test]
fn the_prompt_edits_like_a_line() {
    let mut prompt = FindPrompt::default();
    for c in "héllo".chars() {
        prompt.insert(c);
    }
    assert_eq!((prompt.text.as_str(), prompt.cursor), ("héllo", 5));
    prompt.left();
    prompt.left();
    prompt.backspace();
    assert_eq!((prompt.text.as_str(), prompt.cursor), ("hélo", 2));
    prompt.delete();
    assert_eq!(prompt.text, "hélo".replacen('l', "", 1));
    prompt.home();
    prompt.insert('>');
    assert_eq!((prompt.text.as_str(), prompt.cursor), (">héo", 1));
    prompt.end();
    assert_eq!(prompt.cursor, 4);
    prompt.right();
    assert_eq!(prompt.cursor, 4, "the cursor stops at the end");
    prompt.left();
    prompt.clear_line();
    assert_eq!((prompt.text.as_str(), prompt.cursor), ("o", 0));
    prompt.backspace();
    assert_eq!(prompt.text, "o", "backspace at the start does nothing");
}

#[test]
fn confirming_jumps_to_the_first_match_below_the_top() {
    let state = find(drawn(document()), "needle");
    assert_eq!(state.find.matches, [2, 5, 9]);
    assert_eq!(state.find.current, Some(0));
    assert_eq!(
        state.scroll_offset,
        first_line_of(&state, 2).saturating_sub(1)
    );
    assert!(!state.find.prompt.is_some(), "the prompt closed");
}

#[test]
fn the_search_starts_from_the_viewport_not_the_top() {
    let mut state = drawn(document());
    state.scroll_offset = first_line_of(&state, 6);
    let state = find(state, "needle");
    assert_eq!(
        state.find.current_turn(),
        Some(9),
        "first match at or after"
    );
}

#[test]
fn a_search_past_the_last_match_wraps_to_the_first() {
    let mut state = drawn(document());
    state.scroll_offset = first_line_of(&state, 11);
    let state = find(state, "needle");
    assert_eq!(state.find.current_turn(), Some(2));
}

#[test]
fn n_and_capital_n_step_and_wrap_around() {
    let state = find(drawn(document()), "needle");
    let state = msg(state, ViewerMsg::FindNext);
    assert_eq!(state.find.current_turn(), Some(5));
    let state = msg(state, ViewerMsg::FindNext);
    assert_eq!(state.find.current_turn(), Some(9));
    let state = msg(state, ViewerMsg::FindNext);
    assert_eq!(state.find.current_turn(), Some(2), "wraps to the first");
    assert_eq!(
        state.find.note.as_deref(),
        Some("wrapped to the first match")
    );
    let state = msg(state, ViewerMsg::FindPrev);
    assert_eq!(state.find.current_turn(), Some(9), "wraps to the last");
    assert_eq!(
        state.find.note.as_deref(),
        Some("wrapped to the last match")
    );
    let state = msg(state, ViewerMsg::FindPrev);
    assert_eq!(state.find.current_turn(), Some(5));
    assert_eq!(state.find.note, None, "the note lasts one key");
}

#[test]
fn stepping_scrolls_the_match_into_view() {
    let mut state = find(drawn(document()), "needle");
    state = msg(state, ViewerMsg::FindNext);
    draw(&mut state, 80, 8);
    let line = first_line_of(&state, 5);
    assert!(
        (state.scroll_offset..state.scroll_offset + 8).contains(&line),
        "line {line} is outside the viewport at {}",
        state.scroll_offset
    );
}

#[test]
fn n_without_a_find_says_there_is_nothing_to_step() {
    let state = msg(drawn(document()), ViewerMsg::FindNext);
    assert!(
        state
            .find
            .note
            .as_deref()
            .is_some_and(|n| n.contains("nothing to find"))
    );
}

#[test]
fn escape_cancels_the_prompt_and_keeps_the_earlier_find() {
    let state = find(drawn(document()), "needle");
    let state = msg(state, ViewerMsg::FindOpen);
    assert_eq!(
        state.find.prompt.as_ref().map(|p| p.text.as_str()),
        Some("needle"),
        "the prompt starts from the active query"
    );
    let state = typed(state, "zzz");
    let state = reduce_find(state, ViewerFindMsg::Cancel).0;
    assert!(state.find.prompt.is_none());
    assert_eq!(state.find.query, "needle");
    assert_eq!(state.find.matches.len(), 3);
}

#[test]
fn escape_clears_the_find_before_it_closes_the_viewer() {
    let state = find(drawn(document()), "needle");
    let (state, effect) = reduce(state, ViewerMsg::Close);
    assert_eq!(effect, ViewerEffect::None);
    assert!(!state.find.is_active(), "the find cleared");
    let (_, effect) = reduce(state, ViewerMsg::Close);
    assert_eq!(effect, ViewerEffect::Close);
}

#[test]
fn q_leaves_the_viewer_at_once_even_with_a_find_active() {
    let state = find(drawn(document()), "needle");
    assert!(state.find.is_active());
    let (_, effect) = reduce(state, ViewerMsg::Quit);
    assert_eq!(effect, ViewerEffect::Close);
}

#[test]
fn q_with_the_help_open_closes_only_the_help() {
    let state = find(drawn(document()), "needle");
    let (state, _) = reduce(state, ViewerMsg::ToggleHelp);
    let (state, effect) = reduce(state, ViewerMsg::Quit);
    assert_eq!(effect, ViewerEffect::None);
    assert!(!state.show_help, "the help closed");
    assert!(state.find.is_active(), "the find stayed");
    let (_, effect) = reduce(state, ViewerMsg::Quit);
    assert_eq!(effect, ViewerEffect::Close);
}

#[test]
fn an_empty_query_clears_the_find() {
    let state = find(drawn(document()), "needle");
    let state = msg(state, ViewerMsg::FindOpen);
    let state = reduce_find(state, ViewerFindMsg::ClearLine).0;
    let state = reduce_find(state, ViewerFindMsg::Confirm).0;
    assert!(!state.find.is_active());
    assert!(state.find.matches.is_empty());
}

#[test]
fn a_new_query_replaces_the_old_find() {
    let state = find(drawn(document()), "needle");
    let state = find(state, "plain");
    assert_eq!(state.find.query, "plain");
    assert_eq!(state.find.matches.len(), 9);
}

#[test]
fn the_footer_shows_the_position_and_no_match() {
    let mut state = find(drawn(document()), "needle");
    let screen = draw(&mut state, 100, 8);
    assert!(screen.contains("find \"needle\" 1/3"), "{screen}");
    let mut state = find(state, "zebra");
    let screen = draw(&mut state, 100, 8);
    assert!(screen.contains("find \"zebra\" no match"), "{screen}");
}

#[test]
fn the_prompt_replaces_the_footer_and_shows_the_hint() {
    let mut state = drawn(document());
    state.find_hint = "Enter find · Esc cancel".to_string();
    let state = msg(state, ViewerMsg::FindOpen);
    let mut state = typed(state, "abc");
    let screen = draw(&mut state, 80, 8);
    let last = screen.lines().last().expect("a footer");
    assert!(last.starts_with("/abc"), "{last:?}");
    assert!(last.contains("Enter find · Esc cancel"), "{last:?}");
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
    let mut state = find(drawn(tool_document()), "needle");
    assert_eq!(state.find.matches, [2, 5, 9]);
    assert_eq!(state.find.hidden_matches, 2);
    let mut state2 = find(state.clone(), "tool output");
    let screen = draw(&mut state2, 120, 8);
    assert!(screen.contains("no match · 1 in hidden turns"), "{screen}");
    draw(&mut state, 120, 8);
}

#[test]
fn showing_a_kind_makes_its_matches_findable() {
    let state = find(drawn(tool_document()), "needle");
    let state = msg(state, ViewerMsg::ToggleThinking);
    assert_eq!(state.find.matches, [2, 5, 9, 13]);
    assert_eq!(
        state.find.current_turn(),
        Some(2),
        "stays on the current match"
    );
    let state = msg(state, ViewerMsg::CycleToolDetail);
    let state = msg(state, ViewerMsg::CycleToolDetail);
    let state = msg(state, ViewerMsg::CycleToolDetail);
    assert_eq!(state.tool_detail, ToolDetail::Full);
    assert_eq!(state.find.matches, [2, 5, 9, 12, 13]);
}

#[test]
fn hiding_the_current_match_moves_to_a_shown_one() {
    let state = find(drawn(tool_document()), "needle");
    let state = msg(state, ViewerMsg::ToggleThinking);
    let mut state = msg(state, ViewerMsg::FindPrev);
    assert_eq!(state.find.current_turn(), Some(13));
    draw(&mut state, 80, 8);
    let state = msg(state, ViewerMsg::ToggleThinking);
    assert_eq!(state.find.matches, [2, 5, 9]);
    assert_eq!(state.find.current_turn(), Some(2));
}

#[test]
fn a_collapsed_tool_run_has_one_line_for_all_its_turns() {
    let mut doc = document();
    doc.turns
        .push(turn(TurnRole::Assistant, TurnKind::ToolUse, "Read: /a"));
    doc.turns
        .push(turn(TurnRole::Assistant, TurnKind::ToolResult, "ok"));
    let mut state = ViewerState::new(doc);
    state.tool_detail = ToolDetail::Summary;
    draw(&mut state, 80, 8);
    let starts = &state.rendered.as_ref().unwrap().turn_starts;
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
    let mut state = ViewerState::new(document());
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 5,
    });
    draw(&mut state, 80, 8);
    assert_eq!(state.find.current_turn(), Some(5));
    assert_eq!(state.find.matches, [2, 5, 9]);
    assert_eq!(
        state.scroll_offset,
        first_line_of(&state, 5).saturating_sub(1)
    );
    assert!(!state.stick_to_end);
    assert_eq!(state.find.note, None, "a visible match needs no note");
}

#[test]
fn opening_at_a_hidden_tool_hit_shows_full_tool_output_and_says_so() {
    let mut state = ViewerState::new(tool_document());
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 12,
    });
    assert_eq!(state.tool_detail, ToolDetail::Full);
    assert_eq!(state.find.current_turn(), Some(12));
    assert_eq!(state.find.note.as_deref(), Some("showing full tool output"));
    let screen = draw(&mut state, 120, 8);
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
    let open = |turn_index| ViewerOpen {
        query: "needle".to_string(),
        turn_index,
    };

    let mut state = ViewerState::new(doc.clone());
    state.open_at(&open(12));
    assert!(state.show_aborted && !state.show_hidden && !state.show_thinking);
    assert_eq!(state.find.note.as_deref(), Some("showing aborted turns"));

    let mut state = ViewerState::new(doc.clone());
    state.open_at(&open(13));
    assert!(state.show_hidden && !state.show_aborted);
    assert_eq!(state.find.note.as_deref(), Some("showing hidden turns"));

    let mut state = ViewerState::new(doc);
    state.open_at(&open(14));
    assert!(state.show_aborted && state.show_thinking);
    assert_eq!(
        state.find.note.as_deref(),
        Some("showing aborted turns, showing thinking")
    );
}

#[test]
fn a_changed_transcript_finds_the_nearest_match() {
    let mut state = ViewerState::new(document());
    // The scan saw the match at turn 7; it is now at 5 and 9.
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 7,
    });
    assert_eq!(state.find.current_turn(), Some(5), "nearest, earlier first");
    assert!(
        state
            .find
            .note
            .as_deref()
            .is_some_and(|n| n.contains("nearest match"))
    );
}

#[test]
fn a_match_far_from_the_remembered_turn_is_still_found() {
    let mut doc = document();
    doc.turns = (0..44).map(|i| message(i, "filler")).collect();
    doc.turns.push(message(0, "a lonely needle"));
    let mut state = ViewerState::new(doc);
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 3,
    });
    assert_eq!(
        state.find.current_turn(),
        Some(44),
        "falls back to a global search"
    );
}

#[test]
fn a_query_that_is_gone_says_so_and_leaves_the_viewer_at_the_end() {
    let mut state = ViewerState::new(document());
    state.open_at(&ViewerOpen {
        query: "vanished".to_string(),
        turn_index: 3,
    });
    assert!(state.stick_to_end);
    assert!(state.find.matches.is_empty());
    assert!(
        state
            .find
            .note
            .as_deref()
            .is_some_and(|n| n.contains("no longer in this transcript"))
    );
}

#[test]
fn an_empty_open_query_does_nothing() {
    let mut state = ViewerState::new(document());
    state.open_at(&ViewerOpen {
        query: "  ".to_string(),
        turn_index: 3,
    });
    assert!(!state.find.is_active() && state.stick_to_end);
}

#[test]
fn the_find_view_at_80_columns() {
    let mut state = ViewerState::new(document());
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 5,
    });
    insta::assert_snapshot!("find_at_80_columns", draw(&mut state, 80, 12));
}

#[test]
fn the_find_view_at_120_columns() {
    let mut state = ViewerState::new(tool_document());
    state.open_at(&ViewerOpen {
        query: "needle".to_string(),
        turn_index: 12,
    });
    insta::assert_snapshot!("find_at_120_columns", draw(&mut state, 120, 12));
}
