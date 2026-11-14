//! A console for the loop's tests: the events a script lists arrive at
//! the times it gives, time passes only when the loop waits or a test
//! spends it, and every frame the loop draws is kept as text.
//!
//! The clock is virtual, so a test that needs "no more than 100 ms
//! between frames" or "a load within 150 ms of the last key" reads the
//! frames' times and sleeps for none of them.

use std::cell::Cell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::Console;
use crate::tui::app::App;

/// How long a run lasts when the script gives no end, in virtual time.
const LIMIT: Duration = Duration::from_secs(120);

/// One frame the loop drew.
pub(in crate::tui::runtime) struct Frame {
    /// When the frame began, on the script's clock.
    pub at: Duration,
    /// The screen, one string per row, trailing blanks trimmed.
    pub rows: Vec<String>,
}

impl Frame {
    /// The whole screen as one text.
    pub fn text(&self) -> String {
        self.rows.join("\n")
    }
}

/// A scripted terminal: [`Console`] over a list of timed events and a
/// [`TestBackend`].
pub(in crate::tui::runtime) struct Scripted {
    start: Instant,
    /// Shared with whatever a test hands the time to, such as a fake
    /// preview loader that stamps its loads.
    clock: Rc<Cell<Duration>>,
    end: Duration,
    queue: VecDeque<(Duration, Event)>,
    terminal: Terminal<TestBackend>,
    frame_cost: Duration,
    frames: Vec<Frame>,
}

impl Scripted {
    /// A screen of `width` by `height` cells with nothing scripted.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            start: Instant::now(),
            clock: Rc::new(Cell::new(Duration::ZERO)),
            end: LIMIT,
            queue: VecDeque::new(),
            terminal: Terminal::new(TestBackend::new(width, height)).expect("test terminal"),
            frame_cost: Duration::ZERO,
            frames: Vec::new(),
        }
    }

    /// Drawing a frame takes `cost` of the script's time, as it does on
    /// a real terminal.
    pub fn frame_cost(mut self, cost: Duration) -> Self {
        self.frame_cost = cost;
        self
    }

    /// `event` arrives `at` after the run starts. Events at one time
    /// arrive in the order they were added.
    pub fn event_at(mut self, at: Duration, event: Event) -> Self {
        let place = self.queue.partition_point(|(when, _)| *when <= at);
        self.queue.insert(place, (at, event));
        self
    }

    /// A key press of `code` arrives at `at`.
    pub fn key_at(self, at: Duration, code: KeyCode) -> Self {
        self.event_at(at, key(code))
    }

    /// One key press per code, all arriving at `at`: a burst the loop
    /// finds already queued.
    pub fn keys_at(mut self, at: Duration, codes: impl IntoIterator<Item = KeyCode>) -> Self {
        for code in codes {
            self = self.key_at(at, code);
        }
        self
    }

    /// The run ends `at` after it starts: the first wait for input at or
    /// after that time fails with [`io::ErrorKind::UnexpectedEof`].
    pub fn ends_at(mut self, at: Duration) -> Self {
        self.end = at;
        self
    }

    /// Time passes by `cost`, as when the loop handles an event.
    pub fn spend(&mut self, cost: Duration) {
        self.clock.set(self.clock.get() + cost);
    }

    /// Where the script's clock stands.
    pub fn clock(&self) -> Duration {
        self.clock.get()
    }

    /// The clock, to read from somewhere the console is not passed to,
    /// such as a fake that stamps what it is asked to do.
    pub fn clock_handle(&self) -> Rc<Cell<Duration>> {
        Rc::clone(&self.clock)
    }

    /// The frames drawn so far, oldest first.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Events not yet read.
    pub fn unread(&self) -> usize {
        self.queue.len()
    }
}

/// A key press of `code` with no modifiers.
pub(in crate::tui::runtime) fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

impl Console for Scripted {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        let now = self.clock();
        if now >= self.end {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the script ended",
            ));
        }
        let wake = (now + timeout).min(self.end);
        match self.queue.front() {
            Some(&(at, _)) if at <= now => Ok(true),
            Some(&(at, _)) if at <= wake => {
                self.clock.set(at);
                Ok(true)
            }
            _ => {
                self.clock.set(wake);
                Ok(false)
            }
        }
    }

    fn read(&mut self) -> io::Result<Event> {
        match self.queue.pop_front() {
            Some((at, event)) if at <= self.clock() => Ok(event),
            _ => panic!("the loop read an event that had not arrived"),
        }
    }

    fn draw(&mut self, app: &mut App) -> anyhow::Result<()> {
        app.prepare_toast_for_render();
        self.terminal
            .draw(|frame| crate::tui::runtime::draw_screen(app, frame))?;
        let buffer = self.terminal.backend().buffer();
        let rows = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        self.frames.push(Frame {
            at: self.clock(),
            rows,
        });
        self.spend(self.frame_cost);
        Ok(())
    }

    fn now(&self) -> Instant {
        self.start + self.clock()
    }
}
