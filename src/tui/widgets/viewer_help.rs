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

use crate::tui::Theme;
use crate::tui::keybindings::overlay::{KeyRow, VIEWER_KEYS};
use crate::viewer::input::ViewerMsg;

use super::help::{binding_lines, blank, help_max_scroll, push_footer, render_help_page, section};

/// One help row: the keys bound to a message and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerHelpRow {
    pub keys: String,
    pub description: &'static str,
}

/// A heading and the rows under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerHelpSection {
    pub title: &'static str,
    pub rows: Vec<ViewerHelpRow>,
}

/// The help page's content, generated from `table` (the viewer's key
/// table) and the descriptions in [`ViewerMsg::help`]. A message with no
/// key bound to it is left out, as there is nothing to press.
pub fn sections_for(table: &[KeyRow<ViewerMsg>]) -> Vec<ViewerHelpSection> {
    let mut sections: Vec<ViewerHelpSection> = Vec::new();
    for msg in ViewerMsg::ALL {
        let mut keys: Vec<String> = table
            .iter()
            .filter_map(|row| match row {
                KeyRow::Bind { key, action, .. } if *action == msg => Some(key_notation(key)),
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

/// The same key notation as the keymap help: arrows as arrows, `Ctrl-D`
/// for control chords.
fn key_notation(key: &crate::tui::keybindings::KeyMatcher) -> String {
    match crate::tui::keybindings::key_label(key).as_str() {
        "Up" => "↑".to_string(),
        "Down" => "↓".to_string(),
        "Left" => "←".to_string(),
        "Right" => "→".to_string(),
        " " => "Space".to_string(),
        // The keymap help writes control chords as `Ctrl-R`.
        other => match other.strip_prefix("Ctrl-") {
            Some(rest) if rest.chars().count() == 1 => format!("Ctrl-{}", rest.to_uppercase()),
            _ => other.to_string(),
        },
    }
}

/// The page's lines wrapped to `width`: each section heading, its rows,
/// a blank line, and the footer.
pub fn help_lines(theme: &Theme, width: Option<usize>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for sect in sections_for(VIEWER_KEYS) {
        section(&mut lines, sect.title);
        for row in &sect.rows {
            lines.extend(binding_lines(&row.keys, row.description, theme, width));
        }
        blank(&mut lines);
    }
    push_footer(&mut lines, theme, width);
    lines
}

/// Render the page scrolled to `scroll`.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, scroll: usize) {
    render_help_page(area, buf, theme, scroll, &|width| {
        help_lines(theme, Some(width))
    });
}

/// How far the page can scroll in `area`.
pub fn max_scroll(area: Rect, theme: &Theme) -> usize {
    help_max_scroll(area, &|width| help_lines(theme, Some(width)))
}

/// Draw the viewer's help over the transcript, keeping the viewer's
/// scroll position inside what the page can reach.
pub fn draw(
    state: &mut crate::viewer::state::ViewerState,
    theme: &Theme,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    state.help_scroll = state.help_scroll.min(max_scroll(area, theme));
    render(area, frame.buffer_mut(), theme, state.help_scroll);
}

#[cfg(test)]
#[path = "viewer_help_tests.rs"]
mod tests;
