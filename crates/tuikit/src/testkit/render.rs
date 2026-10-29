//! The one-frame driver: drawing a frame off screen, through the host's
//! own draw callback, into a buffer of cells and styles.
//!
//! A test or a dev tool that wants to see a screen draws it here instead
//! of in a terminal: [`FrameDriver`] holds an off-screen terminal of a
//! fixed size, [`FrameDriver::draw`] runs the host's callback against a
//! [`Frame`] of it exactly as a live loop's draw does, and the buffer it
//! returns is what the terminal would show. [`FrameDriver::drive`] presses
//! a run of keys with a frame drawn before each, so a key that acts on
//! what was drawn (a page key moving by the rows a list showed) does what
//! it does live. [`render`] and [`render_widget`] are the one-call forms.
//!
//! What to do with the buffer is the [`buffer`](super::buffer),
//! [`ansi`](super::ansi), and [`golden`](super::golden) modules' business.
//!
//! ```
//! use ratatui::crossterm::event::KeyEvent;
//! use ratatui::widgets::Paragraph;
//! use tuikit::testkit::{buffer, keys, render::FrameDriver};
//!
//! // A counter the host draws and a key advances.
//! let mut count = 0;
//! let mut driver = FrameDriver::new(10, 1);
//! driver.drive(
//!     &mut count,
//!     keys::script("jj"),
//!     |count, frame| frame.render_widget(Paragraph::new(count.to_string()), frame.area()),
//!     |count, _key: KeyEvent| *count += 1,
//! );
//! let shown = driver.draw(|frame| frame.render_widget(Paragraph::new("2 presses"), frame.area()));
//! assert_eq!(buffer::text(shown), "2 presses \n");
//! assert_eq!(count, 2);
//! ```

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::{Frame, Terminal};

/// An off-screen terminal of a fixed size that draws through the host's
/// callback.
pub struct FrameDriver {
    terminal: Terminal<TestBackend>,
}

impl FrameDriver {
    /// A driver whose frames are `width` by `height` cells.
    pub fn new(width: u16, height: u16) -> Self {
        let Ok(terminal) = Terminal::new(TestBackend::new(width, height));
        Self { terminal }
    }

    /// The whole frame's area.
    pub fn area(&self) -> Rect {
        let size = self.terminal.backend().buffer().area;
        Rect::new(0, 0, size.width, size.height)
    }

    /// Draws one frame with `draw`, as a live loop's draw would, and
    /// returns what it drew.
    pub fn draw(&mut self, draw: impl FnOnce(&mut Frame<'_>)) -> &Buffer {
        let Ok(_) = self.terminal.draw(draw);
        self.terminal.backend().buffer()
    }

    /// What the last frame drew; empty cells before the first.
    pub fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    /// Presses `keys` in order against the host's `state`, drawing a frame
    /// with `draw` before each one, as a live loop draws before it reads
    /// the next key. `press` does what the host does with a key. The frame
    /// after the last key is not drawn: [`draw`](Self::draw) it, perhaps
    /// after settling what the keys started.
    pub fn drive<S>(
        &mut self,
        state: &mut S,
        keys: impl IntoIterator<Item = KeyEvent>,
        mut draw: impl FnMut(&mut S, &mut Frame<'_>),
        mut press: impl FnMut(&mut S, KeyEvent),
    ) {
        for key in keys {
            let Ok(_) = self.terminal.draw(|frame| draw(state, frame));
            press(state, key);
        }
    }
}

/// One frame of `width` by `height` cells drawn with `draw`.
pub fn render(width: u16, height: u16, draw: impl FnOnce(&mut Frame<'_>)) -> Buffer {
    let mut driver = FrameDriver::new(width, height);
    driver.draw(draw).clone()
}

/// `widget` drawn over a whole frame of `width` by `height` cells.
pub fn render_widget(widget: impl Widget, width: u16, height: u16) -> Buffer {
    render(width, height, |frame| {
        frame.render_widget(widget, frame.area());
    })
}
