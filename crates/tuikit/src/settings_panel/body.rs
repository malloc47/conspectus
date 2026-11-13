//! A settings panel laid out at one width, and the body its frame draws.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::row::{Kind, Row, Section};
use crate::frame::{ModalBody, ModalFooter};
use crate::glyphs::marks;
use crate::keys::{Footer, SETTINGS_KEYS};
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::selection::{self, Cursor, GUTTER_WIDTH};
use crate::text;
use crate::theme::Theme;

/// The words after the choice in force.
pub(super) const ACTIVE: &str = "active";

/// What an editor row ends with: its value opens an editor.
pub(super) const EDIT_TAG: &str = "[ Edit ]";

/// What a cycle row ends with: its value steps in place.
pub(super) const CYCLE_TAG: &str = "[ Cycle ]";

/// Where a row sits among the panel's lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Place {
    /// The line the row is drawn on.
    pub line: usize,
    /// The lines the cursor on it keeps in view: the row, and for the
    /// first row of a section its heading too, so moving onto a section
    /// shows what it is.
    pub span: Range<usize>,
}

/// Where every row of `sections` sits, in order: a heading line per
/// section, a line per row, and a blank line between sections.
pub(super) fn places<Id, M>(sections: &[Section<Id, M>]) -> Vec<Place> {
    let mut places = Vec::new();
    let mut line = 0;
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            line += 1;
        }
        let heading = line;
        line += 1;
        for (row, _) in section.rows.iter().enumerate() {
            let start = if row == 0 { heading } else { line };
            places.push(Place {
                line,
                span: start..line + 1,
            });
            line += 1;
        }
    }
    places
}

/// What the layout reads from the panel: the sections as the host has
/// them now, the row the cursor is on, whether the panel has the keys,
/// the width of the name column, and the theme.
pub(super) struct View<'a, Id, M> {
    pub sections: &'a [Section<Id, M>],
    pub cursor: Option<usize>,
    pub focused: bool,
    pub name_width: usize,
    pub theme: &'a Theme,
}

impl<Id, M> View<'_, Id, M> {
    /// The panel's lines `width` cells wide: each section's heading, its
    /// rows behind the cursor gutter, and a blank line between sections,
    /// every line cut with `…` to fit.
    pub fn lines(&self, width: usize) -> Vec<Line<'static>> {
        let room = width.saturating_sub(GUTTER_WIDTH);
        let heading = Style::default().add_modifier(Modifier::BOLD);
        let mut lines = Vec::new();
        let mut index = 0;
        for (number, section) in self.sections.iter().enumerate() {
            if number > 0 {
                lines.push(Line::default());
            }
            let title = Span::styled(section.title.clone(), heading);
            lines.push(Line::from(text::fit_spans(vec![title], width)));
            for row in &section.rows {
                let on = self.cursor == Some(index);
                let cursor = Cursor::of(on, self.focused);
                let spans = text::fit_spans(self.spans(row), room);
                lines.push(selection::row(spans, cursor, self.theme));
                index += 1;
            }
        }
        lines
    }

    /// The text of one row, after the cursor gutter.
    fn spans(&self, row: &Row<Id, M>) -> Vec<Span<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        match &row.kind {
            Kind::Choice { active: true } => {
                vec![Span::styled(format!("{}  {ACTIVE}", row.label), bold)]
            }
            Kind::Choice { active: false } | Kind::Button => vec![Span::raw(row.label.clone())],
            Kind::Checkbox { checked } => {
                let (mark, style) = if *checked {
                    (marks::CHECKED, bold)
                } else {
                    (marks::UNCHECKED, Style::default())
                };
                vec![Span::styled(format!("{mark} {}", row.label), style)]
            }
            Kind::Editor { value } => vec![self.named(&row.label, value, EDIT_TAG)],
            Kind::Cycle { value } => vec![self.named(&row.label, value, CYCLE_TAG)],
        }
    }

    /// An editor's or a cycle's row: the name in its column, the value,
    /// and the tag that says what acting on it does.
    fn named(&self, name: &str, value: &str, tag: &str) -> Span<'static> {
        let name = text::pad(name, self.name_width);
        Span::raw(format!("{name} {value}  {tag}"))
    }

    /// The footer the standard settings table generates.
    pub fn footer(&self) -> ModalFooter<'_> {
        ModalFooter::new(Footer::leaving_last(SETTINGS_KEYS), self.theme)
    }
}

/// The panel inside its frame: the sections, scrolled in the window the
/// panel kept, over the footer, which keeps the frame's last rows.
pub(super) struct Body<'a, Id, M> {
    pub view: View<'a, Id, M>,
    pub places: Vec<Place>,
    pub window: ScrollWindow,
}

impl<Id, M> ModalBody for Body<'_, Id, M> {
    type Output = ScrollWindow;

    fn measure(&mut self, width: u16) -> u16 {
        let width = usize::from(width);
        let wanted = self.view.lines(width).len() + self.view.footer().height(width);
        u16::try_from(wanted).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let Self {
            view,
            places,
            mut window,
        } = self;
        // The footer keeps the frame's last rows and leaves the rows one,
        // so the cursor's row always shows; rows that overflow give a
        // column to the scrollbar, so they and the footer are laid out a
        // cell narrower.
        let mut fit = view
            .footer()
            .fit_beside(area, 1, |width| view.lines(usize::from(width)).len());
        let [rows_area, footer_area] = fit.split(area);
        // The footer rows that do not fit follow the last row. They spill
        // only while the rows are down to the cursor's, which the window
        // keeps, so the panel shows none of them.
        let mut lines = view.lines(usize::from(fit.width));
        lines.append(&mut fit.spill);
        let gutter = ScrollGutter::layout(rows_area, lines.len());
        window.resize(usize::from(rows_area.height), lines.len());
        if let Some(place) = view.cursor.and_then(|cursor| places.get(cursor)) {
            window.follow(place.span.clone());
            // A span taller than the window shows its start: keep the
            // row itself in view.
            window.follow(place.line..place.line + 1);
        }
        Paragraph::new(lines)
            .scroll(window.paragraph_scroll())
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        fit.render(footer_area, buf);
        window
    }
}
