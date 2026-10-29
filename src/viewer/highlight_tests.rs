use super::*;
use crate::viewer::model::{
    TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use crate::viewer::state::ViewerState;
use crate::viewer::widget::render_to_buffer;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

fn document() -> TranscriptDocument {
    let turn = |role, body: &str| TranscriptTurn {
        role,
        kind: TurnKind::Message,
        body: body.to_string(),
        timestamp: None,
        aborted: false,
        hidden: false,
    };
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "h".to_string(),
            cwd: None,
        },
        turns: vec![
            turn(TurnRole::User, "you said the Needle twice: needle"),
            turn(TurnRole::Assistant, "nothing here"),
            turn(TurnRole::User, "another needle for you"),
        ],
    }
}

fn paint_state(query: &str, current_turn: usize) -> (ViewerState, Buffer) {
    let mut state = ViewerState::new(document());
    state.find.query = query.to_string();
    state.find.matches = vec![0, 2];
    state.find.current = state.find.matches.iter().position(|&t| t == current_turn);
    let buffer = render_to_buffer(&mut state, &Theme::default(), 80, 12);
    (state, buffer)
}

/// The text of every body cell (right of the gutter's `│`) that
/// carries `modifier`. The role chips are drawn reversed on their own, so
/// the gutter is left out.
fn styled(buffer: &Buffer, modifier: Modifier) -> Vec<String> {
    let mut rows = Vec::new();
    for y in 0..buffer.area.height {
        let mut text = String::new();
        let body_from = (0..buffer.area.width)
            .find(|&x| buffer[(x, y)].symbol() == "│")
            .map_or(buffer.area.width, |x| x + 1);
        for x in body_from..buffer.area.width {
            let cell = &buffer[(x, y)];
            if cell.modifier.contains(modifier) {
                text.push_str(cell.symbol());
            }
        }
        if !text.is_empty() {
            rows.push(text);
        }
    }
    rows
}

#[test]
fn the_current_match_is_reversed_and_the_others_underlined() {
    let (_, buffer) = paint_state("needle", 0);
    assert_eq!(
        styled(&buffer, Modifier::REVERSED),
        ["Needleneedle"],
        "both matches in the current turn, case kept"
    );
    assert_eq!(styled(&buffer, Modifier::UNDERLINED), ["needle"]);
}

#[test]
fn the_gutter_is_never_painted() {
    let (_, buffer) = paint_state("you", 0);
    let reversed = styled(&buffer, Modifier::REVERSED);
    // The role chip says `you`; only the body's "you" in turn 2 and the
    // body of turn 0 ("you said") are matches, never the chip.
    assert_eq!(reversed, ["you"], "{reversed:?}");
    let underlined = styled(&buffer, Modifier::UNDERLINED);
    assert_eq!(underlined, ["you"], "{underlined:?}");
}

#[test]
fn a_turn_that_is_not_a_match_is_left_alone() {
    let (_, buffer) = paint_state("nothing", 0);
    assert!(styled(&buffer, Modifier::REVERSED).is_empty());
    assert!(styled(&buffer, Modifier::UNDERLINED).is_empty());
}

#[test]
fn no_find_paints_nothing() {
    let mut state = ViewerState::new(document());
    let buffer = render_to_buffer(&mut state, &Theme::default(), 80, 12);
    assert!(styled(&buffer, Modifier::REVERSED).is_empty());
}

#[test]
fn wide_and_combining_text_keeps_its_columns() {
    let mut doc = document();
    doc.turns[0].body = "東京 needle 東京".to_string();
    let mut state = ViewerState::new(doc);
    state.find.query = "needle".to_string();
    state.find.matches = vec![0];
    state.find.current = Some(0);
    let buffer = render_to_buffer(&mut state, &Theme::default(), 80, 12);
    assert_eq!(styled(&buffer, Modifier::REVERSED), ["needle"]);
}

#[test]
fn the_role_words_in_a_turn_header_are_not_matches() {
    // Turn 1's chip says `ai`; no body contains it.
    let mut doc = document();
    doc.turns[0].body = "you spoke".into();
    let mut state = ViewerState::new(doc);
    state.find.query = "ai".to_string();
    state.find.matches = vec![1];
    state.find.current = Some(0);
    let buffer = render_to_buffer(&mut state, &Theme::default(), 80, 12);
    let painted: Vec<_> = [Modifier::REVERSED, Modifier::UNDERLINED]
        .iter()
        .flat_map(|modifier| styled(&buffer, *modifier))
        .collect();
    assert!(painted.is_empty(), "{painted:?}");
}

#[test]
fn a_match_split_across_a_wrap_is_found_but_not_painted() {
    // The wrap breaks at the space inside the query, so neither line
    // holds the whole match: the turn still counts as a match for
    // navigation, but no cell is highlighted.
    let mut doc = document();
    doc.turns[0].body = "aaaaaaaaaaaaaaaaaaaaaaaa two words tail".into();
    let mut state = ViewerState::new(doc);
    state.find.query = "two words".to_string();
    state.find.matches = vec![0];
    state.find.current = Some(0);
    let buffer = render_to_buffer(&mut state, &Theme::default(), 40, 12);
    let mut screen = String::new();
    // The status line quotes the query, so read the body rows only.
    for y in 0..buffer.area.height.saturating_sub(2) {
        for x in 0..buffer.area.width {
            screen.push_str(buffer[(x, y)].symbol());
        }
        screen.push('\n');
    }
    assert!(
        !screen.contains("two words"),
        "the query must straddle a wrap for this test to mean anything:\n{screen}"
    );
    assert!(styled(&buffer, Modifier::REVERSED).is_empty());
}
