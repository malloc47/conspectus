//! Reading a drawn frame: its text, where a piece of text sits and how it
//! is styled, and the frame of a modal drawn over a screen.
//!
//! Positions are cells: `x` counts the cells from the buffer's left edge,
//! as the buffer indexes them, whatever the text in them. A row's text is
//! its cells' symbols in order, so a two-cell character is followed by the
//! cell it covers.
//!
//! - **Text**: [`text`] (every row, each ended by a line break), [`rows`],
//!   and [`region_rows`].
//! - **Token columns**: [`tokens`] splits a row's text into the
//!   whitespace-separated words with the columns each starts and ends at,
//!   for checks that columns line up.
//! - **Probes**: [`find`] and [`find_all`] place a piece of text,
//!   [`cell_at`] and [`style_at`] read the cell it starts in, and
//!   [`count_cells`] counts the cells of a region that match a test.
//! - **Modal frames**: [`frame_rect`] finds the rectangle of the frame a
//!   modal drew by its corners among the cells that differ from the same
//!   screen without it, [`inner_rows`] reads the text inside a frame, and
//!   [`restyled_rows`] lists the rows whose styles differ between two
//!   frames (the rows a moving selection repaints).
//!
//! ```
//! use ratatui::layout::{Position, Rect};
//! use ratatui::widgets::{Block, Paragraph};
//! use tuikit::testkit::{buffer, render};
//!
//! let bare = render::render_widget(Paragraph::new("under"), 12, 5);
//! let framed = render::render(12, 5, |frame| {
//!     frame.render_widget(Paragraph::new("under"), frame.area());
//!     frame.render_widget(
//!         Paragraph::new("hi there").block(Block::bordered()),
//!         Rect::new(1, 1, 10, 3),
//!     );
//! });
//! let rect = buffer::frame_rect(&framed, &bare).expect("a frame");
//! assert_eq!(rect, Rect::new(1, 1, 10, 3));
//! assert_eq!(buffer::inner_rows(&framed, rect), ["hi there"]);
//! assert_eq!(buffer::find(&framed, "there"), Some(Position::new(5, 2)));
//! assert_eq!(buffer::tokens("  one  two")[1].start, 7);
//! ```

use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::symbols::{line, scrollbar};

/// Every row of `buffer`, each ended by a line break.
pub fn text(buffer: &Buffer) -> String {
    let mut out = String::new();
    for row in rows(buffer) {
        out.push_str(&row);
        out.push('\n');
    }
    out
}

/// The text of each row of `buffer`.
pub fn rows(buffer: &Buffer) -> Vec<String> {
    region_rows(buffer, buffer.area)
}

/// The text of each row of `region` (clipped to `buffer`).
pub fn region_rows(buffer: &Buffer, region: Rect) -> Vec<String> {
    let region = region.intersection(buffer.area);
    (region.y..region.bottom())
        .map(|y| {
            (region.x..region.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

/// One whitespace-separated word of a row and the columns it covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The column (in characters of the row's text) the word starts at.
    pub start: usize,
    /// The column just past its last character.
    pub end: usize,
    /// The word.
    pub text: String,
}

/// The whitespace-separated words of `line` with their columns.
pub fn tokens(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut start = None;
    let mut word = String::new();
    let mut column = 0;
    for ch in line.chars() {
        if ch.is_whitespace() {
            if let Some(from) = start.take() {
                out.push(Token {
                    start: from,
                    end: column,
                    text: std::mem::take(&mut word),
                });
            }
        } else {
            start.get_or_insert(column);
            word.push(ch);
        }
        column += 1;
    }
    if let Some(from) = start {
        out.push(Token {
            start: from,
            end: column,
            text: word,
        });
    }
    out
}

/// The cell `needle` starts in, at its first appearance in reading order
/// (row by row, left to right). A needle is found within one row.
pub fn find(buffer: &Buffer, needle: &str) -> Option<Position> {
    find_in(buffer, buffer.area, needle)
}

/// [`find`] inside `region` only.
pub fn find_in(buffer: &Buffer, region: Rect, needle: &str) -> Option<Position> {
    positions(buffer, region, needle).into_iter().next()
}

/// The cell every appearance of `needle` starts in, in reading order.
/// Appearances on one row do not overlap.
pub fn find_all(buffer: &Buffer, needle: &str) -> Vec<Position> {
    positions(buffer, buffer.area, needle)
}

fn positions(buffer: &Buffer, region: Rect, needle: &str) -> Vec<Position> {
    let region = region.intersection(buffer.area);
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    for y in region.y..region.bottom() {
        // The row's text and, for each cell, the byte its symbol starts at.
        let mut row = String::new();
        let mut starts = Vec::new();
        for x in region.x..region.right() {
            starts.push((row.len(), x));
            row.push_str(buffer[(x, y)].symbol());
        }
        for (byte, _) in row.match_indices(needle) {
            if let Some(&(_, x)) = starts.iter().rev().find(|(start, _)| *start <= byte) {
                out.push(Position::new(x, y));
            }
        }
    }
    out
}

/// The cell `needle` starts in, at its first appearance.
pub fn cell_at<'a>(buffer: &'a Buffer, needle: &str) -> Option<&'a Cell> {
    find(buffer, needle).map(|at| &buffer[(at.x, at.y)])
}

/// The style of the cell `needle` starts in, at its first appearance.
pub fn style_at(buffer: &Buffer, needle: &str) -> Option<Style> {
    cell_at(buffer, needle).map(Cell::style)
}

/// How many cells of `region` pass `test`.
pub fn count_cells(buffer: &Buffer, region: Rect, test: impl Fn(&Cell) -> bool) -> usize {
    let region = region.intersection(buffer.area);
    (region.y..region.bottom())
        .flat_map(|y| (region.x..region.right()).map(move |x| (x, y)))
        .filter(|&(x, y)| test(&buffer[(x, y)]))
        .count()
}

/// The line sets a frame's border can be drawn in.
const BORDERS: [line::Set; 4] = [line::NORMAL, line::ROUNDED, line::DOUBLE, line::THICK];

/// Whether `symbol` is the corner of a frame's border.
pub fn is_corner(symbol: &str) -> bool {
    BORDERS.iter().any(|set| {
        [
            set.top_left,
            set.top_right,
            set.bottom_left,
            set.bottom_right,
        ]
        .contains(&symbol)
    })
}

/// Whether `symbol` is the side of a frame's border.
pub fn is_side(symbol: &str) -> bool {
    BORDERS.iter().any(|set| set.vertical == symbol)
}

/// The rectangle of the frame a modal drew: the span of the border corners
/// among the cells of `with` (the screen with the modal) that differ from
/// `without` (the same screen without it). `None` when no corner differs,
/// so nothing framed was drawn.
///
/// A corner the modal draws over an identical corner (a pane's border in
/// the same color, under a modal that reaches the pane's edges) does not
/// differ. When the corners that differ are only one side's pair (the top
/// two, or the bottom two, or the left or right pair), the rectangle is
/// completed from the corners `with` draws across from them.
pub fn frame_rect(with: &Buffer, without: &Buffer) -> Option<Rect> {
    let area = with.area;
    let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0, 0);
    let mut found = false;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &with[(x, y)];
            if without.area.contains(Position::new(x, y))
                && *cell != without[(x, y)]
                && is_corner(cell.symbol())
            {
                found = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    found.then(|| complete_frame(with, Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)))
}

/// The frame `with` draws exactly over a frame of the same look that
/// `without` has: the smallest frame `with` draws around every cell that
/// differs between the two.
///
/// A modal drawn exactly over another (a step that covers the entry it is
/// a step of, at the same size) changes no corner, so [`frame_rect`]
/// finds nothing; what changed inside it (its body, its title, its keys)
/// still lies within its frame, and no smaller frame holds all of it.
/// `None` when nothing differs, or when no frame `with` draws holds every
/// difference.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::widgets::{Block, Paragraph, Widget};
/// use tuikit::testkit::buffer::{covering_frame, frame_rect};
///
/// let area = Rect::new(0, 0, 20, 6);
/// let framed = |text: &str| {
///     let mut buf = Buffer::empty(area);
///     let frame = Rect::new(2, 1, 14, 4);
///     Paragraph::new(text).block(Block::bordered()).render(frame, &mut buf);
///     buf
/// };
/// let (under, over) = (framed("a menu"), framed("a question"));
/// assert_eq!(frame_rect(&over, &under), None, "no corner differs");
/// assert_eq!(covering_frame(&over, &under), Some(Rect::new(2, 1, 14, 4)));
/// ```
pub fn covering_frame(with: &Buffer, without: &Buffer) -> Option<Rect> {
    let area = with.area.intersection(without.area);
    let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0, 0);
    let mut changed = false;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if with[(x, y)] != without[(x, y)] {
                changed = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if !changed {
        return None;
    }
    let is = |x: u16, y: u16, symbol: &str| with[(x, y)].symbol() == symbol;
    let mut best: Option<Rect> = None;
    for set in BORDERS {
        for top in (area.y..=y0).rev() {
            for left in (area.x..=x0).rev() {
                if !is(left, top, set.top_left) {
                    continue;
                }
                let right = (x1.max(left + 1)..area.right()).find(|&x| is(x, top, set.top_right));
                let bottom =
                    (y1.max(top + 1)..area.bottom()).find(|&y| is(left, y, set.bottom_left));
                let (Some(right), Some(bottom)) = (right, bottom) else {
                    continue;
                };
                if !is(right, bottom, set.bottom_right) {
                    continue;
                }
                let rect = Rect::new(left, top, right - left + 1, bottom - top + 1);
                if best.is_none_or(|smallest| rect.area() < smallest.area()) {
                    best = Some(rect);
                }
            }
        }
    }
    best
}

/// `rect`, the span of a frame's corners that differed, completed to the
/// frame `buffer` draws when it is one row or one column: from the pair
/// of corners in place (the top two, the bottom two, the left two, or the
/// right two), the nearest row or column across that holds the other
/// pair. `rect` as it is otherwise, or when no pair across is found.
fn complete_frame(buffer: &Buffer, rect: Rect) -> Rect {
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (rect.right() - 1, rect.bottom() - 1);
    let at = |x: u16, y: u16, pick: fn(&line::Set<'static>) -> &'static str| {
        let symbol = buffer[(x, y)].symbol();
        BORDERS.iter().any(|set| pick(set) == symbol)
    };
    let top_pair = |y| at(left, y, |set| set.top_left) && at(right, y, |set| set.top_right);
    let bottom_pair =
        |y| at(left, y, |set| set.bottom_left) && at(right, y, |set| set.bottom_right);
    let left_pair = |x| at(x, top, |set| set.top_left) && at(x, bottom, |set| set.bottom_left);
    let right_pair = |x| at(x, top, |set| set.top_right) && at(x, bottom, |set| set.bottom_right);
    let area = buffer.area;
    let rows = |from: u16, to: u16| Rect::new(left, from, right - left + 1, to - from + 1);
    let columns = |from: u16, to: u16| Rect::new(from, top, to - from + 1, bottom - top + 1);
    let completed = if top == bottom && top_pair(top) {
        (top + 1..area.bottom())
            .find(|&y| bottom_pair(y))
            .map(|y| rows(top, y))
    } else if top == bottom && bottom_pair(bottom) {
        (area.y..bottom)
            .rev()
            .find(|&y| top_pair(y))
            .map(|y| rows(y, bottom))
    } else if left == right && left_pair(left) {
        (left + 1..area.right())
            .find(|&x| right_pair(x))
            .map(|x| columns(left, x))
    } else if left == right && right_pair(right) {
        (area.x..right)
            .rev()
            .find(|&x| left_pair(x))
            .map(|x| columns(x, right))
    } else {
        None
    };
    completed.unwrap_or(rect)
}

/// The text of each row inside the frame `rect`, border excluded. A
/// scrollbar cell in the last inner column reads as a space, so a word
/// that fills the last text column is not glued to the bar.
pub fn inner_rows(buffer: &Buffer, rect: Rect) -> Vec<String> {
    let bar = scrollbar::DOUBLE_VERTICAL;
    let parts = [bar.begin, bar.end, bar.thumb, bar.track];
    let (left, right) = (rect.x, rect.right().saturating_sub(1));
    (rect.y + 1..rect.bottom().saturating_sub(1))
        .map(|y| {
            (left + 1..right)
                .map(|x| {
                    let symbol = buffer[(x, y)].symbol();
                    if x + 1 == right && parts.contains(&symbol) {
                        " "
                    } else {
                        symbol
                    }
                })
                .collect()
        })
        .collect()
}

/// The rows whose cell styles differ between `a` and `b` (frames of one
/// size): the rows a moving selection repainted.
pub fn restyled_rows(a: &Buffer, b: &Buffer) -> Vec<u16> {
    let area = a.area.intersection(b.area);
    (area.y..area.bottom())
        .filter(|&y| (area.x..area.right()).any(|x| a[(x, y)].style() != b[(x, y)].style()))
        .collect()
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
