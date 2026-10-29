//! A dialog laid out at one width, and the body its frame draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::Warning;
use crate::frame::ModalBody;
use crate::glyphs::marks;
use crate::keys::Footer;
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::text;
use crate::theme::Theme;

/// How each item of a warning list starts: indented under the heading,
/// after a dash.
const ITEM_PREFIX: &str = "  - ";

/// How far a warning heading's continuation rows are indented: past the
/// warning mark and its space, under the heading's first word.
const HEADING_INDENT: usize = 2;

/// What the layout reads from the dialog: its body, its warning list,
/// its footer, and the theme.
pub(super) struct View<'a> {
    pub body: &'a [String],
    pub warning: Option<&'a Warning>,
    pub footer: &'a Footer,
    pub theme: &'a Theme,
}

impl View<'_> {
    /// The body laid out `width` cells wide: a blank row, the body's
    /// lines wrapped, and, when there is a warning list, a blank row, its
    /// heading after the warning mark, and its items.
    pub fn layout(&self, width: usize) -> Vec<Line<'static>> {
        let mut lines = vec![Line::default()];
        for paragraph in self.body {
            if paragraph.is_empty() {
                lines.push(Line::default());
            } else {
                lines.extend(text::wrap_line(&Line::from(paragraph.as_str()), width, 0));
            }
        }
        if let Some(warning) = self.warning {
            if !self.body.is_empty() {
                lines.push(Line::default());
            }
            let style = Style::default()
                .fg(self.theme.warning)
                .add_modifier(Modifier::BOLD);
            let heading = Line::from(Span::styled(
                format!("{} {}", marks::WARNING, warning.heading),
                style,
            ));
            lines.extend(text::wrap_line(&heading, width, HEADING_INDENT));
            let indent = text::width(ITEM_PREFIX);
            for item in &warning.items {
                let item = Line::from(format!("{ITEM_PREFIX}{item}"));
                lines.extend(text::wrap_line(&item, width, indent));
            }
        }
        lines
    }

    /// The footer `width` cells wide under a blank row, broken only
    /// between hints.
    pub fn footer(&self, width: usize) -> Vec<Line<'static>> {
        let mut footer = vec![Line::default()];
        footer.extend(self.footer.lines(width, self.theme));
        footer
    }
}

/// Whether `line` draws nothing.
fn is_blank(line: &Line<'_>) -> bool {
    line.spans.iter().all(|span| span.content.trim().is_empty())
}

/// The dialog inside its frame: the body, scrolled in the window the
/// dialog kept, over the footer, which keeps the bottom rows.
pub(super) struct Body<'a> {
    pub view: View<'a>,
    pub window: ScrollWindow,
    pub lines: Option<Vec<Line<'static>>>,
    pub footer: Vec<Line<'static>>,
}

impl ModalBody for Body<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        let lines = self.view.layout(width);
        self.footer = self.view.footer(width);
        let rows = lines.len() + self.footer.len();
        self.lines = Some(lines);
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self {
            view,
            mut window,
            lines,
            mut footer,
        } = self;
        let width = usize::from(area.width);
        let mut lines = lines.unwrap_or_else(|| view.layout(width));
        if footer.is_empty() {
            footer = view.footer(width);
        }
        // A frame too short for the dialog gives up its blank rows first,
        // as many as it must: the one above the body, then the one
        // between the body and the keys.
        let rows = usize::from(area.height);
        let trimmed = lines.len() + footer.len() > rows && lines.first().is_some_and(is_blank);
        if trimmed {
            lines.remove(0);
        }
        if lines.len() + footer.len() > rows && footer.len() > 1 {
            footer.remove(0);
        }
        // Then the footer keeps the bottom rows, every one of them on a
        // frame too short for more (the title still says what is asked),
        // and the body scrolls in the rest.
        let footer_rows = footer.len().min(rows);
        let footer = footer.split_off(footer.len() - footer_rows);
        let body_rows = rows - footer_rows;
        let body_area = Rect {
            height: u16::try_from(body_rows).unwrap_or(area.height),
            ..area
        };
        // A body that overflows gives a column to the scrollbar, so it is
        // laid out again one cell narrower.
        if lines.len() > body_rows && area.width >= 2 {
            lines = view.layout(width - 1);
            if trimmed {
                lines.remove(0);
            }
        }
        let gutter = ScrollGutter::layout(body_area, lines.len());
        window.resize(body_rows, lines.len());
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        let footer_area = Rect {
            y: area.y + body_area.height,
            height: area.height - body_area.height,
            ..area
        };
        Paragraph::new(footer).render(footer_area, buf);
        window
    }
}
