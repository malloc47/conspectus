//! Mux action menu overlay (ADR 0096).
//!
//! `m`-keyed menu that fronts the mux-specific verbs. Discharges the
//! ADR 0095 follow-up ("when a second bare-mux-shape verb lands, ship
//! the `m`-keyed Mux action menu"): the second verb is
//! `mux launch` (ADR 0096), and this overlay is that menu.
//!
//! Entries:
//! - `New tmux session` — commits [`Msg::OpenNewMuxForm`], opening
//!   the ADR 0095 bare-mux form.
//! - `Launch harness in new mux…` — commits [`Msg::OpenMuxLaunchForm`],
//!   opening the ADR 0096 ephemeral harness launch form.
//!
//! Attach (`a` / `Enter`) and rename (`R`) stay on their polymorphic
//! global bindings; a follow-up story may surface them here for
//! discoverability once operator muscle memory settles.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tuikit::frame::{Lines, ModalFrame, ModalSize};
use tuikit::selection::{self, Cursor, Ends};

use crate::tui::Msg;
use crate::tui::theme::Theme;
use tuikit::overlay::Outcome;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MuxMenuChoice {
    NewBareMux,
    LaunchHarnessMux,
}

impl MuxMenuChoice {
    fn label(self) -> &'static str {
        match self {
            Self::NewBareMux => "New tmux session (bare shell)",
            Self::LaunchHarnessMux => "Launch harness in new mux…",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::NewBareMux => "shell only, no harness",
            Self::LaunchHarnessMux => "spawn harness; optionally pin it",
        }
    }
}

const MENU_ENTRIES: [MuxMenuChoice; 2] =
    [MuxMenuChoice::NewBareMux, MuxMenuChoice::LaunchHarnessMux];

#[derive(Debug, Clone, Default)]
pub struct MuxMenuState {
    cursor: usize,
}

impl MuxMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn entries(&self) -> &'static [MuxMenuChoice] {
        &MENU_ENTRIES
    }

    fn selected(&self) -> MuxMenuChoice {
        MENU_ENTRIES[self.cursor.min(MENU_ENTRIES.len() - 1)]
    }
}

impl MuxMenuState {
    /// One key: move, pick a verb, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        use tuikit::keys::{MENU_KEYS, MenuKey, resolve};
        match resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
            Some(MenuKey::Up) => {
                self.cursor = Ends::Wrap.step(self.cursor, MENU_ENTRIES.len(), -1);
                Outcome::Consumed
            }
            Some(MenuKey::Down) => {
                self.cursor = Ends::Wrap.step(self.cursor, MENU_ENTRIES.len(), 1);
                Outcome::Consumed
            }
            Some(MenuKey::Pick) => match self.selected() {
                MuxMenuChoice::NewBareMux => Outcome::Commit(Msg::OpenNewMuxForm),
                MuxMenuChoice::LaunchHarnessMux => Outcome::Commit(Msg::OpenMuxLaunchForm),
            },
            None => Outcome::Consumed,
        }
    }
}

pub struct MuxMenuWidget<'a> {
    state: &'a MuxMenuState,
    theme: &'a Theme,
}

impl<'a> MuxMenuWidget<'a> {
    pub fn new(state: &'a MuxMenuState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for MuxMenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::styled("Mux actions", Style::default().add_modifier(Modifier::BOLD)),
            Line::from(""),
        ];
        for (idx, entry) in MENU_ENTRIES.iter().enumerate() {
            let cursor = Cursor::of(idx == self.state.cursor, true);
            lines.push(selection::row(
                [Span::raw(entry.label())],
                cursor,
                self.theme,
            ));
            lines.push(Line::from(format!("     {}", entry.subtitle())));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(tuikit::keys::footer(tuikit::keys::MENU_KEYS)));

        ModalFrame::new(self.theme, ModalSize::MENU)
            .title("Mux")
            .render(area, buf, Lines::new(lines));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyCode;
    use tuikit::testkit::keys::press;

    #[test]
    fn enter_on_first_entry_opens_new_mux_form() {
        let mut state = MuxMenuState::new();
        match state.handle_key(press(KeyCode::Enter)) {
            Outcome::Commit(msg) => match msg {
                Msg::OpenNewMuxForm => {}
                other => panic!("expected OpenNewMuxForm, got {other:?}"),
            },
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn enter_on_second_entry_opens_launch_form() {
        let mut state = MuxMenuState::new();
        state.handle_key(press(KeyCode::Down));
        match state.handle_key(press(KeyCode::Enter)) {
            Outcome::Commit(msg) => match msg {
                Msg::OpenMuxLaunchForm => {}
                other => panic!("expected OpenMuxLaunchForm, got {other:?}"),
            },
            other => panic!("expected Commit, got {other:?}"),
        }
    }

    #[test]
    fn the_cursor_wraps_at_both_ends() {
        let mut state = MuxMenuState::new();
        state.handle_key(press(KeyCode::Up));
        assert_eq!(state.cursor(), MENU_ENTRIES.len() - 1, "up from the first");
        state.handle_key(press(KeyCode::Down));
        assert_eq!(state.cursor(), 0, "down from the last");
    }

    #[test]
    fn the_cursor_entry_shows_the_marker_and_the_highlight() {
        let theme = Theme::default();
        let mut state = MuxMenuState::new();
        state.handle_key(press(KeyCode::Down));
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        MuxMenuWidget::new(&state, &theme).render(area, &mut buf);
        let marked: Vec<u16> = (0..area.height)
            .filter(|y| {
                (0..area.width).any(|x| buf[(x, *y)].symbol() == tuikit::glyphs::marks::POINTER)
            })
            .collect();
        assert_eq!(marked.len(), 1, "one marked row");
        let y = marked[0];
        let row: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
        assert!(row.contains(MENU_ENTRIES[1].label()), "{row}");
        let x = (0..area.width)
            .find(|x| buf[(*x, y)].symbol() == tuikit::glyphs::marks::POINTER)
            .unwrap_or(0);
        assert!(buf[(x + 2, y)].modifier.contains(theme.selection_active));
    }
}
