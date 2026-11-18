//! Two keys walk the agents with an unread stop (ADR 0247), the oldest
//! stop first, on and back, from every view and either pane: to the row
//! standing for the agent in the view on screen, with the groups above it
//! opened, wrapping, and the status line says where it went or why it
//! stayed. Unlike the jump to what waits, the walk stays in the view while
//! another row of it has an unread stop, so a mux with no unread agent is
//! never a reason to leave the Mux view.

use std::collections::BTreeMap;

use ratatui::crossterm::event::Event;
use tuikit::feedback::Severity;
use tuikit::testkit::keys::typed;

use super::live::{BESIDE, HOSTED, SOLO, listing_with, live_world, mux, running, session};
use super::*;
use crate::model::LivePane;
use crate::model::live::{LiveDetail, LiveState};
use crate::tui::live_overlay::LiveOverlay;

/// `key`, as the keymap routes it with the app's focus.
fn press(app: &mut App, key: char) -> Vec<crate::tui::Effect> {
    let action = crate::tui::keymap::translate_in(app.focus(), Event::Key(typed(key)))
        .unwrap_or_else(|| panic!("{key} is bound"));
    let crate::tui::keymap::Action::Msg(msg) = action else {
        panic!("{key} is a message: {action:?}");
    };
    app.update(*msg)
}

/// A stop of `state` that began `minutes` ago.
fn stopped(state: LiveState, minutes: i64) -> crate::model::live::LiveBinding {
    let mut binding = running(state, Some(LiveDetail::Finished), 0);
    binding.observation = binding
        .observation
        .with_since_ms(NOW * 1000 - minutes * 60_000);
    binding
}

/// The live world with four stops, none read: the loose session (no mux)
/// waiting for ten minutes, the hosted one for five, the second agent of
/// `conspectus-main` failed two minutes ago, and an agent in a pane of
/// `scratch` no session matches waiting for half a minute.
fn four_stops() -> GraphSnapshot {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([
        (session(SOLO), stopped(LiveState::Waiting, 10)),
        (session(HOSTED), stopped(LiveState::Waiting, 5)),
        (session(BESIDE), stopped(LiveState::Error, 2)),
    ]);
    let mut pane = running(LiveState::Waiting, Some(LiveDetail::Input), 0);
    pane.observation = pane.observation.with_since_ms(NOW * 1000 - 30_000);
    snapshot.pane_live = BTreeMap::from([(LivePane::new(mux("scratch"), Some("%4".into())), pane)]);
    snapshot
}

/// What the selected row stands for, in words a test can match.
fn landed(app: &App) -> String {
    match app.selection().expect("a selection") {
        RowId::AgentSession(NodeId::AgentSession(id)) => format!("session {}", id.session_key),
        RowId::MuxSession(NodeId::MuxSession(id)) => format!("mux {}", id.native_id),
        other => format!("{other:?}"),
    }
}

fn status(app: &App) -> (Severity, String) {
    let status = app.status().expect("the status line says something");
    (status.severity, status.text.clone())
}

/// The walk's places and positions after each of `presses`, from `app`.
fn walk(app: &mut App, presses: &str) -> Vec<(View, String, String)> {
    presses
        .chars()
        .map(|key| {
            press(app, key);
            let (_, text) = status(app);
            let reached = text.split(" unread").next().unwrap_or_default();
            (app.active_view(), landed(app), reached.to_string())
        })
        .collect()
}

#[test]
fn the_next_key_goes_to_the_oldest_unread_stop_then_on_and_wraps() {
    let mut app = listing_with(View::Sessions, &four_stops(), LiveOverlay::off());
    assert_eq!(
        walk(&mut app, "}}}}"),
        [
            (View::Sessions, "session solo000001".into(), "1 of 4".into()),
            (View::Sessions, "session e44d01cb5a".into(), "2 of 4".into()),
            (View::Sessions, "session f00dfeed01".into(), "3 of 4".into()),
            // The agent in a pane no session matches is listed by the Mux
            // view alone; Sessions has rows left, so the walk stays and
            // wraps.
            (View::Sessions, "session solo000001".into(), "1 of 4".into()),
        ]
    );
    let (severity, text) = status(&app);
    assert_eq!(severity, Severity::Info);
    assert!(
        text.starts_with("1 of 4 unread, since ") && text.contains(" ago"),
        "{text}"
    );
}

#[test]
fn the_previous_key_goes_the_other_way_from_the_newest_and_wraps() {
    let mut app = listing_with(View::Sessions, &four_stops(), LiveOverlay::off());
    assert_eq!(
        walk(&mut app, "{{{{"),
        [
            (View::Sessions, "session f00dfeed01".into(), "3 of 4".into()),
            (View::Sessions, "session e44d01cb5a".into(), "2 of 4".into()),
            (View::Sessions, "session solo000001".into(), "1 of 4".into()),
            (View::Sessions, "session f00dfeed01".into(), "3 of 4".into()),
        ]
    );
    // Next and previous undo one another.
    assert_eq!(walk(&mut app, "}{")[1].1, "session f00dfeed01");
}

#[test]
fn the_mux_view_walks_its_own_rows_and_does_not_leave_for_a_session_with_no_mux() {
    let mut app = listing_with(View::Mux, &four_stops(), LiveOverlay::off());
    select(&mut app, "scratch");
    // The loose session is the oldest stop, but it has no mux: the Mux view
    // has nothing to show for it. The Mux view lists the agents of
    // `conspectus-main` on rows of their own, and the agent in a pane no
    // session matches on its mux's.
    assert_eq!(
        walk(&mut app, "}}}}"),
        [
            (View::Mux, "session e44d01cb5a".into(), "2 of 4".into()),
            (View::Mux, "session f00dfeed01".into(), "3 of 4".into()),
            (View::Mux, "mux tmux:scratch".into(), "4 of 4".into()),
            (View::Mux, "session e44d01cb5a".into(), "2 of 4".into()),
        ]
    );
    assert_eq!(walk(&mut app, "{")[0].1, "mux tmux:scratch");
}

#[test]
fn a_view_with_no_unread_row_of_its_own_is_left_for_the_one_that_lists_the_stop() {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([(session(SOLO), stopped(LiveState::Ready, 10))]);
    let mut app = listing_with(View::Mux, &snapshot, LiveOverlay::off());
    let effects = press(&mut app, '}');
    assert_eq!(app.active_view(), View::Sessions);
    assert_eq!(landed(&app), "session solo000001");
    let (severity, text) = status(&app);
    assert_eq!(severity, Severity::Info);
    assert!(
        text.starts_with("1 of 1 unread, since ")
            && text.ends_with("switched to Sessions: Mux doesn't list it"),
        "{text}"
    );
    assert_eq!(effects, vec![crate::tui::Effect::Persist]);
    app.update(Msg::NavBack);
    assert_eq!(app.active_view(), View::Mux, "Backspace returns");
}

#[test]
fn the_keys_say_so_and_stay_when_nothing_is_unread() {
    let mut app = listing_with(View::Sessions, &live_world(), LiveOverlay::off());
    let before = app.selection().cloned();
    for key in ['}', '{'] {
        assert!(press(&mut app, key).is_empty());
        assert_eq!(app.selection().cloned(), before);
        assert_eq!(
            status(&app),
            (Severity::Warning, format!("{key}: no agent is unread"))
        );
    }
}

#[test]
fn reading_every_stop_ends_the_walk() {
    let mut app = listing_with(View::Sessions, &four_stops(), LiveOverlay::off());
    press(&mut app, '}');
    app.update(Msg::ClearUnread);
    press(&mut app, '}');
    assert_eq!(
        status(&app),
        (Severity::Warning, "}: no agent is unread".to_string())
    );
}

#[test]
fn the_keys_stay_on_the_only_unread_session_and_say_no_other_is() {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([(session(HOSTED), stopped(LiveState::Waiting, 5))]);
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    press(&mut app, '}');
    assert_eq!(landed(&app), "session e44d01cb5a");
    for key in ['}', '{'] {
        press(&mut app, key);
        assert_eq!(landed(&app), "session e44d01cb5a");
        assert_eq!(
            status(&app),
            (
                Severity::Warning,
                format!("{key}: no other agent is unread")
            )
        );
    }
}

#[test]
fn the_keys_skip_what_the_filters_hide_and_say_how_many_they_hide() {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([(session(HOSTED), stopped(LiveState::Waiting, 5))]);
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    app.update(Msg::SetFilter(RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    }));
    let before = (app.active_view(), app.selection().cloned());
    press(&mut app, '}');
    assert_eq!((app.active_view(), app.selection().cloned()), before);
    assert_eq!(
        status(&app),
        (
            Severity::Warning,
            "}: 1 unread, but the filters hide it".to_string()
        )
    );
}

#[test]
fn the_status_bar_and_help_name_the_keys_from_the_key_table() {
    let status_line = |snapshot: &GraphSnapshot| {
        let mut app = listing_with(View::Sessions, snapshot, LiveOverlay::off());
        let screen = frame(&mut app, 220, 30);
        screen.lines().last().unwrap_or_default().to_string()
    };
    assert!(
        status_line(&four_stops()).contains("} next unread"),
        "{}",
        status_line(&four_stops())
    );
    assert!(!status_line(&live_world()).contains("next unread"));

    let theme = crate::tui::Theme::default();
    let text = format!("{:?}", crate::tui::widgets::help::sections(&theme));
    assert!(
        text.contains("Jump to the next agent with an unread stop")
            && text.contains("Jump to the previous agent with an unread stop"),
        "both bindings' help rows are on the page"
    );
    assert!(
        text.contains("} and { go to the next and the previous unread stop"),
        "the legend names the keys the keymap binds"
    );
}
