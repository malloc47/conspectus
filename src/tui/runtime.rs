//! Terminal lifecycle and event loop.
//!
//! `ratatui::init` already installs a panic hook that restores the
//! terminal, so this module just sets up the loop and is the only
//! place in the crate that touches stdout in raw mode.

use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::tui::RunConfig;
use crate::tui::app::{App, Msg};
use crate::tui::ui;

/// Run the TUI to completion. Restores the terminal on normal exit,
/// errors, and panics (the panic path is covered by the hook
/// `ratatui::init` installs).
pub fn run(config: RunConfig) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, config);
    ratatui::restore();
    result
}

/// Block on terminal input, dispatching crossterm events to the
/// pure reducer until the app signals quit.
fn event_loop(terminal: &mut DefaultTerminal, config: RunConfig) -> Result<()> {
    let mut app = App::new(config);
    let poll_timeout = Duration::from_millis(100);

    while !app.should_quit() {
        terminal.draw(|frame| ui::draw(&app, frame))?;

        // Block on the next event up to `poll_timeout`. Returning
        // false means no event arrived; iterating without a redraw
        // keeps CPU near idle while still letting future timer
        // ticks (P8-008) break out promptly.
        if event::poll(poll_timeout)? {
            let event = event::read()?;
            let viewport = terminal.size()?.height.saturating_sub(2);
            if let Some(msg) = translate(event, viewport) {
                app.update(msg);
            }
        }
    }

    Ok(())
}

/// Map crossterm events to [`Msg`]s. Returns `None` for events the
/// v1 shell ignores. Pulled out so tests don't need a terminal.
///
/// `viewport_height` is the rendered height of the row tree in
/// rows, used to size PageUp/PageDown jumps. Pass 1 if unknown.
fn translate(event: Event, viewport_height: u16) -> Option<Msg> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Msg::Quit),
            (_, KeyCode::Char('q')) => Some(Msg::Quit),
            (_, KeyCode::Char('j')) | (_, KeyCode::Down) => Some(Msg::NavDown),
            (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Some(Msg::NavUp),
            (_, KeyCode::PageDown) => Some(Msg::PageDown(viewport_height)),
            (_, KeyCode::PageUp) => Some(Msg::PageUp(viewport_height)),
            (_, KeyCode::Home) | (_, KeyCode::Char('g')) => Some(Msg::Home),
            (_, KeyCode::End) | (_, KeyCode::Char('G')) => Some(Msg::End),
            (_, KeyCode::Enter) => Some(Msg::ToggleExpand),
            (_, KeyCode::Tab) => Some(Msg::CycleFocus),
            (_, KeyCode::Char('J')) => Some(Msg::ScrollPreviewDown),
            (_, KeyCode::Char('K')) => Some(Msg::ScrollPreviewUp),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn press(code: KeyCode, mods: KeyModifiers) -> Event {
        let mut key = KeyEvent::new(code, mods);
        key.kind = KeyEventKind::Press;
        Event::Key(key)
    }

    #[test]
    fn translate_q_quits() {
        assert_eq!(
            translate(press(KeyCode::Char('q'), KeyModifiers::NONE), 24),
            Some(Msg::Quit)
        );
    }

    #[test]
    fn translate_ctrl_c_quits() {
        assert_eq!(
            translate(press(KeyCode::Char('c'), KeyModifiers::CONTROL), 24),
            Some(Msg::Quit)
        );
    }

    #[test]
    fn translate_maps_navigation_keys() {
        assert_eq!(
            translate(press(KeyCode::Char('j'), KeyModifiers::NONE), 24),
            Some(Msg::NavDown)
        );
        assert_eq!(
            translate(press(KeyCode::Char('k'), KeyModifiers::NONE), 24),
            Some(Msg::NavUp)
        );
        assert_eq!(
            translate(press(KeyCode::Down, KeyModifiers::NONE), 24),
            Some(Msg::NavDown)
        );
        assert_eq!(
            translate(press(KeyCode::Up, KeyModifiers::NONE), 24),
            Some(Msg::NavUp)
        );
        assert_eq!(
            translate(press(KeyCode::Enter, KeyModifiers::NONE), 24),
            Some(Msg::ToggleExpand)
        );
        assert_eq!(
            translate(press(KeyCode::Tab, KeyModifiers::NONE), 24),
            Some(Msg::CycleFocus)
        );
        assert_eq!(
            translate(press(KeyCode::Char('g'), KeyModifiers::NONE), 24),
            Some(Msg::Home)
        );
        assert_eq!(
            translate(press(KeyCode::Char('G'), KeyModifiers::NONE), 24),
            Some(Msg::End)
        );
        assert_eq!(
            translate(press(KeyCode::PageDown, KeyModifiers::NONE), 20),
            Some(Msg::PageDown(20))
        );
        assert_eq!(
            translate(press(KeyCode::PageUp, KeyModifiers::NONE), 20),
            Some(Msg::PageUp(20))
        );
        assert_eq!(
            translate(press(KeyCode::Char('J'), KeyModifiers::NONE), 24),
            Some(Msg::ScrollPreviewDown)
        );
        assert_eq!(
            translate(press(KeyCode::Char('K'), KeyModifiers::NONE), 24),
            Some(Msg::ScrollPreviewUp)
        );
    }

    #[test]
    fn translate_ignores_unbound_keys() {
        assert_eq!(
            translate(press(KeyCode::Char('z'), KeyModifiers::NONE), 24),
            None
        );
        assert_eq!(
            translate(press(KeyCode::Char('a'), KeyModifiers::CONTROL), 24),
            None
        );
    }

    #[test]
    fn translate_ignores_release_kind_keys() {
        let mut key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert_eq!(translate(Event::Key(key), 24), None);
    }
}
