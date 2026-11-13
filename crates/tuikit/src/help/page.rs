//! The help page: its sections laid out in cells, read in a pager.

use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::content::{HelpRow, HelpSection};
use crate::frame::ModalSize;
use crate::keys::{KeyRow, PagerKey, unavailable_style};
use crate::pager::{NoKeys, Pager, PagerContent, PagerEffect, PagerMode};
use crate::text;
use crate::theme::Theme;

/// Cells a row is indented by.
const INDENT: usize = 2;
/// Cells from the row's start to its description: the indent and the
/// keys. A key label that leaves no space before this column takes a
/// line of its own.
pub(super) const KEY_COLUMN: usize = 16;
/// The fewest cells a legend's name column takes, so short names line up
/// across legends.
const NAME_COLUMN_MIN: usize = 16;
/// The fewest cells a description or blurb is wrapped to. A page too
/// narrow for that many beside its column leaves the text on one row.
const MIN_TEXT: usize = 8;

/// A help page: sections laid out for the width its pager gives, as the
/// content of a [`Pager`].
///
/// The page is the rows; the pager is the reading. [`pager`](Self::pager)
/// puts the page in a [`Pager`] framed at [`HelpPage::SIZE`], titled
/// `Help`, which
/// scrolls it (one window, viewport-sized pages, half pages, `g`/`G` with
/// the end kept), draws the scrollbar when the page is taller than the
/// frame, and generates the footer from the table that scrolls and closes
/// the page. The layout, at any width:
///
/// - **Headings** in bold, word-wrapped, then the section's rows, then a
///   blank line.
/// - **Key rows** indented two cells, the keys in the theme's `accent`
///   padded to a 16-cell column, the description beside them and wrapped
///   under a hanging indent at that column. Keys too long for the column
///   take a line of their own, with the description under them. A row
///   that cannot act now is drawn in
///   [`unavailable_style`] throughout.
/// - **Legend rows** indented two cells: the mark in its own style padded
///   to the section's mark column, the name in bold padded to the
///   section's name column (16 cells, or the longest name and a space),
///   and the blurb beside them under a hanging indent.
///
/// `M` is the message type of the pager the page is read in, so the page
/// can sit on a host's modal stack beside its other overlays; the page
/// itself sends none.
pub struct HelpPage<M = ()> {
    sections: Vec<HelpSection>,
    title: Line<'static>,
    /// The last layout, and the width it was laid out at.
    lines: Vec<Line<'static>>,
    laid_out: Option<u16>,
    message: PhantomData<fn() -> M>,
}

impl HelpPage {
    /// The size every help page draws at: a reader's, as wide as the
    /// screen allows up to 100 columns and as tall as it allows.
    pub const SIZE: ModalSize = ModalSize::WIDE;
}

impl<M> HelpPage<M> {
    /// A page of `sections`, titled `Help`.
    pub fn new(sections: Vec<HelpSection>) -> Self {
        Self {
            sections,
            title: Line::from("Help"),
            lines: Vec::new(),
            laid_out: None,
            message: PhantomData,
        }
    }

    /// Sets the title.
    #[must_use]
    pub fn title(mut self, title: impl Into<Line<'static>>) -> Self {
        self.title = title.into();
        self
    }

    /// The page's sections.
    pub fn sections(&self) -> &[HelpSection] {
        &self.sections
    }

    /// The sections laid out `width` cells wide, as the page's rows. A
    /// width of `usize::MAX` wraps nothing.
    pub fn lines(&self, theme: &Theme, width: usize) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        for section in &self.sections {
            section_lines(section, theme, width, &mut out);
        }
        out
    }

    /// The page in a pager framed at [`HelpPage::SIZE`], resolving
    /// `table`: the keys that scroll and close it, from which its footer
    /// is generated. A page opened by a key adds that key to the table as
    /// another close.
    pub fn pager(self, table: &'static [KeyRow<PagerKey>]) -> Pager<Self> {
        Pager::new(self)
            .mode(PagerMode::Modal(HelpPage::SIZE))
            .keys(table)
    }
}

impl<M> Clone for HelpPage<M> {
    fn clone(&self) -> Self {
        Self {
            sections: self.sections.clone(),
            title: self.title.clone(),
            lines: self.lines.clone(),
            laid_out: self.laid_out,
            message: PhantomData,
        }
    }
}

impl<M> fmt::Debug for HelpPage<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelpPage")
            .field("sections", &self.sections)
            .field("title", &self.title)
            .field("laid_out", &self.laid_out)
            .finish_non_exhaustive()
    }
}

/// A help page is a pager's content: its rows are the laid-out sections,
/// its header the title, and it has no keys of its own.
impl<M> PagerContent for HelpPage<M> {
    type Message = M;
    type Action = NoKeys;
    const KEYS: &'static [KeyRow<NoKeys>] = &[];

    fn layout(&mut self, width: u16, theme: &Theme) -> usize {
        if self.laid_out != Some(width) {
            self.lines = self.lines(theme, usize::from(width));
            self.laid_out = Some(width);
        }
        self.lines.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.lines[range].to_vec()
    }

    fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
        self.title.clone()
    }

    fn apply(&mut self, action: NoKeys) -> PagerEffect<M> {
        match action {}
    }
}

/// Lays out `section` at `width` onto `out`: its heading, its rows, and a
/// blank line.
fn section_lines(section: &HelpSection, theme: &Theme, width: usize, out: &mut Vec<Line<'static>>) {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    for row in text::wrap(&section.title, width) {
        out.push(Line::from(Span::styled(row.trim_end().to_string(), bold)));
    }
    let columns = LegendColumns::of(section);
    for row in &section.rows {
        match row {
            HelpRow::Keys {
                keys,
                description,
                available,
            } => {
                let mut lines = key_lines(keys, description, theme, width);
                if !available {
                    let dim = unavailable_style(theme);
                    lines = lines
                        .into_iter()
                        .map(|line| restyle(line, |style| style.patch(dim)))
                        .collect();
                }
                out.extend(lines);
            }
            HelpRow::Legend { mark, name, blurb } => {
                let lead = vec![
                    Span::raw(" ".repeat(INDENT)),
                    Span::styled(text::pad(&mark.content, columns.mark), mark.style),
                    Span::styled(text::pad(name, columns.name), bold),
                ];
                out.extend(beside(
                    lead,
                    INDENT + columns.mark + columns.name,
                    blurb,
                    width,
                ));
            }
        }
    }
    out.push(Line::default());
}

/// The cells a section's legend marks and names take.
struct LegendColumns {
    mark: usize,
    name: usize,
}

impl LegendColumns {
    /// The widest mark and a space, and the longest name and a space but
    /// at least [`NAME_COLUMN_MIN`].
    fn of(section: &HelpSection) -> Self {
        let (mut mark, mut name) = (0, 0);
        for row in &section.rows {
            if let HelpRow::Legend {
                mark: m, name: n, ..
            } = row
            {
                mark = mark.max(text::width(&m.content) + 1);
                name = name.max(text::width(n) + 1);
            }
        }
        Self {
            mark,
            name: name.max(NAME_COLUMN_MIN),
        }
    }
}

/// A key row's lines at `width`.
fn key_lines(keys: &str, description: &str, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let accent = Style::default().fg(theme.accent);
    let indent = " ".repeat(INDENT);
    if text::width(keys) >= KEY_COLUMN - INDENT {
        // No room for a space before the description: the keys take a
        // line, and the description wraps under them.
        let mut out = vec![Line::from(Span::styled(format!("{indent}{keys}"), accent))];
        let budget = width.saturating_sub(KEY_COLUMN).max(MIN_TEXT);
        out.extend(text::wrap(description, budget).into_iter().map(|row| {
            Line::from(vec![
                Span::raw(" ".repeat(KEY_COLUMN)),
                Span::raw(row.trim_end().to_string()),
            ])
        }));
        return out;
    }
    let lead = vec![Span::styled(
        format!("{indent}{}", text::pad(keys, KEY_COLUMN - INDENT)),
        accent,
    )];
    beside(lead, KEY_COLUMN, description, width)
}

/// `lead`, `lead_width` cells wide, with `body` beside it, wrapped under
/// a hanging indent at the lead's width; on one row when `width` leaves
/// fewer than [`MIN_TEXT`] cells beside the lead.
fn beside(
    mut lead: Vec<Span<'static>>,
    lead_width: usize,
    body: &str,
    width: usize,
) -> Vec<Line<'static>> {
    if width <= lead_width + MIN_TEXT {
        lead.push(Span::raw(body.to_string()));
        return vec![Line::from(lead)];
    }
    let mut rows = text::wrap(body, width - lead_width).into_iter();
    lead.push(Span::raw(
        rows.next().unwrap_or_default().trim_end().to_string(),
    ));
    let mut out = vec![Line::from(lead)];
    out.extend(rows.map(|row| {
        Line::from(vec![
            Span::raw(" ".repeat(lead_width)),
            Span::raw(row.trim_end().to_string()),
        ])
    }));
    out
}

/// `line` with every span's style passed through `restyle`.
fn restyle(mut line: Line<'static>, with: impl Fn(Style) -> Style) -> Line<'static> {
    for span in &mut line.spans {
        span.style = with(span.style);
    }
    line
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
