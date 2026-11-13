//! Scroll: stepping a cursor, keeping it in view, clamping the window,
//! drawing the scrollbar gutter, and counting wrapped rows.
//!
//! This is a layer 1 foundation (house rules 2 and 12). A list, a tree, a
//! property grid, or a pager shows a window onto rows that may not fit:
//! the cursor moves through the rows, the window follows it, and a
//! scrollbar says where the window is. Each of those has one rule here,
//! so every scrolled surface moves, stops, and looks the same.
//!
//! - **Step**: [`wrap_step`] moves a cursor through a short choice list
//!   (a menu, a picker, a checklist), which wraps at the ends;
//!   [`clamp_step`] moves it through scrollable content (a tree, a
//!   property list, a log), which stops at them (house rule 2).
//! - **Follow**: [`follow_offset`] scrolls only as far as it must to show
//!   the cursor's rows ("minimal motion"): no centering, no margin, and
//!   no movement while the cursor stays inside the window.
//! - **Window**: [`ScrollWindow`] is an offset into some rows, shown a
//!   viewport at a time. Every constructor and step clamps it, so the
//!   offset never passes the last full page and no blank rows hang below
//!   the content while rows above are scrolled away (house rule 12).
//! - **Gutter**: [`ScrollGutter`] carves a 1-column scrollbar gutter from
//!   the right edge of a body only when its rows overflow ("fade on
//!   fit") and draws the one scrollbar look.
//! - **Rows**: [`WrappedRows`] wraps lines with
//!   [`text::wrap_line`](crate::text::wrap_line) and remembers the rows
//!   each line took, so a scroll range, a scrollbar's length, and a
//!   wrapped cursor's span are counted from the rows that are drawn.
//! - **Region**: [`ScrollRegion`] is a window with no cursor whose offset
//!   counts from an [`Anchor`]: the top, the bottom (a body that shows the
//!   newest rows keeps its distance from the end as rows arrive), or a row
//!   the content marks (a match it opened on). It steps, clamps, draws
//!   lines or rows the host cut, keeps a banner above them, and draws the
//!   scrollbar; [`rows_under`] is the room a banner leaves.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use ratatui::widgets::{Paragraph, Widget};
//! use tuikit::scroll::{ScrollGutter, ScrollWindow, WrappedRows, clamp_step};
//!
//! let lines: Vec<Line> = (1..=30).map(|n| Line::from(format!("item {n}"))).collect();
//! let area = Rect::new(0, 0, 20, 8);
//!
//! // The cursor stops at the ends of scrollable content.
//! let cursor = clamp_step(0, lines.len(), 12);
//! assert_eq!(cursor, 12);
//!
//! // Wrap, carve the gutter, and follow the cursor from the last frame's window.
//! let rows = WrappedRows::fit(&lines, usize::from(area.width), usize::from(area.height));
//! let gutter = ScrollGutter::layout(area, rows.len());
//! let mut window = ScrollWindow::default();
//! window.resize(usize::from(area.height), rows.len());
//! window.follow(rows.rows_of(cursor));
//! assert_eq!(window.visible(), 5..13);
//!
//! // Draw the rows and the bar; keep `window` for the next frame.
//! let mut buf = Buffer::empty(area);
//! Paragraph::new(rows.into_rows())
//!     .scroll(window.paragraph_scroll())
//!     .render(gutter.content(), &mut buf);
//! gutter.render(&window, &mut buf);
//! ```
//!
//! # Who keeps the offset
//!
//! A control that keeps no offset, such as a short list redrawn from its
//! cursor alone, passes 0 as the previous offset: its window stays at the
//! top until the cursor passes the first screen, then keeps the cursor on
//! the last line. A control that keeps one stores a [`ScrollWindow`]. Each
//! frame it resizes the window to the area it measured, follows the
//! cursor, draws with the window's offset, and returns the window to its
//! owner as a value, which keeps it for the next key and the next frame.
//! The window is a plain `Copy` value: drawing it needs no message sent
//! from inside the draw and no state shared behind interior mutability.
//! A page key moves the cursor by the window's [`page`](ScrollWindow::page),
//! the rows the frame really showed.

mod gutter;
mod region;
mod rows;
mod step;
mod window;

pub use gutter::ScrollGutter;
pub use region::{Anchor, ScrollRegion, rows_under};
pub use rows::WrappedRows;
pub use step::{clamp_step, wrap_step};
pub use window::{ScrollWindow, follow_offset};

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
