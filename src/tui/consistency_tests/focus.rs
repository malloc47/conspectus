//! Accelerators act on what the operator is looking at: the right
//! pane's entity when it has focus, the selected row otherwise.

use super::*;
use crate::tui::actions::{PinSubject, pin_subject};
use crate::tui::app::Focus;
use tuikit::explorer::ExplorerKey;

/// Focus the right pane and walk its cursor onto the first link that
/// leads to an agent session, returning that session.
fn focus_right_on_a_linked_session(app: &mut App) -> AgentSessionId {
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    for _ in 0..40 {
        let target = app.explorer().and_then(|state| {
            let row = app.explorer_cursor_row()?;
            match state.content().drill_target(&row)? {
                NodeId::AgentSession(session) => Some(session),
                _ => None,
            }
        });
        if let Some(session) = target {
            return session;
        }
        app.update(Msg::Explorer(ExplorerKey::Down));
    }
    panic!("the mux's detail links to a session");
}

fn mux_app() -> App {
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    app
}

#[test]
fn with_the_left_pane_focused_accelerators_act_on_the_selected_row() {
    let app = mux_app();
    assert_eq!(app.focus(), Focus::Left);
    assert_eq!(
        pin_subject(&app),
        Ok(PinSubject::Node(NodeId::MuxSession(MuxSessionId::new(
            "tmux:conspectus-main"
        ))))
    );
}

#[test]
fn with_the_right_pane_focused_p_acts_on_the_shown_entity() {
    let mut app = mux_app();
    let session = focus_right_on_a_linked_session(&mut app);
    let subject = pin_subject(&app);
    assert_eq!(
        subject,
        Ok(PinSubject::Node(NodeId::AgentSession(session))),
        "the right pane shows the session, so `p` pins the session"
    );
}

#[test]
fn with_the_right_pane_focused_the_menu_is_for_the_shown_entity() {
    let mut app = mux_app();
    let session = focus_right_on_a_linked_session(&mut app);
    app.update(Msg::OpenRowMenu);
    let menu = app.row_menu().expect("a menu for the shown entity");
    let short = session.session_key.chars().take(8).collect::<String>();
    assert!(
        menu.title().contains(&short) || menu.entries().iter().any(|e| e.label.contains(&short)),
        "the menu is about {short}: {:?}",
        menu.entries().iter().map(|e| &e.label).collect::<Vec<_>>()
    );
}

#[test]
fn returning_focus_to_the_left_pane_acts_on_the_selected_row_again() {
    let mut app = mux_app();
    focus_right_on_a_linked_session(&mut app);
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Left);
    assert_eq!(
        pin_subject(&app),
        Ok(PinSubject::Node(NodeId::MuxSession(MuxSessionId::new(
            "tmux:conspectus-main"
        ))))
    );
}
