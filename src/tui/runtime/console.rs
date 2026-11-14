//! What the event loop needs from the terminal: the input that is
//! waiting, a place to draw, and the time.
//!
//! The loop reads the terminal only through [`Console`], so a test can
//! script the events it receives, count the frames it draws, and run
//! its clock by hand (`scripted`).

use std::io;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

use crate::tui::app::App;

#[cfg(test)]
pub(super) mod scripted;

/// Input, output, and a clock for the event loop.
pub(super) trait Console {
    /// Wait up to `timeout` for an event; `true` when one can be
    /// [`read`](Self::read). A zero timeout asks without waiting.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// The next event. Call it only after [`poll`](Self::poll) said one
    /// is waiting.
    fn read(&mut self) -> io::Result<Event>;

    /// Draw one frame of `app`.
    fn draw(&mut self, app: &mut App) -> anyhow::Result<()>;

    /// The time the loop's decisions are made against.
    fn now(&self) -> Instant;
}

impl Console for DefaultTerminal {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }

    fn draw(&mut self, app: &mut App) -> anyhow::Result<()> {
        super::draw_frame(app, self)
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}
