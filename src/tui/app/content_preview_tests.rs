use super::*;
use crate::model::{AgentSessionNode, GraphNode};
use crate::tui::widgets::search::{Reach, SearchMode, SearchOverlayState};
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::search::{Hit, SessionHits};

fn session(key: &str, title: &str) -> GraphNode {
    let mut node = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", key),
        "claude-code",
    );
    node.title = Some(title.to_string());
    GraphNode::AgentSession(node)
}

fn app_with(sessions: &[(&str, &str)]) -> App {
    let mut app = App::new(RunConfig::defaults());
    let snapshot = GraphSnapshot {
        nodes: sessions.iter().map(|(k, t)| session(k, t)).collect(),
        ..GraphSnapshot::empty()
    };
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 0,
        initial_selection_hint: None,
    });
    app
}

fn key(session: &str) -> SessionKey {
    SessionKey {
        harness_key: "claude-code".into(),
        session_key: session.into(),
    }
}

fn hit(turn: usize, snippet: &str) -> Hit {
    Hit {
        turn_index: turn,
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        timestamp: None,
        snippet: snippet.into(),
        highlight: 0..6,
    }
}

fn hits(count: usize) -> SessionHits {
    SessionHits {
        count,
        best: hit(0, "needle in a haystack"),
        latest: None,
    }
}

fn preview(app: &mut App, text: &str) -> Vec<Effect> {
    app.update(Msg::PreviewContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        reach: crate::tui::widgets::search::Reach::All,
    })
}

fn finish(app: &mut App, found: &[(&str, usize)]) {
    let generation = app.content_generation().expect("a scan is wanted");
    app.update(Msg::ContentHits {
        generation,
        batch: found.iter().map(|(k, n)| (key(k), hits(*n))).collect(),
        progress: Progress {
            scanned: 2,
            total: 2,
            skipped: 0,
        },
        done: true,
    });
}

#[test]
fn a_preview_scans_but_does_not_narrow_the_lists() {
    let mut app = app_with(&[("a", "alpha"), ("b", "beta")]);
    preview(&mut app, "needle");
    assert!(app.content().is_none(), "not an active search yet");
    assert_eq!(
        app.content_preview().map(|c| c.text.as_str()),
        Some("needle")
    );
    assert!(app.content_filter().is_none(), "lists stay whole");
    assert!(app.take_scan_request().is_some(), "but the scan runs");
    finish(&mut app, &[("a", 1)]);
    assert!(app.content_filter().is_none());
}

#[test]
fn applying_the_preview_keeps_its_results_and_narrows_the_lists() {
    let mut app = app_with(&[("a", "alpha"), ("b", "beta")]);
    preview(&mut app, "needle");
    finish(&mut app, &[("a", 1)]);
    let generation = app.content_generation();
    app.update(Msg::ApplyContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        show_results: false,
        reach: crate::tui::widgets::search::Reach::All,
    });
    assert_eq!(app.content().map(|c| c.text.as_str()), Some("needle"));
    assert_eq!(app.content_generation(), generation, "no second scan");
    assert!(app.content_filter().is_some());
}

#[test]
fn applying_a_different_query_starts_it_as_a_new_search() {
    let mut app = app_with(&[("a", "alpha")]);
    preview(&mut app, "needle");
    let before = app.content_generation();
    app.update(Msg::ApplyContentSearch {
        text: "other".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        show_results: false,
        reach: crate::tui::widgets::search::Reach::All,
    });
    assert_eq!(app.content().map(|c| c.text.as_str()), Some("other"));
    assert_ne!(app.content_generation(), before);
}

#[test]
fn applying_with_show_results_opens_the_search_view() {
    let mut app = app_with(&[("a", "alpha")]);
    preview(&mut app, "needle");
    app.update(Msg::ApplyContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Session,
        show_results: true,
        reach: crate::tui::widgets::search::Reach::All,
    });
    assert_eq!(app.active_view(), View::Search);
    assert!(app.content().is_some());
}

#[test]
fn discarding_a_preview_stops_it_and_leaves_no_search() {
    let mut app = app_with(&[("a", "alpha")]);
    preview(&mut app, "needle");
    app.update(Msg::DiscardContentPreview);
    assert!(app.content_preview().is_none() && app.content().is_none());
    assert_eq!(app.content_generation(), None);
}

#[test]
fn discarding_a_preview_brings_back_the_search_it_replaced() {
    let mut app = app_with(&[("a", "alpha"), ("b", "beta")]);
    app.update(Msg::StartContentSearch {
        text: "old".into(),
        kinds: ContentKinds::default(),
    });
    finish(&mut app, &[("a", 2)]);
    preview(&mut app, "new");
    assert!(app.content().is_none(), "the preview is not active");
    assert_eq!(app.content_preview().map(|c| c.text.as_str()), Some("new"));
    app.update(Msg::DiscardContentPreview);
    let restored = app.content().expect("the old search is back");
    assert_eq!(restored.text, "old");
    assert!(restored.status.is_done(), "its results were kept");
    assert!(app.content_filter().is_some());
}

#[test]
fn a_second_preview_keeps_the_first_search_to_restore() {
    let mut app = app_with(&[("a", "alpha")]);
    app.update(Msg::StartContentSearch {
        text: "old".into(),
        kinds: ContentKinds::default(),
    });
    finish(&mut app, &[("a", 2)]);
    preview(&mut app, "new");
    preview(&mut app, "newer");
    app.update(Msg::DiscardContentPreview);
    assert_eq!(app.content().map(|c| c.text.as_str()), Some("old"));
}

#[test]
fn a_new_preview_makes_the_old_scans_results_stale() {
    let mut app = app_with(&[("a", "alpha")]);
    preview(&mut app, "ne");
    let stale = app.content_generation().expect("running");
    preview(&mut app, "needle");
    app.update(Msg::ContentHits {
        generation: stale,
        batch: vec![(key("a"), hits(9))],
        progress: Progress::default(),
        done: true,
    });
    let content = app.content_preview().expect("still previewing");
    assert!(
        content.hits().is_empty(),
        "the stale scan's hit was dropped"
    );
}

#[test]
fn without_live_preview_a_preview_does_not_start_and_keeps_the_active_search() {
    let mut config = RunConfig::defaults();
    config.live_preview_enabled = false;
    let mut app = App::new(config);
    let effects = preview(&mut app, "needle");
    assert!(app.content_preview().is_none());
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Toast(note) if note.text.contains("live preview")))
    );
}

#[test]
fn opening_a_hit_asks_for_the_viewer_at_that_turn() {
    use crate::tui::effect::ExecSpec;
    let mut app = app_with(&[("a", "alpha")]);
    let effects = app.update(Msg::OpenContentHit {
        session: AgentSessionId::new("claude-code", "/state", "a"),
        text: "needle".into(),
        turn_index: 7,
    });
    let [Effect::Exec(ExecSpec::ViewSession(id, Some(open)))] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(id.session_key, "a");
    assert_eq!((open.query.as_str(), open.turn_index), ("needle", 7));
}

#[test]
fn the_overlay_lists_results_ranked_by_hits_with_the_session_name() {
    let mut app = app_with(&[("a", "alpha"), ("b", "beta")]);
    preview(&mut app, "needle");
    finish(&mut app, &[("a", 1), ("b", 5)]);
    let view = app.content_view();
    let labels: Vec<_> = view.rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(labels, ["beta", "alpha"], "more hits first");
    assert_eq!(view.rows[0].chip, "claude");
    assert!(view.done && view.total == 2);
}

#[test]
fn the_per_message_grouping_lists_a_row_per_matching_turn() {
    let mut app = app_with(&[("a", "alpha")]);
    app.update(Msg::PreviewContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
        group: Grouping::Message,
        reach: crate::tui::widgets::search::Reach::All,
    });
    finish(&mut app, &[("a", 2)]);
    let generation = app.content_generation().expect("scan");
    app.update(Msg::ContentDetails {
        generation,
        batch: vec![(
            key("a"),
            vec![hit(2, "first needle"), hit(5, "second needle")],
        )],
    });
    let rows = app.content_view().rows;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.turn.is_some()));
}

#[test]
fn content_search_opens_clean_even_when_a_search_is_active() {
    let mut app = app_with(&[("a", "alpha")]);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    app.open_content_search_overlay();
    let state = app.search_overlay().expect("open");
    assert_eq!(state.mode(), SearchMode::Content);
    assert_eq!(state.query(), "");
}

#[test]
fn quick_select_opens_empty_on_the_current_reach_even_when_a_search_is_active() {
    let mut app = app_with(&[("a", "alpha")]);
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });
    app.open_search_overlay();
    let state = app.search_overlay().expect("open");
    assert_eq!(state.mode(), SearchMode::Quick);
    assert_eq!(state.reach(), Reach::Current);
    assert_eq!(state.query(), "");
}

#[test]
fn quick_select_reopens_on_current_whatever_it_was_left_on() {
    let mut app = app_with(&[("a", "alpha")]);
    app.open_search_overlay();
    app.search_overlay_mut()
        .expect("open")
        .handle_key(crossterm_key(ratatui::crossterm::event::KeyCode::Tab));
    assert_eq!(
        app.search_overlay().map(SearchOverlayState::reach),
        Some(Reach::All)
    );
    app.close_search_overlay();
    app.open_search_overlay();
    let state = app.search_overlay().expect("open");
    assert_eq!(state.reach(), Reach::Current);
    assert_eq!(state.query(), "", "the query is not remembered");
}

#[test]
fn content_search_is_refused_while_live_preview_is_off() {
    let mut app = app_with(&[("a", "alpha")]);
    app.config_mut().live_preview_enabled = false;
    let hint = app.open_content_search_entry();
    assert!(hint.text.contains("live preview"), "{hint:?}");
    assert_eq!(hint.severity, tuikit::feedback::Severity::Warning);
    assert!(app.search_overlay().is_none());
}

fn crossterm_key(code: ratatui::crossterm::event::KeyCode) -> ratatui::crossterm::event::KeyEvent {
    ratatui::crossterm::event::KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
}

#[test]
fn the_tick_starts_the_preview_once_the_query_has_rested() {
    use std::time::Duration;
    let mut app = app_with(&[("a", "alpha")]);
    app.open_content_search_overlay();
    for c in "needle".chars() {
        app.search_overlay_mut()
            .expect("open")
            .handle_key(crossterm_key(ratatui::crossterm::event::KeyCode::Char(c)));
    }
    let now = Instant::now();
    app.search_overlay_tick(now, false);
    assert!(app.content_preview().is_none(), "still resting");
    app.search_overlay_tick(now + Duration::from_millis(400), false);
    assert_eq!(
        app.content_preview().map(|c| c.text.as_str()),
        Some("needle")
    );
}
