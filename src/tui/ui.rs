//! Ratatui draw.
//!
//! v1 shell (P8-003) renders a placeholder frame so the runtime has
//! something to display while the real row tree, right panel, and
//! status bar arrive in later stories. P8-007 replaces this module
//! with the locked two-panel layout from
//! `docs/tui-sessions-mockup.md`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::app::App;

/// Render one frame. Pure with respect to `app`; the runtime calls
/// this on every loop iteration.
pub fn draw(_app: &App, frame: &mut Frame<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let title = "Conspectus · sessions ─ shell only";
    let body = Paragraph::new(
        "Interactive TUI shell is live.\n\
         Discovery, navigation, and the two-panel render arrive in later P8 stories.\n\
         \n\
         Press q or Ctrl-C to exit.",
    )
    .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(body, chunks[0]);

    let status =
        Paragraph::new("q quit · Ctrl-C quit").style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(status, chunks[1]);
}
