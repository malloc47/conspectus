//! The Search view lists the sessions the content search found, ranked,
//! and everything an operator can do to a session still works on its rows.

use chrono::{TimeZone, Utc};

use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::tui::actions::{PinSubject, pin_subject, viewer_open_target};
use crate::tui::rows::RowKind;
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{ContentKinds, Grouping as ContentGroup, Hit, SessionHits};

pub(super) fn hit(turn: usize, epoch: Option<i64>) -> Hit {
    Hit {
        turn_index: turn,
        role: TurnRole::User,
        kind: TurnKind::Message,
        timestamp: epoch.map(|e| Utc.timestamp_opt(e, 0).single().unwrap()),
        snippet: format!("the needle at turn {turn}"),
        highlight: 4..10,
    }
}

pub(super) fn summary(count: usize, best_turn: usize, latest: i64) -> SessionHits {
    SessionHits {
        count,
        best: hit(best_turn, Some(latest)),
        latest: Some(Utc.timestamp_opt(latest, 0).single().unwrap()),
    }
}

pub(super) fn key_of(id: &AgentSessionId) -> SessionKey {
    SessionKey {
        harness_key: id.harness_key.clone(),
        session_key: id.session_key.clone(),
    }
}

pub(super) fn sessions(snapshot: &GraphSnapshot) -> Vec<AgentSessionId> {
    let mut ids: Vec<AgentSessionId> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session.id.clone()),
            _ => None,
        })
        .collect();
    ids.sort_by(|a, b| a.session_key.cmp(&b.session_key));
    ids
}

/// An app on the Search view with a finished search that found `found`.
pub(super) fn searched(found: &[(&AgentSessionId, SessionHits)]) -> App {
    let snapshot = world();
    let mut app = app_over(View::Search, &snapshot);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    let generation = app.content_generation().expect("a search is active");
    let batch = found
        .iter()
        .map(|(id, hits)| (key_of(id), hits.clone()))
        .collect();
    app.update(Msg::ContentHits {
        generation,
        batch,
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        done: true,
    });
    // The app queues work for the runtime; tests have none to run.
    app.take_scan_request();
    app
}

fn session_rows(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::AgentSession(session) => Some(session.session.session_key.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn two() -> (AgentSessionId, AgentSessionId) {
    let ids = sessions(&world());
    (ids[0].clone(), ids[1].clone())
}

#[test]
fn with_no_query_the_view_is_empty() {
    let app = app_over(View::Search, &world());
    assert!(app.tree().rows.is_empty());
}

#[test]
fn only_sessions_with_hits_are_listed_most_hits_first() {
    let (a, b) = two();
    let app = searched(&[(&a, summary(2, 0, 1_000)), (&b, summary(5, 0, 900))]);
    assert_eq!(session_rows(&app), [b.session_key, a.session_key]);
}

#[test]
fn the_recency_sort_puts_the_latest_hit_first() {
    let (a, b) = two();
    let mut app = searched(&[(&a, summary(2, 0, 2_000)), (&b, summary(5, 0, 900))]);
    app.update(Msg::SetSort(Sort::Recency));
    assert_eq!(session_rows(&app), [a.session_key, b.session_key]);
}

#[test]
fn grouping_by_harness_and_project_makes_buckets() {
    let (a, b) = two();
    let mut app = searched(&[(&a, summary(2, 0, 1_000)), (&b, summary(1, 0, 900))]);
    for grouping in [SearchGrouping::Harness, SearchGrouping::Project] {
        app.update(Msg::SetGrouping(Grouping::Search(grouping)));
        assert!(
            app.tree()
                .rows
                .iter()
                .any(|row| matches!(row.id, RowId::Bucket(_))),
            "{grouping:?} makes buckets"
        );
        assert!(
            app.tree()
                .rows
                .iter()
                .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
                .all(|row| row.depth == 1),
            "sessions sit under their bucket"
        );
    }
}

#[test]
fn per_message_results_list_the_matching_turns_newest_first() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 3, 1_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Message));
    let request = app.take_detail_request().expect("the turns are requested");
    assert_eq!(request.sessions, std::slice::from_ref(&a));
    app.update(Msg::ContentDetails {
        generation: request.generation,
        batch: vec![(key_of(&a), vec![hit(3, Some(1_000)), hit(7, Some(2_000))])],
    });
    let turns: Vec<usize> = app
        .tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Hit(hit) => Some(hit.turn_index),
            _ => None,
        })
        .collect();
    assert_eq!(turns, [7, 3], "newest first");
    let session = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .unwrap();
    assert!(session.expandable);
    assert!(
        app.visible_rows()
            .iter()
            .any(|row| matches!(row.id, RowId::Hit { .. })),
        "the session opens by itself so its matches show"
    );
}

#[test]
fn details_from_an_older_search_are_dropped() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 3, 1_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Message));
    let request = app.take_detail_request().unwrap();
    app.update(Msg::ContentDetails {
        generation: request.generation + 1,
        batch: vec![(key_of(&a), vec![hit(9, None)])],
    });
    assert!(
        !app.tree()
            .rows
            .iter()
            .any(|row| matches!(row.kind, RowKind::Hit(_)))
    );
}

pub(super) fn view_effect(
    effects: &[crate::tui::Effect],
) -> Option<(AgentSessionId, Option<(String, usize)>)> {
    effects.iter().find_map(|effect| match effect {
        crate::tui::Effect::Exec(crate::tui::effect::ExecSpec::ViewSession(id, open)) => Some((
            id.clone(),
            open.as_ref().map(|o| (o.query.clone(), o.turn_index)),
        )),
        _ => None,
    })
}

#[test]
fn enter_and_v_open_the_viewer_at_the_best_hit() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 4, 1_000))]);
    select(&mut app, &a.session_key);
    for effects in [enter(&mut app), app.update(Msg::ViewSelected)] {
        assert_eq!(
            view_effect(&effects),
            Some((a.clone(), Some(("needle".to_string(), 4))))
        );
    }
}

#[test]
fn enter_on_a_hit_row_opens_the_viewer_at_that_turn() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 4, 1_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Message));
    let request = app.take_detail_request().unwrap();
    app.update(Msg::ContentDetails {
        generation: request.generation,
        batch: vec![(key_of(&a), vec![hit(4, Some(1_000)), hit(9, Some(900))])],
    });
    app.set_selection(RowId::Hit {
        session: NodeId::AgentSession(a.clone()),
        turn: 9,
    });
    assert_eq!(
        view_effect(&enter(&mut app)),
        Some((a, Some(("needle".to_string(), 9))))
    );
}

#[test]
fn the_viewer_opens_at_a_hit_only_in_the_search_view() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 4, 1_000))]);
    select(&mut app, &a.session_key);
    assert!(viewer_open_target(&app).is_some());
    app.update(Msg::SwitchView(View::Sessions));
    assert!(
        viewer_open_target(&app).is_none(),
        "outside the Search view the viewer opens as always"
    );
}

#[test]
fn a_hit_row_pins_and_menus_as_its_session() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 4, 1_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Message));
    let request = app.take_detail_request().unwrap();
    app.update(Msg::ContentDetails {
        generation: request.generation,
        batch: vec![(key_of(&a), vec![hit(4, Some(1_000))])],
    });
    select(&mut app, &a.session_key);
    let session_menu = menu_labels(&mut app);
    app.set_selection(RowId::Hit {
        session: NodeId::AgentSession(a.clone()),
        turn: 4,
    });
    assert_eq!(menu_labels(&mut app), session_menu);
    assert_eq!(
        pin_subject(&app),
        Ok(PinSubject::Node(NodeId::AgentSession(a)))
    );
}

#[test]
fn clearing_the_filters_keeps_the_search_in_the_view() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 4, 1_000))]);
    let listed = app.tree().rows.len();
    assert!(listed > 0);
    app.update(Msg::ClearFilters);
    assert!(
        app.content().is_none(),
        "nothing filters the lists any more"
    );
    assert_eq!(
        app.tree().rows.len(),
        listed,
        "the remembered search is still shown"
    );
}

#[test]
fn the_search_view_is_not_the_view_a_restart_opens_on() {
    let dir = tempfile::tempdir().unwrap();
    let cache = crate::tui_state::TuiStateCache::new().with_xdg_state_home(dir.path());
    crate::tui_state::write_last_view(&cache, View::Search).unwrap();
    assert_eq!(crate::tui_state::read_last_view(&cache), None);
    crate::tui_state::write_last_view(&cache, View::Pinned).unwrap();
    assert_eq!(crate::tui_state::read_last_view(&cache), Some(View::Pinned));
}

#[test]
fn live_preview_off_says_so_instead_of_searching() {
    let mut config = RunConfig::defaults();
    config.live_preview_enabled = false;
    config.default_view = View::Search;
    let mut app = App::new(config);
    app.update(Msg::SwitchView(View::Search));
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    assert!(app.content().is_none());
}
