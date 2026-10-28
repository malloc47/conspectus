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

fn start(app: &mut App, text: &str) {
    start_with(app, text, ContentKinds::default());
}

fn start_with(app: &mut App, text: &str, kinds: ContentKinds) {
    app.update(Msg::StartContentSearch {
        text: text.into(),
        kinds,
    });
}

#[test]
fn a_repeat_of_an_older_search_moves_it_to_the_newest_place_and_shows_it() {
    let mut app = app_with(&["a"]);
    for text in ["one", "two", "three"] {
        start(&mut app, text);
    }
    start(&mut app, "  one ");
    assert_eq!(texts(&app), ["two", "three", "one"]);
    assert_eq!(app.search_history().position(), Some((3, 3)));
    assert_eq!(app.viewed_search().unwrap().text, "one");
}

#[test]
fn a_repeat_rescans_and_refreshes_the_entry() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "one");
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    start(&mut app, "two");
    start(&mut app, "one");
    let entry = app.search_history().viewed().unwrap();
    assert!(!entry.search.status_is_done(), "it is scanning again");
    app.take_scan_request();
    finish_scan(&mut app, &["a", "b"]);
    assert_eq!(texts(&app), ["two", "one"]);
    assert!(
        app.search_history()
            .viewed()
            .unwrap()
            .search
            .status_is_done()
    );
}

#[test]
fn searches_that_differ_in_kinds_or_scope_are_distinct_entries() {
    let mut app = app_with(&["a"]);
    let tools = ContentKinds {
        tools: true,
        ..ContentKinds::default()
    };
    start(&mut app, "same");
    start_with(&mut app, "same", tools);
    assert_eq!(app.search_history().len(), 2, "kinds differ");
    preview(&mut app, "same", Reach::Current);
    app.commit_modal_search();
    assert_eq!(app.search_history().len(), 3, "scope differs");
    start(&mut app, "same");
    assert_eq!(app.search_history().len(), 3, "the All search is the first");
    let reaches: Vec<_> = app
        .search_history()
        .entries()
        .iter()
        .map(|entry| (entry.search.kinds.tools, entry.search.reach))
        .collect();
    assert_eq!(
        reaches,
        [
            (true, Reach::All),
            (false, Reach::Current),
            (false, Reach::All)
        ]
    );
}

#[test]
fn a_repeat_does_not_push_the_oldest_search_out() {
    let mut config = RunConfig::defaults();
    config.search.history = 3;
    let mut app = App::new(config);
    for text in ["a", "b", "c", "a"] {
        start(&mut app, text);
    }
    assert_eq!(texts(&app), ["b", "c", "a"], "nothing was evicted");
    start(&mut app, "d");
    assert_eq!(texts(&app), ["c", "a", "d"]);
}

#[test]
fn typing_a_prefix_of_an_earlier_search_in_the_modal_keeps_that_search() {
    let mut app = app_with(&["a"]);
    start(&mut app, "ne");
    preview(&mut app, "ne", Reach::All);
    preview(&mut app, "need", Reach::All);
    app.commit_modal_search();
    assert_eq!(texts(&app), ["ne", "need"]);
}

#[test]
fn closing_the_modal_on_a_search_equal_to_an_earlier_one_keeps_one_entry() {
    let mut app = app_with(&["a"]);
    start(&mut app, "ne");
    start(&mut app, "other");
    app.open_content_search_overlay();
    app.search_overlay_mut().unwrap().with_panes_off();
    preview(&mut app, "ne", Reach::All);
    app.commit_modal_search();
    assert_eq!(texts(&app), ["other", "ne"]);
    assert_eq!(app.viewed_search().unwrap().text, "ne");
}

#[test]
fn a_content_search_opens_on_all_unless_it_opens_onto_a_search() {
    use crate::tui::Modal;
    let reach = |app: &App| match app.modal_stack().top() {
        Some(Modal::Search(state)) => state.reach(),
        _ => panic!("no search modal"),
    };
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    assert_eq!(reach(&app), Reach::All);
    let mut quick = app_with(&["a"]);
    quick.open_search_overlay();
    assert_eq!(reach(&quick), Reach::Current, "quick select is unchanged");
}

#[test]
fn typing_in_the_modal_remembers_one_search_when_it_closes() {
    let mut app = app_with(&["a"]);
    preview(&mut app, "ne", Reach::All);
    preview(&mut app, "need", Reach::All);
    preview(&mut app, "needle", Reach::All);
    assert!(texts(&app).is_empty(), "nothing is remembered while typing");
    app.commit_modal_search();
    assert_eq!(texts(&app), ["needle"]);
    preview(&mut app, "other", Reach::All);
    app.commit_modal_search();
    assert_eq!(texts(&app), ["needle", "other"]);
}

#[test]
fn the_search_view_stays_put_while_the_modal_types_and_takes_the_search_on_exit() {
    let mut app = app_with(&["a", "b"]);
    start(&mut app, "first");
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.update(Msg::SwitchView(View::Search));
    app.open_content_search_overlay();
    preview(&mut app, "sec", Reach::All);
    preview(&mut app, "second", Reach::All);
    app.take_scan_request();
    finish_scan(&mut app, &["a", "b"]);
    assert_eq!(texts(&app), ["first"], "nothing is remembered yet");
    assert_eq!(app.viewed_search().unwrap().text, "first");
    app.close_search_overlay();
    assert_eq!(texts(&app), ["first", "second"]);
    assert_eq!(app.viewed_search().unwrap().text, "second");
    assert!(!app.search_history().viewed().unwrap().stopped);
}

#[test]
fn leaving_the_modal_mid_scan_remembers_the_search_as_stopped() {
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    preview(&mut app, "needle", Reach::All);
    app.close_search_overlay();
    app.update(Msg::DiscardContentPreview);
    assert_eq!(texts(&app), ["needle"]);
    assert!(app.search_history().viewed().unwrap().stopped);
}

#[test]
fn a_modal_that_never_searched_remembers_nothing() {
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    app.close_search_overlay();
    app.update(Msg::DiscardContentPreview);
    assert!(app.search_history().is_empty());
}

#[test]
fn applying_or_keeping_from_the_modal_remembers_the_search() {
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    app.update(Msg::KeepContentSearch {
        text: "kept".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: Reach::All,
    });
    assert_eq!(texts(&app), ["kept"]);
    app.open_content_search_overlay();
    app.update(Msg::ApplyContentSearch {
        text: "applied".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: Reach::All,
        show_results: false,
    });
    assert_eq!(texts(&app), ["kept", "applied"]);
}

#[test]
fn a_repeat_of_an_earlier_search_on_exit_moves_it_to_the_newest_end() {
    let mut app = app_with(&["a"]);
    start(&mut app, "one");
    start(&mut app, "two");
    app.open_content_search_overlay();
    app.search_overlay_mut().unwrap().with_panes_off();
    preview(&mut app, "one", Reach::All);
    assert_eq!(texts(&app), ["one", "two"]);
    app.close_search_overlay();
    assert_eq!(texts(&app), ["two", "one"]);
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
    app.commit_modal_search();
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
    app.commit_modal_search();
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
    app.commit_modal_search();
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
    app.commit_modal_search();
    assert!(!app.viewed_search().unwrap().with_panes);
}

#[test]
fn a_current_search_made_in_the_search_view_keeps_the_scope_of_the_search_shown() {
    let mut app = app_with(&["a", "b"]);
    preview(&mut app, "one", Reach::Current);
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.commit_modal_search();
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

#[test]
fn searches_with_and_without_pane_scrollback_are_distinct_entries() {
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    preview(&mut app, "needle", Reach::All);
    app.close_search_overlay();
    app.open_content_search_overlay();
    app.search_overlay_mut().unwrap().with_panes_off();
    preview(&mut app, "needle", Reach::All);
    app.close_search_overlay();
    let panes: Vec<_> = app
        .search_history()
        .entries()
        .iter()
        .map(|entry| entry.search.with_panes)
        .collect();
    assert_eq!(panes, [true, false]);
}

#[test]
fn the_granularity_changed_in_the_modal_does_not_change_the_search_view_or_start_a_scan() {
    let mut app = app_with(&["a"]);
    start(&mut app, "first");
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.update(Msg::SwitchView(View::Search));
    app.open_content_search_overlay();
    preview(&mut app, "second", Reach::All);
    app.take_scan_request();
    app.update(Msg::SetContentGroup(Grouping::Flat));
    assert_eq!(app.viewed_search().unwrap().group, Grouping::Session);
    assert!(app.take_scan_request().is_none(), "no scan for a regroup");
    app.close_search_overlay();
    assert_eq!(app.viewed_search().unwrap().group, Grouping::Flat);
}

#[test]
fn the_search_view_toggles_granularity_on_the_shown_search_without_scanning() {
    let mut app = app_with(&["a"]);
    start(&mut app, "first");
    app.take_scan_request();
    finish_scan(&mut app, &["a"]);
    app.update(Msg::SwitchView(View::Search));
    app.update(Msg::ToggleSearchGranularity);
    assert_eq!(app.viewed_search().unwrap().group, Grouping::Flat);
    app.update(Msg::ToggleSearchGranularity);
    assert_eq!(app.viewed_search().unwrap().group, Grouping::Session);
    assert!(app.take_scan_request().is_none());
    assert_eq!(app.search_history().len(), 1, "not a new search");
}

#[test]
fn the_options_open_the_modal_with_the_pane_showing() {
    let mut app = app_with(&["a"]);
    app.update(Msg::OpenSearchOptions);
    let state = app.search_overlay().expect("the modal is open");
    assert_eq!(state.options_cursor(), Some(0));
}

#[test]
fn a_blank_search_is_never_remembered() {
    let mut app = app_with(&["a"]);
    start(&mut app, "one");
    app.open_content_search_overlay();
    preview(&mut app, "   ", Reach::All);
    app.close_search_overlay();
    assert_eq!(texts(&app), ["one"]);

    let mut blank = app.search_history().newest().unwrap().search.clone();
    blank.text = "  ".into();
    blank.serial = 99;
    app.search_history.record(blank, 10);
    assert_eq!(texts(&app), ["one"]);
}

#[test]
fn content_search_always_opens_with_an_empty_query_on_all() {
    let mut app = app_with(&["a"]);
    start(&mut app, "one");
    app.open_content_search_overlay();
    let overlay = app.search_overlay().unwrap();
    assert_eq!(overlay.query(), "");
    assert_eq!(overlay.reach(), Reach::All);
}

#[test]
fn earlier_search_strings_page_back_and_forward_from_an_empty_field() {
    use crate::tui::widgets::search::SearchOutcome;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    let mut app = app_with(&["a"]);
    start(&mut app, "one");
    start(&mut app, "two");
    app.open_content_search_overlay();
    let overlay = app.search_overlay_mut().unwrap();
    assert_eq!(overlay.handle_key(ctrl('t')), SearchOutcome::Continue);
    assert_eq!(overlay.query(), "", "nothing is newer than the empty field");
    overlay.handle_key(ctrl('r'));
    assert_eq!(overlay.query(), "two");
    overlay.handle_key(ctrl('r'));
    assert_eq!(overlay.query(), "one");
    overlay.handle_key(ctrl('r'));
    assert_eq!(overlay.query(), "one", "no wrap past the oldest");
    overlay.handle_key(ctrl('t'));
    assert_eq!(overlay.query(), "two");
    overlay.handle_key(ctrl('t'));
    assert_eq!(overlay.query(), "");
}

#[test]
fn shift_tab_switches_granularity_and_tab_switches_reach() {
    use crate::tui::widgets::search::SearchOutcome;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = app_with(&["a"]);
    app.open_content_search_overlay();
    let overlay = app.search_overlay_mut().unwrap();
    let back = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert!(matches!(
        overlay.handle_key(back),
        SearchOutcome::Regroup(Grouping::Flat)
    ));
    assert!(matches!(
        overlay.handle_key(back),
        SearchOutcome::Regroup(Grouping::Session)
    ));
    overlay.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(overlay.reach(), Reach::Current);
}
