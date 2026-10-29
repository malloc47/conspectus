//! A drawn frame written as ANSI text: what a terminal would show, colors
//! and modifiers kept, for a dev tool that prints a frame or a check that
//! keeps one.
//!
//! [`to_ansi`] walks a region of a buffer row by row and writes each
//! cell's symbol, with a select-graphic-rendition sequence wherever the
//! style changes from the cell before. Every sequence starts with a reset,
//! so it says the whole style rather than a change; a row ends in a reset,
//! so a background never bleeds into the next line of the terminal it is
//! printed in. Colors are written in the shortest form a terminal reads:
//! the sixteen named colors as their own codes, an indexed color as
//! `38;5;n`, and a true color as `38;2;r;g;b` (`48` for backgrounds); a
//! reset color writes nothing. Every modifier a style can carry is
//! written: bold, dim, italic, underlined, slow and rapid blink, reversed,
//! hidden, and crossed out.
//!
//! [`strip_ansi`](crate::text::strip_ansi) takes the sequences out again.
//!
//! ```
//! use ratatui::style::{Modifier, Style};
//! use ratatui::text::Span;
//! use ratatui::widgets::Paragraph;
//! use tuikit::testkit::{ansi, render};
//!
//! let bold = Span::styled("hi", Style::new().add_modifier(Modifier::BOLD));
//! let frame = render::render_widget(Paragraph::new(bold), 3, 1);
//! assert_eq!(
//!     ansi::to_ansi(&frame, frame.area),
//!     "\x1b[0m\x1b[1mhi\x1b[0m \x1b[0m"
//! );
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// The select-graphic-rendition parameter of each modifier, by the
/// modifier's name, in the order a sequence writes them.
const MODIFIER_CODES: &[(&str, u8)] = &[
    ("BOLD", 1),
    ("DIM", 2),
    ("ITALIC", 3),
    ("UNDERLINED", 4),
    ("SLOW_BLINK", 5),
    ("RAPID_BLINK", 6),
    ("REVERSED", 7),
    ("HIDDEN", 8),
    ("CROSSED_OUT", 9),
];

/// `region` of `buffer` (clipped to it) as ANSI text: the rows joined by
/// line breaks, with no break after the last.
pub fn to_ansi(buffer: &Buffer, region: Rect) -> String {
    let region = region.intersection(buffer.area);
    let mut out = String::new();
    for y in region.y..region.bottom() {
        let mut current: Option<Style> = None;
        for x in region.x..region.right() {
            let cell = &buffer[(x, y)];
            let style = cell.style();
            if current != Some(style) {
                out.push_str(&sequence(style));
                current = Some(style);
            }
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m");
        if y + 1 < region.bottom() {
            out.push('\n');
        }
    }
    out
}

/// The sequences that set `style` from a reset.
pub fn sequence(style: Style) -> String {
    let mut out = String::from("\x1b[0m");
    if let Some(seq) = style.fg.and_then(|color| color_sequence(color, false)) {
        out.push_str(&seq);
    }
    if let Some(seq) = style.bg.and_then(|color| color_sequence(color, true)) {
        out.push_str(&seq);
    }
    for (name, _) in style.add_modifier.iter_names() {
        if let Some((_, code)) = MODIFIER_CODES.iter().find(|(known, _)| *known == name) {
            out.push_str(&format!("\x1b[{code}m"));
        }
    }
    out
}

/// The sequence that sets `color` as the foreground, or the background
/// when `background`; `None` for the terminal's own color.
fn color_sequence(color: Color, background: bool) -> Option<String> {
    let base = if background { 48 } else { 38 };
    let named = |code: u8| {
        let code = if background { code + 10 } else { code };
        Some(format!("\x1b[{code}m"))
    };
    match color {
        Color::Reset => None,
        Color::Black => named(30),
        Color::Red => named(31),
        Color::Green => named(32),
        Color::Yellow => named(33),
        Color::Blue => named(34),
        Color::Magenta => named(35),
        Color::Cyan => named(36),
        Color::Gray => named(37),
        Color::DarkGray => named(90),
        Color::LightRed => named(91),
        Color::LightGreen => named(92),
        Color::LightYellow => named(93),
        Color::LightBlue => named(94),
        Color::LightMagenta => named(95),
        Color::LightCyan => named(96),
        Color::White => named(97),
        Color::Rgb(r, g, b) => Some(format!("\x1b[{base};2;{r};{g};{b}m")),
        Color::Indexed(i) => Some(format!("\x1b[{base};5;{i}m")),
    }
}

#[cfg(test)]
#[path = "ansi_tests.rs"]
mod tests;
