//! A palette laid out inside its frame: the query, the status row, the
//! results or the options pane, and the footer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::options::Options;
use super::row::RowContext;
use crate::frame::{ModalBody, ModalFooter};
use crate::keys::Footer;
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::text_edit::{LineEdit, TextField};
use crate::theme::Theme;

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

    /// The palette's footer: the palette's keys, or the options pane's.
    fn footer(&self) -> ModalFooter<'_> {
        ModalFooter::new(self.footer.clone(), self.theme)
    }

    /// The rows the list shows: the options, the results, or the note.
    fn list_rows(&self) -> usize {
        match self.options {
            Some(options) => options.len(),
            None => self.ranked.len().max(1),
        }
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
        let rows = view.lead() + view.list_rows() + view.footer().height(usize::from(width));
        u16::try_from(rows).unwrap_or(u16::MAX)
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

        // The footer keeps the frame's last rows under the query and the
        // status row, leaving one row of the list above it; a list that
        // overflows gives a column to the scrollbar, so the footer is laid
        // out a cell narrower beside it.
        let lead_rows = u16::try_from(lead).unwrap_or(area.height);
        let below = Rect {
            y: area.y + lead_rows,
            height: area.height - lead_rows,
            ..area
        };
        let mut fit = view.footer().fit_beside(below, 1, |_| view.list_rows());
        let [list, footer_area] = fit.split(below);
        let list_rows = usize::from(list.height);
        let spill = std::mem::take(&mut fit.spill);
        fit.render(footer_area, buf);

        // The footer rows that do not fit follow the list's last row.
        if let Some(options) = view.options {
            let mut lines = options.lines(theme);
            lines.extend(spill);
            Paragraph::new(lines).render(list, buf);
            return window;
        }
        if view.ranked.is_empty() {
            window.resize(list_rows, 0);
            let note = Line::from(Span::styled(text::truncate_end(view.note, width), muted));
            Paragraph::new([note].into_iter().chain(spill).collect::<Vec<_>>()).render(list, buf);
            return window;
        }
        // Results that overflow give the list's last column to the shared
        // scrollbar (a palette that reserves it keeps the column blank
        // while they fit), and the window follows the cursor by the one
        // rule. The footer rows that do not fit follow the last result;
        // they spill only while the list is down to the cursor's row, so a
        // palette shows none of them.
        let results = view.ranked.len();
        let rows = results + spill.len();
        let gutter = ScrollGutter::layout(list, rows);
        let mut content = gutter.content();
        if view.reserve_scrollbar && gutter.bar().is_none() && content.width >= 2 {
            content.width -= 1;
        }
        window.resize(usize::from(content.height), rows);
        window.follow(view.cursor..view.cursor + 1);
        let visible = window.visible();
        let mut lines = view.result_lines(
            visible.start.min(results)..visible.end.min(results),
            usize::from(content.width),
        );
        let after = visible.start.saturating_sub(results)..visible.end.saturating_sub(results);
        lines.extend(spill[after].iter().cloned());
        Paragraph::new(lines).render(content, buf);
        gutter.render(&window, buf);
        window
    }
}
