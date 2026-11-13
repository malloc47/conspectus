//! The detail pane says what a row's live mark says. A session's or a
//! mux's live fields lead with the mark its row draws, in every view that
//! lists it, because both read the values the rows were filled with at
//! one moment; a node with no evidence has none; a store change refills
//! them in place with the rows; and walking the explorer's cursor onto
//! them previews what they mean, while the Related rows below them still
//! drill and come back.

use std::time::Instant;

use tuikit::explorer::ExplorerKey;

use super::live::{
    BESIDE, HOSTED, SOLO, listing_with, live_world, overlay_over, session, with_table,
    written_store,
};
use super::*;
use crate::live_store::{LiveEvent, LiveEventKind, now_ms};
use crate::tui::explorer::{CoreField, ExplorerRow, FieldValue};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::{LiveSign, MixedLive, RowKind};

/// The labels of the live fields a session or a mux can list.
const LIVE_LABELS: [&str; 7] = [
    "state",
    "since",
    "source",
    "evidence",
    "subagents",
    "agents",
    "pane_agents",
];

/// The fields the explorer shows for the node it is on.
fn node_fields(app: &App) -> Vec<CoreField> {
    app.explorer()
        .map(|explorer| explorer.content().core_fields.clone())
        .unwrap_or_default()
}

/// The field labelled `label` the explorer shows, if any.
fn node_field(app: &App, label: &str) -> Option<CoreField> {
    node_fields(app)
        .into_iter()
        .find(|field| field.label == label)
}

/// The sign a row draws for its entity, and the field of the detail pane
/// that must lead with it: `state` for a session, `agents` for a mux.
/// `None` for a row standing for neither, or for a pin, whose detail is
/// the pin's.
fn row_sign(row: &crate::tui::rows::Row) -> Option<(LiveSign, &'static str)> {
    match &row.kind {
        RowKind::AgentSession(session) => Some((LiveSign::of_session(session.live), "state")),
        RowKind::MuxSession(mux) => Some((LiveSign::of_mux(mux.live), "agents")),
        RowKind::Mixed(mixed) => match (&mixed.primary_node, mixed.live) {
            (NodeId::Pin(_), _) => None,
            (NodeId::AgentSession(_), live) => Some((LiveSign::of_mixed(live), "state")),
            (NodeId::MuxSession(_), live) => Some((LiveSign::of_mixed(live), "agents")),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn every_listed_session_and_mux_shows_its_rows_mark_in_the_detail_pane() {
    let snapshot = with_table();
    let mut checked = 0;
    let mut without = 0;
    for view in ALL_VIEWS {
        let mut app = listing_with(view, &snapshot, LiveOverlay::off());
        let rows: Vec<crate::tui::rows::Row> = app.tree().rows.clone();
        for row in rows {
            let Some((sign, label)) = row_sign(&row) else {
                continue;
            };
            app.set_selection(row.id.clone());
            let fields = node_fields(&app);
            let has_state = match &row.kind {
                RowKind::AgentSession(session) => session.live.is_some(),
                RowKind::MuxSession(mux) => !mux.live.is_empty(),
                RowKind::Mixed(mixed) => match mixed.live {
                    MixedLive::None => false,
                    MixedLive::Session(_) => true,
                    MixedLive::Mux(counts) => !counts.is_empty(),
                },
                _ => false,
            };
            if has_state {
                let field = fields
                    .iter()
                    .find(|field| field.label == label)
                    .unwrap_or_else(|| {
                        panic!(
                            "{view:?}: {:?} has live state but no `{label}` field",
                            row.id
                        )
                    });
                assert_eq!(
                    field.live_mark,
                    Some(sign),
                    "{view:?}: {:?}'s `{label}` field leads with another mark than its row",
                    row.id
                );
                checked += 1;
            } else {
                assert!(
                    fields
                        .iter()
                        .all(|field| !LIVE_LABELS.contains(&field.label)),
                    "{view:?}: {:?} has no live state but lists {:?}",
                    row.id,
                    fields.iter().map(|field| field.label).collect::<Vec<_>>()
                );
                without += 1;
            }
        }
    }
    assert!(
        checked >= 8,
        "only {checked} rows with live state were checked"
    );
    assert!(without > 0, "a row without live state was checked");
}

#[test]
fn the_detail_pane_takes_a_store_change_in_place_and_keeps_its_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let mut app = listing_with(View::Sessions, &snapshot, overlay_over(&store));
    select(&mut app, HOSTED);
    walk_to(&mut app, "state");
    assert_eq!(
        node_field(&app, "state").map(|field| field.value),
        Some("waiting · permission".to_string())
    );
    store
        .apply_event(&LiveEvent::new(
            "claude-code",
            HOSTED,
            now_ms(),
            LiveEventKind::WaitEnded { agent: None },
        ))
        .expect("the store takes the event");
    store
        .apply_event(&LiveEvent::new(
            "claude-code",
            HOSTED,
            now_ms(),
            LiveEventKind::TurnEnded { interrupted: false },
        ))
        .expect("the store takes the event");
    app.poll_live(
        Instant::now() + std::time::Duration::from_secs(5),
        crate::tui::rows::tree_current_unix_ms(),
    );
    let state = node_field(&app, "state").expect("the state field");
    assert_eq!(
        state.value, "ready · finished · unseen",
        "the field reads the turn's end, which nobody has looked at yet"
    );
    assert_eq!(
        state.live_mark.map(LiveSign::glyph),
        Some(crate::tui::icons::live::READY)
    );
    assert!(
        matches!(
            app.explorer_cursor_row(),
            Some(ExplorerRow::NodeField { field, .. }) if field.label == "state"
        ),
        "the cursor stays on the field"
    );
}

/// Move the explorer's cursor down to the field labelled `label`.
fn walk_to(app: &mut App, label: &str) {
    for _ in 0..20 {
        if matches!(
            app.explorer_cursor_row(),
            Some(ExplorerRow::NodeField { field, .. }) if field.label == label
        ) {
            return;
        }
        app.update(Msg::Explorer(ExplorerKey::Down));
    }
    panic!("the explorer has no `{label}` field");
}

#[test]
fn the_live_fields_preview_what_they_mean_and_the_related_rows_still_drill() {
    let snapshot = with_table();
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    select(&mut app, HOSTED);
    app.update(Msg::CycleFocus);
    let preview = |app: &mut App| {
        let text = frame(app, 200, 60);
        text.lines()
            .map(|line| {
                line.split('│')
                    .nth(3)
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    walk_to(&mut app, "state");
    assert_eq!(
        crate::tui::preview::preview_subject(&app),
        crate::tui::preview::PreviewSubject::Field(FieldValue::Glossary {
            field: "state",
            values: vec!["waiting".into(), "permission".into()],
        })
    );
    let shown = preview(&mut app);
    assert!(
        shown.contains("The agent is blocked on you.")
            && shown.contains("It asks your permission to go on."),
        "{shown}"
    );

    walk_to(&mut app, "since");
    assert!(matches!(
        crate::tui::preview::preview_subject(&app),
        crate::tui::preview::PreviewSubject::Field(FieldValue::Timestamp(_))
    ));
    walk_to(&mut app, "evidence");
    let shown = preview(&mut app);
    assert!(shown.contains("Recent enough to trust"), "{shown}");

    // Past the fields, the first Related row previews its neighbor and
    // drills into it; Backspace comes back to the session's fields.
    let mut steps = 0;
    while !matches!(
        app.explorer_cursor_row(),
        Some(ExplorerRow::ValidatedLink { .. })
    ) {
        app.update(Msg::Explorer(ExplorerKey::Down));
        steps += 1;
        assert!(steps < 20, "no Related row below the fields");
    }
    let crate::tui::preview::PreviewSubject::Node(neighbor) =
        crate::tui::preview::preview_subject(&app)
    else {
        panic!("a Related row previews its neighbor");
    };
    app.update(Msg::Explorer(ExplorerKey::Activate));
    assert_eq!(
        app.explorer().map(|explorer| explorer.id().clone()),
        Some(neighbor),
        "Enter drills into the neighbor"
    );
    app.update(Msg::Explorer(ExplorerKey::Back));
    assert_eq!(
        app.explorer().map(|explorer| explorer.id().clone()),
        Some(session(HOSTED)),
        "Backspace comes back"
    );
    assert!(node_field(&app, "state").is_some());
}

#[test]
fn a_mux_lists_its_agents_and_the_ones_no_session_matches() {
    let snapshot = with_table();
    let mut app = listing_with(View::Mux, &snapshot, LiveOverlay::off());
    select(&mut app, "conspectus-main");
    assert_eq!(
        node_field(&app, "agents").map(|field| field.value),
        Some("1 waiting · 1 error".to_string()),
        "the mux counts {HOSTED} and {BESIDE}"
    );
    assert!(node_field(&app, "pane_agents").is_none());
    // The starting agent said nothing more for longer than a start may
    // take, so it reads as the rows read it: can't tell.
    select(&mut app, "scratch");
    assert_eq!(
        node_field(&app, "agents").map(|field| field.value),
        Some("1 waiting · 1 unknown".to_string())
    );
    assert_eq!(
        node_field(&app, "pane_agents").map(|field| field.value),
        Some("%4 waiting · input  (+1)".to_string())
    );
}

#[test]
fn stopped_removed_and_missing_rows_never_spell_a_live_word() {
    // Every session of the world reports a state that has a word.
    let mut snapshot = with_table();
    let ready = super::live::running(
        crate::model::live::LiveState::Ready,
        Some(crate::model::live::LiveDetail::Finished),
        0,
    );
    for node in snapshot.nodes.clone() {
        if let GraphNode::AgentSession(session) = node {
            snapshot
                .node_live
                .entry(NodeId::AgentSession(session.id))
                .or_insert(ready);
        }
    }
    let words = ["waiting", "working", "ready", "error"];
    for view in [View::Pinned, View::History] {
        let mut app = listing_with(view, &snapshot, LiveOverlay::off());
        let lines = super::live::row_lines(&mut app, 220);
        let mut dim = 0;
        for row in &app.tree().rows {
            let RowKind::Mixed(mixed) = &row.kind else {
                continue;
            };
            if mixed.tone != crate::tui::rows::MixedTone::Dim {
                continue;
            }
            dim += 1;
            for line in lines.iter().filter(|line| line.contains(&mixed.label)) {
                let (_, rest) = line.split_once(&mixed.label).expect("the label");
                let word = rest.trim_start().split("  ").next().unwrap_or_default();
                assert!(
                    !words.contains(&word),
                    "{view:?}: the {} row ({}) reads `{word}`:\n{line}",
                    mixed.status,
                    mixed.label
                );
            }
        }
        assert!(dim > 0, "{view:?} lists a stopped, removed, or missing row");
    }
    // History lists no sessions, so nothing there has a live word at all.
    let history = listing_with(View::History, &snapshot, LiveOverlay::off());
    assert!(history.tree().rows.iter().all(|row| match &row.kind {
        RowKind::Mixed(mixed) => !matches!(mixed.live, MixedLive::Session(_)),
        _ => true,
    }));
    // The loose session reads `ready` in Pinned, as before.
    let mut pinned = listing_with(View::Pinned, &snapshot, LiveOverlay::off());
    let solo = super::live::row_lines(&mut pinned, 220)
        .into_iter()
        .find(|line| line.contains("solo"))
        .unwrap_or_default();
    assert!(solo.contains("ready"), "{SOLO}: {solo}");
}
