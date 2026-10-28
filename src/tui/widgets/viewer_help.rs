//! The viewers' `?` help pages, generated from their key tables.
//!
//! The transcript viewer and the pane viewer each say what their messages
//! do (`tuikit::help::HelpAction`), and their help page is the framework's
//! page generated from their key table (`tuikit::help::sections_for`),
//! drawn as the keymap help draws (`help::page`). So the three help pages
//! share a frame, a column grid, headings, and a footer, and a key added
//! to a viewer's table cannot be left out of its help.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::help::{HelpAction, HelpScroll, sections_for};
use tuikit::keys::KeyRow;
use tuikit::scroll::ScrollWindow;

use crate::tui::Theme;

/// Render the help page of `table` scrolled to `scroll`, reporting the
/// window its scrolling part showed.
pub fn render<A: HelpAction>(
    table: &[KeyRow<A>],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    scroll: &HelpScroll,
) -> ScrollWindow {
    let sections = sections_for(table);
    super::help::page(theme, &sections)
        .scrolled(scroll)
        .render(area, buf)
}

/// Draw the help page of `table` over a viewer, keeping in `scroll` the
/// window it showed, so its offset stays inside the page and its page
/// keys move by the rows on screen.
pub fn draw<A: HelpAction>(
    table: &[KeyRow<A>],
    scroll: &mut HelpScroll,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
) {
    let window = render(table, area, buf, theme, scroll);
    scroll.keep(window);
}

#[cfg(test)]
#[path = "viewer_help_tests.rs"]
mod tests;
