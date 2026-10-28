use std::time::Duration;

use super::tests::{app_with, deliver, hits, key, load, progress, start};
use super::*;
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::Hit;

fn turn(index: usize) -> Hit {
    Hit {
        turn_index: index,
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        timestamp: None,
        snippet: "found".into(),
        highlight: 0..5,
    }
}

/// Per-message results live in the Search view, where the sessions the
/// operator has open are the ones whose turns get read.
fn group_by_message(app: &mut App) {
    app.update(Msg::SwitchView(View::Search));
    app.update(Msg::SetContentGroup(Grouping::Message));
}

/// Open every session row the way an operator walking the list would.
fn open_every_session(app: &mut App) {
    let rows: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| row.expandable && matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| row.id.clone())
        .collect();
    for id in rows {
        app.set_selection(id);
        if !app.expanded.contains(app.selection().unwrap()) {
            app.update(Msg::ExpandRow);
        }
    }
}

#[test]
fn a_session_that_is_scanned_again_has_its_turns_read_again() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    deliver(&mut app, vec![(key("a"), hits(1))], true);
    group_by_message(&mut app);
    let generation = app.content_generation().unwrap();
    app.update(Msg::ContentDetails {
        generation,
        batch: vec![(key("a"), vec![turn(0)])],
    });
    assert!(
        app.content()
            .unwrap()
            .details
            .get("claude-code", "a")
            .is_some()
    );

    // The transcript grew and the session was scanned again.
    deliver(&mut app, vec![(key("a"), hits(2))], true);

    assert!(
        app.content()
            .unwrap()
            .details
            .get("claude-code", "a")
            .is_none(),
        "turns read for the old result are dropped"
    );
    let request = app.take_detail_request().expect("they are read again");
    assert_eq!(request.sessions.len(), 1);
}

#[test]
fn a_full_rescan_forgets_every_turn_it_had_read() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    deliver(
        &mut app,
        vec![(key("a"), hits(1)), (key("b"), hits(1))],
        true,
    );
    group_by_message(&mut app);
    let generation = app.content_generation().unwrap();
    app.update(Msg::ContentDetails {
        generation,
        batch: vec![(key("a"), vec![turn(0)]), (key("b"), vec![turn(1)])],
    });

    app.update(Msg::RescanContent);

    assert!(app.content().unwrap().details.is_empty());
}

#[test]
fn an_incremental_scan_adds_to_the_counters_of_the_scan_before_it() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    app.update(Msg::ContentHits {
        generation: app.content_generation().unwrap(),
        batch: Vec::new(),
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 1,
        },
        done: true,
    });

    // A refresh brings one more session.
    load(&mut app, &["a", "b"]);
    assert!(matches!(
        app.content().unwrap().status,
        ContentStatus::Running(p) if p.total == 2 && p.scanned == 1 && p.skipped == 1
    ));
    app.update(Msg::ContentHits {
        generation: app.content_generation().unwrap(),
        batch: Vec::new(),
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 2,
        },
        done: true,
    });

    assert_eq!(
        app.content().unwrap().progress(),
        Progress {
            scanned: 2,
            total: 2,
            skipped: 3,
        },
        "the totals cover every session searched"
    );
}

#[test]
fn a_full_rescan_starts_its_counters_from_zero() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    deliver(&mut app, Vec::new(), true);
    app.update(Msg::RescanContent);
    assert!(matches!(
        app.content().unwrap().status,
        ContentStatus::Running(p) if p == Progress { scanned: 0, total: 1, skipped: 0 }
    ));
}

#[test]
fn a_preview_batch_does_not_rebuild_the_lists_but_an_applied_one_does() {
    let mut app = app_with(&["a"]);
    app.update(Msg::PreviewContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: crate::tui::widgets::search::Reach::All,
    });
    let before = app.tree_rebuilds;
    deliver(&mut app, vec![(key("a"), hits(1))], false);
    assert_eq!(
        app.tree_rebuilds, before,
        "a preview leaves the lists alone"
    );

    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    let before = app.tree_rebuilds;
    deliver(&mut app, vec![(key("a"), hits(1))], false);
    assert_eq!(
        app.tree_rebuilds,
        before + 1,
        "an applied search narrows them"
    );
}

#[test]
fn details_for_a_preview_do_not_rebuild_the_lists() {
    let mut app = app_with(&["a"]);
    app.update(Msg::PreviewContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Message,
        reach: crate::tui::widgets::search::Reach::All,
    });
    deliver(&mut app, vec![(key("a"), hits(1))], true);
    let before = app.tree_rebuilds;
    let generation = app.content_generation().unwrap();
    app.update(Msg::ContentDetails {
        generation,
        batch: vec![(key("a"), vec![turn(0)])],
    });
    assert_eq!(app.tree_rebuilds, before);
}

#[test]
fn a_new_query_forgets_which_search_rows_were_opened() {
    let mut app = app_with(&["a"]);
    app.search_auto_expanded.insert(RowId::Bucket("x".into()));
    start(&mut app, "needle");
    assert!(app.search_auto_expanded.is_empty());

    app.search_auto_expanded.insert(RowId::Bucket("y".into()));
    app.update(Msg::ClearContentSearch);
    assert!(app.search_auto_expanded.is_empty());
}

#[test]
fn only_the_best_ranked_sessions_have_their_turns_read_and_never_more_than_the_cap() {
    let keys: Vec<String> = (0..MAX_DETAIL_SESSIONS + 20)
        .map(|i| format!("s{i:03}"))
        .collect();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut app = app_with(&refs);
    start(&mut app, "needle");
    let batch = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (key(k), hits(i + 1)))
        .collect();
    deliver(&mut app, batch, true);

    group_by_message(&mut app);

    // Only the sessions that opened by themselves are read at first.
    let first = app.take_detail_request().expect("details are requested");
    assert_eq!(first.sessions.len(), AUTO_EXPAND_SESSIONS);

    // Opening the rest asks for the best-ranked, up to the cap.
    open_every_session(&mut app);
    let request = app.take_detail_request().expect("details are requested");
    assert_eq!(request.sessions.len(), MAX_DETAIL_SESSIONS);
    let wanted: BTreeSet<&str> = request
        .sessions
        .iter()
        .map(|s| s.session_key.as_str())
        .collect();
    assert!(wanted.contains("s119"), "the most hits come first");
    assert!(!wanted.contains("s000"), "the fewest are left out");

    // With the cap's worth held, nothing more is asked for.
    let generation = app.content_generation().unwrap();
    let batch = request
        .sessions
        .iter()
        .map(|s| (key(&s.session_key), vec![turn(0)]))
        .collect();
    app.update(Msg::ContentDetails { generation, batch });
    app.update(Msg::SetContentGroup(Grouping::Message));
    assert!(app.take_detail_request().is_none());
    assert_eq!(app.content().unwrap().details.len(), MAX_DETAIL_SESSIONS);
}

#[test]
fn the_status_note_tells_progress_skips_and_how_old_the_scan_is() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "needle");
    let now = Instant::now();
    assert_eq!(app.content().unwrap().status_note(now), "searching 0/2");

    app.update(Msg::ContentHits {
        generation: app.content_generation().unwrap(),
        batch: vec![(key("a"), hits(1))],
        progress: Progress {
            scanned: 2,
            total: 2,
            skipped: 1,
        },
        done: true,
    });
    let finished = app.content().unwrap().finished_at().expect("it finished");
    let note = |secs| {
        app.content()
            .unwrap()
            .status_note(finished + Duration::from_secs(secs))
    };
    // The shared age ladder, as every other age the TUI writes as prose.
    assert_eq!(note(1), "1 session · 1 skipped · scanned 1s ago");
    assert_eq!(note(125), "1 session · 1 skipped · scanned 2m ago");
    assert_eq!(note(7300), "1 session · 1 skipped · scanned 2h ago");
    assert_eq!(note(90_000), "1 session · 1 skipped · scanned 1d ago");
}

#[test]
fn a_running_scan_has_no_age() {
    let mut app = app_with(&["a"]);
    start(&mut app, "needle");
    assert!(app.content().unwrap().finished_at().is_none());
    deliver(&mut app, Vec::new(), true);
    app.update(Msg::RescanContent);
    assert!(
        app.content().unwrap().finished_at().is_none(),
        "a rescan is running, so the old age no longer applies"
    );
}

#[test]
fn the_overlay_view_is_rebuilt_only_when_what_it_shows_changes() {
    let mut app = app_with(&["a", "b"]);
    app.update(Msg::PreviewContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: crate::tui::widgets::search::Reach::All,
    });
    deliver(&mut app, vec![(key("a"), hits(1))], false);

    let first = app.content_view();
    assert_eq!(first.rows.len(), 1);
    let cached_key = app.content.as_ref().unwrap().view_cache.borrow().clone();
    assert!(cached_key.is_some(), "the view is kept");
    assert_eq!(
        app.content_view(),
        first,
        "asking again gives the same view"
    );

    deliver(&mut app, vec![(key("b"), hits(2))], false);
    assert_eq!(app.content_view().rows.len(), 2, "a new match shows");
}

#[test]
fn progress_counters_add_up() {
    let sum = add_progress(progress(1, 2), progress(3, 4));
    assert_eq!(sum, progress(4, 6));
}

#[test]
fn a_session_past_the_first_few_is_read_when_it_is_opened_not_before() {
    let keys: Vec<String> = (0..AUTO_EXPAND_SESSIONS + 2)
        .map(|i| format!("s{i:02}"))
        .collect();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut app = app_with(&refs);
    start(&mut app, "needle");
    let batch = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (key(k), hits(i + 1)))
        .collect();
    deliver(&mut app, batch, true);
    group_by_message(&mut app);

    let first = app.take_detail_request().expect("the open ones are read");
    assert_eq!(first.sessions.len(), AUTO_EXPAND_SESSIONS);
    let generation = app.content_generation().unwrap();
    let batch = first
        .sessions
        .iter()
        .map(|s| (key(&s.session_key), vec![turn(0)]))
        .collect();
    app.update(Msg::ContentDetails { generation, batch });
    assert!(
        app.take_detail_request().is_none(),
        "closed sessions are not read"
    );

    // Every session row can be opened, read or not.
    let closed: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| row.expandable && !app.expanded.contains(&row.id))
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| row.id.clone())
        .collect();
    assert_eq!(closed.len(), 2);
    app.set_selection(closed[0].clone());
    app.update(Msg::ExpandRow);

    let next = app.take_detail_request().expect("opening reads it");
    assert_eq!(next.sessions.len(), 1);
}
