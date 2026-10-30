//! The tab bar: the names of the views a pane can show, one of them
//! active, as one line that gives way tab by tab.
//!
//! This is a layer 1 control. A pane that switches among a few views
//! names them all where the reader looks first (usually its title), so the
//! views can be seen at a glance instead of remembered: the active tab in
//! the theme's `accent` and bold, the others in its `inert` color, joined
//! by [`marks::SPACED_SEPARATOR`] (`one · two · three`).
//!
//! When the names are wider than their room, the bar gives way by
//! [`text::elide`], with its mark at the end:
//!
//! - every tab shows while they all fit;
//! - otherwise the inactive tabs give way from the end of the bar, the
//!   active one never, and the shortened bar ends with [`marks::MORE`]
//!   (` …`) and one blank cell after it, so the mark never runs into what
//!   follows the bar (a border's corner);
//! - when the active tab and the mark do not fit, the active tab shows
//!   alone with no mark, cut with `…` only when it alone is wider than the
//!   room.
//!
//! So a bar narrows as `one · two · three · four`, `one · two · three …`,
//! `one · two …`, `one …`, `one`, and then `one` cut. A tab's name is never
//! cut while another tab shows, and the line never takes more cells than
//! its room.
//!
//! The host keeps what the tabs are: their names, their order, which is
//! active, and the keys that switch them.
//!
//! ```
//! use tuikit::tab_bar::TabBar;
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let names = ["inbox", "drafts", "sent", "archive"];
//! let bar = TabBar::new(&names, 1, &theme);
//! let text = |room| bar.line(room).to_string();
//! assert_eq!(text(80), "inbox · drafts · sent · archive");
//! assert_eq!(text(30), "inbox · drafts · sent …");
//! assert_eq!(text(20), "inbox · drafts …");
//! assert_eq!(text(10), "drafts …");
//! assert_eq!(text(6), "drafts");
//! assert_eq!(text(4), "dra…");
//! ```

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::text::{self, Elided, Item, MarkAt, Piece};
use crate::theme::Theme;

/// A strip of named tabs with one active, drawn in a room.
///
/// See the [module documentation](self) for the rule.
#[derive(Clone, Copy, Debug)]
pub struct TabBar<'a> {
    names: &'a [&'a str],
    active: usize,
    active_style: Style,
    style: Style,
}

impl<'a> TabBar<'a> {
    /// The tabs named `names`, in order, with the one at `active` active
    /// (an index past the end is the last tab): the active tab in the
    /// theme's `accent` and bold, the others, the separators, and the mark
    /// in its `inert` color.
    pub fn new(names: &'a [&'a str], active: usize, theme: &Theme) -> Self {
        Self {
            names,
            active,
            active_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            style: Style::default().fg(theme.inert),
        }
    }

    /// Draw the active tab in `style` instead.
    #[must_use]
    pub fn active_style(mut self, style: Style) -> Self {
        self.active_style = style;
        self
    }

    /// Draw the other tabs, the separators, and the mark in `style`
    /// instead.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// The index of the active tab, held inside the tabs.
    pub fn active(&self) -> usize {
        self.active.min(self.names.len().saturating_sub(1))
    }

    /// Which tabs show in `room` cells: the kept tabs by index, whether
    /// the bar ends with its mark, and the cells the active tab is cut to
    /// when it alone is too wide. A host that maps a click on the bar to a
    /// tab reads the kept tabs' places from this.
    pub fn fit(&self, room: usize) -> Elided {
        let active = self.active();
        let pieces: Vec<Piece> = self
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let width = text::width(name);
                if index == active {
                    Piece::unranked(width)
                } else {
                    // One rank for every inactive tab: of a tie the later
                    // gives way first, so they go from the end of the bar.
                    Piece::ranked(width, 1)
                }
            })
            .collect();
        text::elide(
            &pieces,
            text::width(marks::SPACED_SEPARATOR),
            Self::more_cells(),
            room,
            MarkAt::End,
        )
    }

    /// The bar in at most `room` cells; an empty line with no tabs.
    pub fn line(&self, room: usize) -> Line<'static> {
        if self.names.is_empty() {
            return Line::default();
        }
        let active = self.active();
        let fit = self.fit(room);
        let mut spans = Vec::new();
        for (position, item) in fit.items().into_iter().enumerate() {
            if position > 0 {
                spans.push(Span::styled(marks::SPACED_SEPARATOR, self.style));
            }
            // An end mark is never an item, so every item is a tab.
            let Item::Piece(index) = item else { continue };
            let name = self.names[index];
            if index == active {
                let name = match fit.cut {
                    Some(cells) => text::truncate_end(name, cells),
                    None => name.to_string(),
                };
                spans.push(Span::styled(name, self.active_style));
            } else {
                spans.push(Span::styled(name.to_string(), self.style));
            }
        }
        if fit.end_mark {
            spans.push(Span::styled(marks::MORE, self.style));
        }
        Line::from(spans)
    }

    /// The cells the mark is counted at: the mark and the blank cell kept
    /// free after it.
    fn more_cells() -> usize {
        text::width(marks::MORE) + 1
    }
}

#[cfg(test)]
#[path = "tab_bar_tests.rs"]
mod tests;
