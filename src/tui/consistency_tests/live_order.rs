//! Live state orders and narrows the rows only when the tree is built
//! (ADR 0215): the agent-state filter keeps exactly the rows drawn with
//! the mark it names, in every view that filters; the float lifts the
//! rows drawn with `!` above the rest of their group; and a store change
//! between builds refills the marks in place, leaving every row, the
//! order, and the cursor where they were until the tree is built again
//! for an operator action or a snapshot.

use std::time::{Duration, Instant};

use super::live::{
    HOSTED, listing_with, live_world, overlay_over, row_lines, with_table, written_store,
};
use super::*;
use crate::filter::{LiveStateFilter, LiveStateKey};
use crate::live_store::{LiveEvent, LiveEventKind, now_ms};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::{LiveMark, LiveSign, Row, RowKind};
use crate::tui::widgets::controls::LIVE_STATE_STEPS;

/// The views whose lists the filter narrows.
const FILTERED_VIEWS: [View; 4] = [View::Sessions, View::Mux, View::Prs, View::Search];

/// `app` with `filter` set, as Controls sets it.
fn filtered(mut app: App, filter: RowFilter) -> App {
    app.update(Msg::SetFilter(filter));
    app
}

fn keeping(key: LiveStateKey) -> RowFilter {
    RowFilter {
        live_state: Some(LiveStateFilter::from_values([key])),
        ..RowFilter::default()
    }
}

fn floating() -> RowFilter {
    RowFilter {
        float_waiting_top: true,
        ..RowFilter::default()
    }
}

/// The mark a session row draws.
fn session_mark(row: &Row) -> Option<LiveMark> {
    match &row.kind {
        RowKind::AgentSession(session) => Some(LiveSign::of_session(session.live).mark),
        _ => None,
    }
}

/// The sessions `app` lists, by their keys.
fn listed_sessions(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_agent_state_filter_keeps_the_rows_drawn_with_its_mark_in_every_view() {
    let snapshot = with_table();
    for view in FILTERED_VIEWS {
        let all = listing_with(view, &snapshot, LiveOverlay::off());
        let mut kept_any = false;
        for key in LIVE_STATE_STEPS.iter().copied() {
            let mark = LiveMark::from(key);
            let app = filtered(
                listing_with(view, &snapshot, LiveOverlay::off()),
                keeping(key),
            );
            for row in &app.tree().rows {
                match &row.kind {
                    RowKind::AgentSession(_) => {
                        assert_eq!(session_mark(row), Some(mark), "{view:?} {key:?}: {row:?}");
                        kept_any = true;
                    }
                    RowKind::MuxSession(mux) => {
                        assert!(
                            mark.count_in(mux.live.counts) > 0,
                            "{view:?} {key:?}: a mux with no agent drawn {mark:?} stays: {mux:?}"
                        );
                        kept_any = true;
                    }
                    _ => {}
                }
            }
            if view == View::Mux {
                let muxes = |app: &App, wanted: bool| -> Vec<String> {
                    app.tree()
                        .rows
                        .iter()
                        .filter_map(|row| match &row.kind {
                            RowKind::MuxSession(mux)
                                if !wanted || mark.count_in(mux.live.counts) > 0 =>
                            {
                                Some(mux.native_id.clone())
                            }
                            _ => None,
                        })
                        .collect()
                };
                assert_eq!(
                    muxes(&app, false),
                    muxes(&all, true),
                    "{key:?}: every running mux with an agent drawn {mark:?}, and only those"
                );
            }
            if view == View::Sessions {
                let expected: Vec<String> = all
                    .tree()
                    .rows
                    .iter()
                    .filter(|row| session_mark(row) == Some(mark))
                    .filter_map(|row| match &row.kind {
                        RowKind::AgentSession(session) => Some(session.session.session_key.clone()),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    listed_sessions(&app),
                    expected,
                    "{key:?}: every session drawn {mark:?}, and only those"
                );
            }
        }
        if view != View::Prs {
            assert!(kept_any, "{view:?} kept no row for any state");
        }
    }
}

#[test]
fn the_agent_state_filter_keeps_a_mux_for_an_agent_in_its_panes() {
    // `scratch` hosts no session, only an agent in a pane that waits.
    let app = filtered(
        listing_with(View::Mux, &with_table(), LiveOverlay::off()),
        keeping(LiveStateKey::Waiting),
    );
    assert!(
        app.tree().rows.iter().any(|row| matches!(
            &row.kind,
            RowKind::MuxSession(mux) if mux.native_id == "scratch"
        )),
        "{:#?}",
        app.tree().rows
    );
}

/// For each run of sibling rows of one kind (the rows at one depth under
/// one parent), whether each is drawn waiting, in order.
fn sibling_runs(rows: &[Row], waiting: impl Fn(&Row) -> Option<bool>) -> Vec<Vec<bool>> {
    let mut runs: Vec<Vec<bool>> = Vec::new();
    let mut current: Option<(u8, Vec<bool>)> = None;
    for row in rows {
        match (waiting(row), current.as_mut()) {
            (Some(is), Some((depth, run))) if *depth == row.depth => run.push(is),
            (Some(is), _) => {
                if let Some((_, run)) = current.take() {
                    runs.push(run);
                }
                current = Some((row.depth, vec![is]));
            }
            (None, Some((depth, _))) if row.depth > *depth => {}
            (None, _) => {
                if let Some((_, run)) = current.take() {
                    runs.push(run);
                }
            }
        }
    }
    runs.extend(current.map(|(_, run)| run));
    runs
}

#[test]
fn the_float_lifts_the_rows_drawn_waiting_above_the_rest_of_their_group() {
    let mut snapshot = with_table();
    // A mux with no agent, listed first by name.
    let mut idle = MuxSessionNode::new(MuxSessionId::new("tmux:aaa-idle"), "tmux", "aaa-idle");
    idle.cwd = Some("/home/op/src/conspectus".into());
    snapshot.nodes.push(GraphNode::MuxSession(idle));
    for view in [View::Sessions, View::Mux] {
        let waiting = |row: &Row| match &row.kind {
            RowKind::AgentSession(_) if view == View::Sessions => {
                Some(session_mark(row) == Some(LiveMark::Waiting))
            }
            RowKind::MuxSession(mux) if view == View::Mux => {
                Some(LiveSign::of_mux(mux.live).mark == LiveMark::Waiting)
            }
            _ => None,
        };
        let before = listing_with(view, &snapshot, LiveOverlay::off());
        let after = filtered(
            listing_with(view, &snapshot, LiveOverlay::off()),
            floating(),
        );
        let unfloated = sibling_runs(&before.tree().rows, waiting);
        assert!(
            unfloated
                .iter()
                .any(|run| run.windows(2).any(|pair| !pair[0] && pair[1])),
            "{view:?}: the world has a waiting row below another: {unfloated:?}"
        );
        for run in sibling_runs(&after.tree().rows, waiting) {
            assert!(
                run.windows(2).all(|pair| pair[0] || !pair[1]),
                "{view:?}: a waiting row sorts below one that is not: {run:?}"
            );
        }
        assert_eq!(
            after.tree().rows.len(),
            before.tree().rows.len(),
            "{view:?}: the float hides nothing"
        );
    }
}

/// The ids of every row of `app`'s tree, in order.
fn row_ids(app: &App) -> Vec<RowId> {
    app.tree().rows.iter().map(|row| row.id.clone()).collect()
}

/// The mark the hosted session's row draws.
fn hosted_mark(app: &mut App) -> String {
    let lines = row_lines(app, 220);
    let line = lines
        .iter()
        .find(|line| line.contains("e44d01cb"))
        .expect("the hosted row")
        .clone();
    let tokens = tuikit::testkit::buffer::tokens(&line);
    let age = tokens.iter().position(|t| is_age(&t.text)).expect("an age");
    tokens[age + 1].text.clone()
}

/// The hosted session's wait was answered and its turn ended, in the
/// store, and the TUI's next tick has read it.
fn answer_the_hosted_wait(app: &mut App, store: &crate::live_store::LiveStore) {
    for kind in [
        LiveEventKind::WaitEnded { agent: None },
        LiveEventKind::TurnEnded { interrupted: false },
    ] {
        store
            .apply_event(&LiveEvent::new("claude-code", HOSTED, now_ms(), kind))
            .expect("the store takes the event");
    }
    app.poll_live(
        Instant::now() + Duration::from_secs(5),
        crate::tui::rows::tree_current_unix_ms(),
    );
}

#[test]
fn a_store_change_moves_no_floated_row_until_the_tree_is_built_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let natural = listing_with(View::Sessions, &snapshot, overlay_over(&store));
    let mut app = filtered(
        listing_with(View::Sessions, &snapshot, overlay_over(&store)),
        floating(),
    );
    select(&mut app, HOSTED);
    let selected = app.selection().cloned();
    let floated = row_ids(&app);
    assert_ne!(
        floated,
        row_ids(&natural),
        "the waiting session floats above a newer one"
    );
    assert_eq!(hosted_mark(&mut app), "!");

    answer_the_hosted_wait(&mut app, &store);
    assert_eq!(hosted_mark(&mut app), "›", "only its mark changes");
    assert_eq!(row_ids(&app), floated, "no row moves");
    assert_eq!(app.selection().cloned(), selected, "the cursor stays");

    // An operator action builds the tree again: nothing waits now, so the
    // float lifts nothing and the cursor keeps its row by identity.
    let filter = app.filter().clone();
    app.update(Msg::SetFilter(filter));
    assert_eq!(row_ids(&app), row_ids(&natural), "the order catches up");
    assert_eq!(
        app.selection().cloned(),
        selected,
        "the cursor follows its row"
    );
}

#[test]
fn a_store_change_drops_no_filtered_row_until_the_tree_is_built_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let snapshot = live_world();
    let mut app = filtered(
        listing_with(View::Sessions, &snapshot, overlay_over(&store)),
        keeping(LiveStateKey::Waiting),
    );
    select(&mut app, HOSTED);
    let selected = app.selection().cloned();
    let listed = row_ids(&app);
    assert!(listed.contains(selected.as_ref().expect("a selection")));

    answer_the_hosted_wait(&mut app, &store);
    assert_eq!(
        hosted_mark(&mut app),
        "›",
        "its mark says it no longer waits"
    );
    assert_eq!(row_ids(&app), listed, "it stays listed");
    assert_eq!(app.selection().cloned(), selected, "the cursor stays on it");

    let filter = app.filter().clone();
    app.update(Msg::SetFilter(filter));
    assert!(
        !row_ids(&app).contains(selected.as_ref().expect("a selection")),
        "the next build leaves it out"
    );
}

#[test]
fn folding_moves_no_row_the_float_placed() {
    let dir = tempfile::tempdir().unwrap();
    let store = written_store(dir.path());
    let mut app = filtered(
        listing_with(View::Sessions, &live_world(), overlay_over(&store)),
        floating(),
    );
    select(&mut app, HOSTED);
    let placed = row_ids(&app);
    answer_the_hosted_wait(&mut app, &store);
    app.update(Msg::CollapseRow);
    app.update(Msg::ExpandRow);
    assert_eq!(row_ids(&app), placed, "a fold builds nothing");
}

#[test]
fn the_agent_state_filter_shows_as_a_chip_and_narrows_the_session_count() {
    let mut app = filtered(
        listing_with(View::Sessions, &with_table(), LiveOverlay::off()),
        keeping(LiveStateKey::Waiting),
    );
    let screen = frame(&mut app, 200, 30);
    let lines: Vec<&str> = screen.lines().collect();
    let status = lines[lines.len() - 1];
    assert!(status.contains("filter:agent:waiting"), "{status}");
    let total = app
        .snapshot_handle()
        .expect("data")
        .snapshot()
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::AgentSession(_)))
        .count();
    let listed = listed_sessions(&app).len();
    assert!(listed < total, "the filter hides some");
    assert!(
        lines[0].contains(&format!("{listed}/{total} sessions")),
        "{}",
        lines[0]
    );
}

#[test]
fn the_empty_state_names_the_agent_state_chip() {
    let mut app = filtered(
        listing_with(View::Sessions, &live_world(), LiveOverlay::off()),
        keeping(LiveStateKey::Error),
    );
    assert!(listed_sessions(&app).is_empty(), "no live state, no error");
    let screen = frame(&mut app, 200, 30);
    assert!(screen.contains("No rows match `agent:error`."), "{screen}");
}
