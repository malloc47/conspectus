//! A menu laid out at one width, and the body its frame draws.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::entry::Entry;
use crate::frame::ModalBody;
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

    /// The footer `width` cells wide under a blank row, broken only
    /// between hints.
    pub fn footer(&self, width: usize) -> Vec<Line<'static>> {
        let mut footer = vec![Line::default()];
        footer.extend(Footer::new(MENU_KEYS).lines(width, self.theme));
        footer
    }
}

/// The menu inside its frame: the entries, scrolled in the window the
/// menu kept, over the footer, which keeps the bottom rows.
pub(super) struct Body<'a, M> {
    pub view: View<'a, M>,
    pub window: ScrollWindow,
    pub width: usize,
    pub layout: Option<Layout>,
    pub footer: Vec<Line<'static>>,
}

impl<M> ModalBody for Body<'_, M> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        self.width = usize::from(width);
        let layout = self.view.layout(self.width);
        self.footer = self.view.footer(self.width);
        let rows = layout.lines.len() + self.footer.len();
        self.layout = Some(layout);
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self {
            view,
            mut window,
            width,
            layout,
            mut footer,
        } = self;
        let mut layout = layout.unwrap_or_else(|| view.layout(usize::from(area.width)));
        // A frame shorter than the menu gives the footer every row but
        // one (its blank row going first) and the entries the rest.
        let rows = usize::from(area.height);
        let footer_rows = footer.len().min(rows.saturating_sub(1));
        let footer = footer.split_off(footer.len() - footer_rows);
        let entry_rows = rows - footer_rows;
        let entries_area = Rect {
            height: u16::try_from(entry_rows).unwrap_or(area.height),
            ..area
        };
        // Entries that overflow give a column to the scrollbar, so they
        // are laid out again one cell narrower.
        if layout.lines.len() > entry_rows && entries_area.width >= 2 {
            layout = view.layout(width.saturating_sub(1));
        }
        let gutter = ScrollGutter::layout(entries_area, layout.lines.len());
        window.resize(entry_rows, layout.lines.len());
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
        let footer_area = Rect {
            y: area.y + entries_area.height,
            height: area.height - entries_area.height,
            ..area
        };
        Paragraph::new(footer).render(footer_area, buf);
        window
    }
}
