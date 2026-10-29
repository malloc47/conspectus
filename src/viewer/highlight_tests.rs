use super::*;
use crate::viewer::model::{
    TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use crate::viewer::state::Transcript;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

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

/// `document` composed `width` wide with `marks` painted on it, drawn
/// from the top.
fn painted(document: TranscriptDocument, marks: &Marks, width: u16) -> Buffer {
    let theme = Theme::default();
    let mut transcript: Transcript = Transcript::new(document);
    let mut lines = transcript.lines(&theme, width).to_vec();
    marks.paint(&mut lines, 0, transcript.rendered.as_ref().unwrap(), &theme);
    let area = Rect::new(0, 0, width, 12);
    let mut buffer = Buffer::empty(area);
    Paragraph::new(lines).render(area, &mut buffer);
    buffer
}

fn marks(query: &str, matches: Vec<usize>, current: Option<usize>) -> Marks {
    Marks {
        query: query.to_string(),
        matches,
        current,
    }
}

fn paint_state(query: &str, current_turn: usize) -> Buffer {
    let marks = marks(query, vec![0, 2], Some(current_turn));
    painted(document(), &marks, 80)
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
    let buffer = paint_state("needle", 0);
    assert_eq!(
        styled(&buffer, Modifier::REVERSED),
        ["Needleneedle"],
        "both matches in the current turn, case kept"
    );
    assert_eq!(styled(&buffer, Modifier::UNDERLINED), ["needle"]);
}

#[test]
fn the_gutter_is_never_painted() {
    let buffer = paint_state("you", 0);
    let reversed = styled(&buffer, Modifier::REVERSED);
    // The role chip says `you`; only the body's "you" in turn 2 and the
    // body of turn 0 ("you said") are matches, never the chip.
    assert_eq!(reversed, ["you"], "{reversed:?}");
    let underlined = styled(&buffer, Modifier::UNDERLINED);
    assert_eq!(underlined, ["you"], "{underlined:?}");
}

#[test]
fn a_turn_that_is_not_a_match_is_left_alone() {
    let buffer = paint_state("nothing", 0);
    assert!(styled(&buffer, Modifier::REVERSED).is_empty());
    assert!(styled(&buffer, Modifier::UNDERLINED).is_empty());
}

#[test]
fn no_marks_paint_nothing() {
    let buffer = painted(document(), &Marks::default(), 80);
    assert!(styled(&buffer, Modifier::REVERSED).is_empty());
    let unmatched = painted(document(), &marks("needle", Vec::new(), None), 80);
    assert!(styled(&unmatched, Modifier::UNDERLINED).is_empty());
}

#[test]
fn wide_and_combining_text_keeps_its_columns() {
    let mut doc = document();
    doc.turns[0].body = "東京 needle 東京".to_string();
    let buffer = painted(doc, &marks("needle", vec![0], Some(0)), 80);
    assert_eq!(styled(&buffer, Modifier::REVERSED), ["needle"]);
}

#[test]
fn the_role_words_in_a_turn_header_are_not_matches() {
    // Turn 1's chip says `ai`; no body contains it.
    let mut doc = document();
    doc.turns[0].body = "you spoke".into();
    let buffer = painted(doc, &marks("ai", vec![1], Some(1)), 80);
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
    let buffer = painted(doc, &marks("two words", vec![0], Some(0)), 40);
    let mut screen = String::new();
    for y in 0..buffer.area.height {
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
