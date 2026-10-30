//! A checklist laid out at one width, and the body its frame draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::item::Item;
use crate::frame::{ModalBody, ModalFooter};
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

    /// The footer the standard checklist table generates.
    pub fn footer(&self) -> ModalFooter<'_> {
        ModalFooter::new(Footer::leaving_last(CHECKLIST_KEYS), self.theme)
    }
}

/// The checklist inside its frame: a blank row and the items, scrolled
/// in the window the checklist kept, over the footer, which keeps the
/// frame's last rows.
pub(super) struct Body<'a> {
    pub view: View<'a>,
    pub window: ScrollWindow,
}

impl ModalBody for Body<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        // The blank row above the items, the items, and the footer.
        let wanted = 1 + self.view.items.len() + self.view.footer().height(width);
        u16::try_from(wanted).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self { view, mut window } = self;
        let footer = view.footer();
        let height = usize::from(area.height);
        let items = view.items.len();
        // A frame too short for the checklist gives up the blank row above
        // the items first.
        let lead = usize::from(1 + items + footer.height(usize::from(area.width)) <= height);
        // Then the footer keeps the frame's last rows and leaves the items
        // one, so the cursor's row always shows; items that overflow give
        // a column to the scrollbar, so they and the footer are laid out a
        // cell narrower.
        let mut fit = footer.fit_beside(area, 1, |_| lead + items);
        let [items_area, footer_area] = fit.split(area);
        // The footer rows that do not fit follow the last item. They spill
        // only while the items are down to the cursor's row, which the
        // window keeps, so a checklist shows none of them.
        let mut lines = Vec::with_capacity(lead + items + fit.spill.len());
        lines.extend((0..lead).map(|_| Line::default()));
        lines.extend(view.rows(usize::from(fit.width)));
        lines.append(&mut fit.spill);
        let gutter = ScrollGutter::layout(items_area, lines.len());
        window.resize(usize::from(items_area.height), lines.len());
        if view.cursor < items {
            let row = lead + view.cursor;
            window.follow(row..row + 1);
        }
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        fit.render(footer_area, buf);
        window
    }
}
