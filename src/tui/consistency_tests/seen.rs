//! A finished turn the operator has not seen reads as one they have, only
//! heavier: in every view that lists its session or its mux, the mark (in
//! Pinned, the word) keeps its shape and its color and is bold, and
//! nothing else on the row changes. Every listing of one entity agrees,
//! and looking at a session with no mux (resting the cursor on one of its
//! rows) turns them all plain at once.

use std::time::Instant;

use ratatui::style::Modifier;

use super::live::{HOSTED, SOLO, listing_with, mux, running, session, with_table};
use super::*;
use crate::model::live::{LiveDetail, LiveState};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::RowKind;
use crate::tui::rows::seen::SEEN_REST;

/// `app` with every foldable row open, so every row shows.
fn unfolded(mut app: App) -> App {
    let foldable: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| row.expandable)
        .map(|row| row.id.clone())
        .collect();
    for id in foldable {
        app.set_selection(id);
        app.update(Msg::ExpandRow);
    }
    app
}

/// The cell where `needle` starts on the first line of `app` that
/// contains `key` and is not the cursor row (which the highlight draws
/// bold), with that line's text.
fn cell_of(app: &mut App, key: &str, needle: &str) -> (ratatui::buffer::Cell, String) {
    let buffer = buffer_of(app, 220, 60);
    let lines = tuikit::testkit::buffer::rows(&buffer);
    let y = lines
        .iter()
        .position(|line| {
            line.contains(key)
                && line.contains(needle)
                && !line
                    .trim_start_matches('│')
                    .starts_with(tuikit::glyphs::marks::POINTER)
        })
        .unwrap_or_else(|| panic!("no line with {key} and {needle}:\n{}", lines.join("\n")));
    let line = &lines[y];
    let (before, _) = line.split_once(needle).expect("the needle");
    let x = u16::try_from(tuikit::text::width(before)).expect("a column");
    let cell = buffer[(x, u16::try_from(y).expect("a row"))].clone();
    (cell, line.clone())
}

/// Whether `row` stands for the session `key`, whatever its kind.
fn stands_for(row: &crate::tui::rows::Row, key: &str) -> bool {
    match &row.kind {
        RowKind::AgentSession(row) => row.primary_node == session(key),
        // A pin is named for what it points at, its session's key cut
        // to its first word.
        RowKind::Mixed(mixed) => !mixed.label.is_empty() && key.starts_with(mixed.label.as_str()),
        _ => false,
    }
}

/// The row of `app` standing for the session `key`.
fn row_of(app: &App, key: &str) -> RowId {
    app.tree()
        .rows
        .iter()
        .find(|row| stands_for(row, key))
        .unwrap_or_else(|| panic!("no row for {key}"))
        .id
        .clone()
}

/// Put the cursor on the first row that `avoid` does not name, so the
/// highlight (which is bold) is not on the row a test reads.
fn park_away(app: &mut App, avoid: impl Fn(&crate::tui::rows::Row) -> bool) {
    let away = app
        .tree()
        .rows
        .iter()
        .find(|row| !avoid(row))
        .map(|row| row.id.clone())
        .expect("another row");
    app.set_selection(away);
}

#[test]
fn an_unseen_ready_mark_is_bold_and_a_seen_one_plain_with_the_same_shape() {
    // The loose session finished a turn and has no mux.
    let snapshot = with_table();
    let theme = crate::tui::Theme::default();
    // The Search view lists the session alone, always under the cursor,
    // whose highlight is bold; its rows carry the same judgment (below).
    for (view, needle) in [
        (View::Sessions, crate::tui::icons::live::READY),
        (View::Pinned, "ready"),
    ] {
        let mut app = unfolded(listing_with(view, &snapshot, LiveOverlay::off()));
        park_away(&mut app, |row| stands_for(row, SOLO));
        let (unseen, before) = cell_of(&mut app, "solo", needle);
        assert!(
            unseen.modifier.contains(Modifier::BOLD),
            "{view:?}: an unseen turn is bold:\n{before}"
        );
        assert_eq!(unseen.fg, theme.success, "{view:?}: in the ready color");

        // Rest the cursor on the session's row long enough to look at it,
        // then move it away again.
        app.set_selection(row_of(&app, SOLO));
        let start = Instant::now();
        app.note_cursor_rest(start);
        app.note_cursor_rest(start + SEEN_REST);
        park_away(&mut app, |row| stands_for(row, SOLO));
        let (seen, after) = cell_of(&mut app, "solo", needle);
        assert!(
            !seen.modifier.contains(Modifier::BOLD),
            "{view:?}: a seen turn is plain:\n{after}"
        );
        assert_eq!(seen.fg, unseen.fg, "{view:?}: the color stays");
        assert_eq!(seen.symbol(), unseen.symbol(), "{view:?}: the shape stays");
        assert_eq!(after, before, "{view:?}: nothing else on the row changes");
    }
}

#[test]
fn looking_at_a_session_turns_every_listing_of_it_plain() {
    let snapshot = with_table();
    // The Search view lists the session found, the cursor on it.
    let mut app = listing_with(View::Search, &snapshot, LiveOverlay::off());
    let judged = |app: &App| -> Vec<bool> {
        app.tree()
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::AgentSession(row) if row.primary_node == session(SOLO) => {
                    row.live.map(|cell| cell.unseen)
                }
                RowKind::Mixed(mixed) => match mixed.live {
                    crate::tui::rows::MixedLive::Session(cell)
                        if !mixed.label.is_empty() && SOLO.starts_with(&mixed.label) =>
                    {
                        Some(cell.unseen)
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    };
    assert_eq!(judged(&app), [true], "found, and not looked at yet");
    app.set_selection(row_of(&app, SOLO));
    let start = Instant::now();
    app.note_cursor_rest(start);
    app.note_cursor_rest(start + SEEN_REST);
    // The memory belongs to the TUI, not the view: every view the
    // operator switches to reads the turn as seen.
    for view in [View::Search, View::Pinned, View::Sessions] {
        app.update(Msg::SwitchView(view));
        let unseen = judged(&app);
        assert!(!unseen.is_empty(), "{view:?} lists the session");
        assert!(
            unseen.iter().all(|unseen| !unseen),
            "{view:?} still reads the turn unseen"
        );
    }
}

#[test]
fn a_mux_whose_ready_agent_is_unseen_draws_its_mark_bold() {
    let theme = crate::tui::Theme::default();
    let world = |last_attached: Option<i64>| {
        let mut snapshot = with_table();
        snapshot
            .node_live
            .retain(|node, _| *node == session(HOSTED));
        snapshot.pane_live.clear();
        snapshot.node_live.insert(
            session(HOSTED),
            running(LiveState::Ready, Some(LiveDetail::Finished), 0),
        );
        for node in &mut snapshot.nodes {
            if let GraphNode::MuxSession(node) = node
                && node.id == mux("conspectus-main")
            {
                node.last_attached_epoch = last_attached;
            }
        }
        snapshot
    };
    let mark = |snapshot: &GraphSnapshot| {
        let mut app = unfolded(listing_with(View::Mux, snapshot, LiveOverlay::off()));
        park_away(
            &mut app,
            |row| matches!(&row.kind, RowKind::MuxSession(row) if row.mux == mux("conspectus-main")),
        );
        // The mux row, not its nested agents: the line with its name and
        // its age.
        let (cell, line) = cell_of(&mut app, "conspectus-main", crate::tui::icons::live::READY);
        (cell, line)
    };
    let (unseen, before) = mark(&world(None));
    assert!(
        unseen.modifier.contains(Modifier::BOLD),
        "no client has attached since the turn ended:\n{before}"
    );
    assert_eq!(unseen.fg, theme.success);
    // A client attached after the turn ended (it ended 90 seconds before
    // the fixture's time).
    let (seen, after) = mark(&world(Some(NOW)));
    assert!(!seen.modifier.contains(Modifier::BOLD), "{after}");
    assert_eq!(seen.symbol(), unseen.symbol());
    assert_eq!(after, before);
}

#[test]
fn every_listing_of_an_entity_carries_one_judgment() {
    // `one_entity_carries_one_live_value_in_every_view` checks every
    // listing carries one value, the judgment included; here the world
    // has one unseen and one seen turn, so both are carried.
    let mut snapshot = with_table();
    snapshot.node_live.insert(
        session(HOSTED),
        running(LiveState::Ready, Some(LiveDetail::Finished), 0),
    );
    for node in &mut snapshot.nodes {
        if let GraphNode::MuxSession(node) = node
            && node.id == mux("conspectus-main")
        {
            node.last_attached_epoch = Some(NOW);
        }
    }
    let mut judged = std::collections::BTreeMap::<NodeId, Vec<bool>>::new();
    for view in ALL_VIEWS {
        let app = listing_with(view, &snapshot, LiveOverlay::off());
        for row in &app.tree().rows {
            if let RowKind::AgentSession(row) = &row.kind
                && let Some(cell) = row.live
                && cell.state == LiveState::Ready
            {
                judged
                    .entry(row.primary_node.clone())
                    .or_default()
                    .push(cell.unseen);
            }
        }
    }
    assert_eq!(
        judged
            .get(&session(HOSTED))
            .map(|all| all.iter().all(|u| !u)),
        Some(true),
        "its mux was attached after it ended: {judged:?}"
    );
    assert_eq!(
        judged.get(&session(SOLO)).map(|all| all.iter().all(|u| *u)),
        Some(true),
        "no mux, never rested on: {judged:?}"
    );
}
