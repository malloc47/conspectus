//! Menus depend on the entity, not on the view that opened them, and
//! offer only what applies.

use std::collections::BTreeSet;

use super::*;

/// Labels with the pin's name stripped (`Unpin foo` is `Unpin`) and the
/// entry that only makes sense away from the entity's own view removed.
fn normalized(labels: Vec<String>, pin: &str) -> BTreeSet<String> {
    labels
        .into_iter()
        .map(|label| {
            label
                .strip_suffix(&format!(" {pin}"))
                .map_or_else(|| label.clone(), str::to_string)
        })
        .filter(|label| label != "Reveal in its view")
        .collect()
}

fn labels_in(view: View, row: &str) -> Vec<String> {
    let mut app = app_in(view);
    select(&mut app, row);
    menu_labels(&mut app)
}

#[test]
#[ignore = "backlog CSP-692.03: the Mux view's menu for a pinned mux offers only Unpin"]
fn a_pinned_mux_has_the_same_menu_in_the_mux_and_pinned_views() {
    let native = normalized(labels_in(View::Mux, "conspectus-main"), "conspectus-main");
    let pinned = normalized(
        labels_in(View::Pinned, "conspectus-main"),
        "conspectus-main",
    );
    assert!(
        native.len() > 1,
        "the native menu is more than Unpin: {native:?}"
    );
    assert_eq!(native, pinned);
}

#[test]
#[ignore = "backlog CSP-692.03: the Sessions view's menu for a pinned session offers only Unpin"]
fn a_pinned_session_has_the_same_menu_in_the_sessions_and_pinned_views() {
    let native = normalized(labels_in(View::Sessions, "e44d01cb"), "e44d01cb");
    let pinned = normalized(labels_in(View::Pinned, "e44d01cb"), "e44d01cb");
    assert!(
        native.len() > 1,
        "the native menu is more than Unpin: {native:?}"
    );
    assert_eq!(native, pinned);
}

#[test]
fn a_ghost_mux_has_the_same_menu_as_its_history_record() {
    let ghost = normalized(labels_in(View::Mux, "config-agent3"), "config-agent3");
    let mut record = normalized(labels_in(View::History, "h_agent"), "config-agent3");
    record.remove("Clean up…");
    assert_eq!(ghost, record);
}

#[test]
fn every_pinnable_row_offers_pin_or_unpin_first_class() {
    let mut checked = 0;
    for view in ENTITY_VIEWS {
        let mut app = app_in(view);
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        for id in ids {
            app.set_selection(id.clone());
            if crate::tui::actions::pin_subject(&app).is_err() {
                continue;
            }
            let labels = menu_labels(&mut app);
            assert!(
                labels
                    .iter()
                    .any(|label| label == "Pin" || label.starts_with("Unpin")),
                "{}: {labels:?}",
                describe(view, &id)
            );
            checked += 1;
        }
    }
    assert!(checked >= 5, "{checked}");
}

/// Pins in the Pinned view with their kind and status.
fn pins() -> Vec<(String, PinKind, crate::model::SlotStatus)> {
    let snapshot = world();
    crate::tui::rows::pin_nodes(&snapshot)
        .into_iter()
        .map(|pin| (pin.id.id.clone(), pin.kind, pin.status))
        .collect()
}

#[test]
#[ignore = "backlog CSP-692.03: rebuild entries are offered for pins with nothing to rebuild"]
fn entries_that_have_nothing_to_say_are_hidden() {
    use crate::model::SlotStatus;
    let mut offered_plan_for_live = Vec::new();
    let mut offered_rebuild_for_pr = Vec::new();
    for (name, kind, status) in pins() {
        let labels = labels_in(View::Pinned, &name);
        if kind == PinKind::Pr
            && labels
                .iter()
                .any(|l| l == "Show rebuild plan" || l == "Rebuild")
        {
            offered_rebuild_for_pr.push(name.clone());
        }
        if status == SlotStatus::Live && labels.iter().any(|l| l == "Show rebuild plan") {
            offered_plan_for_live.push(name);
        }
    }
    assert!(
        offered_plan_for_live.is_empty(),
        "a live entity has no plan to show: {offered_plan_for_live:?}"
    );
    assert!(
        offered_rebuild_for_pr.is_empty(),
        "a PR can't be rebuilt: {offered_rebuild_for_pr:?}"
    );
}

#[test]
fn every_pin_menu_entry_acts_on_that_pin() {
    let mut checked = 0;
    for (name, _, _) in pins() {
        let mut app = app_in(View::Pinned);
        select(&mut app, &name);
        app.update(Msg::OpenRowMenu);
        let menu = app.row_menu().expect("a pin has a menu");
        assert!(!menu.entries().is_empty(), "{name}");
        checked += 1;
    }
    assert!(checked >= 6, "{checked}");
}
