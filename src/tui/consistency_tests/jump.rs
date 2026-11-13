//! One key goes to the agent waiting on the operator longest (ADR 0216),
//! the same way from every view: to the row standing for it in the view
//! on screen when there is one, else in the view a reveal lands in, with
//! the groups above it opened; the next press goes on to the next one,
//! wrapping; and the status line says where it went, or why it stayed.

use std::collections::BTreeMap;

use ratatui::crossterm::event::Event;
use tuikit::feedback::Severity;
use tuikit::testkit::keys::typed;

use super::live::{BESIDE, HOSTED, SOLO, listing_with, live_world, mux, running, session};
use super::*;
use crate::model::LivePane;
use crate::model::live::{LiveDetail, LiveState};
use crate::tui::live_overlay::LiveOverlay;
use crate::tui::rows::RowKind;

/// `W`, as the keymap routes it with the app's focus.
fn press_w(app: &mut App) -> Vec<crate::tui::Effect> {
    let action =
        crate::tui::keymap::translate_in(app.focus(), Event::Key(typed('W'))).expect("W is bound");
    let crate::tui::keymap::Action::Msg(msg) = action else {
        panic!("W is a message: {action:?}");
    };
    app.update(*msg)
}

/// A wait that began `minutes` ago.
fn waiting_for(minutes: i64) -> crate::model::live::LiveBinding {
    let mut binding = running(LiveState::Waiting, Some(LiveDetail::Permission), 0);
    binding.observation = binding
        .observation
        .with_since_ms(NOW * 1000 - minutes * 60_000);
    binding
}

/// The live world with three agents waiting: the loose session (no mux)
/// for ten minutes, the hosted one for five, and an agent in a pane of
/// `scratch` no session matches for half a minute. The second agent of
/// `conspectus-main` failed its turn.
fn three_waiting() -> GraphSnapshot {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([
        (session(SOLO), waiting_for(10)),
        (session(HOSTED), waiting_for(5)),
        (
            session(BESIDE),
            running(LiveState::Error, Some(LiveDetail::RateLimited), 0),
        ),
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
        RowId::Pin { pin_id } => format!("pin {pin_id}"),
        other => format!("{other:?}"),
    }
}

fn status(app: &App) -> (Severity, String) {
    let status = app.status().expect("the status line says something");
    (status.severity, status.text.clone())
}

#[test]
fn w_goes_to_the_session_waiting_longest_from_every_view() {
    let snapshot = three_waiting();
    for (view, lands_in, row) in [
        (View::Sessions, View::Sessions, "session solo000001"),
        (View::Mux, View::Sessions, "session solo000001"),
        (View::Prs, View::Sessions, "session solo000001"),
        (View::Pinned, View::Pinned, "pin solo"),
        (View::History, View::Sessions, "session solo000001"),
    ] {
        let mut app = listing_with(view, &snapshot, LiveOverlay::off());
        let effects = press_w(&mut app);
        assert_eq!(app.active_view(), lands_in, "{view:?}");
        assert_eq!(landed(&app), row, "{view:?}");
        let (severity, text) = status(&app);
        assert_eq!(severity, Severity::Info, "{view:?}: {text}");
        // The age is read against the clock, as the detail pane's is.
        assert!(
            text.starts_with("1 of 3 waiting, since ") && text.contains(" ago"),
            "{view:?}: {text}"
        );
        if lands_in == view {
            assert!(
                effects.is_empty(),
                "{view:?}: a move in place is view state"
            );
        } else {
            assert!(
                text.ends_with(&format!(
                    "switched to Sessions: {} doesn't list it",
                    crate::tui::widgets::controls::view_label(view)
                )),
                "{view:?}: {text}"
            );
            assert_eq!(
                effects,
                vec![crate::tui::Effect::Persist],
                "{view:?}: the new view is kept, as a switch keeps it"
            );
            app.update(Msg::NavBack);
            assert_eq!(app.active_view(), view, "Backspace returns");
        }
    }
}

#[test]
fn under_a_content_search_w_says_the_others_are_hidden() {
    // The search finds only the loose session, and it narrows every list,
    // so the other two agents waiting are listed nowhere.
    let mut app = listing_with(View::Search, &three_waiting(), LiveOverlay::off());
    assert_eq!(landed(&app), "session solo000001");
    press_w(&mut app);
    assert_eq!(app.active_view(), View::Search);
    assert_eq!(landed(&app), "session solo000001");
    assert_eq!(
        status(&app),
        (
            Severity::Warning,
            "W: no other waiting agent is listed; the filters hide 2".to_string()
        )
    );
}

#[test]
fn w_goes_on_to_the_next_wait_and_wraps() {
    let mut app = listing_with(View::Sessions, &three_waiting(), LiveOverlay::off());
    let mut visits = Vec::new();
    for _ in 0..4 {
        press_w(&mut app);
        let (_, text) = status(&app);
        let reached = text
            .split(" waiting")
            .next()
            .unwrap_or_default()
            .to_string();
        visits.push((app.active_view(), landed(&app), reached));
    }
    assert_eq!(
        visits,
        [
            (View::Sessions, "session solo000001".into(), "1 of 3".into()),
            (View::Sessions, "session e44d01cb5a".into(), "2 of 3".into()),
            // Only the Mux view lists an agent no session matches, on its
            // mux's row.
            (View::Mux, "mux tmux:scratch".into(), "3 of 3".into()),
            (View::Sessions, "session solo000001".into(), "1 of 3".into()),
        ]
    );
}

#[test]
fn w_opens_the_groups_above_the_row_it_lands_on() {
    let mut app = listing_with(View::Sessions, &three_waiting(), LiveOverlay::off());
    // Fold every group, from the last up, so the sessions are hidden.
    let groups: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::Group(_)))
        .map(|row| row.id.clone())
        .collect();
    for group in groups.iter().rev() {
        app.set_selection(group.clone());
        app.update(Msg::CollapseRow);
    }
    let hidden = |app: &App| {
        !app.visible_rows()
            .iter()
            .any(|row| row.id == RowId::AgentSession(session(SOLO)))
    };
    assert!(hidden(&app), "the session starts folded away");
    press_w(&mut app);
    assert!(!hidden(&app), "its groups opened");
    assert_eq!(landed(&app), "session solo000001");
}

#[test]
fn in_the_mux_view_a_session_with_no_row_of_its_own_is_reached_on_its_mux() {
    // The hosted session is the only agent of `conspectus-main`, which
    // shows it on its own row.
    let mut snapshot = world();
    snapshot.node_live = BTreeMap::from([(session(HOSTED), waiting_for(5))]);
    let mut app = listing_with(View::Mux, &snapshot, LiveOverlay::off());
    select(&mut app, "scratch");
    press_w(&mut app);
    assert_eq!(app.active_view(), View::Mux);
    assert_eq!(landed(&app), "mux tmux:conspectus-main");
    let (_, text) = status(&app);
    assert!(text.starts_with("1 of 1 waiting, since "), "{text}");
    press_w(&mut app);
    assert_eq!(
        status(&app).1,
        "W: no other agent is waiting on you",
        "on the mux's row the cursor is on its agent already"
    );
}

#[test]
fn w_stays_and_says_so_when_nothing_waits() {
    let mut app = listing_with(View::Sessions, &live_world(), LiveOverlay::off());
    let before = app.selection().cloned();
    assert!(press_w(&mut app).is_empty());
    assert_eq!(app.selection().cloned(), before);
    assert_eq!(
        status(&app),
        (
            Severity::Warning,
            "W: no agent is waiting on you".to_string()
        )
    );
}

#[test]
fn w_stays_on_the_only_waiting_session_and_says_none_other_waits() {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([(session(HOSTED), waiting_for(5))]);
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    press_w(&mut app);
    assert_eq!(landed(&app), "session e44d01cb5a");
    press_w(&mut app);
    assert_eq!(landed(&app), "session e44d01cb5a");
    assert_eq!(
        status(&app),
        (
            Severity::Warning,
            "W: no other agent is waiting on you".to_string()
        )
    );
}

#[test]
fn w_skips_what_the_filters_hide_and_says_how_many_it_hides() {
    let mut snapshot = live_world();
    snapshot.node_live = BTreeMap::from([(session(HOSTED), waiting_for(5))]);
    let mut app = listing_with(View::Sessions, &snapshot, LiveOverlay::off());
    app.update(Msg::SetFilter(RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    }));
    let before = (app.active_view(), app.selection().cloned());
    press_w(&mut app);
    assert_eq!((app.active_view(), app.selection().cloned()), before);
    assert_eq!(
        status(&app),
        (
            Severity::Warning,
            "W: 1 waiting, but the filters hide it".to_string()
        )
    );
}

#[test]
fn the_status_bar_names_w_only_while_an_agent_waits() {
    let status_line = |snapshot: &GraphSnapshot| {
        let mut app = listing_with(View::Sessions, snapshot, LiveOverlay::off());
        let screen = frame(&mut app, 220, 30);
        screen.lines().last().unwrap_or_default().to_string()
    };
    assert!(
        status_line(&three_waiting()).contains("W next waiting"),
        "{}",
        status_line(&three_waiting())
    );
    assert!(!status_line(&live_world()).contains("W next waiting"));
}

#[test]
fn the_help_page_lists_w_from_its_binding_and_the_legend_names_it() {
    let theme = crate::tui::Theme::default();
    let sections = crate::tui::widgets::help::sections(&theme);
    let text = format!("{sections:?}");
    assert!(
        text.contains("Jump to the next agent waiting on you"),
        "the binding's help row is on the page"
    );
    assert!(
        text.contains("W goes to the one waiting longest"),
        "the waiting legend names the key the keymap binds"
    );
}
