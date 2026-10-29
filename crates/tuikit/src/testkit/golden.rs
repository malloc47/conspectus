//! Style-aware goldens: a frame written as text that keeps every cell's
//! color and modifiers, so a change of color alone fails a comparison.
//!
//! A golden that stores only a frame's characters passes when a control
//! starts drawing its selection in the wrong color, or loses its bold; a
//! golden in this form does not. [`styled`] writes a frame as a header,
//! then for each row its text and its style row, then a legend:
//!
//! ```text
//! frame 12x2
//! text  |Pick one    |
//! style |aaaa bbb    |
//! text  |> first     |
//! style |cccccccccccc|
//! legend
//!   a  fg Yellow, bold
//!   b  fg Yellow
//!   c  fg Black, bg White
//! ```
//!
//! Each cell's style row character is a key into the legend; a space is
//! the default style (the terminal's own colors and no modifier). Keys are
//! given in the order styles first appear, so recoloring a style changes
//! its legend line and nothing else, and the bars keep trailing spaces
//! visible (and out of reach of editors that trim them). A character two
//! cells wide is written once in its text row, over the style keys of both
//! its cells, so the rows stay aligned in a monospace view. An underline
//! color is not kept.
//!
//! A test checks a frame against a file with [`assert_golden`]. When the
//! frame changed on purpose, run the test with the environment variable
//! [`ACCEPT_VAR`] set (`TUIKIT_ACCEPT_GOLDENS=1 cargo test ...`): instead
//! of failing, the check hands the new text to the writer the test passed
//! in (the framework writes no files itself), and the changed golden goes
//! into review with the change that caused it.
//!
//! ```
//! use ratatui::style::{Color, Style};
//! use ratatui::text::Span;
//! use ratatui::widgets::Paragraph;
//! use tuikit::testkit::{buffer, golden, render};
//!
//! let draw = |color| {
//!     let word = Span::styled("ok", Style::new().fg(color));
//!     render::render_widget(Paragraph::new(word), 4, 1)
//! };
//! let (yellow, red) = (draw(Color::Yellow), draw(Color::Red));
//! assert_eq!(buffer::text(&yellow), buffer::text(&red), "the characters agree");
//! assert!(golden::compare(&golden::styled(&yellow), &golden::styled(&red)).is_err());
//! ```

use std::fmt;
use std::io;
use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::text;

/// The environment variable that turns a golden check into accepting the
/// frame as the new golden.
pub const ACCEPT_VAR: &str = "TUIKIT_ACCEPT_GOLDENS";

/// The characters that key styles in a style row, in the order styles
/// are given them. `|` closes a row and a space is the default style, so
/// neither is a key.
const KEYS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789\
                    !#$%&()*+,-./:;<=>?@[]^_{}~";

/// `buffer` in the style-aware golden form.
pub fn styled(buffer: &Buffer) -> String {
    styled_region(buffer, buffer.area)
}

/// `region` of `buffer` (clipped to it) in the style-aware golden form.
///
/// # Panics
///
/// When the region has more distinct styles than there are keys (over
/// ninety), which no screen a person reads has.
pub fn styled_region(buffer: &Buffer, region: Rect) -> String {
    let region = region.intersection(buffer.area);
    let mut legend: Vec<Style> = Vec::new();
    let mut out = format!("frame {}x{}\n", region.width, region.height);
    for y in region.y..region.bottom() {
        let mut row_text = String::new();
        let mut row_style = String::new();
        let mut covered = 0;
        for x in region.x..region.right() {
            let cell = &buffer[(x, y)];
            let symbol = cell.symbol();
            if covered > 0 && symbol.trim().is_empty() {
                covered -= 1;
            } else {
                row_text.push_str(symbol);
                covered = text::width(symbol).saturating_sub(1);
            }
            row_style.push(key_of(cell.style(), &mut legend));
        }
        out.push_str(&format!("text  |{row_text}|\nstyle |{row_style}|\n"));
    }
    out.push_str("legend\n");
    for (style, key) in legend.iter().zip(KEYS.chars()) {
        out.push_str(&format!("  {key}  {}\n", describe(*style)));
    }
    out
}

/// The style row character of `style`, giving it the next key when it is
/// new.
fn key_of(style: Style, legend: &mut Vec<Style>) -> char {
    let style = normalized(style);
    if style == Style::default() {
        return ' ';
    }
    let index = legend
        .iter()
        .position(|known| *known == style)
        .unwrap_or_else(|| {
            legend.push(style);
            legend.len() - 1
        });
    KEYS.chars()
        .nth(index)
        .unwrap_or_else(|| panic!("a golden region has more than {} styles", KEYS.len()))
}

/// What a golden keeps of `style`: its colors (the terminal's own as no
/// color, so a cell that was never styled and one reset explicitly read
/// alike) and its modifiers. An underline color, which only some
/// terminals draw, is not kept.
fn normalized(style: Style) -> Style {
    let reset = Style::reset();
    let mut kept = Style::new()
        .add_modifier(style.add_modifier)
        .remove_modifier(style.sub_modifier);
    if let Some(fg) = style.fg.filter(|color| Some(*color) != reset.fg) {
        kept = kept.fg(fg);
    }
    if let Some(bg) = style.bg.filter(|color| Some(*color) != reset.bg) {
        kept = kept.bg(bg);
    }
    kept
}

/// A legend entry: the colors, then the modifiers, lower case.
fn describe(style: Style) -> String {
    let mut parts = Vec::new();
    if let Some(fg) = style.fg {
        parts.push(format!("fg {fg:?}"));
    }
    if let Some(bg) = style.bg {
        parts.push(format!("bg {bg:?}"));
    }
    for (name, _) in style.add_modifier.iter_names() {
        parts.push(name.to_ascii_lowercase());
    }
    for (name, _) in style.sub_modifier.iter_names() {
        parts.push(format!("not {}", name.to_ascii_lowercase()));
    }
    parts.join(", ")
}

/// Where a frame and its golden disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The golden's text.
    pub expected: String,
    /// The frame's text.
    pub actual: String,
}

impl fmt::Display for Mismatch {
    /// The lines that differ, by line number, golden first.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let expected: Vec<&str> = self.expected.lines().collect();
        let actual: Vec<&str> = self.actual.lines().collect();
        writeln!(f, "the frame does not match its golden:")?;
        for index in 0..expected.len().max(actual.len()) {
            let (old, new) = (expected.get(index), actual.get(index));
            if old != new {
                writeln!(f, "line {}:", index + 1)?;
                if let Some(old) = old {
                    writeln!(f, "  - {old}")?;
                }
                if let Some(new) = new {
                    writeln!(f, "  + {new}")?;
                }
            }
        }
        Ok(())
    }
}

/// `Ok` when `actual` is `expected`, ignoring a difference in how the two
/// end their last line.
pub fn compare(expected: &str, actual: &str) -> Result<(), Mismatch> {
    if expected.trim_end_matches('\n') == actual.trim_end_matches('\n') {
        Ok(())
    } else {
        Err(Mismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

/// Whether the environment asks golden checks to accept what they see.
pub fn accepting() -> bool {
    std::env::var_os(ACCEPT_VAR).is_some_and(|value| !value.is_empty() && value != "0")
}

/// Checks `buffer`, in the style-aware form, against the golden file at
/// `path`. With [`ACCEPT_VAR`] set, it hands the frame's text to `write`
/// for that path instead (a test passes `std::fs::write`), so the golden
/// is rewritten rather than checked.
///
/// # Panics
///
/// When the golden is missing, unreadable, or different (saying how to
/// accept the new frame), or when accepting and `write` fails.
pub fn assert_golden(
    path: &Path,
    buffer: &Buffer,
    write: impl FnOnce(&Path, &str) -> io::Result<()>,
) {
    assert_golden_text(path, &styled(buffer), write);
}

/// [`assert_golden`] for text already in the form a golden keeps (a
/// region, or a frame written some other way).
///
/// # Panics
///
/// As [`assert_golden`].
pub fn assert_golden_text(
    path: &Path,
    actual: &str,
    write: impl FnOnce(&Path, &str) -> io::Result<()>,
) {
    if let Err(report) = check(path, actual, accepting(), write) {
        panic!("{report}");
    }
}

/// What [`assert_golden_text`] does, with whether to accept given rather
/// than read from the environment: `Err` with the report it fails with.
fn check(
    path: &Path,
    actual: &str,
    accept: bool,
    write: impl FnOnce(&Path, &str) -> io::Result<()>,
) -> Result<(), String> {
    if accept {
        return write(path, actual)
            .map_err(|error| format!("write golden {}: {error}", path.display()));
    }
    let expected = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "read golden {}: {error}\nRun with {ACCEPT_VAR}=1 to write it.",
            path.display()
        )
    })?;
    compare(&expected, actual).map_err(|mismatch| {
        format!(
            "{}: {mismatch}Run with {ACCEPT_VAR}=1 to accept the new frame.",
            path.display()
        )
    })
}

#[cfg(test)]
#[path = "golden_tests.rs"]
mod tests;
