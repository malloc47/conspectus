//! A checklist laid out at one width, and the body its frame draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::item::Item;
use crate::frame::ModalBody;
use crate::glyphs::marks;
use crate::keys::{CHECKLIST_KEYS, Footer, unavailable_style};
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::theme::Theme;

/// What the layout reads from the checklist: its items, the cursor, and
/// the theme.
pub(super) struct View<'a> {
    pub items: &'a [Item],
    pub cursor: usize,
    pub theme: &'a Theme,
}

impl View<'_> {
    /// The items laid out `width` cells wide, one row each: the cursor
    /// gutter, the check mark, the label, and the detail after the
    /// separator mark, cut with `…` to fit.
    pub fn rows(&self, width: usize) -> Vec<Line<'static>> {
        let theme = self.theme;
        let room = width.saturating_sub(GUTTER_WIDTH);
        let checked = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let style = if !item.enabled {
                    unavailable_style(theme)
                } else if item.checked {
                    checked
                } else {
                    Style::default()
                };
                let mark = if item.checked {
                    marks::OK
                } else {
                    marks::BULLET
                };
                let mut spans = vec![
                    Span::styled(mark, style),
                    Span::raw(" "),
                    Span::styled(item.label.clone(), style),
                ];
                if let Some(detail) = &item.detail {
                    spans.push(Span::raw(marks::SPACED_SEPARATOR));
                    spans.push(Span::raw(detail.clone()));
                }
                let cursor = Cursor::of(index == self.cursor && item.enabled, true);
                selection::row(text::fit_spans(spans, room), cursor, theme)
            })
            .collect()
    }

    /// The footer `width` cells wide under a blank row, broken only
    /// between hints.
    pub fn footer(&self, width: usize) -> Vec<Line<'static>> {
        let mut footer = vec![Line::default()];
        footer.extend(Footer::new(CHECKLIST_KEYS).lines(width, self.theme));
        footer
    }
}

/// The checklist inside its frame: a blank row and the items, scrolled
/// in the window the checklist kept, over the footer, which keeps the
/// bottom rows.
pub(super) struct Body<'a> {
    pub view: View<'a>,
    pub window: ScrollWindow,
    pub rows: Option<Vec<Line<'static>>>,
    pub footer: Vec<Line<'static>>,
}

impl ModalBody for Body<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        let rows = self.view.rows(width);
        self.footer = self.view.footer(width);
        // The blank row above the items, the items, and the footer.
        let wanted = 1 + rows.len() + self.footer.len();
        self.rows = Some(rows);
        u16::try_from(wanted).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self {
            view,
            mut window,
            rows,
            mut footer,
        } = self;
        let width = usize::from(area.width);
        let mut rows = rows.unwrap_or_else(|| view.rows(width));
        if footer.is_empty() {
            footer = view.footer(width);
        }
        let height = usize::from(area.height);
        // A frame too short for the checklist gives up its blank rows
        // first: the one above the items, then the one above the keys.
        let lead = usize::from(1 + rows.len() + footer.len() <= height);
        if lead + rows.len() + footer.len() > height && footer.len() > 1 {
            footer.remove(0);
        }
        // Then the footer keeps the bottom rows, all but one, so the
        // cursor's row always shows, and the items scroll in the rest.
        let footer_rows = footer.len().min(height.saturating_sub(1));
        let footer = footer.split_off(footer.len() - footer_rows);
        let item_rows = height - footer_rows;
        let items_area = Rect {
            height: u16::try_from(item_rows).unwrap_or(area.height),
            ..area
        };
        // Items that overflow give a column to the scrollbar, so they
        // are laid out again one cell narrower.
        if lead + rows.len() > item_rows && area.width >= 2 {
            rows = view.rows(width - 1);
        }
        let mut lines = Vec::with_capacity(lead + rows.len());
        lines.extend((0..lead).map(|_| Line::default()));
        lines.extend(rows);
        let gutter = ScrollGutter::layout(items_area, lines.len());
        window.resize(item_rows, lines.len());
        if view.cursor < view.items.len() {
            let row = lead + view.cursor;
            window.follow(row..row + 1);
        }
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        let footer_area = Rect {
            y: area.y + items_area.height,
            height: area.height - items_area.height,
            ..area
        };
        Paragraph::new(footer).render(footer_area, buf);
        window
    }
}
