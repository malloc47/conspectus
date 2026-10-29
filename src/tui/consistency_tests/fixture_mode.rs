//! The fixture and scenario TUI (`tui --fixture`, `dev scenario tui`)
//! run the live TUI's overlays with no terminal, mux, or store behind
//! them. Every overlay they can open still moves and closes with the
//! keys it uses in the live TUI, so the operator is never shut in, and a
//! pick that would act outside the TUI is refused out loud.
//!
//! Keys go through the same routing as the interactive loop
//! (`press_in_fixture_mode`), so the checks hold whichever control an
//! overlay is built on.

use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use super::layout::{app_with, builders, name_of};
use super::*;
use crate::tui::runtime::press_in_fixture_mode;
use tuikit::action_menu::ActionMenu;
use tuikit::testkit::keys::chord;

fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    press_in_fixture_mode(app, Event::Key(chord(modifiers, code)));
}

fn key(app: &mut App, code: KeyCode) {
    press(app, code, KeyModifiers::NONE);
}

/// The whole screen with any toast drawn.
fn screen(app: &mut App) -> String {
    app.prepare_toast_for_render();
    frame(app, 120, 30)
}

/// Every modal the suite can build closes on Esc and, as the abort key
/// every overlay binds, on Ctrl-C, without quitting the TUI.
#[test]
fn every_overlay_closes_on_esc_and_on_ctrl_c_in_fixture_mode() {
    let leaves = [
        (KeyCode::Esc, KeyModifiers::NONE, "Esc"),
        (KeyCode::Char('c'), KeyModifiers::CONTROL, "Ctrl-C"),
    ];
    for (code, modifiers, what) in leaves {
        for (name, view, open) in builders() {
            let mut app = app_with(view, open);
            let top = app.modal_stack().top().map(name_of);
            assert_eq!(top, Some(name), "the builder opens {name}");
            let depth = app.modal_stack().len();
            press(&mut app, code, modifiers);
            assert!(
                app.modal_stack().len() < depth,
                "{name}: {what} left it open; the status line reads {:?}",
                app.status_message()
            );
            assert!(!app.should_quit(), "{name}: {what} quit the TUI");
        }
    }
}

/// Cancelling is not an event (house rule 5): closing any overlay with
/// Esc or Ctrl-C puts no message on the status line, which shows the
/// screen's own hints again (a hint the overlay set when it opened goes
/// with it), and posts no toast.
#[test]
fn closing_an_overlay_says_nothing() {
    let leaves = [
        (KeyCode::Esc, KeyModifiers::NONE, "Esc"),
        (KeyCode::Char('c'), KeyModifiers::CONTROL, "Ctrl-C"),
    ];
    for (code, modifiers, what) in leaves {
        for (name, view, open) in builders() {
            let mut app = app_with(view, open);
            app.update(Msg::SetStatus(Some(Feedback::info(
                "a hint the overlay set",
            ))));
            press(&mut app, code, modifiers);
            assert_eq!(
                app.status_message(),
                None,
                "{name}: {what} left a message on the status line"
            );
            assert!(app.toasts().is_empty(), "{name}: {what} posted a toast");
        }
    }
}

/// The reported case: `.` opens the row menu, `j` moves its cursor, Esc
/// closes it, and Ctrl-C closes it too; with nothing open Ctrl-C quits.
#[test]
fn a_menu_opened_with_dot_moves_and_closes_in_fixture_mode() {
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    key(&mut app, KeyCode::Char('.'));
    let menu = app.row_menu().expect("`.` opens the row menu");
    assert!(
        menu.entries().len() > 1,
        "the menu has entries to move between"
    );
    let cursor = menu.cursor();
    key(&mut app, KeyCode::Char('j'));
    assert_ne!(
        app.row_menu().map(ActionMenu::cursor),
        Some(cursor),
        "`j` moves the menu's cursor"
    );
    key(&mut app, KeyCode::Esc);
    assert!(app.row_menu().is_none(), "Esc closes the menu");
    assert!(
        !app.status_message()
            .is_some_and(|status| status.contains("disabled")),
        "closing is not refused: {:?}",
        app.status_message()
    );

    key(&mut app, KeyCode::Char('.'));
    press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(app.row_menu().is_none(), "Ctrl-C closes the menu");
    assert!(!app.should_quit(), "the first Ctrl-C only closes the menu");
    press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(app.should_quit(), "with nothing open, Ctrl-C quits");
}

/// A pick that needs the live TUI (pinning writes the pin store) closes
/// the menu and says it is not available here, as `p` does, instead of
/// running or leaving the menu up.
#[test]
fn a_pick_that_acts_outside_the_tui_is_refused_in_fixture_mode() {
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    key(&mut app, KeyCode::Char('.'));
    let pin = app
        .row_menu()
        .expect("`.` opens the row menu")
        .entries()
        .iter()
        .position(|entry| entry.label == "Pin")
        .expect("an unpinned mux offers Pin");
    for _ in 0..pin {
        key(&mut app, KeyCode::Char('j'));
    }
    key(&mut app, KeyCode::Enter);
    assert!(app.row_menu().is_none(), "the pick closes the menu");
    let shown = screen(&mut app);
    assert!(
        shown.contains("that isn't available in this mode"),
        "the refusal is shown:\n{shown}"
    );

    // `p` on the same mux is refused the same way.
    let mut app = app_in(View::Mux);
    select(&mut app, "scratch");
    key(&mut app, KeyCode::Char('p'));
    let shown = screen(&mut app);
    assert!(
        shown.contains("that isn't available in this mode"),
        "`p` is refused alike:\n{shown}"
    );
}
