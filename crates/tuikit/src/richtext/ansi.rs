//! Text with ANSI escapes as styled lines.

use ansi_to_tui::IntoText;
use ratatui::text::Text;

/// `raw`, text carrying ANSI escapes (a terminal's captured screen, a
/// command's colored output), as styled text: each SGR sequence becomes
/// the style of the spans after it, and the escapes are gone.
///
/// With `color` off the text keeps its characters and loses its styles,
/// as a host drawing without color wants. Text the parser refuses is
/// shown as it is, escapes included, rather than not at all.
///
/// ```
/// use ratatui::style::Color;
/// use tuikit::richtext::ansi_text;
///
/// let colored = ansi_text("\x1b[31mred\x1b[0m plain", true);
/// assert_eq!(colored.lines[0].spans[0].content, "red");
/// assert_eq!(colored.lines[0].spans[0].style.fg, Some(Color::Red));
///
/// let plain = ansi_text("\x1b[31mred\x1b[0m plain", false);
/// assert_eq!(plain.to_string(), "red plain");
/// assert!(plain.lines[0].spans.iter().all(|span| span.style.fg.is_none()));
/// ```
pub fn ansi_text(raw: &str, color: bool) -> Text<'static> {
    if !color {
        // Flattening the parsed text strips the styles whichever way the
        // parser grouped the escapes; text it refuses has none to strip.
        return match raw.into_text() {
            Ok(parsed) => Text::raw(parsed.to_string()),
            Err(_) => Text::raw(raw.to_string()),
        };
    }
    raw.into_text()
        .unwrap_or_else(|_| Text::raw(raw.to_string()))
}

#[cfg(test)]
#[path = "ansi_tests.rs"]
mod tests;
