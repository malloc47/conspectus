use std::time::Duration;

use super::*;
use crate::model::{AgentSessionNode, GraphNode};
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::Hit;
use crate::viewer::search::SessionHits;

pub(super) fn session(key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", key),
        "claude-code",
    ))
}

pub(super) fn snapshot_of(keys: &[&str]) -> GraphSnapshot {
    GraphSnapshot {
        nodes: keys.iter().map(|key| session(key)).collect(),
        ..GraphSnapshot::empty()
    }
}

pub(super) fn load(app: &mut App, keys: &[&str]) {
    let snapshot = snapshot_of(keys);
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 0,
        initial_selection_hint: None,
    });
}

pub(super) fn app_with(keys: &[&str]) -> App {
    let mut app = App::new(RunConfig::defaults());
    load(&mut app, keys);
    app
}

pub(super) fn key(session: &str) -> SessionKey {
    SessionKey {
        harness_key: "claude-code".into(),
        session_key: session.into(),
    }
}

pub(super) fn hits(count: usize) -> SessionHits {
    let best = Hit {
        turn_index: 0,
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        timestamp: None,
        snippet: "found the needle here".into(),
        highlight: 10..16,
    };
    SessionHits {
        count,
        best,
        latest: None,
    }
}

pub(super) fn start(app: &mut App, text: &str) {
    app.update(Msg::StartContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
    });
}

pub(super) fn progress(scanned: usize, total: usize) -> Progress {
    Progress {
        scanned,
        total,
        skipped: 0,
    }
}

pub(super) fn deliver(app: &mut App, batch: Vec<(SessionKey, SessionHits)>, done: bool) {
    let generation = app.content_generation().expect("a search is active");
    app.update(Msg::ContentHits {
        generation,
        batch,
        progress: progress(2, 2),
        done,
    });
}

pub(super) fn listed_sessions(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            crate::tui::rows::RowKind::AgentSession(session) => {
                Some(session.session.session_key.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn starting_a_search_asks_the_runtime_to_scan_every_session() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "  needle ");

    let request = app.take_scan_request().expect("a scan is requested");
    assert_eq!(request.text, "needle", "the text is trimmed");
    assert_eq!(request.sessions.len(), 2);
    assert_eq!(request.generation, app.content_generation().unwrap());
    assert!(app.take_scan_request().is_none(), "a request is taken once");
    assert!(matches!(
        app.content().unwrap().status,
        ContentStatus::Running(progress) if progress.total == 2
    ));
    assert!(
        app.in_flight_ops()
            .iter()
            .any(|op| op.kind == InFlightKind::Search),
        "the status bar shows a spinner chip"
    );
}

#[test]
fn an_empty_query_is_the_same_as_clearing() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    start(&mut app, "   ");

    assert!(app.content().is_none());
    assert!(app.take_scan_request().is_none());
}

#[test]
fn results_narrow_the_session_list_as_they_arrive() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    assert!(
        listed_sessions(&app).is_empty(),
        "nothing matches before the first result"
    );

    deliver(&mut app, vec![(key("a"), hits(3))], false);
    assert_eq!(listed_sessions(&app), ["a"]);
    assert!(!app.content().unwrap().status.is_done());

    deliver(&mut app, vec![(key("b"), hits(1))], true);
    assert_eq!(listed_sessions(&app), ["a", "b"]);
    assert!(app.content().unwrap().status.is_done());
    assert!(
        !app.in_flight_ops()
            .iter()
            .any(|op| op.kind == InFlightKind::Search),
        "the spinner goes when the scan ends"
    );
}

#[test]
fn the_best_snippet_and_count_reach_the_row() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    deliver(&mut app, vec![(key("a"), hits(3))], true);

    let cell = app
        .tree()
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            crate::tui::rows::RowKind::AgentSession(session) => session.content_hit.clone(),
            _ => None,
        })
        .expect("the row carries the hit");
    assert_eq!(cell.count, 3);
    assert_eq!(&cell.snippet[cell.highlight.clone()], "needle");
}

#[test]
fn results_from_a_replaced_scan_are_dropped() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "first");
    let stale = app.content_generation().unwrap();
    start(&mut app, "second");

    app.update(Msg::ContentHits {
        generation: stale,
        batch: vec![(key("a"), hits(1))],
        progress: progress(1, 2),
        done: true,
    });

    assert!(app.content().unwrap().hits().is_empty());
    assert!(!app.content().unwrap().status.is_done());
}

#[test]
fn clearing_stops_the_scan_and_shows_every_session() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    deliver(&mut app, vec![(key("a"), hits(1))], false);
    let running = app.content_generation().unwrap();

    app.update(Msg::ClearContentSearch);

    assert!(app.content().is_none());
    assert_eq!(
        app.content_generation(),
        None,
        "the runtime sees no search and cancels the scan it holds"
    );
    assert_eq!(listed_sessions(&app), ["a", "b"]);
    app.update(Msg::ContentHits {
        generation: running,
        batch: vec![(key("b"), hits(1))],
        progress: progress(2, 2),
        done: true,
    });
    assert!(app.content().is_none(), "late results are ignored");
}

#[test]
fn clear_filters_clears_the_structured_filter_and_the_search() {
    let mut app = app_with(&["a", "b"]);
    app.update(Msg::SetFilter(crate::filter::RowFilter {
        max_age: Some(Duration::from_secs(60)),
        ..crate::filter::RowFilter::default()
    }));
    start(&mut app, "needle");

    app.update(Msg::ClearFilters);

    assert!(app.filter().is_empty());
    assert!(app.content().is_none());
}

#[test]
fn a_search_is_refused_while_live_preview_is_off() {
    let mut config = RunConfig::defaults();
    config.live_preview_enabled = false;
    let mut app = App::new(config);
    load(&mut app, &["a"]);

    let effects = app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });

    assert!(app.content().is_none());
    assert!(app.take_scan_request().is_none());
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(text)] if text == CONTENT_OFF_NOTICE),
        "the refusal is said once, as a toast: {effects:?}"
    );
    assert!(
        app.status_message().is_none(),
        "not also in the status line"
    );
}

#[test]
fn a_query_given_before_the_first_snapshot_scans_when_it_arrives() {
    let mut config = RunConfig::defaults();
    config.initial_contains = Some("needle".into());
    let mut app = App::new(config);
    assert!(app.content().is_some());
    assert!(
        app.take_scan_request().is_none(),
        "nothing to scan before there is a snapshot"
    );

    load(&mut app, &["a", "b"]);

    let request = app
        .take_scan_request()
        .expect("the first snapshot starts it");
    assert_eq!(request.sessions.len(), 2);
}

#[test]
fn a_refresh_scans_only_the_sessions_that_appeared() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    app.take_scan_request();
    deliver(&mut app, vec![(key("a"), hits(1))], true);

    load(&mut app, &["a", "b", "c"]);

    let request = app.take_scan_request().expect("a delta scan");
    let keys: Vec<_> = request
        .sessions
        .iter()
        .map(|id| id.session_key.as_str())
        .collect();
    assert_eq!(keys, ["c"]);
    // The earlier result stays while the delta runs.
    assert_eq!(app.content().unwrap().hits().len(), 1);
}

#[test]
fn a_refresh_during_a_scan_waits_for_it_to_end() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    app.take_scan_request();

    load(&mut app, &["a", "b"]);
    assert!(
        app.take_scan_request().is_none(),
        "the running scan is not interrupted"
    );

    deliver(&mut app, vec![], true);
    let request = app
        .take_scan_request()
        .expect("the new session is scanned next");
    assert_eq!(request.sessions.len(), 1);
}

#[test]
fn a_rescan_keeps_the_old_results_until_it_finishes() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    app.take_scan_request();
    deliver(&mut app, vec![(key("a"), hits(1))], true);

    app.update(Msg::RescanContent);
    let request = app.take_scan_request().expect("a full rescan");
    assert_eq!(request.sessions.len(), 2);
    assert_eq!(
        listed_sessions(&app),
        ["a"],
        "the list does not empty while it reads again"
    );

    deliver(&mut app, vec![(key("b"), hits(2))], true);
    assert_eq!(
        listed_sessions(&app),
        ["b"],
        "the fresh result replaces the old one"
    );
}

#[test]
fn rescan_with_no_search_does_nothing() {
    let mut app = app_with(&["a"]);
    app.update(Msg::RescanContent);
    assert!(app.take_scan_request().is_none());
}

#[test]
fn the_search_is_not_part_of_the_persisted_state() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle-in-secret");
    deliver(&mut app, vec![(key("a"), hits(1))], true);

    let dir = tempfile::tempdir().unwrap();
    let cache = crate::tui_state::TuiStateCache::new().with_xdg_state_home(dir.path());
    crate::tui_state::write_tui_state(&cache, &app.build_persisted_state()).unwrap();
    let text = std::fs::read_to_string(cache.path().unwrap()).unwrap();

    assert!(!text.contains("needle-in-secret"), "{text}");
    assert!(!text.contains("found the needle here"), "{text}");
}
