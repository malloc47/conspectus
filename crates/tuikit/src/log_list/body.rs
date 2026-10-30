//! A log list inside its frame: the entries over a rule, the selected
//! entry's detail, and the footer, which keeps the frame's last rows.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::LogList;
use crate::age::{self, Age};
use crate::feedback::{Entry, Log};
use crate::frame::{FooterFit, ModalBody, ModalFooter};
use crate::glyphs::marks;
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::theme::Theme;

/// The fewest rows the entries take while the log has that many: enough
/// to see the selection and its neighbors.
const MIN_LIST_ROWS: usize = 3;
/// The rows a frame keeps under the entries before it gives them any:
/// the rule, a row of detail, and the footer's last row.
const BELOW_ROWS: usize = 3;

/// The list in its frame, with the log and the time it reads ages against.
pub(super) struct Body<'l, 'a, R, M, Cx: ?Sized> {
    pub list: &'l mut LogList<R, M, Cx>,
    pub log: &'a Log<R>,
    pub now: i64,
    pub theme: &'a Theme,
}

impl<R, M, Cx: ?Sized> ModalBody for Body<'_, '_, R, M, Cx> {
    type Output = ();

    /// The list fills its frame, so it is laid out in `render`.
    fn measure(&mut self, _width: u16) -> u16 {
        0
    }

    /// Up to two fifths of the rows list the entries, then a rule, then
    /// the detail, with the footer on the last rows. A footer taller than
    /// the room under the rule leaves the detail one row: its last rows
    /// stay and its first rows scroll after the detail's last row.
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Self {
            list,
            log,
            now,
            theme,
        } = self;
        let width = usize::from(area.width);
        let height = usize::from(area.height);
        if log.is_empty() {
            empty(list, area, buf, theme);
            return;
        }
        let len = log.len();
        list.selected = list.selected.min(len - 1);
        let shown = len
            .min((height * 2 / 5).max(MIN_LIST_ROWS))
            .min(height.saturating_sub(BELOW_ROWS).max(1))
            .min(height);
        let rule = usize::from(height > shown);
        let room = height - shown - rule;
        let FooterFit { kept, spill, width } = fit_footer(list, width, room, theme);
        let rows = |count: usize| u16::try_from(count).unwrap_or(u16::MAX);
        let list_area = Rect {
            height: rows(shown),
            ..area
        };
        let rule_area = Rect {
            y: area.y + list_area.height,
            height: rows(rule),
            ..area
        };
        let detail_area = Rect {
            y: rule_area.y + rule_area.height,
            height: rows(room - kept.len()),
            ..area
        };
        let footer_area = Rect {
            y: detail_area.y + detail_area.height,
            width: width.min(area.width),
            height: rows(kept.len()),
            ..area
        };
        draw_entries(list, log, now, list_area, buf, theme);
        Paragraph::new(Line::from(Span::styled(
            marks::RULE.repeat(usize::from(area.width)),
            Style::default().add_modifier(theme.muted),
        )))
        .render(rule_area, buf);
        let text = list
            .selected_entry(log)
            .map(|entry| (list.detail_of)(entry))
            .unwrap_or_default();
        if list.detail.content().text() != text {
            list.detail.content_mut().set(text);
            list.detail.refresh();
        }
        if detail_area.height > 0 {
            list.detail.draw_embedded(detail_area, buf, theme, &spill);
        }
        Paragraph::new(kept).render(footer_area, buf);
    }
}

/// An empty log: the host's note, wrapped, over the footer.
fn empty<R, M, Cx: ?Sized>(
    list: &mut LogList<R, M, Cx>,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    list.window = ScrollWindow::default();
    let width = usize::from(area.width);
    // The footer leaves the note a row; its rows that do not fit follow
    // the note, as far as the frame shows them.
    let FooterFit {
        kept: footer,
        spill,
        ..
    } = ModalFooter::new(list.footer(), theme)
        .fit(width, usize::from(area.height).saturating_sub(1));
    let kept = footer.len();
    let note: Vec<Line<'static>> = list
        .empty
        .lines()
        .flat_map(|line| text::wrap(line, width))
        .map(|row| Line::from(row.trim_end().to_string()))
        .chain(spill)
        .collect();
    let footer_height = u16::try_from(kept).unwrap_or(area.height);
    let note_area = Rect {
        height: area.height - footer_height,
        ..area
    };
    Paragraph::new(note).render(note_area, buf);
    let footer_area = Rect {
        y: area.y + note_area.height,
        height: footer_height,
        ..area
    };
    Paragraph::new(footer).render(footer_area, buf);
}

/// The entries in the rows `area` shows, scrolled in the window the list
/// keeps (moving no further than the selection needs), with the shared
/// scrollbar when they overflow.
fn draw_entries<R, M, Cx: ?Sized>(
    list: &mut LogList<R, M, Cx>,
    log: &Log<R>,
    now: i64,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let len = log.len();
    let gutter = ScrollGutter::layout(area, len);
    list.window.resize(usize::from(area.height), len);
    list.window.follow(list.selected..list.selected + 1);
    let room = usize::from(gutter.content().width).saturating_sub(GUTTER_WIDTH);
    let visible = list.window.visible();
    let shown: Vec<Line<'static>> = log
        .newest_first()
        .enumerate()
        .skip(visible.start)
        .take(visible.len())
        .map(|(index, entry)| entry_row(entry, index == list.selected, now, room, theme))
        .collect();
    Paragraph::new(shown).render(gutter.content(), buf);
    gutter.render(&list.window, buf);
}

/// One entry's row `room` cells wide after the cursor gutter: the
/// severity's mark in its color, the age, and the summary with its repeat
/// count, cut with the ellipsis mark to fit.
fn entry_row<R>(
    entry: &Entry<R>,
    selected: bool,
    now: i64,
    room: usize,
    theme: &Theme,
) -> Line<'static> {
    let age = Age::between(entry.at, now);
    let pad = age::CELL_WIDTH.saturating_sub(text::width(&age.label()));
    let spans = vec![
        Span::styled(
            format!("{} ", entry.severity.mark()),
            entry.severity.style(theme),
        ),
        Span::styled(
            format!("{}{}  ", " ".repeat(pad), age.ago()),
            Style::default().fg(theme.inert),
        ),
        Span::raw(entry.summary_with_repeats()),
    ];
    selection::row(
        text::fit_spans(spans, room),
        Cursor::of(selected, true),
        theme,
    )
}

/// The footer fitted to the `room` rows under the rule, leaving the detail
/// at least one ([`ModalFooter::fit`]): the rows kept on the frame's last
/// rows (the footer's last, the keys that leave), and the first rows that
/// do not fit, which scroll after the detail's last row. Rows that scroll
/// there sit beside the detail's scrollbar, so they are laid out a cell
/// narrower.
fn fit_footer<R, M, Cx: ?Sized>(
    list: &LogList<R, M, Cx>,
    width: usize,
    room: usize,
    theme: &Theme,
) -> FooterFit {
    let footer = ModalFooter::new(list.footer(), theme);
    let fit = footer.fit(width, room.saturating_sub(1));
    if fit.spill.is_empty() || width < 2 {
        return fit;
    }
    footer.fit(width - 1, room.saturating_sub(1))
}
