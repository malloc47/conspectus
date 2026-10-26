use super::tests::{app_with, deliver, hits, key, load, progress};
use super::*;
use crate::content_search::{MuxRef, PaneHit, PaneRef};
use crate::tui::widgets::search::Reach;

fn preview(app: &mut App, text: &str, reach: Reach) {
    app.update(Msg::PreviewContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach,
    });
}

fn finish_scan(app: &mut App, found: &[&str]) {
    deliver(app, found.iter().map(|k| (key(k), hits(1))).collect(), true);
}

fn texts(app: &App) -> Vec<String> {
    app.search_history()
        .entries()
        .iter()
        .map(|entry| entry.search.text.clone())
        .collect()
}

fn pane(session: &str, id: &str, count: usize) -> crate::content_search::PaneHits {
    crate::content_search::PaneHits {
        pane: PaneRef {
            mux: MuxRef {
                backend: "tmux".into(),
                socket: None,
                session: session.into(),
            },
            pane_id: id.into(),
            window_index: "0".into(),
            pane_index: "1".into(),
            command: Some("zsh".into()),
        },
        count,
        hits: vec![PaneHit {
            line: 7,
            snippet: "a needle line".into(),
            highlight: 2..8,
        }],
        deduplicated: 0,
        captured_at: 100,
        capture_capped: false,
    }
}

#[test]
fn every_started_search_is_kept_and_the_newest_is_shown() {
    let mut app = app_with(&["a"]);
    app.update(Msg::StartContentSearch {
        text: "one".into(),
        kinds: ContentKinds::default(),
    });
    app.update(Msg::StartContentSearch {
        text: "two".into(),
        kinds: ContentKinds::default(),
    });
    assert_eq!(texts(&app), ["one", "two"]);
    assert_eq!(app.search_history().position(), Some((2, 2)));
    assert_eq!(app.viewed_search().unwrap().text, "two");
}

#[test]
fn an_identical_search_updates_the_newest_entry_instead_of_adding() {
    let mut app = app_with(&["a"]);
    for _ in 0..2 {
        app.update(Msg::StartContentSearch {
            text: "same".into(),
            kinds: ContentKinds::default(),
        });
    }
    assert_eq!(texts(&app), ["same"]);
}

#[test]
fn typing_in_the_modal_replaces_its_draft_and_closing_keeps_it() {
    let mut app = app_with(&["a"]);
    preview(&mut app, "ne", Reach::All);
    preview(&mut app, "need", Reach::All);
    preview(&mut app, "needle", Reach::All);
    assert_eq!(texts(&app), ["needle"], "one entry for the modal session");
    app.finalize_search_drafts();
    preview(&mut app, "other", Reach::All);
    assert_eq!(texts(&app), ["needle", "other"], "a kept search stays");
}

#[test]
fn emptying_the_query_inside_the_modal_abandons_the_draft() {
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    preview(&mut app, "x", Reach::All);
    app.update(Msg::DiscardContentPreview);
    assert!(app.search_history().is_empty());
}

#[test]
fn closing_the_modal_mid_scan_keeps_a_stopped_entry() {
    let mut app = app_with(&["a"]);
    preview(&mut app, "needle", Reach::All);
    app.update(Msg::DiscardContentPreview);
    let entry = app.search_history().viewed().unwrap();
    assert!(entry.stopped);
    assert!(entry.search.status_is_done(), "it will find nothing more");
}

#[test]
fn the_history_is_limited_by_the_config_and_zero_keeps_one() {
    let mut config = RunConfig::defaults();
    config.search.history = 2;
    let mut app = App::new(config.clone());
    for text in ["a", "b", "c"] {
        app.update(Msg::StartContentSearch {
            text: text.into(),
            kinds: ContentKinds::default(),
        });
    }
    assert_eq!(texts(&app), ["b", "c"]);
    config.search.history = 0;
    let mut app = App::new(config);
    for text in ["a", "b"] {
        app.update(Msg::StartContentSearch {
            text: text.into(),
            kinds: ContentKinds::default(),
        });
    }
    assert_eq!(texts(&app), ["b"]);
}

#[test]
fn paging_shows_older_searches_without_scanning_again() {
    let mut app = app_with(&["a", "b"]);
    app.update(Msg::StartContentSearch {
        text: "one".into(),
        kinds: ContentKinds::default(),
    });
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.update(Msg::StartContentSearch {
        text: "two".into(),
        kinds: ContentKinds::default(),
    });
    app.take_scan_request();
    finish_scan(&mut app, &["b"]);

    app.update(Msg::SwitchView(View::Search));
    assert_eq!(listed_titles(&app).len(), 1);
    app.update(Msg::PageSearchHistory(-1));
    assert_eq!(app.viewed_search().unwrap().text, "one");
    assert_eq!(app.search_history().position(), Some((1, 2)));
    assert!(app.take_scan_request().is_none(), "paging never scans");
    assert!(
        app.viewed_search()
            .unwrap()
            .hits()
            .get("claude-code", "a")
            .is_some()
    );
    app.update(Msg::PageSearchHistory(-1));
    assert_eq!(app.search_history().position(), Some((1, 2)), "no wrap");
    app.update(Msg::PageSearchHistory(1));
    assert_eq!(app.viewed_search().unwrap().text, "two");
}

fn listed_titles(app: &App) -> Vec<String> {
    app.tree()
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| format!("{:?}", row.id))
        .collect()
}

#[test]
fn paging_outside_the_search_view_does_nothing() {
    let mut app = app_with(&["a"]);
    for text in ["one", "two"] {
        app.update(Msg::StartContentSearch {
            text: text.into(),
            kinds: ContentKinds::default(),
        });
    }
    app.update(Msg::PageSearchHistory(-1));
    assert_eq!(app.search_history().position(), Some((2, 2)));
}

#[test]
fn the_filter_does_not_follow_paging_and_clearing_it_keeps_the_history() {
    let mut app = app_with(&["a", "b"]);
    for text in ["one", "two"] {
        app.update(Msg::StartContentSearch {
            text: text.into(),
            kinds: ContentKinds::default(),
        });
        app.take_scan_request();
        finish_scan(&mut app, &["a"]);
    }
    app.update(Msg::SwitchView(View::Search));
    app.update(Msg::PageSearchHistory(-1));
    assert_eq!(app.content().unwrap().text, "two", "still the filter");
    assert!(!app.viewed_is_filter());
    app.update(Msg::ApplySearchEntry);
    assert_eq!(app.content().unwrap().text, "one");
    assert!(app.viewed_is_filter());
    app.update(Msg::PageSearchHistory(1));
    assert_eq!(app.content().unwrap().text, "one", "paging left it alone");
    app.update(Msg::ClearFilters);
    assert!(app.content().is_none());
    assert_eq!(app.search_history().len(), 2);
}

#[test]
fn applying_outside_the_search_view_only_explains() {
    let mut app = app_with(&["a"]);
    app.update(Msg::StartContentSearch {
        text: "one".into(),
        kinds: ContentKinds::default(),
    });
    app.update(Msg::ClearContentSearch);
    let effects = app.update(Msg::ApplySearchEntry);
    assert!(matches!(effects.as_slice(), [Effect::Toast(_)]));
    assert!(app.content().is_none());
}

#[test]
fn keeping_a_running_search_lets_it_finish_in_the_background() {
    let mut app = app_with(&["a"]);
    app.update(Msg::StartContentSearch {
        text: "old".into(),
        kinds: ContentKinds::default(),
    });
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    preview(&mut app, "new", Reach::All);
    app.take_scan_request();
    app.finalize_search_drafts();
    app.update(Msg::KeepContentSearch {
        text: "new".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: Reach::All,
    });
    assert_eq!(app.active_view(), View::Search);
    assert!(app.content().is_none(), "the lists are not filtered by it");
    let generation = app.content_generation().expect("still scanning");
    app.update(Msg::ContentHits {
        generation,
        batch: vec![(key("a"), hits(3))],
        progress: progress(1, 1),
        done: false,
    });
    assert_eq!(listed_titles(&app).len(), 1, "results stream into the view");
    app.update(Msg::ContentHits {
        generation,
        batch: vec![],
        progress: progress(1, 1),
        done: true,
    });
    assert_eq!(app.content().unwrap().text, "old", "the filter is back");
    assert_eq!(app.viewed_search().unwrap().text, "new");
    assert!(app.viewed_search().unwrap().status_is_done());
}

#[test]
fn panes_arrive_with_a_search_and_become_rows_of_the_search_view() {
    let mut app = app_with(&["a"]);
    preview(&mut app, "needle", Reach::All);
    let request = app.take_scan_request().unwrap();
    assert!(request.panes.is_some(), "a modal search reads panes");
    let generation = app.content_generation().unwrap();
    app.update(Msg::ContentPanes {
        generation,
        batch: vec![pane("work", "%3", 2)],
    });
    app.finalize_search_drafts();
    finish_scan(&mut app, &[]);
    app.update(Msg::KeepContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: Reach::All,
    });
    let rows = &app.tree().rows;
    let [row] = rows.as_slice() else {
        panic!("{rows:?}");
    };
    let RowKind::Pane(pane_row) = &row.kind else {
        panic!("{row:?}");
    };
    assert_eq!((pane_row.count, pane_row.location.as_str()), (2, "0.1"));
    assert_eq!(
        app.content_view().rows.len(),
        0,
        "the modal's own view is for the running search"
    );
}

#[test]
fn a_search_from_the_filter_flows_does_not_read_panes() {
    let mut app = app_with(&["a"]);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    assert!(app.take_scan_request().unwrap().panes.is_none());
}

#[test]
fn current_reach_reads_only_what_the_view_lists_and_skips_panes_without_muxes() {
    let mut app = app_with(&["a", "b"]);
    // The Sessions view lists both; filter one away by max-age is not
    // needed: Current is every listed session, and no mux is listed.
    preview(&mut app, "needle", Reach::Current);
    let request = app.take_scan_request().unwrap();
    assert_eq!(request.sessions.len(), 2);
    assert!(request.panes.is_none(), "no muxes listed, no panes read");
    assert!(!app.viewed_search().unwrap().with_panes);
}

#[test]
fn a_current_search_made_in_the_search_view_keeps_the_scope_of_the_search_shown() {
    let mut app = app_with(&["a", "b"]);
    preview(&mut app, "one", Reach::Current);
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.finalize_search_drafts();
    app.update(Msg::SwitchView(View::Search));
    preview(&mut app, "two", Reach::Current);
    let request = app.take_scan_request().unwrap();
    assert_eq!(
        request.sessions.len(),
        2,
        "not just the first one's results"
    );
}

#[test]
fn snippets_are_bounded_per_entry() {
    let mut map = crate::filter::ContentHitMap::default();
    for i in 0..5 {
        map.insert("h".into(), format!("s{i}"), hits(i + 1));
    }
    map.keep_snippets(2);
    let kept = map
        .iter()
        .filter(|(_, _, h)| !h.best.snippet.is_empty())
        .count();
    assert_eq!(kept, 2);
    assert_eq!(map.total_hits(), 15, "counts stay");
    assert!(
        !map.get("h", "s4").unwrap().best.snippet.is_empty(),
        "the best-ranked keep their text"
    );
}

#[test]
fn history_details_fill_the_entry_that_is_shown() {
    let mut app = app_with(&["a"]);
    for text in ["one", "two"] {
        app.update(Msg::StartContentSearch {
            text: text.into(),
            kinds: ContentKinds::default(),
        });
        app.take_scan_request();
        finish_scan(&mut app, &["a"]);
    }
    app.update(Msg::SwitchView(View::Search));
    app.update(Msg::PageSearchHistory(-1));
    let serial = app.viewed_search().unwrap().serial;
    app.update(Msg::HistoryDetails {
        serial,
        batch: vec![(key("a"), vec![])],
    });
    assert!(
        app.viewed_search()
            .unwrap()
            .details
            .get("claude-code", "a")
            .is_some()
    );
}

#[test]
fn opening_a_pane_hit_reveals_the_mux_and_asks_for_the_scrollback() {
    let mut app = app_with(&["a"]);
    let target = pane("work", "%3", 1).pane;
    let effects = app.update(Msg::OpenPaneHit {
        pane: target,
        line: 7,
        text: "needle".into(),
        reveal: true,
    });
    assert!(matches!(
        effects.as_slice(),
        [Effect::Exec(crate::tui::effect::ExecSpec::PaneScrollback {
            line: 7,
            ..
        })]
    ));
}

#[test]
fn the_three_ctrl_slash_forms_open_the_contents_search_and_plain_slash_does_not() {
    use crate::tui::keybindings::translate_via_table;
    use crate::tui::keymap::Action;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    for code in ['/', '7', '_'] {
        assert_eq!(
            translate_via_table(KeyModifiers::CONTROL, KeyCode::Char(code)),
            Some(Action::OpenContentSearch),
            "Ctrl-{code}"
        );
    }
    assert_eq!(
        translate_via_table(KeyModifiers::NONE, KeyCode::Char('/')),
        Some(Action::OpenSearch)
    );
    assert_eq!(
        translate_via_table(KeyModifiers::NONE, KeyCode::Char('s')),
        Some(Action::OpenContentSearch)
    );
}

#[test]
fn load_keeps_working_with_a_remembered_search() {
    let mut app = app_with(&["a"]);
    app.update(Msg::StartContentSearch {
        text: "one".into(),
        kinds: ContentKinds::default(),
    });
    load(&mut app, &["a", "b"]);
    assert_eq!(app.search_history().len(), 1);
}
