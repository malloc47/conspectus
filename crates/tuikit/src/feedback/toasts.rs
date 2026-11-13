//! Toasts: short reports that float at the bottom of the screen and go.

use std::collections::VecDeque;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::widgets::{Block, Borders, Clear, Padding, Widget};

use super::Feedback;
use crate::text;
use crate::theme::Theme;

/// The toasts on screen, oldest first, each until its own time is up.
///
/// A toast is a [`Feedback`] drawn in a box at the bottom of the screen,
/// centered across it: a border of quadrant blocks in the severity's
/// color, one cell of padding inside it, and the text in the theme's
/// `toast_text` color on its `surface`, word-wrapped at
/// [`Toasts::MAX_TEXT_WIDTH`] cells (or the screen less the box's
/// edges). The box is as wide as the text's widest line and as tall as
/// its rows; it covers what is under it and is drawn after everything
/// else.
///
/// There are two ways to post one. [`show`](Toasts::show) replaces every
/// toast on screen, so the newest report is the only one, the usual
/// choice when reports answer the user's last key. [`push`](Toasts::push)
/// stacks it above the others, dropping the oldest once
/// [`capacity`](Toasts::with_capacity) are shown. The newest is drawn at
/// the bottom edge and each older one above it.
///
/// Time comes from the host: posting takes the moment it happens and
/// [`tick`](Toasts::tick) the moment to check against, so a test runs
/// without waiting and a frame draws what the last tick left.
#[derive(Clone, Debug)]
pub struct Toasts {
    shown: VecDeque<Shown>,
    capacity: usize,
}

/// A toast on screen and the moment it goes; `None` when its lifetime
/// runs past what the clock can count, so it stays until cleared.
#[derive(Clone, Debug)]
struct Shown {
    feedback: Feedback,
    until: Option<Instant>,
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}

impl Toasts {
    /// How many toasts [`Toasts::new`] keeps on screen at once.
    pub const DEFAULT_CAPACITY: usize = 4;

    /// The widest a toast's text runs before it wraps, in cells. With the
    /// border and the padding a toast is at most four cells wider.
    pub const MAX_TEXT_WIDTH: u16 = 46;

    /// No toasts, keeping at most [`Toasts::DEFAULT_CAPACITY`].
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// No toasts, keeping at most `capacity` (at least one) on screen.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            shown: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Show `feedback` alone, from `now` until its lifetime is up: every
    /// toast on screen goes.
    pub fn show(&mut self, feedback: Feedback, now: Instant) {
        self.shown.clear();
        self.push(feedback, now);
    }

    /// Show `feedback` above the toasts on screen, from `now` until its
    /// lifetime is up. When the stack is full the oldest goes.
    pub fn push(&mut self, feedback: Feedback, now: Instant) {
        if self.shown.len() == self.capacity {
            self.shown.pop_front();
        }
        let until = now.checked_add(feedback.ttl);
        self.shown.push_back(Shown { feedback, until });
    }

    /// Retire every toast whose time is up at `now`. Returns whether any
    /// went, so a host knows the screen changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let before = self.shown.len();
        self.shown
            .retain(|shown| shown.until.is_none_or(|until| now < until));
        self.shown.len() < before
    }

    /// Take every toast off the screen.
    pub fn clear(&mut self) {
        self.shown.clear();
    }

    /// Whether no toast is on screen.
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// How many toasts are on screen.
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    /// The toasts on screen, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Feedback> {
        self.shown.iter().map(|shown| &shown.feedback)
    }

    /// Where each toast is drawn inside `area`, oldest first: the newest
    /// at the bottom edge and each older one stacked above it. A toast
    /// with no room left above the newer ones is not drawn, so it has no
    /// place here.
    pub fn areas(&self, area: Rect) -> Vec<Rect> {
        self.placed(area)
            .into_iter()
            .map(|(_, layout)| layout.rect)
            .collect()
    }

    /// Draw the toasts over whatever `area` already holds.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        for (feedback, layout) in self.placed(area) {
            draw(feedback, &layout, buf, theme);
        }
    }

    /// The toasts that fit in `area`, oldest first, each with its place:
    /// the newest at the bottom and each older one in the room left
    /// above.
    fn placed(&self, area: Rect) -> Vec<(&Feedback, Layout)> {
        let mut free = area;
        let mut placed = Vec::new();
        for shown in self.shown.iter().rev() {
            if free.height == 0 || free.width == 0 {
                break;
            }
            let layout = Layout::of(&shown.feedback.text, free);
            free.height -= layout.rect.height;
            placed.push((&shown.feedback, layout));
        }
        placed.reverse();
        placed
    }
}

/// Cells a toast's border and padding take across and down.
const CHROME: u16 = 4;

/// One toast placed at the bottom of a free area: its box and its rows.
struct Layout {
    rect: Rect,
    rows: Vec<String>,
}

impl Layout {
    /// Place a toast saying `message` at the bottom of `free`, centered
    /// across it (an odd cell left over goes before it).
    fn of(message: &str, free: Rect) -> Self {
        let room = free.width.saturating_sub(CHROME).max(1);
        let widest = message.lines().map(text::width).max().unwrap_or(0).max(1);
        let text_width = u16::try_from(widest)
            .unwrap_or(u16::MAX)
            .min(Toasts::MAX_TEXT_WIDTH)
            .min(room);
        let rows: Vec<String> = text::wrap(message, usize::from(text_width))
            .into_iter()
            .map(|row| row.trim_end().to_string())
            .collect();
        let tall = u16::try_from(rows.len()).unwrap_or(u16::MAX);
        let width = text_width.saturating_add(CHROME).min(free.width);
        let height = tall.saturating_add(CHROME).min(free.height);
        let rect = Rect {
            x: free.x + (free.width - width).div_ceil(2),
            y: free.y + free.height - height,
            width,
            height,
        };
        Self { rect, rows }
    }
}

/// Draw one toast: clear its box, frame it in the severity's color, and
/// write its rows inside the padding.
fn draw(feedback: &Feedback, layout: &Layout, buf: &mut Buffer, theme: &Theme) {
    let surface = Style::default().bg(theme.surface);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::QUADRANT_OUTSIDE)
        .padding(Padding::uniform(1))
        .style(surface)
        .border_style(feedback.severity.style(theme).bg(theme.surface));
    let inner = block.inner(layout.rect);
    Clear.render(layout.rect, buf);
    block.render(layout.rect, buf);
    let text_style = Style::default().fg(theme.toast_text).bg(theme.surface);
    for (row, y) in layout.rows.iter().zip(inner.top()..inner.bottom()) {
        buf.set_stringn(inner.x, y, row, usize::from(inner.width), text_style);
    }
}

#[cfg(test)]
#[path = "toasts_tests.rs"]
mod tests;
