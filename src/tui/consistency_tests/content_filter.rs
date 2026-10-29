//! The content search narrows the session lists the same way in every
//! view, composes with the structured filters, and never leaves memory.

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::tui::rows::RowKind;
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{ContentKinds, Hit, SessionHits};

fn hits(count: usize) -> SessionHits {
    SessionHits {
        count,
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

fn key_of(id: &AgentSessionId) -> SessionKey {
    SessionKey {
        harness_key: id.harness_key.clone(),
        session_key: id.session_key.clone(),
    }
}

fn start(app: &mut App, text: &str) {
    app.update(Msg::StartContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
    });
}

fn finish_with(app: &mut App, matched: &[&AgentSessionId], done: bool) {
    let generation = app.content_generation().expect("a search is active");
    let batch = matched.iter().map(|id| (key_of(id), hits(2))).collect();
    app.update(Msg::ContentHits {
        generation,
        batch,
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        done,
    });
}

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

fn listed_muxes(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
            _ => None,
        })
        .collect()
}

/// The world, a session that matches, and one that does not.
fn matched_and_not() -> (GraphSnapshot, AgentSessionId, AgentSessionId) {
    let snapshot = world();
    let ids = session_ids(&snapshot);
    let matched = ids
        .iter()
        .find(|id| id.session_key == "solo000001")
        .expect("the fixture's loose session")
        .clone();
    let other = ids
        .iter()
        .find(|id| id.session_key != "solo000001")
        .expect("another session")
        .clone();
    (snapshot, matched, other)
}

#[test]
fn only_matching_sessions_remain_in_the_views_that_list_sessions() {
    let (snapshot, matched, other) = matched_and_not();
    for view in ENTITY_VIEWS {
        let mut app = app_over(view, &snapshot);
        let before = listed_sessions(&app);
        start(&mut app, "needle");
        finish_with(&mut app, &[&matched], true);
        let after = listed_sessions(&app);
        assert!(
            !after.contains(&other.session_key),
            "{view:?} still lists a session without a hit: {after:?}"
        );
        if before.contains(&matched.session_key) {
            assert!(
                after.contains(&matched.session_key),
                "{view:?} lost the session that matched: {after:?}"
            );
        }
        assert!(
            after.iter().all(|key| *key == matched.session_key),
            "{view:?}: {after:?}"
        );
    }
}

#[test]
fn a_mux_with_no_matching_session_leaves_the_mux_view() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Mux, &snapshot);
    let before = listed_muxes(&app);
    assert!(before.len() > 1, "the fixture has several muxes");
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    let after = listed_muxes(&app);
    assert!(
        after.len() < before.len(),
        "muxes without a matching session should go: {before:?} -> {after:?}"
    );
}

#[test]
fn the_search_composes_with_the_structured_filters() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);
    assert_eq!(
        listed_sessions(&app),
        std::slice::from_ref(&matched.session_key)
    );

    // Another filter that the matched session passes keeps it.
    app.update(Msg::SetFilter(RowFilter {
        pinned_only: true,
        ..RowFilter::default()
    }));
    assert_eq!(
        listed_sessions(&app),
        std::slice::from_ref(&matched.session_key)
    );

    // One it fails removes it, whatever the search says.
    app.update(Msg::SetFilter(RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    }));
    assert!(listed_sessions(&app).is_empty());

    // Editing the structured filter did not touch the search.
    assert!(app.content().is_some());
}

#[test]
fn grouping_and_sort_changes_keep_the_narrowing() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    for grouping in [
        crate::tui::SessionsGrouping::Workspace,
        crate::tui::SessionsGrouping::Repo,
        crate::tui::SessionsGrouping::None,
    ] {
        app.update(Msg::SetGrouping(Grouping::Sessions(grouping)));
        assert_eq!(
            listed_sessions(&app),
            std::slice::from_ref(&matched.session_key),
            "{grouping:?}"
        );
    }
    app.update(Msg::SetSort(Sort::Recency));
    assert_eq!(
        listed_sessions(&app),
        std::slice::from_ref(&matched.session_key)
    );
}

#[test]
fn the_search_follows_the_operator_across_views() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    app.update(Msg::SwitchView(View::Mux));
    assert!(app.content().is_some(), "the query is app-wide");
    app.update(Msg::SwitchView(View::Sessions));
    assert_eq!(
        listed_sessions(&app),
        std::slice::from_ref(&matched.session_key)
    );
}

#[test]
fn pinned_and_history_are_left_alone_and_say_so() {
    let (snapshot, matched, _) = matched_and_not();
    for view in [View::Pinned, View::History] {
        let mut plain = app_over(view, &snapshot);
        let before: Vec<_> = plain.tree().rows.iter().map(|row| row.id.clone()).collect();
        start(&mut plain, "needle");
        finish_with(&mut plain, &[&matched], true);
        let after: Vec<_> = plain.tree().rows.iter().map(|row| row.id.clone()).collect();
        assert_eq!(before, after, "{view:?} rows should not change");
        let text = frame(&mut plain, 160, 30);
        assert!(
            text.contains("not applied here"),
            "{view:?} should say the search is not applied:\n{text}"
        );
    }
}

#[test]
fn clearing_everything_with_f_removes_the_search_too() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    let before = listed_sessions(&app);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    let key = KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT);
    let action = crate::tui::runtime::action_for_event(&app, Event::Key(key));
    assert!(
        matches!(action, Some(crate::tui::keymap::Action::ClearFilters)),
        "F clears the filters: {action:?}"
    );
    app.update(Msg::ClearFilters);

    assert!(app.content().is_none());
    assert_eq!(listed_sessions(&app), before);
}

#[test]
fn the_controls_clear_button_clears_a_search_with_no_other_filter() {
    use crate::tui::widgets::controls::ControlsCursor;
    use tuikit::overlay::{Outcome, Overlay};
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    assert!(
        app.controls_context().filter.is_empty(),
        "the structured filter is empty; only the search is active"
    );
    app.open_controls_overlay();
    let mut panel = app.controls_overlay().cloned().expect("the panel");
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    for _ in 0..60 {
        if panel.cursor() == Some(&ControlsCursor::FilterClear) {
            break;
        }
        panel.handle(down, &app);
    }
    assert_eq!(panel.cursor(), Some(&ControlsCursor::FilterClear));
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let outcome = panel.handle(enter, &app);
    assert_eq!(outcome, Outcome::Stay(Msg::ClearFilters));
}

#[test]
fn the_search_is_never_serialized_with_a_filter() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], true);

    let mut filter = app.filter().clone();
    filter.content = app.content_filter();
    assert!(filter.content.is_some());
    let with_search = serde_json::to_string(&filter).unwrap();
    let without = serde_json::to_string(&RowFilter::default()).unwrap();
    assert_eq!(with_search, without, "{with_search}");
}

#[test]
fn streaming_results_keep_the_selection_on_its_row() {
    let (snapshot, matched, other) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], false);
    select(&mut app, &matched.session_key[..6]);
    let selected = app.selection().cloned();
    assert!(selected.is_some());

    finish_with(&mut app, &[&other], false);
    assert_eq!(app.selection().cloned(), selected, "the cursor stays put");
}

#[test]
fn the_header_shows_the_query_and_the_scan_and_rows_show_their_hits() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    finish_with(&mut app, &[&matched], false);
    let running = frame(&mut app, 170, 30);
    assert!(
        running.contains("find:\"needle\" searching 1/1"),
        "{running}"
    );
    assert!(running.contains("2 hits"), "{running}");
    assert!(running.contains("mentions the needle"), "{running}");

    finish_with(&mut app, &[], true);
    let done = frame(&mut app, 170, 30);
    assert!(done.contains("find:\"needle\" 1 session"), "{done}");
}

#[test]
fn an_empty_result_explains_itself_while_searching_and_when_done() {
    let (snapshot, _, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "needle");
    let searching = frame(&mut app, 120, 20);
    assert!(
        searching.contains("Searching for \"needle\""),
        "{searching}"
    );

    finish_with(&mut app, &[], true);
    let none = frame(&mut app, 120, 20);
    assert!(none.contains("No sessions match \"needle\""), "{none}");
}

#[test]
fn the_controls_overlay_offers_the_content_row_where_it_applies() {
    use crate::tui::widgets::controls::{ControlsCursor, flatten_rows};
    for view in ENTITY_VIEWS {
        let app = app_over(view, &world());
        assert!(
            flatten_rows(&app.controls_context()).contains(&ControlsCursor::FilterContent),
            "{view:?} should offer the Content row"
        );
    }
    for view in [View::Pinned, View::History] {
        let app = app_over(view, &world());
        assert!(
            !flatten_rows(&app.controls_context()).contains(&ControlsCursor::FilterContent),
            "{view:?} does not apply the search"
        );
    }
}

#[test]
fn stale_results_do_not_change_what_is_listed() {
    let (snapshot, matched, _) = matched_and_not();
    let mut app = app_over(View::Sessions, &snapshot);
    start(&mut app, "first");
    let stale = app.content_generation().unwrap();
    start(&mut app, "second");
    app.update(Msg::ContentHits {
        generation: stale,
        batch: vec![(key_of(&matched), hits(1))],
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        done: true,
    });
    assert!(listed_sessions(&app).is_empty());
}
