//! The transcript viewer's `?` help page.
//!
//! It is drawn by the same page renderer as the keymap help (`help`), so
//! the two modals share a frame, a size, section headings, a key column
//! in the accent color, wrapped descriptions, a scrollbar, and a footer
//! generated from the key table that scrolls and closes them. The rows
//! come from the viewer's key table paired with each message's
//! description (`ViewerMsg::help`), so a key added to the viewer cannot
//! be left out of its help.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tuikit::keys::{KeyRow, key_label, rows};
use tuikit::scroll::ScrollWindow;

use crate::tui::Theme;
use crate::tui::keybindings::overlay::VIEWER_KEYS;
use crate::viewer::input::ViewerMsg;

use super::help::{binding_lines, blank, dim_unavailable, push_footer, render_help_page, section};

/// One help row: the keys bound to a message, what it does, and whether
/// it can act now (a row that cannot is drawn dimmed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerHelpRow {
    pub keys: String,
    pub description: &'static str,
    pub available: bool,
}

/// A heading and the rows under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerHelpSection {
    pub title: &'static str,
    pub rows: Vec<ViewerHelpRow>,
}

/// The help page's content, generated from `table` (the viewer's key
/// table) and the descriptions in `ViewerMsg::help`. A message with no
/// key bound to it is left out, as there is nothing to press; one whose
/// rows are all unavailable is marked so.
pub fn sections_for(table: &[KeyRow<ViewerMsg>]) -> Vec<ViewerHelpSection> {
    let mut sections: Vec<ViewerHelpSection> = Vec::new();
    for msg in ViewerMsg::ALL {
        let mut available = false;
        let mut keys: Vec<String> = rows(table)
            .filter_map(|row| match row {
                KeyRow::Bind {
                    key,
                    action,
                    available: now,
                    ..
                } if *action == msg => {
                    available |= *now;
                    Some(key_label(key))
                }
                _ => None,
            })
            .collect();
        if keys.is_empty() {
            continue;
        }
        // Plain keys first and chords last, as the keymap help writes `q / Ctrl-C`.
        keys.sort_by_key(|k| k.starts_with("Ctrl-"));
        let entry = msg.help();
        let row = ViewerHelpRow {
            keys: keys.join(" / "),
            description: entry.description,
            available,
        };
        match sections.iter_mut().find(|s| s.title == entry.section) {
            Some(section) => section.rows.push(row),
            None => sections.push(ViewerHelpSection {
                title: entry.section,
                rows: vec![row],
            }),
        }
    }
    sections
}

/// The page's lines wrapped to `width`: each section heading, its rows,
/// a blank line, and the footer.
pub fn help_lines(theme: &Theme, width: Option<usize>) -> Vec<Line<'static>> {
    help_lines_for(VIEWER_KEYS, theme, width)
}

/// [`help_lines`] for `table`, whose rows may say an action cannot act
/// now (`tuikit::keys::with_availability`); such a row is dimmed.
pub fn help_lines_for(
    table: &[KeyRow<ViewerMsg>],
    theme: &Theme,
    width: Option<usize>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for sect in sections_for(table) {
        section(&mut lines, sect.title);
        for row in &sect.rows {
            let mut drawn = binding_lines(&row.keys, row.description, theme, width);
            if !row.available {
                dim_unavailable(&mut drawn, theme);
            }
            lines.extend(drawn);
        }
        blank(&mut lines);
    }
    push_footer(&mut lines, theme, width);
    lines
}

/// Render the page scrolled to `scroll`, reporting the window its
/// scrolling part showed.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, scroll: usize) -> ScrollWindow {
    render_help_page(area, buf, theme, scroll, &|width| {
        help_lines(theme, Some(width))
    })
}

/// Draw the viewer's help over the transcript, keeping the viewer's
/// scroll position inside what the page can reach and recording the
/// rows a page key moves.
pub fn draw(
    state: &mut crate::viewer::state::ViewerState,
    theme: &Theme,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    let window = render(area, frame.buffer_mut(), theme, state.help_scroll);
    state.help_scroll = window.offset();
    state.help_page = window.viewport();
}

#[cfg(test)]
#[path = "viewer_help_tests.rs"]
mod tests;
