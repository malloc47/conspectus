//! A session row whose agent runs subagents shows `+N` right after its
//! label, at most three cells wide, and the chip gives way after the
//! notes that follow the label but before the hit count and the label.

use super::*;
use crate::model::live::{LiveConfidence, LiveState};
use crate::model::{AgentSessionId, MuxHost};
use crate::tui::rows::{AgentSessionRow, ContentHitCell, LiveCell};

/// A session row labelled `alias` (none when empty) whose agent is in
/// `state` with `subagents` running, with the notes that can follow a
/// label: a `was in` mux, a project, and a last message.
fn session(alias: &str, state: Option<LiveState>, subagents: u16) -> AgentSessionRow {
    let id = AgentSessionId::new("claude-code", "/state", "0199a3f2ab");
    AgentSessionRow {
        session: id.clone(),
        short_id: "0199a3".into(),
        harness_label: "claude".into(),
        cwd_display: None,
        project_display: Some("ledger".into()),
        recency: Some("1m".into()),
        activity_epoch: None,
        mux_state: MuxIndicator::Unmuxed,
        preview: Some("the last message of the session".into()),
        title: None,
        alias: (!alias.is_empty()).then(|| alias.to_string()),
        title_disambiguates: false,
        mux_host: Some(MuxHost {
            name: "old".into(),
            live: false,
        }),
        primary_node: NodeId::AgentSession(id),
        pin_id: None,
        content_hit: None,
        live: state.map(|state| LiveCell {
            state,
            detail: None,
            confidence: LiveConfidence::Fresh,
            since_ms: 0,
            subagents,
            source: crate::model::live::LiveSource::Hook,
        }),
    }
}

fn text_at(session: &AgentSessionRow, width: usize) -> String {
    let row = Row {
        id: RowId::AgentSession(session.primary_node.clone()),
        depth: 0,
        expandable: false,
        kind: RowKind::AgentSession(session.clone()),
    };
    lay_out(&[row], &Theme::default(), 0, width, None)[0]
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn the_chip_follows_the_label_and_precedes_the_notes_after_it() {
    let drawn = text_at(&session("fix the parser", Some(LiveState::Working), 2), 200);
    let at = |needle: &str| {
        drawn
            .find(needle)
            .unwrap_or_else(|| panic!("no {needle:?} in {drawn:?}"))
    };
    assert!(drawn.contains("fix the parser  +2  "), "{drawn:?}");
    assert!(at("+2") < at("was in old"), "{drawn:?}");
    assert!(at("was in old") < at("ledger"), "{drawn:?}");
    assert!(at("ledger") < at("the last message"), "{drawn:?}");
}

#[test]
fn with_no_label_the_chip_follows_the_link_cell() {
    let mut row = session("", Some(LiveState::Waiting), 1);
    row.mux_host = None;
    let drawn = text_at(&row, 200);
    let marks = format!(
        "{}  {}  +1  ",
        crate::tui::icons::live::WAITING,
        crate::tui::icons::state::UNLINKED
    );
    assert!(drawn.contains(&marks), "{drawn:?}");
}

#[test]
fn no_running_subagent_no_chip() {
    for row in [
        session("fix the parser", Some(LiveState::Working), 0),
        session("fix the parser", None, 0),
        session("fix the parser", Some(LiveState::Exited), 3),
        session("fix the parser", Some(LiveState::Unknown), 3),
    ] {
        let drawn = text_at(&row, 200);
        assert!(!drawn.contains('+'), "{drawn:?}");
        assert_eq!(
            drawn,
            text_at(
                &AgentSessionRow {
                    live: None,
                    ..row.clone()
                },
                200
            )
            .replacen(crate::tui::icons::live::NONE, row_mark(&row), 1),
            "the row is otherwise as it is without live state"
        );
    }
}

/// The live mark `row` draws.
fn row_mark(row: &AgentSessionRow) -> &'static str {
    crate::tui::rows::LiveSign::of_session(row.live).glyph()
}

#[test]
fn the_chip_is_never_wider_than_three_cells() {
    let many = text_at(
        &session("fix the parser", Some(LiveState::Working), 250),
        200,
    );
    assert!(many.contains("fix the parser  +9+  was in"), "{many:?}");
    let ten = text_at(
        &session("fix the parser", Some(LiveState::Working), 10),
        200,
    );
    assert!(ten.contains("+9+"), "{ten:?}");
}

#[test]
fn on_a_narrow_row_the_chip_leaves_after_the_project_and_before_the_hits_and_the_label() {
    let mut row = session("a label of some length", Some(LiveState::Working), 2);
    row.content_hit = Some(ContentHitCell {
        count: 3,
        snippet: "a snippet of the match".into(),
        highlight: 2..9,
    });
    let label = "a label of some length";
    let (mut chip_without_project, mut hits_without_chip) = (false, false);
    for width in (20..=140).rev() {
        let drawn = text_at(&row, width);
        let has = |needle: &str| drawn.contains(needle);
        let chip = has("+2");
        if has("was in") {
            assert!(has("ledger"), "at {width}: {drawn:?}");
        }
        if has("ledger") {
            assert!(chip, "at {width} the project outlasts the chip: {drawn:?}");
        }
        if chip {
            assert!(
                has("3 hits"),
                "at {width} the chip outlasts the hits: {drawn:?}"
            );
            assert!(
                has(label),
                "at {width} the label narrows before the chip leaves: {drawn:?}"
            );
        }
        chip_without_project |= chip && !has("ledger");
        hits_without_chip |= has("3 hits") && !chip;
    }
    assert!(
        chip_without_project,
        "some width keeps the chip, not the project"
    );
    assert!(hits_without_chip, "some width keeps the hits, not the chip");
}
