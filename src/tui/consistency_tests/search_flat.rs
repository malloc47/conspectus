//! The flat listing: every matching message across all sessions as one
//! list with no session rows, newest first, where each row still acts
//! on the session it came from.

use super::search_view::{hit, key_of, searched, sessions, summary, two, view_effect};
use super::*;
use crate::content_search::Progress;
use crate::tui::app::MAX_DETAIL_SESSIONS;
use crate::tui::rows::RowKind;
use crate::viewer::search::{ContentKinds, Grouping as ContentGroup};

/// A finished search over `found`, switched to the flat listing, with
/// the turns of each session delivered as `details`.
fn flat(
    found: &[(&AgentSessionId, crate::viewer::search::SessionHits)],
    details: Vec<(&AgentSessionId, Vec<crate::viewer::search::Hit>)>,
) -> App {
    let mut app = searched(found);
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    let request = app.take_detail_request().expect("the turns are requested");
    app.update(Msg::ContentDetails {
        generation: request.generation,
        batch: details
            .into_iter()
            .map(|(id, hits)| (key_of(id), hits))
            .collect(),
    });
    app
}

/// `(session, turn)` of each hit row, in the order listed.
fn listed(app: &App) -> Vec<(String, usize)> {
    app.tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Hit(hit) => Some((hit.session.session_key.clone(), hit.turn_index)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_message_of_every_session_is_one_list_newest_first() {
    let (a, b) = two();
    let app = flat(
        &[(&a, summary(2, 7, 3_000)), (&b, summary(1, 1, 2_000))],
        vec![
            (&a, vec![hit(3, Some(1_000)), hit(7, Some(3_000))]),
            (&b, vec![hit(1, Some(2_000))]),
        ],
    );
    assert_eq!(
        listed(&app),
        [
            (a.session_key.clone(), 7),
            (b.session_key, 1),
            (a.session_key, 3),
        ],
        "one order across sessions, by the message's own time"
    );
    assert!(
        app.tree()
            .rows
            .iter()
            .all(|row| matches!(row.kind, RowKind::Hit(_)) && row.depth == 0 && !row.expandable),
        "no session rows, nothing nested"
    );
}

#[test]
fn both_sorts_order_flat_rows_by_message_time() {
    let (a, b) = two();
    let mut app = flat(
        &[(&a, summary(9, 0, 1_000)), (&b, summary(1, 0, 5_000))],
        vec![
            (&a, vec![hit(0, Some(1_000))]),
            (&b, vec![hit(0, Some(5_000))]),
        ],
    );
    let hierarchy = listed(&app);
    app.update(Msg::SetSort(Sort::Recency));
    assert_eq!(
        listed(&app),
        hierarchy,
        "relevance has no say between messages"
    );
    assert_eq!(hierarchy[0].0, b.session_key);
}

#[test]
fn ties_fall_to_the_session_then_the_turn_and_untimed_messages_come_last() {
    let (a, b) = two();
    let app = flat(
        &[(&a, summary(3, 0, 1_000)), (&b, summary(2, 0, 1_000))],
        vec![
            (
                &a,
                vec![hit(5, Some(1_000)), hit(2, Some(1_000)), hit(0, None)],
            ),
            (&b, vec![hit(1, Some(1_000)), hit(4, None)]),
        ],
    );
    let (first, second) = if a.session_key < b.session_key {
        (&a, &b)
    } else {
        (&b, &a)
    };
    let timed: Vec<_> = listed(&app).into_iter().take(3).collect();
    let expected: Vec<(String, usize)> = {
        let mut rows = vec![
            (a.session_key.clone(), 2),
            (a.session_key.clone(), 5),
            (b.session_key.clone(), 1),
        ];
        rows.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)));
        rows
    };
    assert_eq!(
        timed, expected,
        "{} before {}",
        first.session_key, second.session_key
    );
    assert!(
        listed(&app)[3..].iter().all(|(session, turn)| {
            (*session == a.session_key && *turn == 0) || (*session == b.session_key && *turn == 4)
        }),
        "messages with no time sit at the end"
    );
}

#[test]
fn each_row_names_its_session_and_keeps_a_unique_stable_id() {
    let (a, b) = two();
    let app = flat(
        &[(&a, summary(2, 3, 1_000)), (&b, summary(1, 3, 900))],
        vec![
            (&a, vec![hit(3, Some(1_000)), hit(4, Some(990))]),
            (&b, vec![hit(3, Some(900))]),
        ],
    );
    let ids: std::collections::BTreeSet<_> = app.tree().rows.iter().map(|r| r.id.clone()).collect();
    assert_eq!(
        ids.len(),
        app.tree().rows.len(),
        "same turn in two sessions"
    );
    for row in &app.tree().rows {
        let RowKind::Hit(hit) = &row.kind else {
            panic!("only hit rows");
        };
        let origin = hit.origin.as_ref().expect("a flat row names its session");
        assert!(!origin.label.is_empty() && !origin.harness_label.is_empty());
    }
}

#[test]
fn the_turns_of_every_matching_session_are_requested_newest_matches_first() {
    let (a, b) = two();
    let mut app = searched(&[(&a, summary(2, 0, 1_000)), (&b, summary(9, 0, 5_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    let request = app
        .take_detail_request()
        .expect("requested without opening a row");
    assert_eq!(
        request.sessions,
        [b, a],
        "the session with the newest match is read first, whatever its hit count"
    );
}

#[test]
fn only_the_cap_is_read_and_the_note_says_so() {
    let snapshot = world_with(MAX_DETAIL_SESSIONS + 10);
    let ids = sessions(&snapshot);
    assert!(ids.len() > MAX_DETAIL_SESSIONS, "{} sessions", ids.len());
    let mut app = app_over(View::Search, &snapshot);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    let generation = app.content_generation().unwrap();
    let batch = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (key_of(id), summary(2, 0, 1_000 + i64::try_from(i).unwrap())))
        .collect();
    app.update(Msg::ContentHits {
        generation,
        batch,
        progress: Progress {
            scanned: ids.len(),
            total: ids.len(),
            skipped: 0,
        },
        done: true,
    });
    app.take_scan_request();
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    let request = app.take_detail_request().unwrap();
    assert_eq!(request.sessions.len(), MAX_DETAIL_SESSIONS);
    // The oldest matches are the ones left out.
    let newest: std::collections::BTreeSet<_> = ids
        .iter()
        .rev()
        .take(MAX_DETAIL_SESSIONS)
        .map(|id| id.session_key.clone())
        .collect();
    assert!(
        request
            .sessions
            .iter()
            .all(|id| newest.contains(&id.session_key))
    );
    let note = app
        .content()
        .unwrap()
        .status_note(std::time::Instant::now());
    assert!(note.contains("reading messages"), "{note}");
    // Deliver the turns: two per session, the cap was the only limit.
    let batch = request
        .sessions
        .iter()
        .map(|id| (key_of(id), vec![hit(0, Some(900)), hit(1, Some(800))]))
        .collect();
    app.update(Msg::ContentDetails {
        generation: request.generation,
        batch,
    });
    let note = app
        .content()
        .unwrap()
        .status_note(std::time::Instant::now());
    let shown = MAX_DETAIL_SESSIONS * 2;
    let total = ids.len() * 2;
    assert!(
        note.contains(&format!("showing {shown} of {total} messages")),
        "{note}"
    );
}

#[test]
fn a_session_read_in_full_leaves_no_note() {
    let (a, _) = two();
    let app = flat(
        &[(&a, summary(2, 0, 1_000))],
        vec![(&a, vec![hit(0, Some(1_000)), hit(1, Some(900))])],
    );
    let note = app
        .content()
        .unwrap()
        .status_note(std::time::Instant::now());
    assert!(
        !note.contains("showing") && !note.contains("reading"),
        "{note}"
    );
    assert!(app.content().unwrap().flat_coverage().is_none());
}

#[test]
fn before_the_turns_arrive_the_view_says_it_is_reading_them() {
    let (a, _) = two();
    let mut app = searched(&[(&a, summary(2, 0, 1_000))]);
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    assert!(app.tree().rows.is_empty());
    let text = frame(&mut app, 120, 24);
    assert!(text.contains("Reading the messages"), "{text}");
    assert!(!text.contains("No sessions match"), "{text}");
}

#[test]
fn a_request_still_waiting_forgets_turns_that_arrived() {
    let (a, b) = two();
    let mut app = searched(&[(&a, summary(2, 0, 1_000)), (&b, summary(2, 0, 900))]);
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    let first = app.take_detail_request().unwrap();
    // A second ask is queued behind the running read ...
    app.update(Msg::SetContentGroup(ContentGroup::Flat));
    // ... and the first read delivers one session.
    app.update(Msg::ContentDetails {
        generation: first.generation,
        batch: vec![(key_of(&a), vec![hit(0, Some(1_000))])],
    });
    let waiting = app
        .take_detail_request()
        .expect("one session is still unread");
    assert_eq!(waiting.sessions, std::slice::from_ref(&b));
}

#[test]
fn buckets_hold_their_messages_in_the_same_order() {
    let (a, b) = two();
    let mut app = flat(
        &[(&a, summary(1, 0, 1_000)), (&b, summary(1, 0, 900))],
        vec![
            (&a, vec![hit(0, Some(1_000))]),
            (&b, vec![hit(0, Some(900))]),
        ],
    );
    for grouping in [SearchGrouping::Harness, SearchGrouping::Project] {
        app.update(Msg::SetGrouping(Grouping::Search(grouping)));
        let rows = &app.tree().rows;
        assert!(rows.iter().any(|row| matches!(row.id, RowId::Bucket(_))));
        assert!(
            rows.iter()
                .filter(|row| matches!(row.kind, RowKind::Hit(_)))
                .all(|row| row.depth == 1),
            "messages sit under their bucket"
        );
    }
}

#[test]
fn a_flat_row_acts_on_its_session() {
    let (a, b) = two();
    let mut app = flat(
        &[(&a, summary(2, 4, 1_000)), (&b, summary(1, 0, 900))],
        vec![
            (&a, vec![hit(4, Some(1_000)), hit(9, Some(950))]),
            (&b, vec![hit(0, Some(900))]),
        ],
    );
    app.set_selection(RowId::Hit {
        session: NodeId::AgentSession(a.clone()),
        turn: 9,
    });
    assert_eq!(
        view_effect(&enter(&mut app)),
        Some((a.clone(), Some(("needle".to_string(), 9)))),
        "Enter opens the viewer at that message"
    );
    assert_eq!(
        view_effect(&app.update(Msg::ViewSelected)),
        Some((a.clone(), Some(("needle".to_string(), 9))))
    );
    assert_eq!(
        crate::tui::actions::pin_subject(&app),
        Ok(crate::tui::actions::PinSubject::Node(NodeId::AgentSession(
            a
        )))
    );
}

#[test]
fn the_menu_of_a_flat_row_is_its_sessions_menu() {
    let (a, _) = two();
    // The same session's row in the one-row-per-session listing.
    let mut sessions_app = searched(&[(&a, summary(1, 4, 1_000))]);
    select(&mut sessions_app, &a.session_key);
    let session_menu = menu_labels(&mut sessions_app);
    let mut app = flat(
        &[(&a, summary(1, 4, 1_000))],
        vec![(&a, vec![hit(4, Some(1_000))])],
    );
    app.set_selection(RowId::Hit {
        session: NodeId::AgentSession(a),
        turn: 4,
    });
    assert!(!session_menu.is_empty());
    assert_eq!(menu_labels(&mut app), session_menu);
}

#[test]
fn the_selection_stays_on_its_row_as_more_turns_arrive() {
    let (a, b) = two();
    let mut app = flat(
        &[(&a, summary(1, 0, 1_000)), (&b, summary(1, 0, 900))],
        vec![(&a, vec![hit(0, Some(1_000))])],
    );
    let wanted = RowId::Hit {
        session: NodeId::AgentSession(a),
        turn: 0,
    };
    app.set_selection(wanted.clone());
    app.update(Msg::ContentDetails {
        generation: app.content_generation().unwrap(),
        batch: vec![(key_of(&b), vec![hit(0, Some(2_000))])],
    });
    assert_eq!(app.selection(), Some(&wanted));
    assert_eq!(
        listed(&app)[0].0,
        b.session_key,
        "the new message sorted above"
    );
}

#[test]
fn the_columns_line_up_down_the_list() {
    let (a, b) = two();
    let mut app = flat(
        &[(&a, summary(2, 4, 1_000)), (&b, summary(1, 0, 900))],
        vec![
            (&a, vec![hit(4, Some(1_000)), hit(10, Some(950))]),
            (&b, vec![hit(0, Some(900))]),
        ],
    );
    let text = frame(&mut app, 170, 24);
    let rows: Vec<String> = left_pane_lines(&text, 170)
        .into_iter()
        .filter(|line| line.contains("turn "))
        .collect();
    assert_eq!(rows.len(), 3, "{text}");
    let columns: Vec<usize> = rows
        .iter()
        .map(|line| {
            line.chars()
                .position(|c| c == 't')
                .and_then(|_| line.find("turn "))
                .unwrap()
        })
        .collect();
    assert!(
        columns.windows(2).all(|w| w[0] == w[1]),
        "the turn column starts in one place on every row: {rows:#?}"
    );
}

#[test]
fn the_modes_cycle_session_message_flat() {
    assert_eq!(ContentGroup::Session.next(), ContentGroup::Message);
    assert_eq!(ContentGroup::Message.next(), ContentGroup::Flat);
    assert_eq!(ContentGroup::Flat.next(), ContentGroup::Session);
    for mode in ContentGroup::ALL {
        assert_eq!(ContentGroup::parse(mode.as_str()), Some(mode));
    }
}
