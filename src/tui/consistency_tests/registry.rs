//! Every modal is registered, routed, drawn, and closed by one rule.
//!
//! The `modals!` registration in `modal.rs` is the one list of modals: it
//! declares the `Modal` enum, and the stack routes and draws whatever it
//! lists (ADR 0161). These checks hold each registered modal to what the
//! stack promises, so a new modal fails here until it is built below:
//!
//! - **Registered.** Every name the registration lists is built here, and
//!   each entry opens the modal it names.
//! - **Routed.** With the modal on top, every key press is the modal's (the
//!   main keymap sees none).
//! - **Drawn under another.** The modal stays drawn with a prompt opened
//!   over it.
//! - **Closed by one rule inside it** (house rules 4 and 5). Inside a
//!   modal, Esc steps back one level, Ctrl-C dismisses the whole modal,
//!   and so does `q` where nothing is typed.
//!
//! That every modal is drawn and closes by the one rule at its first
//! level, silently, is the framework's conformance suite's business
//! (`conventions.rs`), over the same modals.

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tuikit::testkit::keys::{ctrl, press, release};

use super::layout::{Open, app_with, builders};
use super::*;
use crate::tui::Modal;
use crate::tui::keymap::Action;
use crate::tui::widgets::input::TextInputState;

pub(super) fn open_viewer(app: &mut App) {
    let document = crate::viewer::model::TranscriptDocument::default();
    app.open_viewer_modal(crate::viewer::state::ViewerState::new(document));
}

pub(super) fn open_pane_viewer(app: &mut App) {
    use crate::content_search::{MuxRef, PaneRef};
    let pane = PaneRef {
        mux: MuxRef {
            backend: "tmux".into(),
            socket: None,
            session: "shell".into(),
        },
        pane_id: "%0".into(),
        window_index: "0".into(),
        pane_index: "0".into(),
        command: Some("bash".into()),
    };
    let lines = (1..=30).map(|n| format!("line {n} of output")).collect();
    app.open_pane_viewer(crate::tui::widgets::pane_viewer::PaneViewerState::open(
        pane, lines, 4, "",
    ));
}

/// Every modal, built: the layout suite's framed modals and the two
/// full-screen viewers.
fn registry() -> Vec<(&'static str, View, Open)> {
    let mut all = builders();
    all.push(("Viewer", View::Pinned, open_viewer));
    all.push(("PaneViewer", View::Pinned, open_pane_viewer));
    all
}

/// One key the way the loops route it. `false` when no modal took it.
fn route(app: &mut App, key: KeyEvent) -> bool {
    match crate::tui::runtime::action_for_event(app, Event::Key(key)) {
        Some(Action::ModalKey(key)) => {
            crate::tui::runtime::modal_key_pure(app, key);
            true
        }
        _ => false,
    }
}

/// What a key did to the modal on top of an app opened with `open`.
#[derive(Debug, PartialEq)]
enum After {
    /// The modal closed, silently.
    Closed,
    /// The modal is still on top.
    Open,
}

fn after(view: View, open: Open, key: KeyEvent, name: &str) -> After {
    let mut app = app_with(view, open);
    // A hint the modal set when it opened, which closing must not leave.
    app.update(Msg::SetStatus(Some(Feedback::info("a hint"))));
    let depth = app.modal_stack().len();
    assert!(route(&mut app, key), "{name}: {key:?} was not the modal's");
    if app.modal_stack().len() < depth {
        assert_eq!(app.status_message(), None, "{name}: {key:?} said something");
        assert!(app.toasts().is_empty(), "{name}: {key:?} posted a toast");
        After::Closed
    } else {
        assert_eq!(app.top_modal_name(), Some(name), "{name}: {key:?}");
        After::Open
    }
}

#[test]
fn every_registered_modal_is_built_here() {
    let built: Vec<&str> = registry().iter().map(|(name, _, _)| *name).collect();
    for name in Modal::NAMES {
        assert!(
            built.contains(name),
            "Modal::{name} is registered but not built in registry.rs, so nothing checks that it is routed and drawn"
        );
    }
}

#[test]
fn each_entry_opens_the_modal_it_names() {
    for (name, view, open) in registry() {
        let app = app_with(view, open);
        assert_eq!(app.top_modal_name(), Some(name));
        assert!(Modal::NAMES.contains(&name), "{name} is not registered");
    }
}

#[test]
fn an_open_modal_owns_every_key_press() {
    let keys = tuikit::testkit::keys::script("jq?1<Enter><Esc><Tab><F5><C-c><C-g>");
    for (name, view, open) in registry() {
        let app = app_with(view, open);
        for &key in &keys {
            let action = crate::tui::runtime::action_for_event(&app, Event::Key(key));
            assert_eq!(
                action,
                Some(Action::ModalKey(key)),
                "{name}: {key:?} reached the main keymap"
            );
        }
        let release = release(press(KeyCode::Char('j')));
        assert_eq!(
            crate::tui::runtime::action_for_event(&app, Event::Key(release)),
            None,
            "{name}: a key release is nobody's"
        );
    }
}

/// A small prompt over every modal: the frame with both differs from the
/// frame with the prompt alone, so the modal under it is still drawn. A
/// modal whose whole frame lies inside the prompt's (a prompt or a short
/// checklist, the smallest modals) is hidden by it, not missing, so it is
/// left out; the larger ones must all be checked.
#[test]
fn every_modal_stays_drawn_under_another() {
    use super::layout::modal_rect;
    let prompt = || TextInputState::new("Above", "typed");
    let (width, height) = (120, 40);
    let mut bare = app_in(View::Pinned);
    let screen = buffer_of(&mut bare, width, height);
    bare.open_rename_overlay(prompt());
    let prompt_rect = modal_rect(&buffer_of(&mut bare, width, height), &screen).expect("a prompt");
    let inside = |(x0, y0, x1, y1): (u16, u16, u16, u16)| {
        x0 >= prompt_rect.0 && y0 >= prompt_rect.1 && x1 <= prompt_rect.2 && y1 <= prompt_rect.3
    };
    let mut checked = 0;
    for (name, view, open) in registry() {
        let mut app = app_with(view, open);
        let alone = buffer_of(&mut app, width, height);
        app.pop_top_modal();
        let under = buffer_of(&mut app, width, height);
        if modal_rect(&alone, &under).is_some_and(inside) {
            continue;
        }
        let mut app = app_with(view, open);
        app.open_rename_overlay(prompt());
        let both = frame(&mut app, width, height);
        let mut app = app_with(view, open);
        app.pop_top_modal();
        app.open_rename_overlay(prompt());
        let prompt_only = frame(&mut app, width, height);
        assert!(
            both.contains("Above"),
            "{name}: the prompt is drawn\n{both}"
        );
        assert_ne!(
            both, prompt_only,
            "{name} was not drawn under the prompt\n{both}"
        );
        checked += 1;
    }
    assert!(
        checked >= 12,
        "only {checked} modals were checked under the prompt"
    );
}

/// A level inside a modal: whether its keys type, and how to reach it.
struct Level {
    name: &'static str,
    modal: &'static str,
    typed: bool,
    open: fn(&mut App),
}

fn controls_at_filters(app: &mut App) {
    let state = crate::tui::widgets::controls::ControlsOverlayState::new_at_filters(
        &app.controls_context(),
    );
    app.open_modal(Modal::Controls(state));
}

fn worktree_menu(app: &mut App) {
    use crate::tui::widgets::worktree_menu::{WorktreeMenuContext, WorktreeMenuState};
    let context = WorktreeMenuContext {
        context: crate::tui::worktree_actions::WorktreeContext::Worktree,
        repo_root: Some("/wt/feature".into()),
        branch: Some("feature".into()),
        guard_sessions: Vec::new(),
        can_mutate: true,
        reveal_target: None,
    };
    app.open_worktree_menu(WorktreeMenuState::new(context).expect("a worktree menu"));
}

fn keys(app: &mut App, keys: &[KeyEvent]) {
    for key in keys {
        assert!(route(app, *key), "{key:?} was not the modal's");
    }
}

fn levels() -> Vec<Level> {
    vec![
        Level {
            name: "the Controls harness checklist",
            modal: "Controls",
            typed: false,
            open: |app| {
                controls_at_filters(app);
                keys(app, &[press(KeyCode::Enter)]);
            },
        },
        Level {
            name: "the Controls max-age editor",
            modal: "Controls",
            typed: true,
            open: |app| {
                controls_at_filters(app);
                keys(app, &[press(KeyCode::Down), press(KeyCode::Enter)]);
            },
        },
        Level {
            name: "the worktree branch prompt",
            modal: "WorktreeMenu",
            typed: true,
            open: |app| {
                worktree_menu(app);
                keys(app, &[press(KeyCode::Enter)]);
            },
        },
        Level {
            name: "the worktree merge confirmation",
            modal: "WorktreeMenu",
            typed: false,
            open: |app| {
                worktree_menu(app);
                keys(app, &[press(KeyCode::Down), press(KeyCode::Enter)]);
            },
        },
        Level {
            name: "the worktree close-down choice",
            modal: "WorktreeMenu",
            typed: false,
            open: |app| {
                worktree_menu(app);
                let down = press(KeyCode::Down);
                keys(app, &[down, down, down, press(KeyCode::Enter)]);
            },
        },
        Level {
            name: "the content search options",
            modal: "Search",
            typed: false,
            open: |app| {
                app.open_content_search_overlay();
                keys(app, &[ctrl('o')]);
            },
        },
        Level {
            name: "the transcript viewer's help",
            modal: "Viewer",
            typed: false,
            open: |app| {
                open_viewer(app);
                keys(app, &[press(KeyCode::Char('?'))]);
            },
        },
        Level {
            name: "the transcript viewer's find prompt",
            modal: "Viewer",
            typed: true,
            open: |app| {
                open_viewer(app);
                keys(app, &[press(KeyCode::Char('/'))]);
            },
        },
        Level {
            name: "the pane viewer's help",
            modal: "PaneViewer",
            typed: false,
            open: |app| {
                open_pane_viewer(app);
                keys(app, &[press(KeyCode::Char('?'))]);
            },
        },
        Level {
            name: "the pane viewer's find prompt",
            modal: "PaneViewer",
            typed: true,
            open: |app| {
                open_pane_viewer(app);
                keys(app, &[press(KeyCode::Char('/'))]);
            },
        },
    ]
}

/// Inside a modal, Esc steps back one level and the modal stays open;
/// Ctrl-C dismisses the whole modal, and so does `q` where nothing is
/// typed (house rule 4).
#[test]
fn esc_steps_back_and_ctrl_c_and_q_dismiss_from_inside_a_modal() {
    for level in levels() {
        let Level {
            name,
            modal,
            typed,
            open,
        } = level;
        assert_eq!(
            after(View::Pinned, open, press(KeyCode::Esc), modal),
            After::Open,
            "Esc in {name} left the modal"
        );
        assert_eq!(
            after(View::Pinned, open, ctrl('c'), modal),
            After::Closed,
            "Ctrl-C in {name}"
        );
        let q = if typed { After::Open } else { After::Closed };
        assert_eq!(
            after(View::Pinned, open, press(KeyCode::Char('q')), modal),
            q,
            "q in {name}"
        );
    }
}
