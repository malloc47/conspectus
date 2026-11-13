//! The keys the main view's own notes name come from the keymap binding
//! that takes each one: the left pane's empty states, the Search view's
//! paging hint, and the Preview's notes. Each test builds its text from
//! `keybindings::key_name`, so a rebound key changes the note with it;
//! `keybindings_tests.rs` pins the labels, so the words are the ones the
//! notes always wrote.

use super::left_panel::{empty_left_panel, search_header};
use super::preview_pane::preview_text_for_selection;
use super::tests::{history_app_over, muxed_app, pinned_app};
use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::model::GraphSnapshot;
use crate::resolve::resolve_snapshot;
use crate::tui::app::Msg;
use crate::tui::keybindings::{key_name, key_names};
use crate::tui::preview::preview_source;
use crate::tui::{RunConfig, View};
use crate::viewer::search::{ContentKinds, Grouping};

/// An App on `view` over `snapshot`, its tree built for that view.
fn app_over(view: View, snapshot: GraphSnapshot) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = view;
    let mut app = App::new(config);
    let snapshot = resolve_snapshot(snapshot);
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

/// A snapshot of one agent session, `claude-code` key `a`.
fn one_session() -> GraphSnapshot {
    GraphSnapshot {
        nodes: vec![crate::model::GraphNode::AgentSession(
            crate::model::AgentSessionNode::new(
                crate::model::AgentSessionId::new("claude-code", "/state", "a"),
                "claude-code",
            ),
        )],
        ..GraphSnapshot::empty()
    }
}

/// Starts a content search for `text`.
fn start(app: &mut App, text: &str) {
    app.update(Msg::StartContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
    });
}

/// Delivers the running search's last batch, with one hit in session `a`
/// when `hit`.
fn finish(app: &mut App, hit: bool) {
    let generation = app.content_generation().expect("a search is running");
    let batch = if hit {
        let found = crate::viewer::search::Hit {
            turn_index: 0,
            role: crate::viewer::model::TurnRole::Assistant,
            kind: crate::viewer::model::TurnKind::Message,
            timestamp: None,
            snippet: "found the needle here".into(),
            highlight: 10..16,
        };
        vec![(
            SessionKey {
                harness_key: "claude-code".into(),
                session_key: "a".into(),
            },
            crate::viewer::search::SessionHits {
                count: 1,
                best: found,
                latest: None,
            },
        )]
    } else {
        Vec::new()
    };
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
}

/// The empty state's message and hint.
fn empty(app: &App) -> (String, Option<String>) {
    let state = empty_left_panel(app);
    (
        state.message().to_owned(),
        state.hint_text().map(str::to_owned),
    )
}

#[test]
fn an_empty_world_names_the_refresh_and_quit_keys() {
    let app = app_over(View::Sessions, GraphSnapshot::empty());
    assert_eq!(
        empty(&app),
        (
            "No sessions discovered.".to_owned(),
            Some(format!(
                "Press `{}` to refresh or `{}` to quit.",
                key_name("refresh"),
                key_name("quit")
            ))
        )
    );
}

#[test]
fn an_empty_prs_view_names_the_hard_refresh_and_quit_keys() {
    let app = app_over(View::Prs, GraphSnapshot::empty());
    assert_eq!(
        empty(&app).1,
        Some(format!(
            "Press `{}` to ask the forges again or `{}` to quit.",
            key_name("refresh.hard"),
            key_name("quit")
        ))
    );
}

#[test]
fn a_filter_that_hides_every_row_names_the_clear_and_controls_keys() {
    let mut app = app_over(View::Sessions, one_session());
    app.update(Msg::SetFilter(crate::filter::RowFilter {
        max_age: Some(std::time::Duration::from_secs(60)),
        ..crate::filter::RowFilter::default()
    }));
    assert_eq!(
        empty(&app).1,
        Some(format!(
            "Press `{}` to clear filters, `{}` to edit.",
            key_name("filters.clear"),
            key_name("controls")
        ))
    );
}

#[test]
fn an_empty_history_names_the_menu_key_and_a_pinned_one_the_controls_key() {
    let app = history_app_over(
        &GraphSnapshot::empty(),
        crate::tui::HistoryGrouping::None,
        RunConfig::defaults(),
    );
    assert_eq!(
        empty(&app).1,
        Some(format!(
            "Press `{}` for the menu (Clean up…).",
            key_name("row.menu")
        ))
    );

    // This world's gone records include pinned ones, which the filter
    // leaves to the Pinned view.
    let app = history_app_over(
        &crate::tui::rows::pinned::fixture_world(),
        crate::tui::HistoryGrouping::None,
        RunConfig::defaults(),
    );
    let (message, hint) = empty(&app);
    assert!(message.starts_with("Every stopped or removed record is pinned"));
    assert_eq!(
        hint,
        Some(format!(
            "Clear `Unpinned only` in Controls (`{}`) to list them here.",
            key_name("controls")
        ))
    );
}

#[test]
fn the_search_view_names_the_search_keys() {
    let mut app = app_over(View::Search, one_session());
    let search = key_names("search.contents");
    assert_eq!(
        empty(&app),
        (
            format!(
                "Press `{}` (or {}) to search the contents of sessions and mux panes.",
                search[0], search[1]
            ),
            None
        )
    );

    start(&mut app, "needle");
    finish(&mut app, false);
    assert_eq!(
        empty(&app),
        (
            "Nothing matches \"needle\".".to_owned(),
            Some(format!(
                "Press `{}` to search again, `{}` for an earlier search.",
                key_name("search.contents"),
                key_name("search.older")
            ))
        )
    );
}

#[test]
fn a_search_shown_on_the_lists_names_the_search_and_clear_keys() {
    let mut app = app_over(View::Sessions, one_session());
    let clear = key_name("filters.clear");
    start(&mut app, "needle");
    assert_eq!(
        empty(&app).1,
        Some(format!(
            "Sessions that match appear as they are found. Press `{clear}` to clear."
        ))
    );

    finish(&mut app, false);
    assert_eq!(
        empty(&app).1,
        Some(format!(
            "Press `{}` to search again, `{clear}` to clear.",
            key_name("search.contents")
        ))
    );

    // Messages are listed once their text is read.
    start(&mut app, "needle");
    finish(&mut app, true);
    app.update(Msg::SetContentGroup(Grouping::Flat));
    assert_eq!(
        empty(&app),
        (
            "Reading the messages that match \"needle\"…".to_owned(),
            Some(format!("Press `{clear}` to clear."))
        )
    );
}

#[test]
fn the_search_header_names_the_paging_keys() {
    let mut app = app_over(View::Sessions, one_session());
    for text in ["one", "two"] {
        start(&mut app, text);
        finish(&mut app, true);
    }
    app.update(Msg::SwitchView(View::Search));
    let header = search_header(&app, 120).expect("the Search view has a header");
    let text: String = header
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    let paging = format!(
        "{} older  {} newer",
        key_name("search.older"),
        key_name("search.newer")
    );
    assert!(text.ends_with(&paging), "{text:?}");
}

#[test]
fn a_mux_with_no_target_names_the_refresh_key() {
    let mut app = muxed_app("work", None);
    let source = preview_source(&app).expect("the row previews its pane");
    app.update(Msg::SetPreview {
        source,
        content: crate::tui::preview::PreviewContent::NoTarget,
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        format!(
            "tmux target not found — try `{}` to refresh",
            key_name("refresh")
        )
    );
}

#[test]
fn a_pin_the_snapshot_no_longer_holds_names_the_refresh_key() {
    let mut app = pinned_app(None);
    // The tree still lists the pin, but the snapshot it came from is gone.
    let tree = app.tree().clone();
    let empty = resolve_snapshot(GraphSnapshot::empty());
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&empty),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        format!("pin unavailable — try `{}` to refresh", key_name("refresh"))
    );
}
