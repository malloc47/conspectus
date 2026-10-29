//! A palette laid out inside its frame: the query, the status row, the
//! results or the options pane, and the footer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::options::Options;
use super::row::RowContext;
use crate::frame::ModalBody;
use crate::keys::Footer;
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::text_edit::{LineEdit, TextField};
use crate::theme::Theme;

/// The most rows the footer takes; a longer footer shows its first rows.
const MAX_FOOTER_ROWS: usize = 4;

/// What the layout reads from the palette.
pub(super) struct View<'a, R> {
    pub query: &'a LineEdit,
    pub prompt: &'a str,
    pub status: Option<&'a str>,
    pub rows: &'a [R],
    pub ranked: &'a [usize],
    pub cursor: usize,
    /// What the list says when it is empty.
    pub note: &'a str,
    /// The options pane, while it is open.
    pub options: Option<&'a Options>,
    pub footer: Footer,
    pub theme: &'a Theme,
    /// Lays a result out: the row's own cells, or the host's while it
    /// draws.
    pub cells: &'a dyn Fn(&R, &RowContext<'_>) -> Vec<Span<'static>>,
    /// Whether the scrollbar's column stays while the results fit.
    pub reserve_scrollbar: bool,
}

impl<R> View<'_, R> {
    /// Rows above the list: the query, then the status row when there is
    /// one.
    fn lead(&self) -> usize {
        1 + usize::from(self.status.is_some())
    }

    /// The footer `width` cells wide, receding, broken only between hints.
    fn footer_lines(&self, width: usize) -> Vec<Line<'static>> {
        let muted = Style::default().add_modifier(self.theme.muted);
        self.footer
            .lines(width, self.theme)
            .into_iter()
            .map(|line| line.patch_style(muted))
            .collect()
    }

    /// The query row: the prompt, then the field with its caret while the
    /// results have the keys.
    fn query(&self, area: Rect, buf: &mut Buffer) {
        let prompt = Span::styled(
            self.prompt.to_string(),
            Style::default().fg(self.theme.accent),
        );
        TextField::new(self.query)
            .prefix(prompt)
            .focused(self.options.is_none())
            .render(area, buf, self.theme);
    }

    /// The list's rows `width` cells wide (gutter included) for the
    /// listed positions `shown`.
    fn result_lines(&self, shown: std::ops::Range<usize>, width: usize) -> Vec<Line<'static>> {
        let room = width.saturating_sub(GUTTER_WIDTH);
        let query = self.query.value();
        shown
            .filter_map(|position| {
                let row = self.rows.get(*self.ranked.get(position)?)?;
                let cursor = Cursor::of(position == self.cursor, true);
                let context = RowContext::new(room, cursor, query, self.theme);
                let cells = text::fit_spans((self.cells)(row, &context), room);
                Some(selection::row(cells, cursor, self.theme))
            })
            .collect()
    }
}

/// The palette inside its frame. Its draw returns the results' window,
/// which the palette keeps for the next key.
pub(super) struct Body<'a, R> {
    pub view: View<'a, R>,
    pub window: ScrollWindow,
}

impl<R> ModalBody for Body<'_, R> {
    type Output = ScrollWindow;

    /// The rows a palette wants when its frame follows its body: the
    /// query, the status row, every result (or the note, or the options),
    /// and the footer. The named palette size fills its extent instead.
    fn measure(&mut self, width: u16) -> u16 {
        let view = &self.view;
        let list = match view.options {
            Some(options) => options.len(),
            None => view.ranked.len(),
        };
        let footer = view
            .footer_lines(usize::from(width))
            .len()
            .min(MAX_FOOTER_ROWS);
        u16::try_from(view.lead() + list.max(1) + footer).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self { view, mut window } = self;
        let height = usize::from(area.height);
        if height == 0 || area.width == 0 {
            return window;
        }
        let width = usize::from(area.width);
        let theme = view.theme;
        let muted = Style::default().add_modifier(theme.muted);

        view.query(Rect { height: 1, ..area }, buf);
        let lead = view.lead().min(height);
        if let (Some(status), true) = (view.status, lead > 1) {
            let line = Line::from(Span::styled(text::truncate_end(status, width), muted));
            buf.set_line(area.x, area.y + 1, &line, area.width);
        }

        // The footer keeps the bottom rows, leaving the query, the status
        // row, and one row of the list above it.
        let mut footer = view.footer_lines(width);
        let footer_rows = footer
            .len()
            .min(MAX_FOOTER_ROWS)
            .min(height.saturating_sub(lead + 1));
        footer.truncate(footer_rows);
        let list_rows = height - lead - footer_rows;
        let top = area.y + u16::try_from(lead).unwrap_or(area.height);
        let list = Rect {
            y: top,
            height: u16::try_from(list_rows).unwrap_or(0),
            ..area
        };
        let footer_area = Rect {
            y: list.y + list.height,
            height: u16::try_from(footer_rows).unwrap_or(0),
            ..area
        };
        Paragraph::new(footer).render(footer_area, buf);

        if let Some(options) = view.options {
            Paragraph::new(options.lines(theme)).render(list, buf);
            return window;
        }
        if view.ranked.is_empty() {
            window.resize(list_rows, 0);
            let note = Line::from(Span::styled(text::truncate_end(view.note, width), muted));
            Paragraph::new(note).render(list, buf);
            return window;
        }
        // Results that overflow give the list's last column to the shared
        // scrollbar (a palette that reserves it keeps the column blank
        // while they fit), and the window follows the cursor by the one
        // rule.
        let gutter = ScrollGutter::layout(list, view.ranked.len());
        let mut content = gutter.content();
        if view.reserve_scrollbar && gutter.bar().is_none() && content.width >= 2 {
            content.width -= 1;
        }
        window.resize(usize::from(content.height), view.ranked.len());
        window.follow(view.cursor..view.cursor + 1);
        let lines = view.result_lines(window.visible(), usize::from(content.width));
        Paragraph::new(lines).render(content, buf);
        gutter.render(&window, buf);
        window
    }
}
