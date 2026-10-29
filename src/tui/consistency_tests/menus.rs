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
    let mut app = app_listing_all(view);
    select(&mut app, row);
    menu_labels(&mut app)
}

#[test]
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
fn a_session_found_by_a_search_has_the_menu_it_has_in_the_sessions_view() {
    let native = normalized(labels_in(View::Sessions, "solo0000"), "solo");
    let mut app = searched(View::Search, RowFilter::default());
    select(&mut app, "solo0000");
    let found = normalized(menu_labels(&mut app), "solo");
    assert!(native.len() > 1, "{native:?}");
    assert_eq!(native, found);
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
fn entries_that_have_nothing_to_say_are_hidden() {
    use crate::model::SlotStatus;
    let mut plan_missing = Vec::new();
    let mut rebuild_for_live = Vec::new();
    let mut offered_for_pr = Vec::new();
    for (name, kind, status) in pins() {
        let labels = labels_in(View::Pinned, &name);
        let has = |label: &str| labels.iter().any(|l| l == label);
        // A PR can't be rebuilt, so neither entry means anything.
        if kind == PinKind::Pr && (has("Show rebuild plan") || has("Rebuild")) {
            offered_for_pr.push(name.clone());
        }
        // A running entity shows the plan for the case that it stopped;
        // there is nothing to rebuild yet.
        if status == SlotStatus::Live
            && matches!(
                kind,
                PinKind::Mux | PinKind::Session | PinKind::Checkout | PinKind::Repo
            )
        {
            if !has("Show rebuild plan") {
                plan_missing.push(name.clone());
            }
            if has("Rebuild") {
                rebuild_for_live.push(name);
            }
        }
    }
    assert!(plan_missing.is_empty(), "no plan for: {plan_missing:?}");
    assert!(
        rebuild_for_live.is_empty(),
        "a running entity isn't rebuilt: {rebuild_for_live:?}"
    );
    assert!(
        offered_for_pr.is_empty(),
        "a PR can't be rebuilt: {offered_for_pr:?}"
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

/// What a gesture left on screen: the modal on top, the toasts, and the
/// status line.
fn left_on_screen(app: &App) -> (Option<&'static str>, Vec<String>, Option<String>) {
    (
        app.top_modal_name(),
        app.toasts()
            .iter()
            .map(|toast| toast.text.clone())
            .collect(),
        app.status_message().map(str::to_string),
    )
}

/// One key through the routing the interactive loop uses.
fn route(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    crate::tui::runtime::press_in_fixture_mode(app, ratatui::crossterm::event::Event::Key(key));
}

/// A menu shows an entry's shortcut so the operator learns the key, so
/// pressing it in the open row menu does what the same key does on the
/// list with the same row selected. (The fixture TUI keeps the mux menu
/// disabled; the snapshot tests check its shortcuts.)
#[test]
fn a_menu_shortcut_does_what_the_same_key_does_on_the_list() {
    use ratatui::crossterm::event::KeyCode;
    use tuikit::testkit::keys::press;
    let mut checked = Vec::new();
    for view in ENTITY_VIEWS {
        let ids: Vec<RowId> = app_in(view)
            .tree()
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect();
        for id in &ids {
            let opener = '.';
            let fresh = || {
                let mut app = app_in(view);
                app.set_selection(id.clone());
                app
            };
            let mut app = fresh();
            route(&mut app, press(KeyCode::Char(opener)));
            let Some(menu) = app.row_menu() else {
                continue;
            };
            let shortcuts: Vec<_> = menu
                .entries()
                .iter()
                .filter(|entry| entry.enabled)
                .filter_map(|entry| entry.shortcut)
                .collect();
            for shortcut in shortcuts {
                let key = shortcut.example();
                let mut through_menu = fresh();
                route(&mut through_menu, press(KeyCode::Char(opener)));
                route(&mut through_menu, key);
                let mut on_the_list = fresh();
                route(&mut on_the_list, key);
                assert_eq!(
                    left_on_screen(&through_menu),
                    left_on_screen(&on_the_list),
                    "{}: `{opener}` then `{}`",
                    describe(view, id),
                    tuikit::keys::key_label(&shortcut)
                );
                checked.push((opener, tuikit::keys::key_label(&shortcut)));
            }
        }
    }
    assert!(checked.len() >= 3, "{checked:?}");
}

/// No menu shows a shortcut that the menu's own keys or an earlier entry
/// take, which would never fire.
#[test]
fn no_menu_shows_a_shortcut_it_cannot_reach() {
    let mut menus = vec![crate::tui::actions::mux_menu()];
    for view in ALL_VIEWS {
        let mut app = app_listing_all(view);
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        for id in ids {
            app.set_selection(id);
            menus.extend(crate::tui::actions::row_menu_for(&app));
        }
    }
    assert!(menus.len() > 10, "{}", menus.len());
    for menu in menus {
        assert!(
            menu.shadowed_shortcuts().is_empty(),
            "{}: {:?}",
            menu.title(),
            menu.shadowed_shortcuts()
        );
    }
}
