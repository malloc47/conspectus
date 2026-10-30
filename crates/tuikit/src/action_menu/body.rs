//! A menu laid out at one width, and the body its frame draws.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::entry::Entry;
use crate::frame::{ModalBody, ModalFooter};
use crate::glyphs::marks;
use crate::keys::{Footer, KeyRow, MENU_KEYS, MenuKey, key_label, rows, unavailable_style};
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::theme::Theme;

/// The cells between the key column and the labels.
pub(super) const KEY_GAP: usize = 2;

/// How far an entry's detail lines are indented past its label.
pub(super) const DETAIL_INDENT: usize = 3;

/// The words an armed entry adds after its label, saying what the next
/// press of the pick key does: `press Enter again to confirm`, with the
/// key written as the menu's table shows it.
pub fn confirm_note() -> String {
    let key = rows(MENU_KEYS)
        .find_map(|row| match row {
            KeyRow::Bind {
                key,
                action: MenuKey::Pick,
                ..
            } => Some(key_label(key)),
            _ => None,
        })
        .unwrap_or_default();
    format!("press {key} again to confirm")
}

/// What the layout reads from the menu: its entries, the cursor, the
/// armed entry, and the theme.
pub(super) struct View<'a, M> {
    pub entries: &'a [Entry<M>],
    pub cursor: usize,
    pub armed: Option<usize>,
    pub theme: &'a Theme,
}

/// The entries laid out at one width: a blank row, then each entry's
/// label row and its detail rows.
pub(super) struct Layout {
    pub lines: Vec<Line<'static>>,
    /// The rows each entry takes; the first entry's include the blank
    /// row above it, so the cursor on it scrolls the menu to its top.
    pub spans: Vec<Range<usize>>,
    /// The row each entry's label is on.
    pub labels: Vec<usize>,
}

impl<M> View<'_, M> {
    /// The entries laid out `width` cells wide.
    pub fn layout(&self, width: usize) -> Layout {
        let theme = self.theme;
        let keys: Vec<Option<String>> = self
            .entries
            .iter()
            .map(|entry| entry.shortcut.as_ref().map(key_label))
            .collect();
        let key_width = keys
            .iter()
            .flatten()
            .map(|key| text::width(key))
            .max()
            .unwrap_or(0);
        let key_column = if key_width == 0 {
            0
        } else {
            key_width + KEY_GAP
        };
        let label_width = width.saturating_sub(GUTTER_WIDTH + key_column);
        let indent = GUTTER_WIDTH + key_column + DETAIL_INDENT;
        let detail_width = width.saturating_sub(indent).max(1);
        let mut lines = vec![Line::default()];
        let mut spans = Vec::with_capacity(self.entries.len());
        let mut labels = Vec::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            let first = if index == 0 { 0 } else { lines.len() };
            let armed = self.armed == Some(index);
            let (label, style) = if !entry.enabled {
                (entry.label.clone(), unavailable_style(theme))
            } else if armed {
                (
                    format!(
                        "{}{}{}",
                        entry.label,
                        marks::SPACED_SEPARATOR,
                        confirm_note()
                    ),
                    Style::default()
                        .fg(theme.error)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (entry.label.clone(), Style::default())
            };
            let mut row = Vec::with_capacity(3);
            if key_column > 0 {
                let key = keys[index].as_deref().unwrap_or_default();
                let key_style = if entry.enabled {
                    Style::default()
                } else {
                    unavailable_style(theme)
                };
                row.push(Span::styled(text::pad(key, key_width), key_style));
                row.push(Span::raw(" ".repeat(KEY_GAP)));
            }
            row.push(Span::styled(text::truncate_end(&label, label_width), style));
            let cursor = Cursor::of(index == self.cursor && entry.enabled, true);
            labels.push(lines.len());
            lines.push(selection::row(row, cursor, theme));
            if let Some(detail) = &entry.detail {
                for piece in text::wrap(detail, detail_width) {
                    lines.push(Line::from(format!(
                        "{}{}",
                        " ".repeat(indent),
                        piece.trim_end()
                    )));
                }
            }
            spans.push(first..lines.len());
        }
        Layout {
            lines,
            spans,
            labels,
        }
    }

    /// The footer the standard menu table generates.
    pub fn footer(&self) -> ModalFooter<'_> {
        ModalFooter::new(Footer::leaving_last(MENU_KEYS), self.theme)
    }
}

/// The menu inside its frame: the entries, scrolled in the window the
/// menu kept, over the footer, which keeps the frame's last rows.
pub(super) struct Body<'a, M> {
    pub view: View<'a, M>,
    pub window: ScrollWindow,
}

impl<M> ModalBody for Body<'_, M> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        let rows = self.view.layout(width).lines.len() + self.view.footer().height(width);
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self { view, mut window } = self;
        // The footer keeps the frame's last rows and leaves the entries
        // one; entries that overflow give a column to the scrollbar, so
        // they and the footer are laid out a cell narrower.
        let mut fit = view
            .footer()
            .fit_beside(area, 1, |width| view.layout(usize::from(width)).lines.len());
        let [entries_area, footer_area] = fit.split(area);
        let mut layout = view.layout(usize::from(fit.width));
        // The footer rows that do not fit follow the last entry. They spill
        // only while the entries are down to the cursor's row, which the
        // window keeps, so a menu shows none of them.
        layout.lines.append(&mut fit.spill);
        let gutter = ScrollGutter::layout(entries_area, layout.lines.len());
        window.resize(usize::from(entries_area.height), layout.lines.len());
        if let Some(span) = layout.spans.get(view.cursor) {
            window.follow(span.clone());
        }
        // A span taller than the window shows its start, which for the
        // first entry is the blank row above it: keep the label in view.
        if let Some(&label) = layout.labels.get(view.cursor) {
            window.follow(label..label + 1);
        }
        Paragraph::new(layout.lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        fit.render(footer_area, buf);
        window
    }
}
