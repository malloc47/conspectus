//! A dialog laid out at one width, and the body its frame draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::Warning;
use crate::frame::{ModalBody, ModalFooter};
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

    /// The dialog's footer.
    pub fn footer(&self) -> ModalFooter<'_> {
        ModalFooter::new(self.footer.clone(), self.theme)
    }
}

/// Whether `line` draws nothing.
fn is_blank(line: &Line<'_>) -> bool {
    line.spans.iter().all(|span| span.content.trim().is_empty())
}

/// The dialog inside its frame: the body, scrolled in the window the
/// dialog kept, over the footer, which keeps the frame's last rows.
pub(super) struct Body<'a> {
    pub view: View<'a>,
    pub window: ScrollWindow,
}

impl ModalBody for Body<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        let rows = self.view.layout(width).len() + self.view.footer().height(width);
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self { view, mut window } = self;
        let footer = view.footer();
        // A frame too short for the dialog gives up the blank row above
        // the body first.
        let rows = usize::from(area.height);
        let width = usize::from(area.width);
        let trimmed = view.layout(width).len() + footer.height(width) > rows;
        let lines = |width: u16| {
            let mut lines = view.layout(usize::from(width));
            if trimmed && lines.first().is_some_and(is_blank) {
                lines.remove(0);
            }
            lines
        };
        // Then the footer keeps the frame's last rows and leaves the body
        // one (the title still says what is asked); a body that overflows
        // gives a column to the scrollbar, so it and the footer are laid
        // out a cell narrower.
        let mut fit = footer.fit_beside(area, 1, |width| lines(width).len());
        let [body_area, footer_area] = fit.split(area);
        // The footer rows that do not fit scroll after the body's last.
        let mut lines = lines(fit.width);
        lines.append(&mut fit.spill);
        let gutter = ScrollGutter::layout(body_area, lines.len());
        window.resize(usize::from(body_area.height), lines.len());
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        fit.render(footer_area, buf);
        window
    }
}
