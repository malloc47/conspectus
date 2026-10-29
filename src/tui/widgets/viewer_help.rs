//! The viewers' `?` help pages, generated from their key tables.
//!
//! The transcript viewer and the pane viewer each say what their messages
//! do (`tuikit::help::HelpAction`), and their help page is the framework's
//! page generated from their key table (`tuikit::help::sections_for`), so
//! it shares the keymap help's frame, column grid, and headings, and a key
//! added to a viewer's table cannot be left out of its help. Its footer is
//! a pager help page's (`tuikit::keys::PAGER_HELP_KEYS`): `Esc` and `?` go
//! back to the viewer and `q` leaves it, which is what those keys do there.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::help::{HelpAction, HelpPage, HelpScroll, sections_for};
use tuikit::keys::{KeyRow, PAGER_HELP_KEYS};
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
    HelpPage::new(theme, &sections)
        .footer(PAGER_HELP_KEYS)
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
