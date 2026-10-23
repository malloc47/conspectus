//! One gesture, one meaning: Enter, `v`, and `p` do the same thing to
//! the same entity whichever view it is shown in.

use super::*;
use crate::tui::Effect;
use crate::tui::effect::{ExecSpec, lifecycle};

fn row_ids(app: &App) -> Vec<RowId> {
    app.tree().rows.iter().map(|row| row.id.clone()).collect()
}

/// The agent session a row stands for, if it stands for one.
fn session_of(app: &App, id: &RowId) -> Option<AgentSessionId> {
    match id {
        RowId::AgentSession(NodeId::AgentSession(session))
        | RowId::Nested {
            node: NodeId::AgentSession(session),
            ..
        } => Some(session.clone()),
        RowId::Pin { pin_id } => {
            let snapshot = app.snapshot_handle()?.snapshot();
            let pin = crate::tui::rows::pin_nodes(snapshot)
                .into_iter()
                .find(|pin| pin.id.id == *pin_id)?;
            match pin.target_node.as_ref()? {
                NodeId::AgentSession(session) => Some(session.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_native(id: &RowId) -> bool {
    matches!(id, RowId::AgentSession(_))
}

/// Rows (view, id) that stand for a session, with the effect `v` has.
fn viewer_outcomes(only_native: bool) -> Vec<(String, AgentSessionId, Vec<Effect>)> {
    let mut out = Vec::new();
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for id in row_ids(&app) {
            let Some(session) = session_of(&app, &id) else {
                continue;
            };
            if is_native(&id) != only_native {
                continue;
            }
            app.set_selection(id.clone());
            let effects = app.update(Msg::ViewSelected);
            out.push((describe(view, &id), session, effects));
        }
    }
    out
}

fn assert_all_view_the_session(outcomes: Vec<(String, AgentSessionId, Vec<Effect>)>) {
    assert!(!outcomes.is_empty(), "the fixture shows sessions here");
    let wrong: Vec<String> = outcomes
        .into_iter()
        .filter(|(_, session, effects)| {
            *effects != vec![Effect::Exec(ExecSpec::ViewSession(session.clone()))]
        })
        .map(|(row, _, effects)| format!("{row}: {effects:?}"))
        .collect();
    assert!(
        wrong.is_empty(),
        "`v` must open the viewer:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn v_opens_the_viewer_on_a_native_session_row() {
    assert_all_view_the_session(viewer_outcomes(true));
}

#[test]
#[ignore = "backlog CSP-692.03: v errors on pinned and nested session rows"]
fn v_opens_the_viewer_wherever_a_session_is_shown() {
    assert_all_view_the_session(viewer_outcomes(false));
}

fn enter_in(view: View, label: &str) -> Vec<Effect> {
    let mut app = app_in(view);
    select(&mut app, label);
    enter(&mut app)
}

#[test]
fn enter_on_a_native_session_attaches_or_views_by_whether_it_is_in_a_mux() {
    let in_mux = enter_in(View::Sessions, "e44d01cb");
    assert!(
        matches!(in_mux.as_slice(), [Effect::Exec(ExecSpec::AttachMux(_))]),
        "{in_mux:?}"
    );
    let outside = enter_in(View::Sessions, "solo0000");
    assert!(
        matches!(outside.as_slice(), [Effect::Exec(ExecSpec::ViewSession(_))]),
        "{outside:?}"
    );
}

#[test]
fn enter_on_a_pinned_session_in_a_mux_attaches_like_its_native_row() {
    let effects = enter_in(View::Pinned, "e44d01cb");
    assert!(
        matches!(effects.as_slice(), [Effect::Exec(ExecSpec::AttachMux(_))]),
        "{effects:?}"
    );
}

#[test]
#[ignore = "backlog CSP-692.03: Enter on a live pinned session outside a mux tries to rebuild it as a mux"]
fn enter_on_a_pinned_session_outside_a_mux_opens_the_viewer() {
    let effects = enter_in(View::Pinned, "solo");
    assert!(
        matches!(effects.as_slice(), [Effect::Exec(ExecSpec::ViewSession(_))]),
        "{effects:?}"
    );
}

/// Rows standing for a mux that is gone or not built yet, by view.
fn unstarted_mux_rows() -> Vec<(View, &'static str)> {
    vec![
        (View::History, "h_agent"),
        (View::Mux, "config-agent3"),
        (View::Pinned, "config-agent3"),
        (View::Pinned, "nightly"),
    ]
}

#[test]
fn enter_on_an_unstarted_mux_is_a_rebuild_in_every_view() {
    for (view, label) in unstarted_mux_rows() {
        let effects = enter_in(view, label);
        assert!(
            matches!(effects.as_slice(), [Effect::Exec(ExecSpec::Rebuild { .. })]),
            "{view:?}/{label}: {effects:?}"
        );
    }
}

#[test]
#[ignore = "backlog CSP-692.02: Enter on a gone mux rebuilds without attaching"]
fn enter_on_an_unstarted_mux_starts_it_and_attaches() {
    for (view, label) in unstarted_mux_rows() {
        let effects = enter_in(view, label);
        assert!(
            effects.iter().any(|effect| lifecycle(effect).attaches),
            "{view:?}/{label} must start the mux and attach to it: {effects:?}"
        );
    }
}

#[test]
fn p_pins_exactly_what_the_menus_pin_entry_pins() {
    let mut compared = 0;
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for id in row_ids(&app) {
            app.set_selection(id.clone());
            let Ok(subject) = crate::tui::actions::pin_subject(&app) else {
                continue;
            };
            let snapshot = app.snapshot_handle().expect("data").snapshot().clone();
            if !crate::tui::actions::slots_pinning(&snapshot, &subject).is_empty() {
                continue;
            }
            app.update(Msg::OpenRowMenu);
            let Some(pin) = app
                .row_menu()
                .and_then(|menu| menu.entries().iter().find(|entry| entry.label == "Pin"))
                .map(|entry| entry.msg.clone())
            else {
                app.close_row_menu();
                panic!("{}: no Pin entry for a pinnable row", describe(view, &id));
            };
            app.close_row_menu();
            assert_eq!(
                app.update(Msg::TogglePin),
                app.update(pin),
                "{}",
                describe(view, &id)
            );
            compared += 1;
        }
    }
    assert!(compared >= 4, "the fixture has pinnable rows: {compared}");
}

#[test]
fn rows_with_nothing_to_pin_say_why_instead_of_doing_nothing() {
    let mut app = app_in(View::Pinned);
    let bucket = row_ids(&app)
        .into_iter()
        .find(|id| matches!(id, RowId::Bucket(_)))
        .expect("a bucket");
    app.set_selection(bucket);
    let effects = app.update(Msg::TogglePin);
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(text)] if !text.is_empty()),
        "{effects:?}"
    );
}
