//! The viewers' `?` help pages, generated from their key tables.
//!
//! The transcript viewer and the pane viewer each say what their messages
//! do (`tuikit::help::HelpAction`), and their help page is the framework's
//! page generated from their key table (`tuikit::help::sections_for`),
//! read in a pager of its own, so it shares the keymap help's frame,
//! column grid, headings, and scrolling, and a key added to a viewer's
//! table cannot be left out of its help. Its footer is a pager help
//! page's (`tuikit::keys::PAGER_HELP_KEYS`): `Esc` and `?` go back to the
//! viewer and `q` leaves it, which is what those keys do there.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::help::{HelpAction, HelpPage, sections_for};
use tuikit::keys::{KeyRow, PAGER_HELP_KEYS};
use tuikit::pager::Pager;

use crate::tui::Theme;

/// The help page of `table`, at its top, in a pager over the help level's
/// keys.
pub fn page<A: HelpAction>(table: &[KeyRow<A>]) -> Pager<HelpPage> {
    HelpPage::new(sections_for(table)).pager(PAGER_HELP_KEYS)
}

/// Draw `help`, the help page of `table`, over a viewer; its pager keeps
/// the window it showed, so its offset stays inside the page and its page
/// keys move by the rows on screen. The viewer's state does not name the
/// program's key tables, so it starts with an empty page, which the first
/// draw generates from `table`.
pub fn draw<A: HelpAction>(
    table: &[KeyRow<A>],
    help: &mut Pager<HelpPage>,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
) {
    if help.content().sections().is_empty() {
        *help.content_mut() = HelpPage::new(sections_for(table));
        help.refresh();
    }
    help.draw(area, buf, theme);
}

#[cfg(test)]
#[path = "viewer_help_tests.rs"]
mod tests;
