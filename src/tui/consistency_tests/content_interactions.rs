//! The content search keeps its place when everything around it
//! changes: pins and undo, filters, refreshes, rescans, and resizes.

use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::tui::rows::RowKind;
use crate::tui::undo::Undo;
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{ContentKinds, Hit, SessionHits};

fn hits() -> SessionHits {
    SessionHits {
        count: 2,
        best: Hit {
            turn_index: 0,
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            timestamp: None,
            snippet: "mentions the needle".into(),
            highlight: 13..19,
        },
        latest: None,
    }
}

fn key_of(id: &AgentSessionId) -> SessionKey {
    SessionKey {
        harness_key: id.harness_key.clone(),
        session_key: id.session_key.clone(),
    }
}

fn session_ids(snapshot: &GraphSnapshot) -> Vec<AgentSessionId> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session.id.clone()),
            _ => None,
        })
        .collect()
}

fn deliver(app: &mut App, matched: &[&AgentSessionId], done: bool) {
    let generation = app.content_generation().expect("a search is active");
    app.update(Msg::ContentHits {
        generation,
        batch: matched.iter().map(|id| (key_of(id), hits())).collect(),
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        done,
    });
}

fn listed(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.clone()),
            _ => None,
        })
        .collect()
}

fn searching(view: View) -> (App, GraphSnapshot, AgentSessionId, AgentSessionId) {
    let snapshot = world();
    let ids = session_ids(&snapshot);
    let matched = ids
        .iter()
        .find(|id| id.session_key == "solo000001")
        .expect("the loose session")
        .clone();
    let other = ids
        .iter()
        .find(|id| id.session_key != "solo000001")
        .expect("another session")
        .clone();
    let mut app = app_over(view, &snapshot);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    deliver(&mut app, &[&matched], true);
    (app, snapshot, matched, other)
}

/// A refreshed snapshot reaches the app the way the runtime delivers it:
/// the tree is derived with the content search joined in.
fn refresh(app: &mut App, snapshot: &GraphSnapshot) {
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        snapshot, app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: NOW,
        initial_selection_hint: None,
    });
}

#[test]
fn pinning_unpinning_and_undoing_leave_the_search_and_the_selection_alone() {
    let (mut app, _, matched, _) = searching(View::Sessions);
    select(&mut app, &matched.session_key[..6]);
    let selected = app.selection().cloned();
    let before = listed(&app);

    app.update(Msg::TogglePin);
    app.update(Msg::UndoRecorded(Undo::Unpin {
        id: "pinned-by-test".into(),
    }));
    app.update(Msg::Undo);

    assert!(app.content().is_some(), "the search is still active");
    assert_eq!(listed(&app), before);
    assert_eq!(app.selection().cloned(), selected);
}

#[test]
fn the_stopped_mux_and_pinned_only_toggles_narrow_within_the_search() {
    let (mut app, _, matched, _) = searching(View::Sessions);
    for filter in [
        RowFilter {
            pinned_only: true,
            ..RowFilter::default()
        },
        RowFilter {
            show_stopped_muxes: true,
            ..RowFilter::default()
        },
        RowFilter::default(),
    ] {
        app.update(Msg::SetFilter(filter));
        assert!(app.content().is_some(), "the search outlives the filter");
        let sessions = listed(&app);
        assert!(
            sessions.iter().all(|key| *key == matched.session_key),
            "only matching sessions are listed: {sessions:?}"
        );
    }
}

#[test]
fn an_explicit_rescan_keeps_the_list_and_selection_until_the_new_results_land() {
    let (mut app, _, matched, _) = searching(View::Sessions);
    select(&mut app, &matched.session_key[..6]);
    let selected = app.selection().cloned();

    app.update(Msg::RescanContent);
    let request = app.take_scan_request().expect("a full scan was asked for");
    assert_eq!(
        request.sessions.len(),
        session_ids(&world()).len(),
        "an explicit refresh rescans every session"
    );
    assert_eq!(
        listed(&app),
        std::slice::from_ref(&matched.session_key),
        "the old results stay while the new ones are collected"
    );
    assert_eq!(app.selection().cloned(), selected);

    // The new results agree: the swap leaves the selection where it was.
    deliver(&mut app, &[&matched], true);
    assert_eq!(app.selection().cloned(), selected);
}

#[test]
fn the_refresh_key_is_the_one_that_rescans() {
    use crate::tui::keybindings::KEYBINDINGS;
    use crate::tui::keymap::Action;
    let has_refresh_key = KEYBINDINGS
        .iter()
        .any(|binding| (binding.action)() == Action::Refresh);
    assert!(
        has_refresh_key,
        "an explicit refresh key exists, and the live loop answers it with RescanContent"
    );
}

#[test]
fn a_refresh_that_adds_or_drops_sessions_mid_scan_stays_consistent() {
    let (mut app, snapshot, matched, _) = searching(View::Sessions);
    // A session appears while results are still being collected: it is
    // scanned once the scan in flight is done, and only it.
    let mut bigger = world_with(1);
    bigger.pins = snapshot.pins.clone();
    let grown = session_ids(&bigger);
    assert!(!grown.is_empty());
    refresh(&mut app, &bigger);
    if let Some(request) = app.take_scan_request() {
        assert!(
            request.sessions.len() < grown.len(),
            "only the sessions not yet scanned are read: {request:?}"
        );
    }

    // The matched session disappears: it leaves the list, nothing panics.
    let mut smaller = snapshot;
    smaller.nodes.retain(|node| match node {
        GraphNode::AgentSession(session) => session.id != matched,
        _ => true,
    });
    refresh(&mut app, &smaller);
    assert!(!listed(&app).contains(&matched.session_key));
    assert!(app.content().is_some());
}

#[test]
fn the_selection_moves_when_its_session_stops_matching_after_a_rescan() {
    let (mut app, _, matched, other) = searching(View::Sessions);
    select(&mut app, &matched.session_key[..6]);
    app.update(Msg::RescanContent);
    let _ = app.take_scan_request();

    // The rescan finds a different session and not the selected one.
    deliver(&mut app, &[&other], true);

    assert_eq!(listed(&app), std::slice::from_ref(&other.session_key));
    let selection = app.selection().cloned();
    let visible: Vec<_> = app
        .visible_rows()
        .iter()
        .map(|row| row.id.clone())
        .collect();
    assert!(
        selection.is_none_or(|id| visible.contains(&id)),
        "the selection is a row that is still listed"
    );
}

#[test]
fn the_overlay_in_the_contents_scope_redraws_at_every_size() {
    let mut app = app_in(View::Sessions);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    app.open_content_search_overlay();
    for (width, height) in [(60, 20), (160, 40), (80, 24), (60, 20)] {
        let buffer = buffer_of(&mut app, width, height);
        let text = crate::tui::ui::buffer_to_string(&buffer);
        assert!(
            text.contains("search contents"),
            "the contents overlay is drawn at {width}x{height}:\n{text}"
        );
    }
    assert!(app.search_overlay().is_some(), "resizing never closes it");
}

fn live_preview_off() -> App {
    let snapshot = world();
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    config.live_preview_enabled = false;
    let mut app = App::new(config);
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: NOW,
        initial_selection_hint: None,
    });
    app
}

#[test]
fn a_startup_search_with_live_preview_off_says_it_cannot_run() {
    let mut config = RunConfig::defaults();
    config.live_preview_enabled = false;
    config.initial_contains = Some("needle".into());

    let app = App::new(config);

    assert!(app.content().is_none(), "nothing is searched");
    assert!(!app.toasts().is_empty(), "the operator is told why");
}

#[test]
fn the_controls_content_row_refuses_with_the_shared_notice_when_live_preview_is_off() {
    use crate::tui::runtime::modal_key_pure;
    use crate::tui::widgets::controls::ControlsCursor;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = live_preview_off();
    app.open_controls_overlay();
    let press = |app: &mut App, code| {
        modal_key_pure(app, KeyEvent::new(code, KeyModifiers::NONE));
    };
    for _ in 0..60 {
        if app
            .controls_overlay()
            .and_then(|panel| panel.cursor().copied())
            == Some(ControlsCursor::FilterContent)
        {
            break;
        }
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(
        app.controls_overlay()
            .and_then(|panel| panel.cursor().copied()),
        Some(ControlsCursor::FilterContent),
        "the Content row is reachable"
    );
    press(&mut app, KeyCode::Enter);
    for ch in "needle".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    press(&mut app, KeyCode::Enter);

    assert!(app.content().is_none(), "no search starts");
    assert!(!app.toasts().is_empty(), "the shared notice is shown");
}

#[test]
fn the_current_scope_reads_sessions_under_collapsed_groups() {
    let world = world();
    let mut app = app_over(View::Sessions, &world);
    super::search_quick::collapse_groups(&mut app);
    let visible = app.visible_rows().len();
    assert!(
        app.tree().rows.len() > visible,
        "the world has rows hidden under collapsed groups"
    );

    let scope = app.current_scope().expect("a view has a scope");

    let listed: Vec<_> = session_ids(&world)
        .into_iter()
        .filter(|id| {
            app.tree().rows.iter().any(|row| {
                crate::tui::rows::row_node(&row.id) == Some(&NodeId::AgentSession(id.clone()))
            })
        })
        .collect();
    assert!(!listed.is_empty());
    for id in listed {
        assert!(
            scope.sessions.contains(&SessionKey {
                harness_key: id.harness_key.clone(),
                session_key: id.session_key.clone(),
            }),
            "{id:?} is in scope"
        );
    }
}
